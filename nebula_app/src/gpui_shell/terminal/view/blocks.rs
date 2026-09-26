//! Optional command-block interaction over the terminal's existing grid and selection.
use super::*;
use crate::gpui_shell::copy_feedback::CopyFeedback;
use crate::i18n::Message;
use gpui::{AnyElement, Entity, Hsla, Subscription, fill, size};
use gpui_component::{IconName, button::Button};
use nebula_terminal::term::{Term, blocks::PromptBlock};

pub(in super::super) fn enabled(cx: &App) -> bool {
    cx.try_global::<Settings>().is_some_and(|settings| settings.terminal_blocks)
}

struct Press {
    position: Point<Pixels>,
    start: TermPoint<usize>,
}

struct Region {
    block: PromptBlock,
    selected: bool,
    hovered: bool,
}

#[derive(Default)]
pub(in super::super) struct BlockInteraction {
    selected: Option<TermPoint<usize>>,
    hovered: Option<TermPoint<usize>>,
    press: Option<Press>,
    regions: Vec<Region>,
    feedback: Option<Entity<CopyFeedback>>,
    subscription: Option<Subscription>,
}

fn absolute<T>(term: &Term<T>, point: TermPoint) -> TermPoint<usize> {
    let origin = term.grid().scrolled_out() + term.grid().history_size();
    TermPoint::new((origin as i64 + i64::from(point.line.0)) as usize, point.column)
}

impl BlockInteraction {
    pub(super) fn clear_target(&mut self) {
        self.selected = None;
        self.press = None;
        self.feedback = None;
        self.subscription = None;
    }

    pub(super) fn cancel_press(&mut self) {
        self.press = None;
    }

    fn selected_block<T>(&self, term: &Term<T>) -> Option<PromptBlock> {
        let start = self.selected?;
        term.selection.as_ref()?;
        let origin = term.grid().scrolled_out() + term.grid().history_size();
        let line = Line(i32::try_from(start.line as i64 - origin as i64).ok()?);
        term.prompt_block_at(TermPoint::new(line, start.column))
            .filter(|block| absolute(term, block.start) == start)
    }

    // Reuse the renderer's single snapshot lock and this vector's capacity.
    // No output strings, extra grid scans or per-block locks in the paint path.
    pub(in super::super) fn capture<T>(&mut self, term: &mut Term<T>, rows: usize, on: bool) {
        self.regions.clear();
        if !on {
            return;
        }
        if self.selected.is_some() {
            if let Some(block) = self.selected_block(term) {
                term.selection = block.selection(term);
            } else {
                self.clear_target();
            }
        }
        let top = term.viewport_origin_for(rows);
        self.regions.extend(term.prompt_blocks(top..top + rows).map(|mut block| {
            let key = absolute(term, block.start);
            block.start.line -= top.0;
            block.end.line -= top.0;
            Region {
                block,
                selected: self.selected == Some(key),
                hovered: self.hovered == Some(key),
            }
        }));
    }

    pub(in super::super) fn paint(
        &self,
        bounds: Bounds<Pixels>,
        cell_width: Pixels,
        line_height: Pixels,
        border: Hsla,
        foreground: Hsla,
        window: &mut Window,
    ) {
        for region in &self.regions {
            let block = region.block;
            let first = block.start.line.0 as f32;
            let last = (block.end.line.0 + i32::from(block.end.column.0 > 0))
                .max(block.start.line.0 + 1) as f32;
            let top = (line_height * first).max(px(0.0));
            let bottom = (line_height * last).min(bounds.size.height);
            if bottom <= top {
                continue;
            }
            let color =
                if region.selected || region.hovered { foreground.opacity(0.65) } else { border };
            let width = if region.selected { px(4.0) } else { px(2.0) };
            window.paint_quad(fill(
                Bounds::new(
                    point(bounds.origin.x - px(6.0), bounds.origin.y + top),
                    size(width, bottom - top),
                ),
                color,
            ));
            if first >= 0.0 {
                let x = cell_width * block.start.column.0 as f32;
                window.paint_quad(fill(
                    Bounds::new(
                        bounds.origin + point(x, top),
                        size((bounds.size.width - x).max(px(0.0)), px(1.0)),
                    ),
                    color,
                ));
            }
        }
    }
}

impl TerminalView {
    pub(super) fn clear_block_selection(&mut self) {
        if self.blocks.selected.is_some()
            && let Some(session) = &self.session
        {
            session.term.lock().selection = None;
        }
        self.blocks = BlockInteraction::default();
    }

    pub(super) fn begin_block_click(&mut self, event: &MouseDownEvent, cx: &App) {
        self.blocks.clear_target();
        if !enabled(cx) || event.click_count != 1 || event.modifiers != gpui::Modifiers::default() {
            return;
        }
        let (point, _) = self.grid_point(event.position);
        let Some(session) = &self.session else { return };
        let term = session.term.lock();
        if let Some(block) = term.prompt_block_at(point)
            && (!block.accepting_input || event.position.x < self.origin.x)
        {
            self.blocks.press =
                Some(Press { position: event.position, start: absolute(&term, block.start) });
        }
    }

    pub(super) fn track_block_pointer(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.blocks.press.as_ref().is_some_and(|press| {
            (event.position.x - press.position.x).abs() > px(3.0)
                || (event.position.y - press.position.y).abs() > px(3.0)
        }) {
            self.blocks.cancel_press();
        }
        if !enabled(cx) || event.pressed_button.is_some() {
            return;
        }
        let (point, _) = self.grid_point(event.position);
        let hovered = self.session.as_ref().and_then(|session| {
            let term = session.term.lock();
            term.prompt_block_at(point).map(|block| absolute(&term, block.start))
        });
        if hovered != self.blocks.hovered {
            self.blocks.hovered = hovered;
            cx.notify();
        }
    }

    pub(super) fn finish_block_click(&mut self, event: &MouseUpEvent, cx: &mut Context<Self>) {
        let Some(press) = self.blocks.press.take() else { return };
        if !enabled(cx)
            || event.modifiers != gpui::Modifiers::default()
            || (event.position.x - press.position.x).abs() > px(3.0)
            || (event.position.y - press.position.y).abs() > px(3.0)
        {
            return;
        }
        let (point, _) = self.grid_point(event.position);
        let Some(session) = &self.session else { return };
        let mut term = session.term.lock();
        if let Some(block) = term.prompt_block_at(point)
            && absolute(&term, block.start) == press.start
        {
            term.selection = block.selection(&term);
            self.blocks.selected = term.selection.as_ref().map(|_| press.start);
        }
    }

    pub(super) fn block_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !enabled(cx) {
            return false;
        }
        let key = &event.keystroke;
        let mods = &key.modifiers;
        if key.key == "escape"
            && self.blocks.selected.is_some()
            && *mods == gpui::Modifiers::default()
        {
            self.clear_block_selection();
        } else if mods.alt
            && mods.shift
            && !mods.control
            && !mods.platform
            && matches!(key.key.as_str(), "up" | "down")
        {
            let Some(session) = &self.session else { return false };
            let mut term = session.term.lock();
            let current =
                self.blocks.selected_block(&term).map(|block| block.start).unwrap_or_else(|| {
                    let cursor = term.grid().cursor.point;
                    term.prompt_blocks(cursor.line..cursor.line + 1)
                        .last()
                        .map_or(cursor, |block| block.start)
                });
            let mut blocks =
                term.prompt_blocks(term.grid().topmost_line()..term.grid().bottommost_line() + 1);
            let block = if key.key == "up" {
                blocks.filter(|block| block.start < current).last()
            } else {
                blocks.find(|block| block.start > current)
            };
            let Some(block) = block else { return false };
            self.blocks.clear_target();
            self.blocks.selected = Some(absolute(&term, block.start));
            term.selection = block.selection(&term);
            let offset = (-block.start.line.0).max(0).min(term.history_size() as i32);
            let delta = offset - term.grid().display_offset() as i32;
            term.scroll_display(Scroll::Delta(delta));
            window.focus(&self.focus_handle, cx);
        } else {
            // Normal editing and shell history stay native, not a second input editor.
            if self.blocks.selected.is_some()
                && !mods.control
                && !mods.alt
                && !mods.platform
                && (key.key_char.is_some()
                    || matches!(
                        key.key.as_str(),
                        "enter" | "backspace" | "delete" | "up" | "down" | "left" | "right"
                    ))
            {
                self.clear_block_selection();
            }
            return false;
        }
        cx.notify();
        cx.stop_propagation();
        true
    }

    fn block_feedback(&mut self, cx: &mut Context<Self>) -> Entity<CopyFeedback> {
        if let Some(feedback) = &self.blocks.feedback {
            return feedback.clone();
        }
        let feedback = cx.new(|_| CopyFeedback::new());
        self.blocks.subscription = Some(cx.observe(&feedback, |_, _, cx| cx.notify()));
        self.blocks.feedback = Some(feedback.clone());
        feedback
    }

    pub(super) fn copy_selected_block(&mut self, cx: &mut Context<Self>) -> bool {
        if !enabled(cx) {
            return false;
        }
        let text = self.session.as_ref().and_then(|session| {
            let mut term = session.term.lock();
            let block = self.blocks.selected_block(&term)?;
            term.selection = block.selection(&term);
            term.selection_to_string().filter(|text| !text.is_empty())
        });
        let Some(text) = text else { return false };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.block_feedback(cx).update(cx, |feedback, cx| feedback.mark_copied(cx));
        cx.notify();
        true
    }

    pub(super) fn block_controls(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !enabled(cx) {
            return None;
        }
        let (block, top) = {
            let term = self.session.as_ref()?.term.lock();
            (self.blocks.selected_block(&term)?, term.viewport_origin_for(self.rows))
        };
        let y = (self.line_height * (block.start.line.0 - top.0) as f32 + px(8.0))
            .max(px(8.0))
            .min((self.line_height * self.rows as f32 - px(32.0)).max(px(8.0)));
        let feedback = self.block_feedback(cx);
        let copied = feedback.read(cx).is_copied();
        let language = crate::gpui_shell::config::ui_language(cx);
        let label = language.text(if copied {
            Message::TerminalBlocksCopied
        } else {
            Message::TerminalBlocksCopy
        });
        Some(
            div()
                .absolute()
                .top(y)
                .right(px(16.0))
                .debug_selector(|| "terminal-block-controls".to_owned())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    Button::new("copy-terminal-block")
                        .h(px(32.0))
                        .icon(if copied { IconName::Check } else { IconName::Copy })
                        .label(label)
                        .tooltip(language.text(Message::TerminalBlocksCopyHint))
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.copy_selected_block(cx);
                            window.focus(&view.focus_handle, cx);
                        })),
                )
                .into_any_element(),
        )
    }
}

#[cfg(all(test, feature = "gpui-test-support"))]
#[path = "blocks_tests.rs"]
mod tests;
