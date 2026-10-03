//! Fork-only diagnosis: controls are synthetic, agy bytes are unauthenticated.
use std::fs;

use nebula_terminal::event::VoidListener;
use nebula_terminal::grid::Dimensions;
use nebula_terminal::render::{CellMetrics, RenderSnapshot, SnapshotConfig, TerminalViewport};
use nebula_terminal::term::Config;
use nebula_terminal::vte::ansi::{Processor, StdSyncHandler};
use nebula_terminal::Term;

struct Size(usize, usize);
impl Dimensions for Size {
    fn total_lines(&self) -> usize { self.1 }
    fn screen_lines(&self) -> usize { self.1 }
    fn columns(&self) -> usize { self.0 }
}

fn replay(bytes: &[u8], cols: usize, chunk: usize) -> Vec<String> {
    let mut term = Term::new(Config::default(), &Size(cols, 24), VoidListener);
    let mut parser = Processor::<StdSyncHandler>::default();
    for bytes in bytes.chunks(chunk) { parser.advance(&mut term, bytes); }
    let snap = RenderSnapshot::capture(&term, &SnapshotConfig { rows: 24, cols: cols as u16 });
    let mut rows = vec![vec![String::from(" "); cols]; 24];
    for seg in snap.segments {
        for cell in seg.cells { rows[seg.row as usize][cell.col as usize] = cell.text; }
    }
    rows.into_iter().map(|row| row.concat()).collect()
}

fn controls() {
    for cols in [80, 113, 160] {
        for wrap in ["h", "l"] {
            for label in ["low", "medium", "high"] {
                let bytes = format!("\x1b[?7{wrap}\x1b[24;{}H{label}\x1b[0K\x1b[1;1H", cols - label.len() + 1);
                for chunk in [1, 2, bytes.len()] {
                    let rows = replay(bytes.as_bytes(), cols, chunk);
                    assert!(rows[23].ends_with(label), "cols={cols}, wrap={wrap}, label={label}, chunk={chunk}: {:?}", rows[23]);
                }
            }
        }
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let metrics = CellMetrics { cell_width: 9.6, cell_height: 20.0, scale };
            let width = cols as f32 * metrics.cell_width + 0.5;
            let viewport = TerminalViewport::from_content_size(width, 480.0, &metrics, 0);
            assert_eq!(viewport.cols as usize, cols);
            assert!(f32::from(viewport.cols) * metrics.cell_width <= width);
        }
    }
    println!("synthetic ASCII last-column VT/snapshot/viewport controls passed; no GPUI pixel claim");
}

#[cfg(windows)]
fn capture(name: &str, command: String, cols: usize, seconds: u64) -> Vec<String> {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    use nebula_terminal::event::{Event, EventListener, WindowSize};
    use nebula_terminal::tty::{self, EventedReadWrite, Options, Shell};
    struct Replies(std::sync::mpsc::Sender<Event>);
    impl EventListener for Replies {
        fn send_event(&self, event: Event) { self.0.send(event).unwrap(); }
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut term = Term::new(Config::default(), &Size(cols, 24), Replies(sender));
    let mut parser = Processor::<StdSyncHandler>::default();
    let window_size = WindowSize { num_lines: 24, num_cols: cols as u16, cell_width: 10, cell_height: 20 };
    let config = Options {
        shell: Some(Shell::new("powershell.exe".into(), vec!["-NoLogo".into(), "-NoProfile".into(), "-Command".into(), command])),
        escape_args: true,
        ..Options::default()
    };
    let mut pty = tty::new(&config, window_size, 0).unwrap();
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut bytes = Vec::new();
    let mut buf = [0u8; 8192];
    while Instant::now() < deadline {
        match pty.reader().read(&mut buf) {
            Ok(0) => break,
            Ok(count) => {
                bytes.extend_from_slice(&buf[..count]);
                parser.advance(&mut term, &buf[..count]);
                for event in receiver.try_iter() {
                    let reply = match event {
                        Event::PtyWrite(text) => text,
                        Event::TextAreaSizeRequest(format) => format(window_size),
                        Event::ColorRequest(index, format) => {
                            let value = if index == 257 { 26 } else { 224 };
                            format(nebula_terminal::vte::ansi::Rgb { r: value, g: value, b: value })
                        },
                        _ => continue,
                    };
                    pty.writer().write_all(reply.as_bytes()).unwrap();
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => panic!("PTY read failed: {error}"),
        }
    }
    drop(pty);
    fs::write(format!("diagnostic-output/{name}-{cols}.vt"), &bytes).unwrap();
    let rows = replay(&bytes, cols, 1);
    fs::write(format!("diagnostic-output/{name}-{cols}.txt"), rows.join("\n")).unwrap();
    println!("{name}, cols={cols}, bytes={}: {:?}", bytes.len(), rows);
    rows
}

fn main() {
    fs::create_dir_all("diagnostic-output").unwrap();
    controls();
    if std::env::args().any(|arg| arg == "--controls-only") { return; }
    #[cfg(windows)] {
        for cols in [80, 113, 160] {
            let command = "[Console]::CursorVisible=$false; $e=[char]27; [Console]::Write([string]$e+'[?7l'); $labels=@('low','medium','high'); for($i=0;$i -lt 3;$i++) { [Console]::SetCursorPosition([Console]::WindowWidth-$labels[$i].Length,20+$i); [Console]::Write($labels[$i]); [Console]::Write([string]$e+'[0K') }; [Console]::SetCursorPosition(0,0); Start-Sleep -Seconds 60";
            let rows = capture("powershell-control", command.into(), cols, 10);
            for (row, label) in [(20, "low"), (21, "medium"), (22, "high")] {
                assert!(rows[row].ends_with(label), "actual PS5.1/ConPTY control dropped {label}: {:?}", rows[row]);
            }
        }
        let agy = std::env::var("AGY_BINARY").unwrap();
        let escaped = agy.replace('\'', "''");
        let rows = capture("agy-unauthenticated", format!("& '{escaped}'"), 113, 10);
        println!("agy real startup captured; authenticated model/effort status is unavailable: {}", rows.join("\n"));
    }
}
