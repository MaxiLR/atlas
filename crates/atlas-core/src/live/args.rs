//! The live command's arguments.

use std::fmt;
use std::path::PathBuf;

use super::LiveTask;

/// How `atlas-live` is called.
pub const USAGE: &str = "usage: atlas-live (--all | --task <name>...) [--model <slug>] \
                         [--capture <dir>]";

/// What the command was asked to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    /// The tasks selected, each once, in the order named.
    pub tasks: Vec<LiveTask>,
    /// The model to use instead of the first one listed.
    pub model: Option<String>,
    /// Where to write the raw streams received.
    pub capture: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgsError {
    /// `--help` was asked for.
    Help,
    Invalid(String),
}

impl fmt::Display for ArgsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Help => write!(formatter, "{USAGE}\n\nThe tasks: {}", names()),
            Self::Invalid(message) => write!(formatter, "{message}\n{USAGE}"),
        }
    }
}

impl std::error::Error for ArgsError {}

fn names() -> String {
    LiveTask::ALL.map(LiveTask::name).join(", ")
}

/// Reads `--task <name>` (repeatable), `--all`, `--model <slug>` and
/// `--capture <dir>`. At least one task must be selected.
pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, ArgsError> {
    let invalid = |message: String| Err(ArgsError::Invalid(message));
    let mut args = args.into_iter();
    let mut tasks = Vec::new();
    let mut all = false;
    let mut model = None;
    let mut capture = None;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .filter(|value| !value.starts_with("--"))
                .ok_or_else(|| ArgsError::Invalid(format!("{name} needs a value")))
        };
        match arg.as_str() {
            "--all" => all = true,
            "--task" => {
                let name = value("--task")?;
                let Some(task) = LiveTask::from_name(&name) else {
                    return invalid(format!(
                        "unknown task \"{name}\"; the tasks are: {}",
                        names()
                    ));
                };
                if !tasks.contains(&task) {
                    tasks.push(task);
                }
            }
            "--model" => model = Some(value("--model")?),
            "--capture" => capture = Some(PathBuf::from(value("--capture")?)),
            "--help" | "-h" => return Err(ArgsError::Help),
            other => return invalid(format!("unknown argument \"{other}\"")),
        }
    }
    if all {
        tasks = LiveTask::ALL.to_vec();
    }
    if tasks.is_empty() {
        return invalid(format!(
            "select tasks with --task <name> or --all; the tasks are: {}",
            names()
        ));
    }
    Ok(Args {
        tasks,
        model,
        capture,
    })
}
