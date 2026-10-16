mod cli;
mod config;
mod cron;
mod lock;
#[cfg(feature = "nagios")]
mod nagios;
mod packagekit;

use anyhow::{Context, Result};
use chrono::Local;
use clap::Parser;
use futures::stream::StreamExt;
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::cli::Args;
use crate::config::Config;
use crate::lock::FileLock;
#[cfg(feature = "nagios")]
use crate::nagios::{NagiosOutput, NagiosStatus};
use crate::packagekit::PackageManager;

#[cfg(feature = "nagios")]
type Report = NagiosOutput;
#[cfg(not(feature = "nagios"))]
type Report = ();

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = Config::from_args(&args);

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    let mut signals = Signals::new([SIGTERM, SIGINT, SIGQUIT, SIGHUP])?;
    tokio::spawn(async move {
        while let Some(signal) = signals.next().await {
            match signal {
                SIGTERM | SIGINT | SIGQUIT => {
                    eprintln!("Received signal {}, terminating...", signal);
                    r.store(false, Ordering::Relaxed);
                }
                _ => {}
            }
        }
    });

    let _lock = if let Some(lock_path) = &config.lock_file {
        let mut lock = FileLock::new(lock_path)?;

        if !lock.try_lock()? {
            if config.cron_spec.is_some() {
                return Ok(());
            } else {
                #[cfg(feature = "nagios")]
                println!(
                    "{}",
                    NagiosOutput {
                        status: NagiosStatus::Warning,
                        message: "Failed to acquire lock file".to_string(),
                        perfdata: None,
                    }
                );
                #[cfg(not(feature = "nagios"))]
                eprintln!("Error: Failed to acquire lock file");
                std::process::exit(1);
            }
        }

        if let Some(cron_spec) = &config.cron_spec {
            let last_run = lock.read_timestamp()?;
            let now = Local::now();

            if let Some(last_run) = last_run
                && !cron::should_run(cron_spec, last_run, now)?
            {
                return Ok(());
            }

            lock.write_timestamp(now)?;
        }

        Some(lock)
    } else {
        None
    };

    match run_update_check(config, running).await {
        #[cfg(feature = "nagios")]
        Ok(output) => {
            println!("{}", output);
            std::process::exit(output.status.exit_code());
        }
        #[cfg(not(feature = "nagios"))]
        Ok(()) => Ok(()),
        #[cfg(feature = "nagios")]
        Err(e) => {
            let output = NagiosOutput {
                status: NagiosStatus::Critical,
                message: format!("An error occurred: {:#}", e),
                perfdata: None,
            };
            println!("{}", output);
            std::process::exit(2);
        }
        #[cfg(not(feature = "nagios"))]
        Err(e) => {
            eprintln!("Error: {:#}", e);
            std::process::exit(1);
        }
    }
}

async fn run_update_check(config: Config, running: Arc<AtomicBool>) -> Result<Report> {
    let pm = PackageManager::new().await?;

    if running.load(Ordering::Relaxed) {
        eprintln!("Refreshing package cache...");
        pm.refresh_cache()
            .await
            .context("Failed to refresh package cache")?;
    }

    if !running.load(Ordering::Relaxed) {
        #[cfg(feature = "nagios")]
        return Ok(NagiosOutput {
            status: NagiosStatus::Critical,
            message: "Operation cancelled".to_string(),
            perfdata: None,
        });
        #[cfg(not(feature = "nagios"))]
        {
            eprintln!("Operation cancelled");
            return Ok(());
        }
    }

    eprintln!("Getting available updates...");
    let updates = pm.get_updates().await.context("Failed to get updates")?;

    if updates.is_empty() {
        #[cfg(feature = "nagios")]
        eprintln!("Everything is up to date.");
        #[cfg(feature = "nagios")]
        return Ok(NagiosOutput {
            status: NagiosStatus::Ok,
            message: "Everything is up to date".to_string(),
            perfdata: Some("'Total Update'=0 'Security Update'=0".to_string()),
        });
        #[cfg(not(feature = "nagios"))]
        {
            println!("Everything is up to date");
            return Ok(());
        }
    }

    eprintln!("Getting update details...");
    let detailed_updates = pm
        .get_update_details(&updates)
        .await
        .context("Failed to get update details")?;

    let total_count = detailed_updates.len();
    let mut security_count = 0;
    let mut security_updates = Vec::new();
    let mut all_updates = Vec::new();

    for update in &detailed_updates {
        if update.is_security {
            security_count += 1;
            security_updates.push(update.clone());
        }
        all_updates.push(update.clone());
    }

    #[cfg(feature = "nagios")]
    if config.apply_updates || config.apply_security_updates {
        eprintln!("The following packages will be updated:");
    } else {
        eprintln!("The following packages are security updates:");
    }

    #[cfg(feature = "nagios")]
    let mut details = String::from("Security updates:\n");
    #[cfg(feature = "nagios")]
    for update in &detailed_updates {
        if update.is_security || config.apply_updates {
            let security_tag = if update.is_security {
                " (SECURITY)"
            } else {
                ""
            };
            let line = format!("{} {}{}\n", update.name, update.version, security_tag);
            eprintln!("{}", line.trim());
            details.push_str(&line);
        }
    }

    #[cfg(feature = "nagios")]
    if security_count == 0 && !config.apply_updates {
        details.push_str("(none)\n");
        eprintln!("(none)");
    }

    #[cfg(not(feature = "nagios"))]
    {
        println!(
            "\nAvailable updates: {} total ({} security)",
            total_count, security_count
        );
        println!("{}", "-".repeat(60));

        for update in &detailed_updates {
            let security_tag = if update.is_security {
                " [SECURITY]"
            } else {
                ""
            };
            println!("{} {}{}", update.name, update.version, security_tag);
        }
    }

    if config.apply_updates || config.apply_security_updates {
        let updates_to_apply = if config.apply_updates {
            all_updates
        } else {
            security_updates
        };

        if !updates_to_apply.is_empty() {
            if !config.non_interactive && !prompt_confirmation()? {
                #[cfg(feature = "nagios")]
                return Ok(NagiosOutput {
                    status: NagiosStatus::Critical,
                    message: "Cancelled by user".to_string(),
                    perfdata: None,
                });
                #[cfg(not(feature = "nagios"))]
                {
                    eprintln!("Cancelled by user");
                    return Ok(());
                }
            }

            eprintln!("Applying updates...");
            pm.apply_updates(&updates_to_apply)
                .await
                .context("Failed to apply updates")?;
        }
    }

    #[cfg(not(feature = "nagios"))]
    {
        Ok(())
    }

    #[cfg(feature = "nagios")]
    {
        let status = if security_count >= config.critical_threshold {
            NagiosStatus::Critical
        } else if security_count >= config.warning_threshold {
            NagiosStatus::Warning
        } else {
            NagiosStatus::Ok
        };

        Ok(NagiosOutput {
            status,
            message: format!(
                "Security-Update = {} | 'Total Update' = {}\n{}",
                security_count, total_count, details
            ),
            perfdata: Some(format!(
                "'Total Update'={} 'Security Update'={}",
                total_count, security_count
            )),
        })
    }
}

fn prompt_confirmation() -> Result<bool> {
    use std::io::{self, Write};

    print!("\nProceed with installation? [y/n] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_lowercase() == "y")
}
