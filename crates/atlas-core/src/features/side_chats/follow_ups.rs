//! The reserved block a reply may close with, offering follow-up prompts:
//! a line of three backticks and `atlas-follow-ups`, one prompt per line, and
//! a closing line of three backticks. It never reaches the reply's text.

/// The line that opens the block.
const FENCE: &str = "```atlas-follow-ups";

/// The most prompts kept.
const MOST: usize = 3;

/// Follows the reply as it arrives and holds the follow-up block back. The
/// start of a line is held until it can no longer be the block's opening
/// line; from that line on, everything is the block.
#[derive(Debug, Default)]
pub(super) struct FollowUps {
    /// The start of the current line, which may still open the block.
    held: String,
    /// The current line is shown as it arrives: it cannot open the block.
    passing: bool,
    /// What followed the opening line's fence, once the block opened.
    block: Option<String>,
}

impl FollowUps {
    /// Reads more of the reply; answers the text to show now.
    pub(super) fn push(&mut self, text: &str) -> String {
        let mut shown = String::new();
        for next in text.chars() {
            if let Some(block) = &mut self.block {
                block.push(next);
                continue;
            }
            if self.passing {
                shown.push(next);
                self.passing = next != '\n';
                continue;
            }
            self.held.push(next);
            let line = self.held.trim_start_matches([' ', '\t']);
            if let Some(rest) = line.strip_prefix(FENCE) {
                self.block = Some(rest.to_owned());
                self.held.clear();
            } else if next == '\n' || !FENCE.starts_with(line) {
                shown.push_str(&self.held);
                self.held.clear();
                self.passing = next != '\n';
            }
        }
        shown
    }

    /// The reply is complete: answers the text still held, which did not
    /// open the block.
    pub(super) fn finish(&mut self) -> String {
        std::mem::take(&mut self.held)
    }

    /// The block's prompts, at most three, in order, without list marks.
    pub(super) fn prompts(&self) -> Vec<String> {
        let Some(block) = &self.block else {
            return Vec::new();
        };
        // The rest of the opening line is not a prompt.
        let body = block.split_once('\n').map_or("", |(_, body)| body);
        body.lines()
            .map(str::trim)
            .take_while(|line| *line != "```")
            .map(unmarked)
            .filter(|line| !line.is_empty())
            .take(MOST)
            .map(str::to_owned)
            .collect()
    }
}

/// A line without the list mark it may start with: `-`, `*`, `•`, or a
/// number followed by `.` or `)`.
fn unmarked(line: &str) -> &str {
    let rest = line
        .strip_prefix(['-', '*', '•'])
        .or_else(|| {
            let digits = line.trim_start_matches(|c: char| c.is_ascii_digit());
            (digits.len() < line.len())
                .then(|| digits.strip_prefix(['.', ')']))
                .flatten()
        })
        .unwrap_or(line);
    rest.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `text` pushed one character at a time, then finished.
    fn read(text: &str) -> (String, FollowUps) {
        let mut follow_ups = FollowUps::default();
        let mut shown = String::new();
        for next in text.chars() {
            shown.push_str(&follow_ups.push(&next.to_string()));
        }
        shown.push_str(&follow_ups.finish());
        (shown, follow_ups)
    }

    #[test]
    fn a_reply_without_the_block_is_shown_whole_with_its_code_fences() {
        let text = "Run it:\n```sh\nls\n```\nDone ``.";
        let (shown, follow_ups) = read(text);
        assert_eq!(shown, text);
        assert_eq!(follow_ups.prompts(), Vec::<String>::new());
    }

    #[test]
    fn the_block_is_held_back_and_its_marked_prompts_read_without_marks() {
        let (shown, follow_ups) = read(
            "Slowly.\n\n```atlas-follow-ups\n- How slowly?\n\n2) Why?\n* Who measured it?\n• Where?\n```\nignored",
        );
        assert_eq!(shown, "Slowly.\n\n");
        assert_eq!(
            follow_ups.prompts(),
            ["How slowly?", "Why?", "Who measured it?"]
        );
    }

    #[test]
    fn a_block_cut_off_keeps_the_prompts_that_arrived() {
        let (shown, follow_ups) = read("Yes.\n```atlas-follow-ups\nWhy?\nHow");
        assert_eq!(shown, "Yes.\n");
        assert_eq!(follow_ups.prompts(), ["Why?", "How"]);
    }
}
