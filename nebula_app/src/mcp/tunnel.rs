//! Managed official helper, never an implementation of OpenAI's tunnel protocol.
use super::*;
use std::collections::HashSet;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use zeroize::Zeroizing;

// Two helpers polling one tunnel would deliver requests to arbitrary terminals.
// Keep the lease until the child has actually exited, not until its UI closes.
static TUNNELS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
struct Lease(String);
impl Lease {
    fn acquire(id: &str) -> Option<Self> {
        TUNNELS
            .get_or_init(Mutex::default)
            .lock()
            .unwrap()
            .insert(id.to_owned())
            .then(|| Self(id.to_owned()))
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        TUNNELS.get().unwrap().lock().unwrap().remove(&self.0);
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Status {
    Starting,
    Running,
    Stopped,
    Failed(String),
}

pub(crate) struct Tunnel {
    cancel: CancellationToken,
}

impl Tunnel {
    pub fn start(
        runtime: &tokio::runtime::Handle,
        share: &Share,
        executable: String,
        id: String,
        key: Zeroizing<String>,
    ) -> (Self, UnboundedReceiver<Status>) {
        let cancel = share.cancel.child_token();
        let stopped = cancel.clone();
        let (sender, receiver) = unbounded();
        let url = share.url.clone();
        let auth = Zeroizing::new(format!("Bearer {}", share.token));
        runtime.spawn(async move {
            let Some(_lease) = Lease::acquire(&id) else {
                let _ = sender.unbounded_send(Status::Failed(
                    "Tunnel ID is already attached to another terminal".into(),
                ));
                return;
            };
            let mut command = tokio::process::Command::new(executable);
            command
                .arg("run")
                .env_remove("TUNNEL_CLIENT_CONFIG")
                .env_remove("TUNNEL_CLIENT_PROFILE")
                .env_remove("TUNNEL_CLIENT_PROFILE_FILE")
                .env_remove("MCP_COMMAND")
                .env("CONTROL_PLANE_BASE_URL", "https://api.openai.com")
                .env("CONTROL_PLANE_TUNNEL_ID", id)
                .env("CONTROL_PLANE_API_KEY", key.as_str())
                .env("MCP_SERVER_URL", url)
                .env("MCP_EXTRA_HEADERS", "Authorization: env:PEBREL_MCP_AUTH")
                .env("PEBREL_MCP_AUTH", auth.as_str())
                .env("HEALTH_LISTEN_ADDR", "127.0.0.1:0")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true);
            #[cfg(windows)]
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
            if stopped.is_cancelled() {
                return;
            }
            let child = command.spawn();
            drop(command);
            drop(key);
            drop(auth);
            let status = match child {
                Err(error) => Status::Failed(error.to_string()),
                Ok(mut child) => {
                    // A running process is not evidence of an authenticated tunnel.
                    let _ = sender.unbounded_send(Status::Running);
                    tokio::select! {
                        _ = stopped.cancelled() => {
                            let _ = child.kill().await;
                            Status::Stopped
                        },
                        result = child.wait() => match result {
                            Ok(status) if status.success() => Status::Stopped,
                            Ok(status) => Status::Failed(status.to_string()),
                            Err(error) => Status::Failed(error.to_string()),
                        },
                    }
                },
            };
            let _ = sender.unbounded_send(status);
        });
        (Self { cancel }, receiver)
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::Lease;
    #[test]
    fn one_terminal_per_tunnel_until_child_exit() {
        let first = Lease::acquire("test-exclusive-tunnel").unwrap();
        assert!(Lease::acquire("test-exclusive-tunnel").is_none());
        drop(first);
        assert!(Lease::acquire("test-exclusive-tunnel").is_some());
    }
}
