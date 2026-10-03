use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;
use nebula_terminal::event::VoidListener;
use nebula_terminal::term::{Config, Term, test::TermSize};
use nebula_terminal::vte::ansi::Handler;

struct Counting;
static ENABLED: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ENABLED.load(Ordering::Relaxed) { ALLOCATIONS.fetch_add(1, Ordering::Relaxed); }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if ENABLED.load(Ordering::Relaxed) { ALLOCATIONS.fetch_add(1, Ordering::Relaxed); }
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn main() {
    let mut cases = vec![
        ("ascii".to_owned(), "printf hello world 1234567890\r\n".repeat(12000)),
        ("cjk".to_owned(), "中文终端输出测试\r\n".repeat(20000)),
        ("emoji".to_owned(), "👨‍👩‍👧 🏳️‍🌈 ❤️‍🔥 🐦‍⬛ 🙂‍↔️ 👍🏽 🇨🇳\r\n".repeat(6000)),
        ("zwj-chain".to_owned(), format!("👨{}\r\n", "‍👩".repeat(2000)).repeat(40)),
        ("selectors".to_owned(), format!("❤{}\r\n", "️".repeat(20000)).repeat(8)),
    ];
    for count in [1000, 2000, 4000, 16000] {
        cases.push((format!("marks-{count}"), format!("👨{}‍👩\r\n", "́".repeat(count)).repeat(160000 / count)));
    }
    for (label, text) in cases {
        let chars = text.chars().count();
        let mut samples = Vec::new();
        let mut allocations = 0;
        for _ in 0..5 {
            let mut term = Term::new(Config { scrolling_history: 0, ..Config::default() }, &TermSize::new(120, 32), VoidListener);
            ALLOCATIONS.store(0, Ordering::Relaxed);
            ENABLED.store(true, Ordering::Relaxed);
            let start = Instant::now();
            for c in text.chars() { term.input(c); }
            let elapsed = start.elapsed().as_nanos();
            ENABLED.store(false, Ordering::Relaxed);
            allocations = ALLOCATIONS.load(Ordering::Relaxed);
            std::hint::black_box(&term);
            samples.push(elapsed as f64 / chars as f64);
        }
        samples.sort_by(f64::total_cmp);
        println!("{{\"case\":\"{label}\",\"chars\":{chars},\"ns_per_char\":{},\"allocations\":{allocations}}}", samples[2]);
    }
}
