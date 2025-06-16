use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum NagiosStatus {
    Ok,
    Warning,
    Critical,
    Unknown,
}

impl NagiosStatus {
    pub fn exit_code(&self) -> i32 {
        match self {
            NagiosStatus::Ok => 0,
            NagiosStatus::Warning => 1,
            NagiosStatus::Critical => 2,
            NagiosStatus::Unknown => 3,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            NagiosStatus::Ok => "OK",
            NagiosStatus::Warning => "Warning",
            NagiosStatus::Critical => "Critical",
            NagiosStatus::Unknown => "Unknown",
        }
    }
}

pub struct NagiosOutput {
    pub status: NagiosStatus,
    pub message: String,
    pub perfdata: Option<String>,
}

impl fmt::Display for NagiosOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UPDATE {} - {}", self.status.as_str(), self.message)?;

        if let Some(perfdata) = &self.perfdata {
            write!(f, " | {}", perfdata)?;
        }

        Ok(())
    }
}
