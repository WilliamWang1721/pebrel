use super::*;
use crate::gpui_shell::copy_feedback::CopyFeedback;
use crate::i18n::Message;
use gpui_component::{ActiveTheme as _, Disableable as _, button::Button};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockPart {
    Command,
    Output,
    All,
}

pub(in crate::gpui_shell::terminal) struct Blocks {
    pub(in crate::gpui_shell::terminal) selected: Option<u64>,
    pub(in crate::gpui_shell::terminal) hovered: Option<u64>,
    enabled: bool,
    copied: Option<(u64, BlockPart)>,
    feedback: gpui::Entity<CopyFeedback>,
    _subscription: gpui::Subscription,
}

impl Blocks {
    pub(super) fn new(cx: &mut Context<TerminalView>) -> Self {
        let feedback = cx.new(|_| CopyFeedback::new());
        let subscription = cx.observe(&feedback, |_, _, cx| cx.notify());
        Self {
            selected: None,
            hovered: None,
            enabled: cx.try_global::<Settings>().is_some_and(|settings| settings.block_terminal),
            copied: None,
            feedback,
            _subscription: subscription,
        }
    }
}

impl TerminalView {
    fn command_at(&self, point: TermPoint) -> Option<u64> {
        if !self.blocks.enabled {
            return None;
        }
        let term = self.session.as_ref()?.term.lock();
        if term.mode().intersects(TermMode::ALT_SCREEN | TermMode::MOUSE_MODE | TermMode::VI) {
            return None;
        }
        let line = (term.grid().scrolled_out() as i64
            + term.history_size() as i64
            + point.line.0 as i64) as usize;
        term.command_regions()
            .rev()
            .find(|region| {
                region.output.is_some()
                    && region.prompt_line <= line
                    && line
                        < region.end.map_or(term.nebula_cursor_abs_line() + 1, |end| {
                            end.0 + usize::from(end.1.0 > 0)
                        })
            })
            .map(|region| region.id)
    }

    pub(super) fn select_command_at(&mut self, point: TermPoint, cx: &mut Context<Self>) {
        self.blocks.selected = self.command_at(point);
        cx.notify();
    }

    pub(super) fn hover_command_at(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.blocks.enabled {
            return;
        }
        let next = if self.selecting { None } else { self.command_at(self.grid_point(position).0) };
        if self.blocks.hovered != next {
            self.blocks.hovered = next;
            cx.notify();
        }
    }

    pub(super) fn copy_block(&mut self, part: BlockPart, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.blocks.selected else { return false };
        let text = self.session.as_ref().and_then(|session| {
            let term = session.term.lock();
            if term.mode().intersects(TermMode::ALT_SCREEN | TermMode::MOUSE_MODE | TermMode::VI) {
                return None;
            }
            let region = term.command_regions().find(|region| region.id == id)?;
            let output =
                (part != BlockPart::Command).then_some(region.output).flatten().map(|start| {
                    term.command_region_text(
                        start,
                        region.end.unwrap_or((
                            term.nebula_cursor_abs_line(),
                            term.grid().cursor.point.column,
                        )),
                    )
                });
            Some(match part {
                BlockPart::Command => region.command.clone(),
                BlockPart::Output => output.unwrap_or_default(),
                BlockPart::All => match output.filter(|output| !output.is_empty()) {
                    Some(output) => format!("{}\n{output}", region.command),
                    None => region.command.clone(),
                },
            })
        });
        let Some(text) = text.filter(|text| !text.is_empty()) else { return false };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.blocks.copied = Some((id, part));
        self.blocks.feedback.update(cx, |feedback, cx| feedback.mark_copied(cx));
        cx.notify();
        true
    }

    fn block_reinput(&self) -> Option<String> {
        if self.exited.is_some() {
            return None;
        }
        let term = self.session.as_ref()?.term.lock();
        if term.mode().intersects(TermMode::ALT_SCREEN | TermMode::MOUSE_MODE | TermMode::VI)
            || term.nebula_prompt_input_point()? != term.grid().cursor.point
        {
            return None;
        }
        term.command_regions()
            .find(|region| Some(region.id) == self.blocks.selected)
            .filter(|region| {
                !region.command.is_empty()
                    && (term.mode().contains(TermMode::BRACKETED_PASTE)
                        || !region.command.contains(['\n', '\r']))
            })
            .map(|region| region.command.clone())
    }

    fn reinput_block(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(command) = self.block_reinput() {
            self.blocks.selected = None;
            self.request_paste(command, window, cx);
            window.focus(&self.focus_handle, cx);
        }
    }

    pub(super) fn block_key(&mut self, key: &gpui::Keystroke, cx: &mut Context<Self>) -> bool {
        let mods = key.modifiers;
        if mods.control && mods.shift && !mods.alt && matches!(key.key.as_str(), "up" | "down") {
            let Some(session) = &self.session else { return false };
            let mut term = session.term.lock();
            if term.mode().intersects(TermMode::ALT_SCREEN | TermMode::MOUSE_MODE | TermMode::VI) {
                return false;
            }
            let regions =
                term.command_regions().filter(|region| region.output.is_some()).collect::<Vec<_>>();
            if regions.is_empty() {
                return false;
            }
            let index = regions.iter().position(|r| Some(r.id) == self.blocks.selected).map_or(
                regions.len() - 1,
                |index| {
                    if key.key == "up" {
                        index.saturating_sub(1)
                    } else {
                        (index + 1).min(regions.len() - 1)
                    }
                },
            );
            let (id, start) = (regions[index].id, regions[index].prompt_line);
            self.blocks.selected = Some(id);
            term.selection = None;
            let offset = (term.grid().scrolled_out() + term.history_size())
                .saturating_sub(start)
                .min(term.history_size());
            let delta = offset as i32 - term.grid().display_offset() as i32;
            term.scroll_display(Scroll::Delta(delta));
            cx.notify();
            return true;
        }
        if self.blocks.selected.is_some()
            && key.key == "escape"
            && !self
                .term_mode()
                .intersects(TermMode::ALT_SCREEN | TermMode::MOUSE_MODE | TermMode::VI)
        {
            self.blocks.selected = None;
            cx.notify();
            return true;
        }
        false
    }

    pub(super) fn block_toolbar(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let id = self.blocks.selected?;
        let session = self.session.as_ref()?;
        let (has_command, has_output) = {
            let term = session.term.lock();
            if term.mode().intersects(TermMode::ALT_SCREEN | TermMode::MOUSE_MODE | TermMode::VI) {
                self.blocks.selected = None;
                return None;
            }
            let Some(region) = term.command_regions().find(|region| region.id == id) else {
                self.blocks.selected = None;
                return None;
            };
            (
                !region.command.is_empty(),
                region.output.is_some_and(|start| {
                    start
                        < region.end.unwrap_or((
                            term.nebula_cursor_abs_line(),
                            term.grid().cursor.point.column,
                        ))
                }),
            )
        };
        let language = crate::gpui_shell::config::ui_language(cx);
        let mut toolbar = div()
            .absolute()
            .right(px(16.0))
            .bottom(px(12.0))
            .flex()
            .flex_col()
            .gap_1()
            .p_1()
            .rounded_md()
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().border)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    view.blocks.selected = None;
                    window.focus(&view.focus_handle, cx);
                    cx.notify();
                }
                if event.keystroke.key != "tab" {
                    cx.stop_propagation();
                }
            }));
        for (part, message, available) in [
            (BlockPart::Command, Message::TerminalBlocksCopyCommand, has_command),
            (BlockPart::Output, Message::TerminalBlocksCopyOutput, has_output),
            (BlockPart::All, Message::TerminalBlocksCopyBlock, has_command || has_output),
        ] {
            let copied =
                self.blocks.feedback.read(cx).is_copied() && self.blocks.copied == Some((id, part));
            toolbar = toolbar.child(
                div().debug_selector(move || format!("block-copy-{}", part as usize)).child(
                    Button::new(("block-copy", part as usize))
                        .small()
                        .w(px(148.0))
                        .h(px(32.0))
                        .label(language.text(if copied {
                            Message::TerminalBlocksCopied
                        } else {
                            message
                        }))
                        .tooltip(language.text(message))
                        .disabled(!available)
                        .on_key_down(cx.listener(move |view, event: &KeyDownEvent, _, cx| {
                            if available
                                && matches!(event.keystroke.key.as_str(), "enter" | "space")
                            {
                                view.copy_block(part, cx);
                                cx.stop_propagation();
                            }
                        }))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.copy_block(part, cx);
                        })),
                ),
            );
        }
        Some(
            toolbar
                .debug_selector(|| "block-toolbar".to_owned())
                .child(
                    div().debug_selector(|| "block-reinput".to_owned()).child(
                        Button::new("block-reinput")
                            .small()
                            .w(px(148.0))
                            .h(px(32.0))
                            .label(language.text(Message::TerminalBlocksReinput))
                            .tooltip(language.text(Message::TerminalBlocksReinputDescription))
                            .disabled(self.block_reinput().is_none())
                            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                                    view.reinput_block(window, cx);
                                    cx.stop_propagation();
                                }
                            }))
                            .on_click(
                                cx.listener(|view, _, window, cx| view.reinput_block(window, cx)),
                            ),
                    ),
                )
                .into_any_element(),
        )
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
mod tests;
