use crate::cli::Args;

#[derive(Debug)]
pub struct Config {
    pub lock_file: Option<String>,
    pub cron_spec: Option<String>,
    pub warning_threshold: usize,
    pub critical_threshold: usize,
    pub apply_security_updates: bool,
    pub apply_updates: bool,
    pub non_interactive: bool,
}

impl Config {
    pub fn from_args(args: &Args) -> Self {
        if args.cron.is_some() && args.lock.is_none() {
            eprintln!("Error: --cron requires --lock");
            std::process::exit(1);
        }

        Self {
            lock_file: args.lock.clone(),
            cron_spec: args.cron.clone(),
            warning_threshold: args.warning,
            critical_threshold: args.critical,
            apply_security_updates: args.security_update,
            apply_updates: args.update,
            non_interactive: args.yes,
        }
    }
}
