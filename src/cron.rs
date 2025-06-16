use anyhow::{Context, Result, bail};
use chrono::{DateTime, Datelike, Local, Timelike};

#[derive(Debug, Clone)]
pub struct CronSpec {
    pub minute: CronField,
    pub hour: CronField,
    pub day: CronField,
    pub month: CronField,
}

#[derive(Debug, Clone)]
pub enum CronField {
    Any,
    Value(i32),
    Step(i32),
}

impl CronField {
    fn fix(&self, value: i32) -> i32 {
        match self {
            CronField::Any => value,
            CronField::Value(v) => *v,
            CronField::Step(step) => value - (value % step),
        }
    }
}

pub fn parse_cron_spec(spec: &str) -> Result<CronSpec> {
    let spec = match spec {
        "@hourly" => "0 * * *",
        "@daily" | "@midnight" => "0 0 * *",
        "@monthly" => "0 0 1 *",
        "@annually" | "@yearly" => "0 0 1 1",
        _ => spec,
    };

    let parts: Vec<&str> = spec.split_whitespace().collect();
    if parts.len() != 4 {
        bail!("Invalid cron spec: expected 4 fields");
    }

    Ok(CronSpec {
        minute: parse_field(parts[0], 0, 59)?,
        hour: parse_field(parts[1], 0, 23)?,
        day: parse_field(parts[2], 1, 31)?,
        month: parse_field(parts[3], 1, 12)?,
    })
}

fn parse_field(field: &str, min: i32, max: i32) -> Result<CronField> {
    if field == "*" {
        Ok(CronField::Any)
    } else if let Some(step_str) = field.strip_prefix("*/") {
        let step = step_str.parse::<i32>().context("Invalid step value")?;
        if step <= 0 {
            bail!("Step value must be positive");
        }
        Ok(CronField::Step(step))
    } else {
        let value = field.parse::<i32>().context("Invalid numeric value")?;
        if value < min || value > max {
            bail!("Value {} out of range [{}, {}]", value, min, max);
        }
        Ok(CronField::Value(value))
    }
}

pub fn should_run(spec: &str, last_run: DateTime<Local>, now: DateTime<Local>) -> Result<bool> {
    let cron = parse_cron_spec(spec)?;
    let last_period = calculate_last_period(&cron, now);

    Ok(last_period > last_run)
}

fn calculate_last_period(cron: &CronSpec, now: DateTime<Local>) -> DateTime<Local> {
    let minute = cron.minute.fix(now.minute() as i32) as u32;
    let hour = cron.hour.fix(now.hour() as i32) as u32;
    let day = cron.day.fix(now.day() as i32) as u32;
    let month = cron.month.fix(now.month() as i32) as u32;

    now.with_second(0)
        .unwrap()
        .with_minute(minute)
        .unwrap_or(now)
        .with_hour(hour)
        .unwrap_or(now)
        .with_day(day)
        .unwrap_or(now)
        .with_month(month)
        .unwrap_or(now)
}
