"""Source-extracted A/B probe; real SharedString, no native GPU/text shaping."""
import pathlib, re, subprocess, sys
base, head, output = map(pathlib.Path, sys.argv[1:])
output.mkdir(parents=True, exist_ok=True)
path = 'nebula_app/src/gpui_shell/terminal/element.rs'
parts = []
for label, root in [('before', base), ('after', head)]:
    source = (root / path).read_text()
    start = source.index('fn grid_text(')
    end = source.index('\nconst COMPLETION_PANEL_WIDTH:', start)
    parts.append(source[start:end].replace('fn grid_text(', f'fn {label}_grid('))
    call = re.search(r'paint_cell_text\(\s*window,\s*cx,\s*(SharedString::from\([^\n]+\)),\s*run,', source)
    if call is None:
        raise RuntimeError('Cannot locate the single-cell conversion at this revision')
    parts.append(f'fn {label}_cell(cell: &CellText) -> SharedString {{ {call[1]} }}')
(output / 'Cargo.toml').write_text('''[package]
name = "pebrel-cell-text-probe"
version = "0.0.0"
edition = "2024"
[workspace]
[features]
count-alloc = []
[dependencies]
gpui_shared_string = { git = "https://github.com/Kuddev/zed", rev = "7c662384cd77c1d1e1566206920e3a420ea6767b" }
smol_str = "=0.3.6"
unicode-width = "=0.2.2"
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 16
''')
code = r'''
#![allow(dead_code)]
use gpui_shared_string::SharedString;
use std::hint::black_box;
use std::time::Instant;

#[cfg(feature = "count-alloc")]
mod allocation {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }
    struct Counter;
    fn record() { ACTIVE.with(|a| { if a.get() { COUNT.with(|n| n.set(n.get()+1)); } }); }
    unsafe impl GlobalAlloc for Counter {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 { record(); unsafe { System.alloc(l) } }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 { record(); unsafe { System.alloc_zeroed(l) } }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 { record(); unsafe { System.realloc(p,l,n) } }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) { unsafe { System.dealloc(p,l) } }
    }
    #[global_allocator] static ALLOC: Counter = Counter;
    pub fn count(f: impl FnOnce()) -> usize {
        COUNT.with(|c| c.set(0)); ACTIVE.with(|a| a.set(true)); f();
        ACTIVE.with(|a| a.set(false)); COUNT.with(Cell::get)
    }
}
// These are recording sinks, NOT GPUI windows or a substitute for native acceptance.
mod gpui {
    #[derive(Clone)] pub struct Font;
    #[derive(Clone, Copy, Debug, PartialEq)] pub struct Point<T> { pub x: T, pub y: T }
}
type Pixels = f32;
#[derive(Clone, Copy)] struct Hsla;
struct App;
struct TextRun {
    len: usize, font: gpui::Font, color: Hsla,
    background_color: Option<()>, underline: Option<()>, strikethrough: Option<()>,
}
fn point(x: f32, y: f32) -> gpui::Point<f32> { gpui::Point { x, y } }
struct Window {
    captured: [Option<(SharedString, usize, gpui::Point<f32>)>; 128],
    count: usize,
    record: bool,
}
impl Window {
    fn new(record: bool) -> Self { Self { captured: std::array::from_fn(|_| None), count: 0, record } }
}
fn paint_cell_text(w: &mut Window, _: &mut App, text: SharedString, run: TextRun, _: Pixels, origin: gpui::Point<Pixels>, _: Pixels) {
    assert_eq!(text.len(), run.len);
    if w.record { w.captured[w.count] = Some((text, run.len, origin)); }
    else { black_box((text, run.len, origin)); }
    w.count += 1;
}
struct CellText { text: String }
// Production constructor expressions and grid_text bodies are inserted here verbatim.
__PRODUCTION__
type Grid = fn(&mut Window, &mut App, &str, &gpui::Font, Pixels, Hsla, gpui::Point<Pixels>, Pixels, Pixels, usize) -> usize;
fn grid(f: Grid, text: &str, limit: usize, window: &mut Window) -> usize {
    f(window, &mut App, text, &gpui::Font, 14., Hsla, point(3., 7.), 8., 16., limit)
}
fn check_grid(text: &str, limit: usize) {
    let mut a = Window::new(true); let mut b = Window::new(true);
    let n = grid(before_grid, text, limit, &mut a);
    assert_eq!(n, grid(after_grid, text, limit, &mut b));
    assert_eq!(a.count, b.count); assert_eq!(a.captured, b.captured);
}
fn time(f: &mut dyn FnMut(), iterations: usize) -> f64 {
    let start = Instant::now(); for _ in 0..iterations { f(); }
    start.elapsed().as_nanos() as f64 / iterations as f64
}
fn bench(name: &str, mut old: impl FnMut(), mut new: impl FnMut()) {
    for _ in 0..1_000 { old(); new(); }
    let mut a = Vec::new(); let mut b = Vec::new();
    for round in 0..15 {
        if round % 2 == 0 { a.push(time(&mut old, 100_000)); b.push(time(&mut new, 100_000)); }
        else { b.push(time(&mut new, 100_000)); a.push(time(&mut old, 100_000)); }
    }
    a.sort_by(f64::total_cmp); b.sort_by(f64::total_cmp);
    println!("TIMING {name}: before median={:.2}ns range={:.2}..{:.2}; after median={:.2}ns range={:.2}..{:.2}", a[7], a[0], a[14], b[7], b[0], b[14]);
}
fn main() {
    let samples = ["a", "界", "😀", "e\u{301}", "a\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}\u{301}"];
    for s in samples {
        let cell = CellText { text: s.to_owned() };
        assert_eq!(before_cell(&cell), after_cell(&cell));
        let owned = after_cell(&cell); drop(cell); assert_eq!(owned.as_str(), s);
    }
    let mut checked = 0;
    for n in 0..=0x10ffff {
        let Some(c) = char::from_u32(n) else { continue };
        let cell = CellText { text: c.to_string() };
        assert_eq!(before_cell(&cell), after_cell(&cell));
        // Test all UTF-8 encodings and zero/wide-character clipping at the exact width.
        check_grid(&cell.text, 1); check_grid(&cell.text, 2); checked += 1;
    }
    for text in ["", "abc", "中文abc😀", "e\u{301}x", "\t\r\n", "a\u{200d}😀b", "==> != <=", " foo bar "] {
        for limit in [0, 1, 2, 3, 5, 80] { check_grid(text, limit); }
    }
    println!("EQUIVALENCE: {checked} Unicode scalar values; mixed text, clipping, byte lengths, origins and owned-lifetime checks passed");
    for (i, s) in samples.iter().enumerate() {
        let cell = CellText { text: (*s).to_owned() };
        #[cfg(feature = "count-alloc")]
        {
            let a = allocation::count(|| { for _ in 0..10_000 { black_box(before_cell(black_box(&cell))); } });
            let b = allocation::count(|| { for _ in 0..10_000 { black_box(after_cell(black_box(&cell))); } });
            println!("ALLOCATION cell-{i} bytes={}: before={a}; after={b}; calls=10000", s.len());
            assert_eq!(a-b, 10_000); if s.len() <= 4 { assert_eq!(b, 0); }
        }
        #[cfg(not(feature = "count-alloc"))]
        bench(&format!("cell-{i}"), || { black_box(before_cell(black_box(&cell))); }, || { black_box(after_cell(black_box(&cell))); });
    }
    let text = "cargo build --release 中文😀 output";
    let mut a = Window::new(false); let mut b = Window::new(false);
    #[cfg(feature = "count-alloc")]
    {
        let before = allocation::count(|| { black_box(grid(before_grid, text, 80, &mut a)); });
        let after = allocation::count(|| { black_box(grid(after_grid, text, 80, &mut b)); });
        println!("ALLOCATION overlay: before={before}; after={after}; painted_cells={}", a.count);
        assert_eq!(before, a.count); assert_eq!(after, 0);
    }
    #[cfg(not(feature = "count-alloc"))]
    bench("overlay", || { black_box(grid(before_grid, black_box(text), 80, &mut a)); }, || { black_box(grid(after_grid, black_box(text), 80, &mut b)); });
}
'''
(output / 'src').mkdir(exist_ok=True)
(output / 'src/main.rs').write_text(code.replace('__PRODUCTION__', '\n'.join(parts)))
