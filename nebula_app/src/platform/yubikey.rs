//! PIV tooling adapter. No private-key writes, PIN storage, shell evaluation,
//! persistent agent, or claim that Pebrel's russh authentication is connected.
//! Call asynchronous probes on the existing Tokio runtime, never from Render.

use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use russh::keys::ssh_key::PublicKey;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::task::JoinHandle;

const OUTPUT_LIMIT: usize = 64 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const AUTH_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tool {
    Ykman,
    SshKeygen,
    SshAdd,
    Ykcs11,
}

/// Typed failures: the UI owns localization. Never include raw subprocess
/// stderr in a log or a success result.
#[derive(Debug)]
pub(crate) enum PivError {
    UnsupportedPlatform,
    MissingTool(Tool),
    InvalidPath(Tool),
    MissingAgent,
    NoDevice,
    MultipleDevices,
    DeviceChanged,
    InvalidOutput,
    NoPublicKeys,
    NoMatchingAgentKey,
    TerminalRequired,
    OutputLimit,
    Timeout,
    CommandFailed(Option<i32>),
    Io(io::Error),
}

impl From<io::Error> for PivError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AgentContext {
    socket: PathBuf,
}

impl AgentContext {
    /// Capture once and reuse for loading and verification. This does not
    /// launch an agent or mutate process-global environment variables.
    pub(crate) fn from_environment() -> Result<Self, PivError> {
        require_supported_platform()?;
        let socket = std::env::var_os("SSH_AUTH_SOCK")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or(PivError::MissingAgent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt as _;
            if !std::fs::metadata(&socket).map(|m| m.file_type().is_socket()).unwrap_or(false) {
                return Err(PivError::MissingAgent);
            }
        }
        Ok(Self { socket })
    }

    pub(crate) fn socket(&self) -> &Path {
        &self.socket
    }

    fn apply(&self, command: &mut Command) {
        command.env("SSH_AUTH_SOCK", &self.socket);
        command.env("SSH_ASKPASS_REQUIRE", "never");
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PivTools {
    ykman: PathBuf,
    ssh_keygen: PathBuf,
    ssh_add: PathBuf,
    provider: PathBuf,
}

#[derive(Debug)]
pub(crate) struct DeviceInfo {
    pub(crate) serial: String,
    pub(crate) piv_info: String,
}

impl PivTools {
    /// Fixed installation prefixes avoid resolving tools from the current
    /// directory. Nonstandard, explicitly selected installations use from_paths.
    pub(crate) fn discover() -> Result<Self, PivError> {
        require_supported_platform()?;
        let binary = |name: &str, tool| {
            ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"]
                .into_iter()
                .map(|dir| Path::new(dir).join(name))
                .find(|path| path.is_file())
                .ok_or(PivError::MissingTool(tool))
        };
        let provider = [
            "/opt/homebrew/lib/libykcs11.dylib",
            "/usr/local/lib/libykcs11.dylib",
            "/usr/local/lib/libykcs11.so",
            "/usr/lib/x86_64-linux-gnu/libykcs11.so",
            "/usr/lib/aarch64-linux-gnu/libykcs11.so",
            "/usr/lib/libykcs11.so",
            "/usr/lib64/libykcs11.so",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .ok_or(PivError::MissingTool(Tool::Ykcs11))?;
        Self::from_paths(
            binary("ykman", Tool::Ykman)?,
            binary("ssh-keygen", Tool::SshKeygen)?,
            binary("ssh-add", Tool::SshAdd)?,
            provider,
        )
    }

    /// The caller must select a trusted YKCS11 library: loading a provider
    /// executes native code. Paths from remote hosts must never be used here.
    pub(crate) fn from_paths(
        ykman: PathBuf,
        ssh_keygen: PathBuf,
        ssh_add: PathBuf,
        provider: PathBuf,
    ) -> Result<Self, PivError> {
        require_supported_platform()?;
        for (path, tool) in [
            (&ykman, Tool::Ykman),
            (&ssh_keygen, Tool::SshKeygen),
            (&ssh_add, Tool::SshAdd),
            (&provider, Tool::Ykcs11),
        ] {
            if !path.is_absolute() || !path.is_file() {
                return Err(PivError::InvalidPath(tool));
            }
        }
        Ok(Self { ykman, ssh_keygen, ssh_add, provider })
    }

    async fn single_serial(&self) -> Result<String, PivError> {
        let mut command = Command::new(&self.ykman);
        command.args(["list", "--serials"]);
        parse_single_serial(&capture(command, PROBE_TIMEOUT).await?)
    }

    pub(crate) async fn device_info(&self) -> Result<DeviceInfo, PivError> {
        let serial = self.single_serial().await?;
        let mut command = Command::new(&self.ykman);
        command.args(["--device", serial.as_str(), "piv", "info"]);
        let piv_info = capture(command, PROBE_TIMEOUT).await?;
        if piv_info.trim().is_empty() {
            return Err(PivError::InvalidOutput);
        }
        Ok(DeviceInfo { serial, piv_info })
    }

    fn export_command(&self) -> Command {
        let mut command = Command::new(&self.ssh_keygen);
        command.arg("-D").arg(&self.provider);
        command.env("SSH_ASKPASS_REQUIRE", "never");
        command
    }

    fn agent_command(&self, agent: &AgentContext, load: bool) -> Command {
        let mut command = Command::new(&self.ssh_add);
        if load {
            command.arg("-s").arg(&self.provider);
        } else {
            command.arg("-L");
        }
        agent.apply(&mut command);
        command
    }

    /// Return all public keys, without inventing a mapping from output order
    /// to PIV slots. No private key is read or written.
    pub(crate) async fn export_public_keys(&self) -> Result<Vec<String>, PivError> {
        let serial = self.single_serial().await?;
        let keys = parse_public_keys(&capture(self.export_command(), PROBE_TIMEOUT).await?)?;
        if self.single_serial().await? != serial {
            return Err(PivError::DeviceChanged);
        }
        Ok(keys)
    }

    /// This entry requires an actual interactive terminal. A GUI must first
    /// supply its PTY bridge, not capture PIN input in a text box or log it.
    /// Returned keys prove agent loading only, not an authenticated SSH session.
    pub(crate) async fn activate_in_terminal(
        &self,
        agent: &AgentContext,
    ) -> Result<Vec<String>, PivError> {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return Err(PivError::TerminalRequired);
        }
        let expected = self.export_public_keys().await?;
        let mut command = tokio::process::Command::from(self.agent_command(agent, true));
        command.kill_on_drop(true);
        command.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
        let mut child = command.spawn()?;
        let status = match tokio::time::timeout(AUTH_TIMEOUT, child.wait()).await {
            Ok(result) => result?,
            Err(_) => {
                stop_child(&mut child).await;
                return Err(PivError::Timeout);
            },
        };
        if !status.success() {
            return Err(PivError::CommandFailed(status.code()));
        }
        self.verify_agent(agent, &expected).await
    }

    pub(crate) async fn verify_agent(
        &self,
        agent: &AgentContext,
        expected: &[String],
    ) -> Result<Vec<String>, PivError> {
        let loaded = parse_public_keys(
            &capture(self.agent_command(agent, false), PROBE_TIMEOUT).await?,
        )?;
        let matching = loaded.into_iter().filter(|key| expected.contains(key)).collect::<Vec<_>>();
        if matching.is_empty() {
            return Err(PivError::NoMatchingAgentKey);
        }
        Ok(matching)
    }
}

fn require_supported_platform() -> Result<(), PivError> {
    if cfg!(any(target_os = "macos", target_os = "linux")) {
        Ok(())
    } else {
        Err(PivError::UnsupportedPlatform)
    }
}

fn parse_single_serial(output: &str) -> Result<String, PivError> {
    let serials = output.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>();
    match serials.as_slice() {
        [] => Err(PivError::NoDevice),
        [serial] if serial.len() <= 20 && serial.bytes().all(|byte| byte.is_ascii_digit()) => {
            Ok((*serial).to_owned())
        },
        [_] => Err(PivError::InvalidOutput),
        _ => Err(PivError::MultipleDevices),
    }
}

fn parse_public_keys(output: &str) -> Result<Vec<String>, PivError> {
    let mut keys = Vec::new();
    for line in output.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let mut key = PublicKey::from_openssh(line).map_err(|_| PivError::InvalidOutput)?;
        // Comments can differ between ssh-keygen and ssh-add. Compare actual
        // parsed key material and never assign the first key to slot 9a.
        key.set_comment("");
        let key = key.to_openssh().map_err(|_| PivError::InvalidOutput)?;
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    if keys.is_empty() {
        return Err(PivError::NoPublicKeys);
    }
    Ok(keys)
}

struct Readers {
    stdout: JoinHandle<Result<Vec<u8>, PivError>>,
    stderr: JoinHandle<Result<Vec<u8>, PivError>>,
}

impl Drop for Readers {
    fn drop(&mut self) {
        self.stdout.abort();
        self.stderr.abort();
    }
}

async fn read_bounded(stream: impl AsyncRead + Unpin) -> Result<Vec<u8>, PivError> {
    let mut bytes = Vec::new();
    stream.take((OUTPUT_LIMIT + 1) as u64).read_to_end(&mut bytes).await?;
    if bytes.len() > OUTPUT_LIMIT {
        return Err(PivError::OutputLimit);
    }
    Ok(bytes)
}

async fn stop_child(child: &mut tokio::process::Child) {
    let _ = child.start_kill();
    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
}

/// Only noninteractive, read-only probes use capture. Concurrent bounded reads
/// prevent pipe deadlocks; dropping the future kills the child and reader tasks.
async fn capture(mut command: Command, timeout: Duration) -> Result<String, PivError> {
    super::process::hidden_command(&mut command);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut command = tokio::process::Command::from(command);
    command.kill_on_drop(true);
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().ok_or(PivError::InvalidOutput)?;
    let stderr = child.stderr.take().ok_or(PivError::InvalidOutput)?;
    let mut readers = Readers {
        stdout: tokio::spawn(read_bounded(stdout)),
        stderr: tokio::spawn(read_bounded(stderr)),
    };
    let operation = async {
        let status = child.wait().await?;
        let stdout = (&mut readers.stdout).await.map_err(io::Error::other)??;
        let _stderr = (&mut readers.stderr).await.map_err(io::Error::other)??;
        if !status.success() {
            return Err(PivError::CommandFailed(status.code()));
        }
        String::from_utf8(stdout).map_err(|_| PivError::InvalidOutput)
    };
    match tokio::time::timeout(timeout, operation).await {
        Ok(result) => result,
        Err(_) => {
            stop_child(&mut child).await;
            Err(PivError::Timeout)
        },
    }
}

#[cfg(test)]
mod tests;
