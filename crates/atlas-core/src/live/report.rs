//! The report as the engineer reads it: one line per task, its course-memory
//! details beneath it, then the models listed, the usage reading and the
//! captured streams.

use std::fmt::Write as _;
use std::time::Duration;

use super::{LiveResult, Memory, RecordState, Report, Search, Structured, TaskReport, Validity};
use crate::protocol::{UsageState, UsageWindow};

impl Report {
    /// Whether a task failed; a skipped one did not.
    pub fn failed(&self) -> bool {
        self.tasks
            .iter()
            .any(|line| matches!(line.result, LiveResult::Failed(_)))
    }

    /// The report as a table, for a terminal.
    pub fn table(&self) -> String {
        let mut table = String::new();
        let header = [
            "Task",
            "Result",
            "Structured",
            "Search",
            "Repairs",
            "Model",
            "Time",
        ];
        let rows: Vec<[String; 7]> = self.tasks.iter().map(row).collect();
        let mut widths = header.map(str::len);
        for row in &rows {
            for (width, cell) in widths.iter_mut().zip(row) {
                *width = (*width).max(cell.chars().count());
            }
        }
        let line = |cells: &[&str]| {
            let mut line = String::new();
            for (cell, width) in cells.iter().zip(widths) {
                let _ = write!(line, "{cell:<width$}  ");
            }
            line.trim_end().to_owned()
        };
        let _ = writeln!(table, "{}", line(&header));
        for (report, row) in self.tasks.iter().zip(&rows) {
            let cells: Vec<&str> = row.iter().map(String::as_str).collect();
            let _ = writeln!(table, "{}", line(&cells));
            if let LiveResult::Failed(error) = &report.result {
                let _ = writeln!(table, "    error: {error}");
            }
            for memory in &report.memory {
                let _ = writeln!(table, "    {}", memory_line(memory));
            }
        }

        table.push('\n');
        match &self.models {
            Ok(list) => {
                let _ = writeln!(table, "Models listed, in order (the first is the default):");
                for (number, model) in (1..).zip(&list.models) {
                    let current = if list.current.as_deref() == Some(model.slug.as_str()) {
                        "  (in use)"
                    } else {
                        ""
                    };
                    let _ = writeln!(
                        table,
                        "  {number}. {} ({}){current}",
                        model.slug, model.display_name
                    );
                }
            }
            Err(error) => {
                let _ = writeln!(table, "Models: the list could not be read: {error}");
            }
        }
        let _ = writeln!(table, "Usage: {}", usage_line(&self.usage));
        match &self.captured {
            None => {}
            Some(Ok(files)) => {
                let _ = writeln!(table, "Captured streams: {}", files.len());
                for file in files {
                    let _ = writeln!(table, "  {}", file.display());
                }
            }
            Some(Err(error)) => {
                let _ = writeln!(table, "Captured streams: not written: {error}");
            }
        }
        table
    }
}

fn row(report: &TaskReport) -> [String; 7] {
    let result = match &report.result {
        LiveResult::Passed => "passed",
        LiveResult::Failed(_) => "FAILED",
        LiveResult::Skipped => "skipped",
    };
    let structured = match report.structured {
        Some(Structured::Accepted) => "accepted",
        Some(Structured::Fallback) => "fallback used",
        None => "-",
    };
    let search = match report.search {
        Some(Search::Allowed) => "allowed",
        Some(Search::Refused) => "refused",
        None => "-",
    };
    [
        report.task.name().to_owned(),
        result.to_owned(),
        structured.to_owned(),
        search.to_owned(),
        report.repairs.to_string(),
        report.model.clone().unwrap_or_else(|| "-".to_owned()),
        seconds(report.took),
    ]
}

fn seconds(took: Duration) -> String {
    format!("{:.1} s", took.as_secs_f64())
}

fn validity(valid: Validity) -> &'static str {
    match valid {
        Validity::AtOnce => "valid at once",
        Validity::AfterRepair => "valid after its repair",
    }
}

fn memory_line(memory: &Memory) -> String {
    match memory {
        Memory::Map {
            valid,
            concepts,
            conventions,
        } => format!(
            "map: {}, {concepts} concepts, {conventions} conventions",
            validity(*valid)
        ),
        Memory::Record {
            section,
            record,
            rejected,
            input_chars,
        } => {
            let record = match record {
                RecordState::Confirmed => "record confirmed",
                RecordState::Unconfirmed => "record unconfirmed (the predicted one was stored)",
            };
            format!(
                "\"{section}\": {record}, {rejected} convention entries rejected as \
                 conflicting, input {input_chars} characters"
            )
        }
        Memory::Extension { valid } => format!("map extension: {}", validity(*valid)),
    }
}

fn usage_line(usage: &Result<UsageState, crate::protocol::CoreError>) -> String {
    match usage {
        Err(error) => format!("could not be read: {error}"),
        Ok(UsageState::NotConnected) => "no usage login".to_owned(),
        Ok(UsageState::Connecting { .. }) => "the usage login is waiting for its code".to_owned(),
        Ok(UsageState::SignedOut) => "the usage login has ended".to_owned(),
        Ok(UsageState::FirstReadFailed { reason }) => format!("the reading failed: {reason:?}"),
        Ok(UsageState::Connected { reading }) => {
            let windows: Vec<String> = reading.windows.iter().map(window).collect();
            let stale = match reading.stale {
                Some(reason) => format!(" (stale: {reason:?})"),
                None => String::new(),
            };
            format!(
                "{} at {}{stale}",
                windows.join(", "),
                reading.taken_at.to_rfc3339()
            )
        }
    }
}

fn window(window: &UsageWindow) -> String {
    let resets = window
        .resets_at
        .map(|at| format!(", resets {}", at.to_rfc3339()))
        .unwrap_or_default();
    format!(
        "{} {:.0} % used ({:?}{resets})",
        window.name,
        window.used * 100.0,
        window.level
    )
}
