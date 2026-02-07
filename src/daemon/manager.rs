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

impl Default for DaemonManager {
    fn default() -> Self {
        Self::new()
    }
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
        #[cfg(unix)]
        {
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
        #[cfg(not(unix))]
        {
            false
        }
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
        #[cfg(unix)]
        {
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
        }
        #[cfg(not(unix))]
        {
            eprintln!("Daemon stop is not supported on Windows");
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
#[cfg(unix)]
extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[cfg(unix)]
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

    #[cfg(unix)]
    #[test]
    fn daemon_not_running_when_no_pid_file() {
        let dm = DaemonManager {
            socket_path: PathBuf::from("/tmp/nonexistent.sock"),
            pid_file: PathBuf::from("/tmp/nonexistent.pid"),
        };
        assert!(!dm.is_running());
    }

    #[cfg(unix)]
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

    #[cfg(unix)]
    #[tokio::test]
    async fn daemon_start_when_not_running() {
        let dm = DaemonManager {
            socket_path: PathBuf::from("/tmp/mcplug_test_start.sock"),
            pid_file: PathBuf::from("/tmp/mcplug_test_start.pid"),
        };
        // start() should succeed (stub prints message)
        let result = dm.start(None, false).await;
        assert!(result.is_ok());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn daemon_stop_when_not_running() {
        let dm = DaemonManager {
            socket_path: PathBuf::from("/tmp/mcplug_test_stop.sock"),
            pid_file: PathBuf::from("/tmp/mcplug_test_stop.pid"),
        };
        // stop when not running should be a no-op (Ok)
        let result = dm.stop(None).await;
        assert!(result.is_ok());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn daemon_restart_when_not_running() {
        let dm = DaemonManager {
            socket_path: PathBuf::from("/tmp/mcplug_test_restart.sock"),
            pid_file: PathBuf::from("/tmp/mcplug_test_restart.pid"),
        };
        // restart when not running should succeed (stop is no-op, start succeeds)
        let result = dm.restart(None, false).await;
        assert!(result.is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn daemon_is_running_with_stale_pid() {
        // Write a PID that doesn't correspond to a running process
        let pid_path = PathBuf::from("/tmp/mcplug_test_stale.pid");
        std::fs::write(&pid_path, "99999999").unwrap();
        let dm = DaemonManager {
            socket_path: PathBuf::from("/tmp/mcplug_test_stale.sock"),
            pid_file: pid_path.clone(),
        };
        // PID 99999999 should not exist
        assert!(!dm.is_running());
        let _ = std::fs::remove_file(&pid_path);
    }

    #[cfg(unix)]
    #[test]
    fn daemon_is_running_with_invalid_pid_content() {
        let pid_path = PathBuf::from("/tmp/mcplug_test_invalid_pid.pid");
        std::fs::write(&pid_path, "not_a_number").unwrap();
        let dm = DaemonManager {
            socket_path: PathBuf::from("/tmp/mcplug_test_invalid_pid.sock"),
            pid_file: pid_path.clone(),
        };
        assert!(!dm.is_running());
        let _ = std::fs::remove_file(&pid_path);
    }

    #[cfg(unix)]
    #[test]
    fn daemon_is_running_with_empty_pid_file() {
        let pid_path = PathBuf::from("/tmp/mcplug_test_empty_pid.pid");
        std::fs::write(&pid_path, "").unwrap();
        let dm = DaemonManager {
            socket_path: PathBuf::from("/tmp/mcplug_test_empty_pid.sock"),
            pid_file: pid_path.clone(),
        };
        assert!(!dm.is_running());
        let _ = std::fs::remove_file(&pid_path);
    }

    #[test]
    fn daemon_status_structure() {
        let status = DaemonStatus {
            running: true,
            pid: Some(1234),
            uptime_secs: Some(600),
            managed_servers: vec!["server-a".to_string(), "server-b".to_string()],
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["running"], true);
        assert_eq!(json["pid"], 1234);
        assert_eq!(json["uptime_secs"], 600);
        assert_eq!(json["managed_servers"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn daemon_manager_new_uses_home_dir() {
        let dm = DaemonManager::new();
        let socket_str = dm.socket_path().to_string_lossy().to_string();
        let pid_str = dm.pid_file().to_string_lossy().to_string();
        assert!(socket_str.contains(".mcplug"));
        assert!(pid_str.contains(".mcplug"));
    }
}
