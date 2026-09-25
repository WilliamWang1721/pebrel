//! The setting owns the listener lifetime; terminal views own individual shares.
use super::config::Settings;
use gpui::{App, Global};

#[derive(Default)]
pub(crate) struct McpService {
    pub host: Option<crate::mcp::Host>,
    pub error: Option<String>,
}
impl Global for McpService {}

pub(crate) fn init(cx: &mut App) {
    cx.set_global(McpService::default());
    sync(cx);
    cx.observe_global::<Settings>(sync).detach();
}

fn sync(cx: &mut App) {
    let enabled = cx.global::<Settings>().mcp_enabled;
    let service = cx.global_mut::<McpService>();
    if !enabled {
        service.host = None;
        service.error = None;
    } else if service.host.is_none() {
        match crate::mcp::Host::start() {
            Ok(host) => {
                service.host = Some(host);
                service.error = None;
            },
            Err(error) => {
                service.error = Some(error.to_string());
            },
        }
    }
}
