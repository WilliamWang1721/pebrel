// Investigation-only probe, appended to the existing real-layout test module by CI.
#[gpui::test]
fn issue_270_ssh_control_keys_keep_focus_and_reach_transport(cx: &mut TestAppContext) {
    let (terminal, mut cx, receiver) = link_fixture(cx, b"user@host:~$ ");
    cx.update(|window, cx| {
        crate::gpui_shell::terminal::init(cx);
        crate::gpui_shell::workspace::init(cx);
        terminal.update(cx, |view, cx| {
            view.ssh_destination = Some("issue-270.invalid".into());
            view.ssh_stage = Some(crate::ssh_session::SshStage::Ready);
            view.suggest.suggest_env = crate::display::SuggestEnv::Ssh {
                destination: "issue-270.invalid".into(),
            };
            view.suggest.suggestion.clear();
            view.suggest.completion_items.clear();
            view.focus_handle.focus(window, cx);
            cx.notify();
        });
    });
    draw(&mut cx);
    for (key, expected) in [
        ("tab", b"\t".as_slice()),
        ("ctrl-l", b"\x0c".as_slice()),
        ("shift-tab", b"\x1b[Z".as_slice()),
        ("ctrl-a", b"\x01".as_slice()),
        ("up", b"\x1b[A".as_slice()),
    ] {
        receiver.try_iter().for_each(drop);
        cx.simulate_keystrokes(key);
        draw(&mut cx);
        let bytes: Vec<u8> = receiver.try_iter().flat_map(|message| match message {
            Msg::Input(bytes) => bytes.into_owned(),
            _ => Vec::new(),
        }).collect();
        assert_eq!(bytes, expected, "{key} must reach SSH exactly once");
        cx.update(|window, cx| {
            assert!(terminal.read(cx).focus_handle.is_focused(window), "{key} moved focus");
        });
    }
}
