use super::*;
use crate::event::VoidListener;
use crate::event_loop::StreamProcessor;
use crate::term::{Config, test::TermSize};

const TRANSCRIPT: &[u8] = b"\x1b]7;file://localhost/tmp\x07\x1b]133;A\x07/tmp\r\n> \x1b]133;B\x07printf test\r\n\x1b]133;C\x07\x1b[31mhello world\x1b[0m\r\n\x1b]133;D;3\x07\x1b]133;A\x07/tmp\r\n> \x1b]133;B\x07";

fn terminal(columns: usize, rows: usize) -> Term<VoidListener> {
    Term::new(
        Config { command_regions: true, ..Config::default() },
        &TermSize::new(columns, rows),
        VoidListener,
    )
}

fn output(term: &Term<VoidListener>, region: &CommandRegion) -> String {
    term.command_region_text(region.output.unwrap(), region.end.unwrap())
}

#[test]
fn command_regions_follow_stream_boundaries_even_in_a_single_read_or_split_sequences() {
    for chunk in [1, 7, TRANSCRIPT.len()] {
        let mut term = terminal(80, 12);
        let mut processor = StreamProcessor::default();
        for bytes in TRANSCRIPT.chunks(chunk) {
            processor.feed(&mut term, &VoidListener, bytes);
        }
        let region = term.command_regions().next().unwrap();
        assert_eq!(region.command, "printf test");
        assert_eq!(region.cwd, "/tmp");
        assert_eq!(region.exit_code, Some(3));
        assert_eq!(output(&term, region), "hello world");
        assert_eq!(term.command_regions().count(), 2);
        assert!(term.nebula_prompt_active());
    }
}

#[test]
fn command_regions_keep_identity_and_plain_text_through_width_and_height_reflow() {
    for conpty in [false, true] {
        let mut term = terminal(80, 12);
        term.config.conpty_resize = conpty;
        let mut processor = StreamProcessor::default();
        processor.feed(&mut term, &VoidListener, TRANSCRIPT);
        let id = term.command_regions().next().unwrap().id;
        for (columns, rows) in [(9, 12), (20, 5), (60, 20), (80, 12)] {
            term.resize(TermSize::new(columns, rows));
            let region = term.command_regions().find(|r| r.id == id).unwrap();
            assert_eq!(region.command, "printf test");
            assert_eq!(output(&term, region), "hello world", "{columns}x{rows}");
            assert!(term.nebula_prompt_input_point().is_some());
        }
    }
}

#[test]
fn multiline_and_wide_output_are_separate_from_the_shell_prompt() {
    let mut term = terminal(9, 8);
    let bytes = "\x1b]133;A\x07/tmp\r\n> \x1b]133;B\x07echo first\r\n>> echo second\r\n\x1b]1337;SetUserVar=pebrel_command=ZWNobyBmaXJzdAplY2hvIHNlY29uZA==\x07\x1b]133;C\x07你好 world\r\nsecond\x1b]133;D;0\x07";
    StreamProcessor::default().feed(&mut term, &VoidListener, bytes.as_bytes());
    let id = term.command_regions().next().unwrap().id;
    assert_eq!(term.command_regions().next().unwrap().command, "echo first\necho second");
    for columns in [6, 30] {
        term.resize(TermSize::new(columns, 8));
        let region = term.command_regions().find(|r| r.id == id).unwrap();
        assert_eq!(
            output(&term, region),
            "你好 world\nsecond",
            "width={columns}, region={region:?}, cursor={:?}",
            term.grid.cursor
        );
    }
}

#[test]
fn regions_are_bounded_by_scrollback_and_reset_with_the_terminal() {
    let mut term = Term::new(
        Config { command_regions: true, scrolling_history: 8, ..Config::default() },
        &TermSize::new(20, 4),
        VoidListener,
    );
    let mut processor = StreamProcessor::default();
    for _ in 0..20 {
        processor.feed(&mut term, &VoidListener, TRANSCRIPT);
    }
    assert!(term.command_regions().count() <= 6);
    assert!(term.command_regions().all(|r| r.prompt_line >= term.grid.scrolled_out()));
    term.resize(TermSize::new(10, 8));
    assert!(term.command_regions().count() <= 6);
    processor.feed(&mut term, &VoidListener, b"\x1bc");
    assert_eq!(term.command_regions().count(), 0);
}

#[test]
fn alternate_screen_cannot_open_or_finish_shell_regions() {
    let mut term = terminal(80, 12);
    let mut processor = StreamProcessor::default();
    processor.feed(&mut term, &VoidListener, b"\x1b]133;A\x07> \x1b]133;B\x07vim\r\n\x1b]133;C\x07\x1b[?1049h\x1b]133;A\x07fake\x1b]133;D;9\x07");
    term.resize(TermSize::new(60, 10));
    processor.feed(&mut term, &VoidListener, b"\x1b[?1049l\x1b]133;D;0\x07");
    let region = term.command_regions().next().unwrap();
    assert_eq!(region.command, "vim");
    assert_eq!(region.exit_code, Some(0));
    assert_eq!(term.command_regions().count(), 1);
}

#[test]
fn default_terminal_does_not_retain_command_metadata() {
    let mut term = Term::new(Config::default(), &TermSize::new(80, 12), VoidListener);
    StreamProcessor::default().feed(&mut term, &VoidListener, TRANSCRIPT);
    assert_eq!(term.command_regions().count(), 0);
    assert!(term.nebula_shell.commands.is_none());
}
