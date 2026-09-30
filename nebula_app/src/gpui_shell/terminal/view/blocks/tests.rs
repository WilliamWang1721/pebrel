use super::*;
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::Root;
use nebula_terminal::event_loop::{Msg, StreamProcessor};
use std::sync::mpsc::Receiver;

const TRANSCRIPT: &[u8] = b"\x1b]7;file://localhost/tmp\x07\x1b]133;A\x07/tmp\r\n> \x1b]133;B\x07echo first\r\n\x1b]133;C\x07first\r\n\x1b]133;D;0\x07\x1b]133;A\x07/tmp\r\n> \x1b]133;B\x07";

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

fn open(cx: &mut TestAppContext) -> (Entity<TerminalView>, VisualTestContext, Receiver<Msg>) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::gpui_shell::math_view::register(cx);
        let mut settings = Settings::load(nebula_settings::ThemeName::Nord);
        settings.block_terminal = true;
        settings.focus_follows_mouse = false;
        cx.set_global(settings);
    });
    let mut result = None;
    let (_, window) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            TerminalView::new(
                1,
                (80, 24),
                TerminalLaunch::Local {
                    cwd: None,
                    shell: Some(nebula_terminal::tty::Shell::new(
                        "pebrel-test-missing-shell-executable".into(),
                        vec![],
                    )),
                    shell_name: None,
                },
                window,
                cx,
            )
        });
        result = Some(view.clone());
        window.focus(&view.read(cx).focus_handle.clone(), cx);
        Root::new(view, window, cx)
    });
    let mut cx = window.clone();
    cx.simulate_resize(gpui::size(px(700.0), px(500.0)));
    draw(&mut cx);
    let view = result.unwrap();
    let receiver = view.update(&mut cx, |view, cx| {
        let (session, receiver, _, proxy) = session::test_session_with_events();
        {
            let mut term = session.term.lock();
            *term = nebula_terminal::Term::new(
                nebula_terminal::term::Config { command_regions: true, ..Default::default() },
                &*term,
                proxy.clone(),
            );
            StreamProcessor::default().feed(&mut term, &proxy, TRANSCRIPT);
        }
        view.session = Some(session);
        view.error = None;
        view.exited = None;
        view.exec_context = None;
        view.ghost_enabled = false;
        view.copy_on_select = false;
        cx.notify();
        receiver
    });
    draw(&mut cx);
    (view, cx, receiver)
}

fn cell(
    view: &Entity<TerminalView>,
    cx: &VisualTestContext,
    col: usize,
    row: usize,
) -> Point<Pixels> {
    view.read_with(cx, |view, _| {
        point(
            view.origin.x + view.cell_width * (col as f32 + 0.5),
            view.origin.y + view.line_height * (row as f32 + 0.5),
        )
    })
}

fn click(cx: &mut VisualTestContext, position: Point<Pixels>) {
    cx.simulate_click(position, Modifiers::default());
    draw(cx);
}

#[gpui::test]
fn block_click_copy_feedback_resize_and_reinput_use_real_controls(cx: &mut TestAppContext) {
    let (view, mut cx, receiver) = open(cx);
    let position = cell(&view, &cx, 2, 2);
    cx.simulate_mouse_move(position, None, Modifiers::default());
    assert!(view.read_with(&cx, |view, _| view.blocks.hovered.is_some()));
    click(&mut cx, position);
    let selected = view.read_with(&cx, |view, _| view.blocks.selected);
    assert!(selected.is_some());
    for (selector, expected) in [
        ("block-copy-0", "echo first"),
        ("block-copy-1", "first"),
        ("block-copy-2", "echo first\nfirst"),
    ] {
        let bounds = cx.debug_bounds(selector).expect("copy action hitbox");
        assert!(bounds.size.height >= px(32.0));
        click(&mut cx, bounds.center());
        assert_eq!(
            cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text())).as_deref(),
            Some(expected)
        );
        assert_eq!(view.read_with(&cx, |view, _| view.blocks.selected), selected);
        assert!(view.read_with(&cx, |view, cx| view.blocks.feedback.read(cx).is_copied()));
    }
    cx.executor().advance_clock(crate::gpui_shell::copy_feedback::COPY_FEEDBACK_TTL);
    draw(&mut cx);
    assert!(!view.read_with(&cx, |view, cx| view.blocks.feedback.read(cx).is_copied()));
    cx.simulate_resize(gpui::size(px(900.0), px(600.0)));
    draw(&mut cx);
    assert_eq!(view.read_with(&cx, |view, _| view.blocks.selected), selected);
    let bounds = cx.debug_bounds("block-reinput").expect("reinput action");
    click(&mut cx, bounds.center());
    let bytes: Vec<u8> = receiver
        .try_iter()
        .filter_map(|message| match message {
            Msg::Input(bytes) => Some(bytes.into_owned()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(bytes, b"echo first", "reinput must not send Enter");
    assert!(view.read_with(&cx, |view, _| view.blocks.selected.is_none()));
}

#[gpui::test]
fn keyboard_block_selection_escape_and_text_drag_coexist(cx: &mut TestAppContext) {
    let (view, mut cx, receiver) = open(cx);
    cx.simulate_keystrokes("ctrl-shift-up");
    draw(&mut cx);
    assert!(view.read_with(&cx, |view, _| view.blocks.selected.is_some()));
    cx.simulate_keystrokes("tab");
    draw(&mut cx);
    cx.update(|window, cx| assert!(!view.read(cx).focus_handle.is_focused(window)));
    cx.simulate_keystrokes("enter");
    draw(&mut cx);
    assert_eq!(
        cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text())).as_deref(),
        Some("echo first")
    );
    cx.simulate_keystrokes("escape");
    draw(&mut cx);
    assert!(view.read_with(&cx, |view, _| view.blocks.selected.is_none()));
    assert!(!receiver.try_iter().any(|message| matches!(message, Msg::Input(_))));
    let quarter = view.read_with(&cx, |view, _| view.cell_width * 0.25);
    let mut start = cell(&view, &cx, 0, 2);
    start.x -= quarter;
    let mut end = cell(&view, &cx, 4, 2);
    end.x += quarter;
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    draw(&mut cx);
    assert!(view.read_with(&cx, |view, _| view.blocks.selected.is_none()));
    assert!(!view.read_with(&cx, |view, _| view.selection_is_empty()));
    cx.simulate_keystrokes("ctrl-shift-c");
    assert_eq!(
        cx.update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text())).as_deref(),
        Some("first")
    );
}
