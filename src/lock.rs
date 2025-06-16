use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub struct FileLock {
    file: File,
}

impl FileLock {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .context("Failed to open lock file")?;

        Ok(Self { file })
    }

    pub fn try_lock(&mut self) -> Result<bool> {
        match self.file.try_lock_exclusive() {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
            Err(e) => Err(e).context("Failed to acquire lock"),
        }
    }

    pub fn read_timestamp(&mut self) -> Result<Option<DateTime<Local>>> {
        let mut contents = String::new();
        self.file.seek(SeekFrom::Start(0))?;
        self.file.read_to_string(&mut contents)?;

        if contents.is_empty() {
            return Ok(None);
        }

        let timestamp = contents
            .trim()
            .parse::<i64>()
            .context("Failed to parse timestamp")?;

        let datetime = DateTime::from_timestamp(timestamp, 0)
            .context("Invalid timestamp")?
            .with_timezone(&Local);

        Ok(Some(datetime))
    }

    pub fn write_timestamp(&mut self, time: DateTime<Local>) -> Result<()> {
        self.file.seek(SeekFrom::Start(0))?;
        self.file.set_len(0)?;
        write!(self.file, "{}", time.timestamp())?;
        self.file.flush()?;
        Ok(())
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}
