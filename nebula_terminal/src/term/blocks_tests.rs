use super::*;
use crate::event::VoidListener;
use crate::event_loop::StreamProcessor;
use crate::term::{Config, test::TermSize};

const SAMPLE: &[u8] =
    b"\x1b]133;A\x07$ printf hello\x1b]133;C\x07\r\nhello\r\n\x1b]133;D;0\x07\x1b]133;A\x07$ ";

fn feed(term: &mut Term<VoidListener>, bytes: &[u8]) {
    StreamProcessor::default().feed(term, &VoidListener, bytes);
}

fn contents(term: &mut Term<VoidListener>) -> Vec<String> {
    let blocks: Vec<_> =
        term.prompt_blocks(term.grid().topmost_line()..term.grid().bottommost_line() + 1).collect();
    blocks
        .into_iter()
        .filter(|block| !block.accepting_input)
        .filter_map(|block| {
            term.selection = block.selection(term);
            term.selection_to_string()
        })
        .collect()
}

#[test]
fn streaming_boundaries_select_only_their_own_command_and_output() {
    for split in 0..=SAMPLE.len() {
        let mut term = Term::new(Config::default(), &TermSize::new(40, 4), VoidListener);
        let mut stream = StreamProcessor::default();
        stream.feed(&mut term, &VoidListener, &SAMPLE[..split]);
        stream.feed(&mut term, &VoidListener, &SAMPLE[split..]);
        assert_eq!(contents(&mut term), ["$ printf hello\nhello"]);
        assert!(term.prompt_block_at(Point::new(Line(2), Column(0))).unwrap().accepting_input);
    }
}

#[test]
fn same_row_prompts_unicode_and_ansi_keep_exact_cell_boundaries() {
    let mut term = Term::new(Config::default(), &TermSize::new(12, 8), VoidListener);
    feed(&mut term, "prefix\x1b]133;A\x07$ \x1b[31m中e\u{301}文\x1b[0m\x1b]133;C\x07\r\nabcdefghi中z\x1b]133;D;1\x07\x1b]133;A\x07$ ".as_bytes());
    assert_eq!(contents(&mut term), ["$ 中e\u{301}文\nabcdefghi中z"]);
}

#[test]
fn blocks_follow_reflow_height_changes_and_inactive_primary_screen() {
    for conpty in [false, true] {
        let mut term = Term::new(
            Config { conpty_resize: conpty, ..Config::default() },
            &TermSize::new(40, 8),
            VoidListener,
        );
        feed(&mut term, SAMPLE);
        for (cols, rows) in [(7, 8), (12, 4), (40, 8), (9, 6), (40, 4)] {
            term.resize(TermSize::new(cols, rows));
            assert_eq!(contents(&mut term), ["$ printf hello\nhello"], "{conpty} {cols}x{rows}");
        }
        feed(&mut term, b"\x1b[?1049h");
        assert!(term.prompt_blocks(Line(-100)..Line(100)).next().is_none());
        term.resize(TermSize::new(8, 8));
        feed(&mut term, b"\x1b[?1049l");
        assert_eq!(contents(&mut term), ["$ printf hello\nhello"]);
    }
}

#[test]
fn stale_clipped_or_reset_blocks_never_refer_to_newer_output() {
    let mut term = Term::new(
        Config { scrolling_history: 3, ..Config::default() },
        &TermSize::new(20, 3),
        VoidListener,
    );
    feed(&mut term, SAMPLE);
    for _ in 0..10 {
        feed(&mut term, b"\x1b]133;C\x07\r\nnext\r\n\x1b]133;A\x07$ ");
    }
    assert!(contents(&mut term).iter().all(|text| text == "$\nnext"), "{:?}", contents(&mut term));
    term.resize(TermSize::new(3, 3));
    assert!(contents(&mut term).iter().all(|text| !text.contains("hello")));
    feed(&mut term, b"\x1b[3J");
    assert!(term.prompt_blocks(Line(-100)..Line(100)).all(|b| b.start.line >= Line(0)));
    feed(&mut term, b"\x1bc");
    assert!(term.prompt_blocks(Line(-100)..Line(100)).next().is_none());
}

#[test]
fn unintegrated_shells_and_mouse_reporting_are_not_block_interactions() {
    let mut term = Term::new(Config::default(), &TermSize::new(40, 4), VoidListener);
    feed(&mut term, b"fake prompt > command\r\noutput\r\n");
    assert!(term.prompt_blocks(Line(0)..Line(4)).next().is_none());
    feed(&mut term, SAMPLE);
    feed(&mut term, b"\x1b[?1000h");
    assert!(term.prompt_blocks(Line(-100)..Line(100)).next().is_none());
    feed(&mut term, b"\x1b[?1000l");
    assert!(term.prompt_blocks(Line(-100)..Line(100)).next().is_some());
}
