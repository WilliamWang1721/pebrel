//! Local approval and visible execution for one terminal. No global pane routing.
use super::*;
use crate::gpui_shell::{copy_feedback::CopyFeedback, mcp::McpService, prelude::*};
use crate::i18n::Message;
use crate::mcp::{
    Call, Operation, Share,
    tunnel::{Status, Tunnel},
};
use futures::{FutureExt as _, StreamExt as _};
use gpui::{Entity, Subscription, Task};
use serde_json::json;

pub(super) struct Sharing {
    pub share: Arc<Share>,
    full: bool,
    expanded: bool,
    pending: Option<(Call, u64)>,
    _requests: Option<Task<()>>,
    expiry: Option<Task<()>>,
    helper: Option<Tunnel>,
    helper_task: Option<Task<()>>,
    helper_status: Status,
    executable: Entity<InputState>,
    tunnel_id: Entity<InputState>,
    runtime_key: Entity<InputState>,
    copy: Entity<CopyFeedback>,
    _copy_observer: Subscription,
}

impl Drop for Sharing {
    fn drop(&mut self) {
        self.share.stop();
    }
}

impl TerminalView {
    pub(crate) fn expose_to_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(sharing) = &mut self.mcp {
            sharing.expanded = !sharing.expanded;
            cx.notify();
            return;
        }
        if self.ssh_destination.is_some() {
            cx.emit(TerminalViewEvent::Notification(crate::notify::Notification::Text {
                body: crate::gpui_shell::config::ui_language(cx).text(Message::McpLocalOnly).into(),
                program: None,
            }));
            return;
        }
        let service = cx.global::<McpService>();
        let result = service
            .host
            .as_ref()
            .ok_or_else(|| service.error.clone().unwrap_or_else(|| "MCP is disabled".into()))
            .and_then(|host| host.share().map_err(|e| e.to_string()));
        let (share, mut receiver) = match result {
            Ok(value) => value,
            Err(body) => {
                cx.emit(TerminalViewEvent::Notification(crate::notify::Notification::Text {
                    body,
                    program: None,
                }));
                return;
            },
        };
        let language = crate::gpui_shell::config::ui_language(cx);
        let executable = cx.new(|cx| InputState::new(window, cx).default_value("tunnel-client"));
        let tunnel_id = cx
            .new(|cx| InputState::new(window, cx).placeholder(language.text(Message::McpTunnelId)));
        let runtime_key = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder(language.text(Message::McpRuntimeKey))
        });
        let cancel = share.cancel.clone();
        let requests = cx.spawn(async move |this, cx| {
            loop {
                let next = receiver.next().fuse();
                let stopped = cancel.cancelled().fuse();
                futures::pin_mut!(next, stopped);
                futures::select_biased! {
                    _ = stopped => {
                        let _ = this.update(cx, |view, cx| { view.mcp = None; cx.notify(); });
                        break;
                    },
                    call = next => match call {
                        Some(call) => { if this.update(cx, |view, cx| view.receive_mcp(call, cx)).is_err() { break; } },
                        None => break,
                    },
                }
            }
        });
        let copy = cx.new(|_| CopyFeedback::new());
        let copy_observer = cx.observe(&copy, |_, _, cx| cx.notify());
        self.mcp = Some(Sharing {
            share,
            full: false,
            expanded: true,
            pending: None,
            _requests: Some(requests),
            expiry: None,
            helper: None,
            helper_task: None,
            helper_status: Status::Stopped,
            executable,
            tunnel_id,
            runtime_key,
            copy,
            _copy_observer: copy_observer,
        });
        cx.notify();
    }

    fn receive_mcp(&mut self, call: Call, cx: &mut Context<Self>) {
        if !call.is_live() {
            return;
        }
        let Some(sharing) = &mut self.mcp else {
            call.respond(Err("sharing stopped".into()));
            return;
        };
        if sharing.share.cancel.is_cancelled() {
            call.respond(Err("sharing stopped".into()));
            return;
        }
        if !call.operation.is_write() || sharing.full {
            let result = self.execute_mcp(&call.operation, cx);
            call.respond(result);
        } else {
            let body = call.operation.description();
            let cancel = call.cancel.clone();
            sharing.pending = Some((call, self.prompt_input_epoch));
            sharing.expiry = Some(cx.spawn(async move |this, cx| {
                cancel.cancelled().await;
                let _ = this.update(cx, |view, cx| {
                    if let Some(sharing) = &mut view.mcp {
                        if sharing.pending.as_ref().is_some_and(|(call, _)| !call.is_live()) {
                            sharing.pending = None;
                            cx.notify();
                        }
                    }
                });
            }));
            cx.emit(TerminalViewEvent::Notification(crate::notify::Notification::RemoteApproval {
                body,
            }));
        }
        cx.notify();
    }

    fn execute_mcp(
        &mut self,
        operation: &Operation,
        cx: &mut Context<Self>,
    ) -> Result<serde_json::Value, String> {
        use crate::runtime_api::ApiError;
        let result: Result<_, ApiError> = match operation {
            Operation::Read { lines } => self.runtime_read(0, *lines).map(|read| json!({
                "text":read.text,"returned_lines":read.returned_lines,"history_available":read.history_available,
                "truncated":read.truncated,"state":read.task_state,"exited":read.exited,
                "active_run":self.runtime_active_run(),"last_run":self.runtime_last_run()
            })),
            Operation::Run { command } => self.runtime_run(command.clone(), cx).map(|id| json!({"submitted":true,"run_id":id})),
            Operation::Input { text: Some(text), submit, .. } if !text.contains(['\n', '\r', '\t']) => self.runtime_prompt(text.clone(), *submit, cx).map(|_| json!({"submitted":true})),
            Operation::Input { text: Some(text), submit, .. } => self.runtime_paste(text.clone(), *submit, InputOrigin::Program, cx).map(|_| json!({"submitted":true})),
            Operation::Input { key: Some(key), modifiers, .. } => self.runtime_send_key(*key, *modifiers, 1, cx).map(|bytes| json!({"bytes_sent":bytes})),
            _ => return Err("invalid terminal input".into()),
        };
        result.map_err(|e| format!("{}: {}", e.code, e.message))
    }

    fn approve_mcp(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(sharing) = &mut self.mcp else { return };
        let call = crate::mcp::take_approval(&mut sharing.pending, id, self.prompt_input_epoch);
        if let Some(call) = call {
            sharing.expiry = None;
            let result = self.execute_mcp(&call.operation, cx);
            call.respond(result);
        }
        cx.notify();
    }

    fn start_mcp_tunnel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sharing) = &mut self.mcp else { return };
        if sharing.helper.is_some() {
            sharing.helper = None;
            sharing.helper_task = None;
            sharing.helper_status = Status::Stopped;
            cx.notify();
            return;
        }
        let id = sharing.tunnel_id.read(cx).value().to_string();
        let key = zeroize::Zeroizing::new(sharing.runtime_key.read(cx).value().to_string());
        let executable = sharing.executable.read(cx).value().to_string();
        if id.trim().is_empty() || key.is_empty() || executable.trim().is_empty() {
            sharing.helper_status = Status::Failed(
                crate::gpui_shell::config::ui_language(cx).text(Message::McpTunnelRequired).into(),
            );
            cx.notify();
            return;
        }
        let Some(host) = cx.global::<McpService>().host.as_ref() else { return };
        let (helper, mut statuses) =
            Tunnel::start(&host.runtime, &sharing.share, executable, id, key);
        sharing.runtime_key.update(cx, |input, cx| input.set_value("", window, cx));
        sharing.helper = Some(helper);
        sharing.helper_status = Status::Starting;
        sharing.helper_task = Some(cx.spawn(async move |this, cx| {
            while let Some(status) = statuses.next().await {
                if this
                    .update(cx, |view, cx| {
                        if let Some(sharing) = &mut view.mcp {
                            if matches!(status, Status::Stopped | Status::Failed(_)) {
                                sharing.helper = None;
                            }
                            sharing.helper_status = status;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn copy_mcp_connection(&mut self, cx: &mut Context<Self>) {
        let Some(sharing) = &self.mcp else { return };
        let connection = json!({"url":sharing.share.url,"headers":{
            "Authorization":format!("Bearer {}",sharing.share.token)
        }})
        .to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(connection));
        sharing.copy.update(cx, |copy, cx| copy.mark_copied(cx));
    }

    pub(super) fn render_mcp(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let language = crate::gpui_shell::config::ui_language(cx);
        let Some(sharing) = &self.mcp else { return div().into_any_element() };
        let url = sharing.share.url.clone();
        let copy = sharing.copy.clone();
        let status = match &sharing.helper_status {
            Status::Starting => language.text(Message::McpHelperStarting).to_owned(),
            Status::Running => language.text(Message::McpHelperRunning).to_owned(),
            Status::Stopped => language.text(Message::McpHelperStopped).to_owned(),
            Status::Failed(error) => language.format(Message::McpHelperFailed, &[("error", error)]),
        };
        let header = h_flex()
            .gap_2()
            .flex_wrap()
            .items_center()
            .child(
                Button::new("mcp-expand")
                    .label(language.text(Message::McpSharing))
                    .ghost()
                    .small()
                    .on_click(cx.listener(|view, _, _, cx| {
                        if let Some(s) = &mut view.mcp {
                            s.expanded = !s.expanded;
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("mcp-stop")
                    .label(language.text(Message::McpStop))
                    .ghost()
                    .small()
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.mcp = None;
                        cx.notify();
                    })),
            );
        let mut panel = v_flex()
            .id("mcp-panel")
            .max_h(px(320.0))
            .overflow_y_scroll()
            .flex_shrink_0()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .p_2()
            .gap_2()
            .child(header);
        if sharing.expanded {
            panel = panel
                .child(div().text_xs().child(language.text(Message::McpWarning)))
                .child(h_flex().gap_2().flex_wrap().children([false, true].into_iter().map(
                    |full| {
                        Button::new(if full { "mcp-full" } else { "mcp-ask" })
                            .small()
                            .label(language.text(if full {
                                Message::McpFull
                            } else {
                                Message::McpAsk
                            }))
                            .selected(sharing.full == full)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if let Some(s) = &mut view.mcp {
                                    s.full = full;
                                    s.pending = None;
                                    s.expiry = None;
                                }
                                cx.notify();
                            }))
                    },
                )))
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().flex_1().min_w_0().text_xs().text_ellipsis().child(url))
                        .child(
                            Button::new("mcp-copy")
                                .icon(if copy.read(cx).is_copied() {
                                    IconName::Check
                                } else {
                                    IconName::Copy
                                })
                                .ghost()
                                .small()
                                .tooltip(language.text(Message::McpCopyConnection))
                                .on_click(
                                    cx.listener(|view, _, _, cx| view.copy_mcp_connection(cx)),
                                ),
                        ),
                )
                .child(div().text_xs().child(language.text(Message::McpTunnelHelp)))
                .child(
                    h_flex()
                        .gap_2()
                        .child(Input::new(&sharing.executable).small())
                        .child(Input::new(&sharing.tunnel_id).small())
                        .child(Input::new(&sharing.runtime_key).small()),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("mcp-tunnel")
                                .small()
                                .label(language.text(if sharing.helper.is_some() {
                                    Message::McpStopHelper
                                } else {
                                    Message::McpStartHelper
                                }))
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.start_mcp_tunnel(window, cx)
                                })),
                        )
                        .child(div().text_xs().child(status)),
                );
        }
        if let Some((call, epoch)) = &sharing.pending {
            let id = call.id;
            panel = panel
                .child(div().text_sm().child(language.text(Message::McpApprovalRequested)))
                .child(
                    div()
                        .id("mcp-command")
                        .max_h(px(120.0))
                        .overflow_y_scroll()
                        .text_sm()
                        .child(call.operation.description()),
                )
                .child(
                    Button::new("mcp-approve")
                        .primary()
                        .label(language.text(Message::McpApprove))
                        .disabled(!call.is_live() || *epoch != self.prompt_input_epoch)
                        .on_click(cx.listener(move |view, _, _, cx| view.approve_mcp(id, cx))),
                );
        }
        panel.into_any_element()
    }
}
