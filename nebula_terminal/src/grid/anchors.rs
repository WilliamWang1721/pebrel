//! Cold-path mapping of sorted cell anchors through reflow. No cell metadata or
//! transcript copies: two linear row walks, with storage proportional to anchors.
use super::{Dimensions, Grid, GridCell};
use crate::index::{Column, Line, Point};
use std::collections::VecDeque;

#[derive(Default)]
pub(super) struct ReflowTrim {
    pub lines: usize,
    pub partial: bool,
}

pub(super) struct LogicalAnchor {
    line: usize,
    offset: usize,
}

impl<T: GridCell + Default + PartialEq> Grid<T> {
    pub(super) fn retain_anchors(&self, marks: &mut VecDeque<Point<usize>>) {
        let end = self.scrolled_out() + self.total_lines();
        marks.retain(|p| {
            p.line >= self.scrolled_out() && p.line < end && p.column.0 <= self.columns()
        });
    }

    pub(super) fn capture_anchors(&self, marks: &VecDeque<Point<usize>>) -> Vec<LogicalAnchor> {
        let mut anchors = Vec::with_capacity(marks.len());
        let mut marks = marks.iter().filter(|p| p.line >= self.scrolled_out()).peekable();
        let (mut logical, mut offset) = (0, 0);
        for row in self.topmost_line().0..=self.bottommost_line().0 {
            let abs = (self.scrolled_out() + self.history_size()) as i64 + i64::from(row);
            while let Some(p) = marks.peek().filter(|p| p.line as i64 == abs) {
                anchors.push(LogicalAnchor { line: logical, offset: offset + p.column.0 });
                marks.next();
            }
            if marks.peek().is_none() {
                break;
            }
            if self.has_wrapline(Line(row)) {
                offset += self.logical_width(Line(row));
            } else {
                logical += 1;
                offset = 0;
            }
        }
        anchors
    }

    pub(super) fn restore_anchors(
        &self,
        anchors: &[LogicalAnchor],
        trim: ReflowTrim,
        marks: &mut VecDeque<Point<usize>>,
    ) {
        marks.clear();
        // A partly evicted logical line has no reliable original beginning.
        // Drop its anchors instead of shifting them into unrelated retained text.
        let mut anchors = anchors
            .iter()
            .filter(|a| a.line >= trim.lines && !(trim.partial && a.line == trim.lines))
            .peekable();
        let (mut logical, mut offset) = (trim.lines, 0);
        for row in self.topmost_line().0..=self.bottommost_line().0 {
            let wraps = self.has_wrapline(Line(row));
            let width = self.logical_width(Line(row));
            while let Some(a) = anchors.peek().filter(|a| a.line == logical) {
                if a.offset >= offset + width && wraps {
                    break;
                }
                if a.offset >= offset && a.offset <= offset + width {
                    let abs = (self.scrolled_out() + self.history_size()) as i64 + i64::from(row);
                    marks.push_back(Point::new(abs as usize, Column(a.offset - offset)));
                }
                anchors.next();
            }
            if anchors.peek().is_none() {
                break;
            }
            if wraps {
                offset += width;
            } else {
                logical += 1;
                offset = 0;
            }
        }
    }
}
