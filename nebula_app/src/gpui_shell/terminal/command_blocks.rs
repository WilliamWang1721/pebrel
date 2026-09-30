use gpui::{App, Bounds, Pixels, Point, Window, fill, outline, point, px, size};
use gpui_component::ActiveTheme as _;
use nebula_terminal::Term;
use nebula_terminal::event::EventListener;
use nebula_terminal::grid::Dimensions as _;
use nebula_terminal::term::TermMode;

pub(super) struct VisibleBlock {
    pub id: u64,
    pub start: usize,
    pub end: usize,
}

pub(super) fn visible<T: EventListener>(term: &Term<T>, rows: usize) -> Vec<VisibleBlock> {
    if term.mode().intersects(TermMode::ALT_SCREEN | TermMode::MOUSE_MODE | TermMode::VI) {
        return Vec::new();
    }
    let base = term.grid().scrolled_out() + term.history_size();
    let top = base as i64 + i64::from(term.viewport_origin_for(rows).0);
    let bottom = top + rows as i64;
    term.command_regions()
        .rev()
        .take_while(|region| {
            region.end.map_or(term.nebula_cursor_abs_line() + 1, |end| end.0 + 1) as i64 >= top
        })
        .filter_map(|region| {
            let end = region
                .end
                .map_or(term.nebula_cursor_abs_line() + 1, |end| end.0 + usize::from(end.1.0 > 0));
            let start = region.prompt_line as i64;
            (start < bottom).then_some(VisibleBlock {
                id: region.id,
                start: (start - top).max(0) as usize,
                end: (end as i64 - top).clamp(0, rows as i64) as usize,
            })
        })
        .collect()
}

pub(super) fn paint(
    blocks: &[VisibleBlock],
    selected: Option<u64>,
    hovered: Option<u64>,
    origin: Point<Pixels>,
    width: Pixels,
    line_height: Pixels,
    window: &mut Window,
    cx: &App,
) {
    for block in blocks {
        let bounds = Bounds::new(
            point(origin.x - px(6.0), origin.y + line_height * block.start as f32 - px(2.0)),
            size(width + px(10.0), line_height * block.end.saturating_sub(block.start) as f32),
        );
        let is_selected = selected == Some(block.id);
        let color = if is_selected {
            cx.theme().ring
        } else if hovered == Some(block.id) {
            cx.theme().foreground.opacity(0.4)
        } else {
            cx.theme().border
        };
        window.paint_quad(outline(bounds, color, gpui::BorderStyle::Solid).corner_radii(px(4.0)));
        if is_selected {
            window.paint_quad(fill(
                Bounds::new(bounds.origin, size(px(3.0), bounds.size.height)),
                color,
            ));
        }
    }
}
