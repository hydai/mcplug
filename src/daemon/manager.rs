use std::path::PathBuf;

use crate::error::McplugError;

/// Status information for the daemon.
#[derive(Debug, serde::Serialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub managed_servers: Vec<String>,
}

/// Manages the mcplug background daemon process.
pub struct DaemonManager {
    socket_path: PathBuf,
    pid_file: PathBuf,
}

impl DaemonManager {
    pub fn new() -> Self {
        let base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".mcplug");
        Self {
            socket_path: base.join("daemon.sock"),
            pid_file: base.join("daemon.pid"),
        }
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    pub fn pid_file(&self) -> &PathBuf {
        &self.pid_file
    }

    pub fn is_running(&self) -> bool {
        if let Ok(pid_str) = std::fs::read_to_string(&self.pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                // Check if process exists (signal 0)
                unsafe {
                    return libc_kill(pid as i32, 0) == 0;
                }
            }
        }
        false
    }

    pub async fn start(
        &self,
        _server: Option<&str>,
        _log: bool,
    ) -> Result<(), McplugError> {
        if self.is_running() {
            eprintln!("Daemon is already running");
            return Ok(());
        }
        // TODO: Fork/spawn background process, write PID file, start managing keep-alive servers
        eprintln!("Daemon started (stub — full implementation requires fork/daemonize)");
        Ok(())
    }

    pub async fn stop(&self, _server: Option<&str>) -> Result<(), McplugError> {
        if !self.is_running() {
            eprintln!("Daemon is not running");
            return Ok(());
        }
        if let Ok(pid_str) = std::fs::read_to_string(&self.pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                unsafe {
                    libc_kill(pid, 15); // SIGTERM
                }
                let _ = std::fs::remove_file(&self.pid_file);
                let _ = std::fs::remove_file(&self.socket_path);
                eprintln!("Daemon stopped");
            }
        }
        Ok(())
    }

    pub async fn restart(
        &self,
        server: Option<&str>,
        log: bool,
    ) -> Result<(), McplugError> {
        self.stop(server).await?;
        self.start(server, log).await
    }

    pub async fn status(&self) -> Result<DaemonStatus, McplugError> {
        let running = self.is_running();
        let pid = if running {
            std::fs::read_to_string(&self.pid_file)
                .ok()
                .and_then(|s| s.trim().parse().ok())
        } else {
            None
        };
        Ok(DaemonStatus {
            running,
            pid,
            uptime_secs: None, // Would need start time tracking
            managed_servers: vec![],
        })
    }
}

// Minimal libc kill binding to avoid full libc dependency
extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

unsafe fn libc_kill(pid: i32, sig: i32) -> i32 {
    unsafe { kill(pid, sig) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_manager_paths() {
        let dm = DaemonManager::new();
        assert!(dm.socket_path().ends_with("daemon.sock"));
        assert!(dm.pid_file().ends_with("daemon.pid"));
    }

    #[test]
    fn daemon_not_running_when_no_pid_file() {
        let dm = DaemonManager {
            socket_path: PathBuf::from("/tmp/nonexistent.sock"),
            pid_file: PathBuf::from("/tmp/nonexistent.pid"),
        };
        assert!(!dm.is_running());
    }

    #[tokio::test]
    async fn daemon_status_when_not_running() {
        let dm = DaemonManager {
            socket_path: PathBuf::from("/tmp/nonexistent.sock"),
            pid_file: PathBuf::from("/tmp/nonexistent.pid"),
        };
        let status = dm.status().await.unwrap();
        assert!(!status.running);
        assert!(status.pid.is_none());
    }
}
