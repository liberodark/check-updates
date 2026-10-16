use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use zbus::fdo::{DBusProxy, NameOwnerChangedStream};
use zbus::message::{Message, Type};
use zbus::{Connection, MatchRule, MessageStream, proxy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub package_id: String,
    pub name: String,
    pub version: String,
    pub is_security: bool,
}

const PK_SERVICE: &str = "org.freedesktop.PackageKit";
const PK_TRANSACTION_INTERFACE: &str = "org.freedesktop.PackageKit.Transaction";
const PK_FILTER_ENUM_NONE: u64 = 0;
const PK_TRANSACTION_FLAG_ENUM_ONLY_TRUSTED: u64 = 1 << 1;
const PK_INFO_ENUM_SECURITY: u32 = 8;
const PK_INFO_ENUM_MASK: u32 = 0xFFFF;
const PK_EXIT_ENUM_SUCCESS: u32 = 1;

#[proxy(
    interface = "org.freedesktop.PackageKit",
    default_service = "org.freedesktop.PackageKit",
    default_path = "/org/freedesktop/PackageKit"
)]
trait PackageKit {
    async fn create_transaction(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.PackageKit.Transaction",
    default_service = "org.freedesktop.PackageKit"
)]
trait Transaction {
    async fn refresh_cache(&self, force: bool) -> zbus::Result<()>;
    async fn get_updates(&self, filter: u64) -> zbus::Result<()>;
    async fn get_update_detail(&self, package_ids: &[&str]) -> zbus::Result<()>;
    async fn update_packages(
        &self,
        transaction_flags: u64,
        package_ids: &[&str],
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn package(&self, info: u32, package_id: String, summary: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn update_detail(
        &self,
        package_id: String,
        updates: Vec<String>,
        obsoletes: Vec<String>,
        vendor_urls: Vec<String>,
        bugzilla_urls: Vec<String>,
        cve_urls: Vec<String>,
        restart: u32,
        update_text: String,
        changelog: String,
        state: u32,
        issued: String,
        updated: String,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn finished(&self, exit: u32, runtime: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    fn error_code(&self, code: u32, details: String) -> zbus::Result<()>;
}

pub struct PackageManager {
    connection: Connection,
}

struct TransactionSignals {
    messages: MessageStream,
    owner_changes: NameOwnerChangedStream,
}

impl TransactionSignals {
    async fn collect(
        mut self,
        call: impl Future<Output = zbus::Result<()>>,
    ) -> Result<Vec<Message>> {
        let wait = async {
            let mut signals = Vec::new();
            loop {
                tokio::select! {
                    message = self.messages.next() => {
                        let message = message.context("D-Bus connection closed")??;
                        if let Some(finished) = Finished::from_message(message.clone()) {
                            if finished.args()?.exit == PK_EXIT_ENUM_SUCCESS {
                                return Ok(signals);
                            }
                            match signals.into_iter().filter_map(ErrorCode::from_message).next_back() {
                                Some(error) => bail!("{}", error.args()?.details),
                                None => bail!("PackageKit transaction failed"),
                            }
                        }
                        signals.push(message);
                    }
                    Some(change) = self.owner_changes.next() => {
                        if change.args()?.new_owner().is_none() {
                            bail!("PackageKit stopped before the transaction finished");
                        }
                    }
                }
            }
        };

        let ((), signals) = tokio::try_join!(async { Ok(call.await?) }, wait)?;
        Ok(signals)
    }
}

impl PackageManager {
    pub async fn new() -> Result<Self> {
        let connection = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;

        Ok(Self { connection })
    }

    async fn transaction(&self) -> Result<(TransactionProxy<'_>, TransactionSignals)> {
        let pk_proxy = PackageKitProxy::new(&self.connection)
            .await
            .context("Failed to create PackageKit proxy")?;

        let transaction_path = pk_proxy
            .create_transaction()
            .await
            .context("Failed to create transaction")?;

        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(PK_TRANSACTION_INTERFACE)?
            .path(transaction_path.clone())?
            .build();
        let messages = MessageStream::for_match_rule(rule, &self.connection, None).await?;
        let owner_changes = DBusProxy::new(&self.connection)
            .await?
            .receive_name_owner_changed_with_args(&[(0, PK_SERVICE)])
            .await?;

        let transaction = TransactionProxy::builder(&self.connection)
            .path(transaction_path)?
            .build()
            .await
            .context("Failed to create transaction proxy")?;

        Ok((
            transaction,
            TransactionSignals {
                messages,
                owner_changes,
            },
        ))
    }

    pub async fn refresh_cache(&self) -> Result<()> {
        eprintln!("Creating transaction for cache refresh...");

        let (transaction, signals) = self.transaction().await?;
        // Not fatal: refreshing requires authorization that a non-root caller
        // (e.g. NRPE) usually lacks, and the check can use the current cache.
        if let Err(e) = signals.collect(transaction.refresh_cache(true)).await {
            eprintln!("Warning: Failed to refresh cache: {:#}", e);
        }

        Ok(())
    }

    /// Returns the package ID of each available update, and whether PackageKit
    /// classifies it as a security update.
    pub async fn get_updates(&self) -> Result<Vec<(String, bool)>> {
        eprintln!("Creating transaction for getting updates...");

        let (transaction, signals) = self.transaction().await?;
        let signals = signals
            .collect(transaction.get_updates(PK_FILTER_ENUM_NONE))
            .await
            .context("Failed to get updates")?;

        let mut updates = Vec::new();
        for signal in signals.into_iter().filter_map(Package::from_message) {
            let args = signal.args()?;
            let is_security = args.info & PK_INFO_ENUM_MASK == PK_INFO_ENUM_SECURITY;
            updates.push((args.package_id, is_security));
        }

        Ok(updates)
    }

    pub async fn get_update_details(&self, updates: &[(String, bool)]) -> Result<Vec<UpdateInfo>> {
        if updates.is_empty() {
            return Ok(Vec::new());
        }

        eprintln!("Getting update details for {} packages", updates.len());

        let (transaction, signals) = self.transaction().await?;
        let package_refs: Vec<&str> = updates.iter().map(|(id, _)| id.as_str()).collect();
        let signals = signals
            .collect(transaction.get_update_detail(&package_refs))
            .await
            .context("Failed to get update details")?;

        let mut cve_fixes = HashSet::new();
        for signal in signals.into_iter().filter_map(UpdateDetail::from_message) {
            let args = signal.args()?;
            if !args.cve_urls.is_empty()
                || args.update_text.contains("CVE-")
                || args.changelog.contains("CVE-")
            {
                cve_fixes.insert(args.package_id);
            }
        }

        Ok(updates
            .iter()
            .filter_map(|(package_id, is_security)| {
                parse_package_id(package_id, *is_security || cve_fixes.contains(package_id))
            })
            .collect())
    }

    pub async fn apply_updates(&self, updates: &[UpdateInfo]) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        eprintln!("Applying {} updates", updates.len());

        let (transaction, signals) = self.transaction().await?;
        let package_ids: Vec<&str> = updates.iter().map(|u| u.package_id.as_str()).collect();
        signals
            .collect(
                transaction.update_packages(PK_TRANSACTION_FLAG_ENUM_ONLY_TRUSTED, &package_ids),
            )
            .await
            .context("Package update failed")?;

        Ok(())
    }
}

fn parse_package_id(package_id: &str, is_security: bool) -> Option<UpdateInfo> {
    let parts: Vec<&str> = package_id.split(';').collect();
    if parts.is_empty() {
        return None;
    }

    Some(UpdateInfo {
        package_id: package_id.to_string(),
        name: parts.first()?.to_string(),
        version: parts.get(1).unwrap_or(&"unknown").to_string(),
        is_security,
    })
}
