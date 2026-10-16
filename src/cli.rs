use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "check_updates")]
#[command(about = "Check for system updates via PackageKit", long_about = None)]
#[command(version)]
pub struct Args {
    #[arg(long, value_name = "FILE")]
    pub lock: Option<String>,

    #[arg(long, value_name = "CRON_SPEC")]
    pub cron: Option<String>,

    #[cfg(feature = "nagios")]
    #[arg(short, long, default_value = "10")]
    pub warning: usize,

    #[cfg(feature = "nagios")]
    #[arg(short, long, default_value = "20")]
    pub critical: usize,

    #[arg(long = "security-update")]
    pub security_update: bool,

    #[arg(long = "update")]
    pub update: bool,

    #[arg(short = 'y', long)]
    pub yes: bool,
}
