use super::super::pointer::tests::{clipboard, draw, link_fixture};
use super::*;
use crate::gpui_shell::copy_feedback::COPY_FEEDBACK_TTL;
use gpui::{Modifiers, TestAppContext, VisualTestContext};
use nebula_terminal::event_loop::StreamProcessor;

fn open(cx: &mut TestAppContext) -> (Entity<TerminalView>, VisualTestContext) {
    let (view, mut window, _) = link_fixture(cx, b"");
    view.update(&mut window, |view, cx| {
        cx.global_mut::<Settings>().terminal_blocks = true;
        let (session, _, _, proxy) = session::test_session_with_events();
        StreamProcessor::default().feed(&mut session.term.lock(), &proxy,
            b"\x1b]133;A\x07$ printf hello\x1b]133;C\x07\r\nhello\r\n\x1b]133;D;0\x07\x1b]133;A\x07$ ");
        view.session = Some(session);
        cx.notify();
    });
    draw(&mut window);
    (view, window)
}

fn cell(view: &Entity<TerminalView>, cx: &VisualTestContext, col: f32, row: f32) -> Point<Pixels> {
    view.read_with(cx, |view, _| view.origin + point(view.cell_width * col, view.line_height * row))
}

fn click(position: Point<Pixels>, cx: &mut VisualTestContext) {
    cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::default());
    draw(cx);
}

#[gpui::test]
fn block_click_copy_feedback_and_keyboard_exit_use_real_hitboxes(cx: &mut TestAppContext) {
    let (view, mut window) = open(cx);
    let geometry = view.read_with(&window, |view, _| (view.origin, view.rows, view.cols));
    click(cell(&view, &window, 2.5, 1.5), &mut window);
    assert!(view.read_with(&window, |view, _| view.blocks.selected.is_some()));
    let controls = window.debug_bounds("terminal-block-controls").expect("visible copy control");
    assert!(controls.size.height >= px(32.0));
    click(controls.center(), &mut window);
    assert_eq!(clipboard(&mut window).as_deref(), Some("$ printf hello\nhello"));
    assert!(view.read_with(&window, |view, cx| {
        view.blocks.feedback.as_ref().unwrap().read(cx).is_copied()
    }));
    window.simulate_mouse_move(point(px(0.0), px(0.0)), None, Modifiers::default());
    draw(&mut window);
    assert!(window.debug_bounds("terminal-block-controls").is_some());
    assert_eq!(geometry, view.read_with(&window, |view, _| (view.origin, view.rows, view.cols)));
    window.executor().advance_clock(COPY_FEEDBACK_TTL);
    draw(&mut window);
    assert!(!view.read_with(&window, |view, cx| {
        view.blocks.feedback.as_ref().unwrap().read(cx).is_copied()
    }));
    window.simulate_keystrokes("escape");
    draw(&mut window);
    assert!(window.debug_bounds("terminal-block-controls").is_none());
    window.simulate_keystrokes("alt-shift-up");
    draw(&mut window);
    assert!(window.debug_bounds("terminal-block-controls").is_some());
    window.simulate_keystrokes("ctrl-shift-c");
    assert_eq!(clipboard(&mut window).as_deref(), Some("$ printf hello\nhello"));
}

#[gpui::test]
fn block_mode_preserves_drag_and_live_input_and_disables_without_replacing_session(
    cx: &mut TestAppContext,
) {
    let (view, mut window) = open(cx);
    let start = cell(&view, &window, 0.1, 1.5);
    let end = cell(&view, &window, 4.9, 1.5);
    window.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    window.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::default());
    window.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    draw(&mut window);
    assert!(view.read_with(&window, |view, _| view.blocks.selected.is_none()));
    assert_eq!(
        view.read_with(&window, |view, _| view
            .session
            .as_ref()
            .unwrap()
            .term
            .lock()
            .selection_to_string())
            .as_deref(),
        Some("hello")
    );
    click(cell(&view, &window, 0.5, 2.5), &mut window);
    assert!(view.read_with(&window, |view, _| view.blocks.selected.is_none()));
    // The existing left padding selects even the live input block explicitly.
    click(cell(&view, &window, -0.5, 2.5), &mut window);
    assert!(view.read_with(&window, |view, _| view.blocks.selected.is_some()));
    view.update(&mut window, |view, cx| {
        let session = view.session.as_ref().unwrap().term.clone();
        cx.global_mut::<Settings>().terminal_blocks = false;
        view.apply_settings(cx);
        assert!(Arc::ptr_eq(&session, &view.session.as_ref().unwrap().term));
        assert!(view.blocks.selected.is_none());
        assert!(view.blocks.feedback.is_none());
    });
    draw(&mut window);
    assert!(window.debug_bounds("terminal-block-controls").is_none());
    click(cell(&view, &window, 2.5, 1.5), &mut window);
    assert!(view.read_with(&window, |view, _| view.blocks.selected.is_none()));
}
