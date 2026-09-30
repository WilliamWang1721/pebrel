use super::*;
use crate::index::Point;

/// Shell-reported boundaries in absolute grid coordinates; output stays in scrollback.
#[derive(Clone, Debug)]
pub struct CommandRegion {
    pub id: u64,
    pub prompt_line: usize,
    pub(super) input: Option<(usize, Column)>,
    pub output: Option<(usize, Column)>,
    pub end: Option<(usize, Column)>,
    pub command: String,
    pub cwd: String,
    pub exit_code: Option<i32>,
}

#[derive(Default)]
pub(super) struct CommandRegions {
    pub(super) entries: VecDeque<CommandRegion>,
    cwd: String,
    next_id: u64,
    pub(super) reported_command: Option<String>,
}

impl<T> Term<T> {
    pub fn command_regions(&self) -> impl DoubleEndedIterator<Item = &CommandRegion> {
        let floor = self.grid.scrolled_out();
        self.nebula_shell
            .commands
            .iter()
            .flat_map(|commands| commands.entries.iter())
            .filter(move |region| region.prompt_line >= floor)
    }

    pub fn nebula_command_cwd(&mut self, cwd: &str) {
        if let Some(commands) = &mut self.nebula_shell.commands {
            commands.cwd.clear();
            commands.cwd.push_str(cwd);
        }
    }

    pub fn nebula_command_line_report(&mut self, command: &str) {
        if !self.mode.contains(TermMode::ALT_SCREEN) {
            if let Some(commands) = &mut self.nebula_shell.commands {
                commands.reported_command = Some(command.to_owned());
            }
        }
    }

    pub(super) fn record_command_prompt(&mut self, prompt_line: usize) {
        let floor = self.grid.scrolled_out();
        let Some(commands) = &mut self.nebula_shell.commands else { return };
        while commands.entries.front().is_some_and(|region| region.prompt_line < floor) {
            commands.entries.pop_front();
        }
        while commands.entries.back().is_some_and(|region| region.prompt_line >= prompt_line) {
            commands.entries.pop_back();
        }
        commands.reported_command = None;
        commands.next_id += 1;
        commands.entries.push_back(CommandRegion {
            id: commands.next_id,
            prompt_line,
            input: None,
            output: None,
            end: None,
            command: String::new(),
            cwd: commands.cwd.clone(),
            exit_code: None,
        });
    }

    pub fn nebula_command_start(&mut self) {
        if self.nebula_shell.commands.is_some() && !self.mode.contains(TermMode::ALT_SCREEN) {
            let output = (self.nebula_cursor_abs_line(), self.grid.cursor.point.column);
            let input = self.nebula_shell.input;
            let reported = self
                .nebula_shell
                .commands
                .as_mut()
                .and_then(|commands| commands.reported_command.take());
            let command = reported.unwrap_or_else(|| {
                input.map(|start| self.command_region_text(start, output)).unwrap_or_default()
            });
            if let Some(region) =
                self.nebula_shell.commands.as_mut().and_then(|c| c.entries.back_mut())
            {
                if region.output.is_none() {
                    region.input = input;
                    region.output = Some(output);
                    region.command = command;
                }
            }
        }
        self.nebula_end_prompt();
    }

    pub fn nebula_command_done(&mut self, exit_code: Option<i32>) {
        if self.nebula_shell.commands.is_some() && !self.mode.contains(TermMode::ALT_SCREEN) {
            let end = (self.nebula_cursor_abs_line(), self.grid.cursor.point.column);
            if let Some(region) =
                self.nebula_shell.commands.as_mut().and_then(|c| c.entries.back_mut())
            {
                if region.output.is_some() && region.end.is_none() {
                    region.end = Some(end);
                    region.exit_code = exit_code;
                }
            }
        }
        self.nebula_end_prompt();
    }

    /// Plain text in a half-open region, excluding prompt/command decorations.
    pub fn command_region_text(&self, start: (usize, Column), end: (usize, Column)) -> String {
        let base = self.grid.scrolled_out() + self.grid.history_size();
        if start >= end || start.0 < self.grid.scrolled_out() {
            return String::new();
        }
        let start = Point::new(Line((start.0 as i64 - base as i64) as i32), start.1);
        let end = if end.1 == Column(0) {
            Point::new(Line((end.0 as i64 - base as i64 - 1) as i32), self.grid.last_column())
        } else {
            Point::new(Line((end.0 as i64 - base as i64) as i32), end.1 - 1)
        };
        self.bounds_to_string(start, end)
    }
}
