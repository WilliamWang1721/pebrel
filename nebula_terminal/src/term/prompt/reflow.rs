use super::*;
use crate::grid::Grid;
use crate::term::cell::{Cell, Flags, LineLength};

/// Logical line and cell offset, measured from the last occupied line.
pub(in crate::term) struct ShellAnchors(Vec<Option<(i64, usize)>>);

struct LogicalLines {
    rows: Vec<(usize, usize)>,
    starts: Vec<usize>,
    floor: usize,
    reference: usize,
}

impl LogicalLines {
    fn new(grid: &Grid<Cell>) -> Self {
        let floor = grid.scrolled_out();
        let base = floor + grid.history_size();
        let top = grid.topmost_line();
        let bottom = grid.bottommost_line();
        let reference = (top.0..=bottom.0)
            .rev()
            .map(Line)
            .find(|&line| grid[line].line_length() > Column(0))
            .unwrap_or(top);
        let mut rows = Vec::new();
        let mut starts = vec![floor];
        let mut offset = 0;
        for line in (top.0..=bottom.0).map(Line) {
            rows.push((starts.len() - 1, offset));
            let row = &grid[line];
            let last = &row[grid.last_column()];
            if last.flags.contains(Flags::WRAPLINE) {
                offset += grid.columns()
                    - usize::from(last.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER));
            } else if line != bottom {
                starts.push((base as i64 + line.0 as i64 + 1) as usize);
                offset = 0;
            }
        }
        let reference = rows[(reference.0 - top.0) as usize].0;
        Self { rows, starts, floor, reference }
    }

    fn anchor(&self, point: (usize, Column), columns: usize) -> Option<(i64, usize)> {
        let row = point.0.checked_sub(self.floor)?;
        let (line, offset) = *self.rows.get(row).or_else(|| {
            (row == self.rows.len() && point.1 == Column(0)).then(|| self.rows.last()).flatten()
        })?;
        let column = if row == self.rows.len() { columns } else { point.1.0 };
        Some((self.reference as i64 - line as i64, offset + column))
    }

    fn point(&self, anchor: (i64, usize), columns: usize) -> Option<(usize, Column)> {
        let index = usize::try_from(self.reference as i64 - anchor.0).ok()?;
        if index >= self.starts.len() {
            return None;
        }
        let start = self.starts[index] - self.floor;
        let end = self.starts.get(index + 1).map_or(self.rows.len(), |next| *next - self.floor);
        let row = (start..end).rev().find(|&row| self.rows[row].1 <= anchor.1)?;
        let column = anchor.1 - self.rows[row].1;
        (column <= columns).then_some((self.floor + row, Column(column)))
    }
}

impl<T> Term<T> {
    fn primary_grid(&self) -> &Grid<Cell> {
        if self.mode.contains(TermMode::ALT_SCREEN) { &self.inactive_grid } else { &self.grid }
    }

    pub(in crate::term) fn shell_anchors(&self) -> Option<ShellAnchors> {
        let commands = self.nebula_shell.commands.as_ref()?;
        let lines = LogicalLines::new(self.primary_grid());
        let mut anchors = Vec::new();
        for region in commands.entries.iter() {
            anchors.push(lines.anchor((region.prompt_line, Column(0)), self.columns()));
            for point in [region.input, region.output, region.end].into_iter().flatten() {
                anchors.push(lines.anchor(point, self.columns()));
            }
        }
        if let Some(input) = self.nebula_shell.input {
            anchors.push(lines.anchor(input, self.columns()));
        }
        Some(ShellAnchors(anchors))
    }

    pub(in crate::term) fn restore_shell_anchors(&mut self, anchors: Option<ShellAnchors>) {
        self.nebula_shell.marks.clear();
        let Some(ShellAnchors(anchors)) = anchors else {
            self.nebula_shell.input = None;
            return;
        };
        let lines = LogicalLines::new(self.primary_grid());
        let columns = self.columns();
        let commands = self.nebula_shell.commands.as_mut().unwrap();
        // Consume exactly the same boundaries even when a reflow evicts a region.
        let mut anchors = anchors.into_iter();
        commands.entries.retain_mut(|region| {
            let prompt = anchors.next().flatten().and_then(|a| lines.point(a, columns));
            for point in [&mut region.input, &mut region.output, &mut region.end] {
                if point.is_some() {
                    *point = anchors.next().flatten().and_then(|a| lines.point(a, columns));
                }
            }
            if let Some((line, _)) = prompt {
                region.prompt_line = line;
                true
            } else {
                false
            }
        });
        self.nebula_shell.input = anchors.next().flatten().and_then(|a| lines.point(a, columns));
        self.nebula_shell.marks.extend(commands.entries.iter().map(|region| region.prompt_line));
    }
}
