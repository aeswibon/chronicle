use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// Exclusive process lock — released automatically when the daemon exits.
pub struct DaemonLock {
    _file: std::fs::File,
}

impl DaemonLock {
    pub fn acquire(lock_path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(lock_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let ret = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if ret != 0 {
                anyhow::bail!(
                    "another chronicle daemon is already running (lock: {})",
                    lock_path.display()
                );
            }
        }

        #[cfg(not(unix))]
        {
            anyhow::bail!("chronicle daemon requires a unix platform");
        }

        let mut file = file;
        writeln!(file, "{}", std::process::id())?;

        Ok(Self { _file: file })
    }
}
