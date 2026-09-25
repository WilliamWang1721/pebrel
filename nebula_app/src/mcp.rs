//! Per-terminal MCP capabilities. The UI owns execution and approval; the transport
//! never receives a global Runtime credential or a caller-selectable pane id.
mod http;
#[cfg(test)]
mod tests;
pub(crate) mod tunnel;

use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::{Semaphore, oneshot};
use tokio_util::sync::CancellationToken;

pub(crate) use http::Host;

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Operation {
    Read {
        #[serde(default = "default_lines")]
        lines: usize,
    },
    Run {
        command: String,
    },
    Input {
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        key: Option<crate::runtime_api::RuntimeKey>,
        #[serde(default)]
        modifiers: crate::runtime_api::RuntimeKeyModifiers,
        #[serde(default)]
        submit: bool,
    },
}

fn default_lines() -> usize {
    120
}

impl Operation {
    pub fn is_write(&self) -> bool {
        !matches!(self, Self::Read { .. })
    }

    pub fn validate(&self) -> Result<(), String> {
        use crate::runtime_api::{validate_command_line, validate_paste_text};
        match self {
            Self::Read { lines } if !(1..=4000).contains(lines) => {
                Err("lines must be 1..4000".into())
            },
            Self::Run { command } => validate_command_line(command).map_err(|e| e.message),
            Self::Input { text: Some(text), key: None, modifiers, .. }
                if *modifiers == Default::default() =>
            {
                validate_paste_text(text).map_err(|e| e.message)
            },
            Self::Input { text: None, key: Some(key), modifiers, submit: false }
                if key.letter().is_none() || modifiers.control || modifiers.alt =>
            {
                Ok(())
            },
            Self::Input { .. } => {
                Err("provide text or one named key; modifiers apply only to keys".into())
            },
            _ => Ok(()),
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::Run { command } => command.clone(),
            Self::Input { text: Some(text), submit, .. } => {
                format!("Paste (submit={submit}):\n{text}")
            },
            Self::Input { key, modifiers, .. } => format!("Key: {key:?}, modifiers: {modifiers:?}"),
            Self::Read { .. } => String::new(),
        }
    }
}

static NEXT_CALL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub(crate) struct Call {
    pub id: u64,
    pub operation: Operation,
    pub cancel: CancellationToken,
    deadline: Instant,
    reply: oneshot::Sender<Result<Value, String>>,
}

impl Call {
    pub fn is_live(&self) -> bool {
        !self.cancel.is_cancelled() && !self.reply.is_closed() && Instant::now() < self.deadline
    }

    pub fn respond(self, result: Result<Value, String>) {
        let _ = self.reply.send(result);
    }
}

pub(crate) struct Share {
    pub id: String,
    pub url: String,
    pub token: String,
    pub cancel: CancellationToken,
    sender: UnboundedSender<Call>,
    writer: Semaphore,
    requests: Semaphore,
}

impl Share {
    fn new(
        port: u16,
        parent: &CancellationToken,
    ) -> std::io::Result<(Arc<Self>, UnboundedReceiver<Call>)> {
        let id = random_secret()?;
        let (sender, receiver) = unbounded();
        let share = Arc::new(Self {
            url: format!("http://127.0.0.1:{port}/mcp/{id}"),
            id,
            token: random_secret()?,
            cancel: parent.child_token(),
            sender,
            writer: Semaphore::new(1),
            requests: Semaphore::new(16),
        });
        Ok((share, receiver))
    }

    pub fn stop(&self) {
        self.cancel.cancel();
    }

    async fn invoke(
        &self,
        operation: Operation,
        client_cancel: &CancellationToken,
    ) -> Result<Value, String> {
        operation.validate()?;
        let _capacity = self.requests.try_acquire().map_err(|_| "too many requests")?;
        let _writer = operation
            .is_write()
            .then(|| self.writer.try_acquire())
            .transpose()
            .map_err(|_| "another terminal operation is awaiting approval or submission")?;
        let cancel = self.cancel.child_token();
        let _cancel_on_drop = cancel.clone().drop_guard();
        let (reply, result) = oneshot::channel();
        let timeout = Duration::from_secs(300);
        if cancel.is_cancelled() || client_cancel.is_cancelled() {
            return Err("sharing stopped".into());
        }
        self.sender
            .unbounded_send(Call {
                id: NEXT_CALL.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                operation,
                cancel: cancel.clone(),
                deadline: Instant::now() + timeout,
                reply,
            })
            .map_err(|_| "terminal closed")?;
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err("sharing stopped; unsubmitted operations cancelled".into()),
            _ = client_cancel.cancelled() => Err("request cancelled".into()),
            result = tokio::time::timeout(timeout, result) => result
                .map_err(|_| "approval expired; do not retry a submission with an unknown outcome".to_owned())?
                .map_err(|_| "terminal closed".to_owned())?,
        }
    }
}

impl Drop for Share {
    fn drop(&mut self) {
        self.stop();
    }
}

fn random_secret() -> std::io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// A button can resolve only the exact immutable request it displayed.
pub(crate) fn take_approval(
    pending: &mut Option<(Call, u64)>,
    id: u64,
    epoch: u64,
) -> Option<Call> {
    if pending.as_ref()?.0.id != id {
        return None;
    }
    let (call, expected) = pending.take()?;
    if !call.is_live() || epoch != expected {
        call.respond(Err("request expired or terminal input changed; submit a new request".into()));
        None
    } else {
        Some(call)
    }
}
