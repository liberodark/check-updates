use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use zbus::{Connection, proxy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub package_id: String,
    pub name: String,
    pub version: String,
    pub is_security: bool,
}

const PK_FILTER_ENUM_NONE: u64 = 0;
const PK_TRANSACTION_FLAG_ENUM_ONLY_TRUSTED: u64 = 1 << 1;

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

impl PackageManager {
    pub async fn new() -> Result<Self> {
        let connection = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;

        Ok(Self { connection })
    }

    pub async fn refresh_cache(&self) -> Result<()> {
        eprintln!("Creating transaction for cache refresh...");

        let pk_proxy = PackageKitProxy::new(&self.connection)
            .await
            .context("Failed to create PackageKit proxy")?;

        let transaction_path = pk_proxy
            .create_transaction()
            .await
            .context("Failed to create transaction")?;

        let transaction = TransactionProxy::builder(&self.connection)
            .path(&transaction_path)?
            .build()
            .await
            .context("Failed to create transaction proxy")?;

        let finished = Arc::new(Mutex::new(false));
        let finished_clone = finished.clone();

        let mut finished_stream = transaction.receive_finished().await?;
        tokio::spawn(async move {
            while (finished_stream.next().await).is_some() {
                *finished_clone.lock().await = true;
            }
        });

        transaction
            .refresh_cache(true)
            .await
            .context("Failed to refresh cache")?;

        while !*finished.lock().await {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        Ok(())
    }

    pub async fn get_updates(&self) -> Result<Vec<String>> {
        eprintln!("Creating transaction for getting updates...");

        let pk_proxy = PackageKitProxy::new(&self.connection)
            .await
            .context("Failed to create PackageKit proxy")?;

        let transaction_path = pk_proxy
            .create_transaction()
            .await
            .context("Failed to create transaction")?;

        let transaction = TransactionProxy::builder(&self.connection)
            .path(&transaction_path)?
            .build()
            .await
            .context("Failed to create transaction proxy")?;

        let packages = Arc::new(Mutex::new(Vec::new()));
        let packages_clone = packages.clone();
        let finished = Arc::new(Mutex::new(false));
        let finished_clone = finished.clone();

        let mut package_stream = transaction.receive_package().await?;
        tokio::spawn(async move {
            while let Some(signal) = package_stream.next().await {
                if let Ok(args) = signal.args() {
                    packages_clone.lock().await.push(args.package_id);
                }
            }
        });

        let mut finished_stream = transaction.receive_finished().await?;
        tokio::spawn(async move {
            while (finished_stream.next().await).is_some() {
                *finished_clone.lock().await = true;
            }
        });

        transaction
            .get_updates(PK_FILTER_ENUM_NONE)
            .await
            .context("Failed to get updates")?;

        while !*finished.lock().await {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        let result = packages.lock().await.clone();
        Ok(result)
    }

    pub async fn get_update_details(&self, package_ids: &[String]) -> Result<Vec<UpdateInfo>> {
        if package_ids.is_empty() {
            return Ok(Vec::new());
        }

        eprintln!("Getting update details for {} packages", package_ids.len());

        let pk_proxy = PackageKitProxy::new(&self.connection)
            .await
            .context("Failed to create PackageKit proxy")?;

        let transaction_path = pk_proxy
            .create_transaction()
            .await
            .context("Failed to create transaction")?;

        let transaction = TransactionProxy::builder(&self.connection)
            .path(&transaction_path)?
            .build()
            .await
            .context("Failed to create transaction proxy")?;

        let details = Arc::new(Mutex::new(HashMap::new()));
        let details_clone = details.clone();
        let finished = Arc::new(Mutex::new(false));
        let finished_clone = finished.clone();

        let mut update_detail_stream = transaction.receive_update_detail().await?;
        tokio::spawn(async move {
            while let Some(signal) = update_detail_stream.next().await {
                if let Ok(args) = signal.args() {
                    let is_security = !args.cve_urls.is_empty()
                        || args.update_text.contains("CVE-")
                        || args.changelog.contains("CVE-");

                    details_clone
                        .lock()
                        .await
                        .insert(args.package_id, is_security);
                }
            }
        });

        let mut finished_stream = transaction.receive_finished().await?;
        tokio::spawn(async move {
            while (finished_stream.next().await).is_some() {
                *finished_clone.lock().await = true;
            }
        });

        let package_refs: Vec<&str> = package_ids.iter().map(|s| s.as_str()).collect();
        transaction
            .get_update_detail(&package_refs)
            .await
            .context("Failed to get update details")?;

        while !*finished.lock().await {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        let details_map = details.lock().await;
        let mut results = Vec::new();

        for package_id in package_ids {
            let is_security = details_map.get(package_id).copied().unwrap_or(false);

            if let Some(update_info) = parse_package_id(package_id, is_security) {
                results.push(update_info);
            }
        }

        Ok(results)
    }

    pub async fn apply_updates(&self, updates: &[UpdateInfo]) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        eprintln!("Applying {} updates", updates.len());

        let pk_proxy = PackageKitProxy::new(&self.connection)
            .await
            .context("Failed to create PackageKit proxy")?;

        let transaction_path = pk_proxy
            .create_transaction()
            .await
            .context("Failed to create transaction")?;

        let transaction = TransactionProxy::builder(&self.connection)
            .path(&transaction_path)?
            .build()
            .await
            .context("Failed to create transaction proxy")?;

        let finished = Arc::new(Mutex::new(false));
        let finished_clone = finished.clone();
        let error = Arc::new(Mutex::new(None));
        let error_clone = error.clone();

        let mut finished_stream = transaction.receive_finished().await?;
        tokio::spawn(async move {
            while (finished_stream.next().await).is_some() {
                *finished_clone.lock().await = true;
            }
        });

        let mut error_stream = transaction.receive_error_code().await?;
        tokio::spawn(async move {
            while let Some(signal) = error_stream.next().await {
                if let Ok(args) = signal.args() {
                    *error_clone.lock().await = Some(args.details);
                }
            }
        });

        let package_ids: Vec<&str> = updates.iter().map(|u| u.package_id.as_str()).collect();
        transaction
            .update_packages(PK_TRANSACTION_FLAG_ENUM_ONLY_TRUSTED, &package_ids)
            .await
            .context("Failed to update packages")?;

        while !*finished.lock().await {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        if let Some(error_msg) = error.lock().await.as_ref() {
            bail!("Package update failed: {}", error_msg);
        }

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
