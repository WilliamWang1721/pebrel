//! Shell-reported command regions over the existing scrollback. Text is read
//! only when selected/copied; regions never own another copy of terminal output.
use super::{Term, TermMode};
use crate::grid::Dimensions;
use crate::index::{Boundary, Column, Line, Point, Side};
use crate::selection::{Selection, SelectionType};
use std::ops::Range;

/// A half-open command/input-and-output region in current grid coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptBlock {
    pub start: Point,
    pub end: Point,
    pub accepting_input: bool,
}

impl PromptBlock {
    pub fn contains(&self, point: Point) -> bool {
        self.start <= point && point < self.end
    }

    pub fn selection<T>(&self, term: &Term<T>) -> Option<Selection> {
        if self.start >= self.end {
            return None;
        }
        let mut selection = Selection::new(SelectionType::Simple, self.start, Side::Left);
        let end = if self.end.column == Column(0) {
            self.end.sub(term, Boundary::Grid, 1)
        } else {
            Point::new(self.end.line, self.end.column - 1)
        };
        selection.update(end, Side::Right);
        Some(selection)
    }
}

impl<T> Term<T> {
    /// O(log(mark count) + visible blocks), without allocations. Unsupported
    /// shells have no marks; alternate-screen and mouse-reporting apps opt out.
    pub fn prompt_blocks(&self, rows: Range<Line>) -> impl Iterator<Item = PromptBlock> + '_ {
        let origin = self.grid.scrolled_out() + self.grid.history_size();
        let top = (origin as i64 + i64::from(rows.start.0)).max(0) as usize;
        let marks = &self.nebula_prompt_marks;
        let first = marks.partition_point(|p| p.line < top).saturating_sub(1);
        let allowed =
            !self.mode.intersects(TermMode::ALT_SCREEN | TermMode::MOUSE_MODE | TermMode::VI);
        (first..marks.len())
            .take_while(move |&i| {
                allowed && (marks[i].line as i64 - origin as i64) < i64::from(rows.end.0)
            })
            .filter_map(move |i| {
                let start = marks[i];
                if start.line < self.grid.scrolled_out() {
                    return None;
                }
                let last = i + 1 == marks.len();
                let end = marks.get(i + 1).copied().unwrap_or_else(|| {
                    Point::new(
                        self.nebula_cursor_abs_line(),
                        self.grid.cursor.point.column
                            + usize::from(self.grid.cursor.input_needs_wrap),
                    )
                });
                let relative = |p: Point<usize>| {
                    let mut p = Point::new(Line((p.line as i64 - origin as i64) as i32), p.column);
                    if p.column.0 == self.columns() {
                        p.line += 1;
                        p.column = Column(0);
                    }
                    p
                };
                let start = relative(start);
                let end = relative(end);
                (start <= end && end.line >= rows.start && start.line >= self.grid.topmost_line())
                    .then_some(PromptBlock {
                        start,
                        end,
                        accepting_input: last && self.nebula_prompt_active(),
                    })
            })
    }

    pub fn prompt_block_at(&self, point: Point) -> Option<PromptBlock> {
        self.prompt_blocks(point.line..point.line + 1).find(|block| block.contains(point))
    }
}

#[cfg(test)]
#[path = "blocks_tests.rs"]
mod tests;
