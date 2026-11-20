//! Web grounding of a section: its citations placed on its prose blocks as
//! the output arrives, and the route's refusal of the search tool.

use crate::harness::citations::{Citations, PlacedCitations};
use crate::harness::splitter::{SplitEvent, Splitter};
use crate::protocol::{Citation, Source};
use crate::provider::{ProviderCitation, ProviderError};

/// Follows the output text to place its citations. A splitter of its own,
/// fed one line at a time, tells which output characters are the text of
/// which prose block: the prose a line completes ends what was read.
#[derive(Default)]
pub(super) struct Grounding {
    splitter: Splitter,
    citations: Citations,
    /// Output characters read so far.
    read: u32,
}

impl Grounding {
    /// Reads more of the output; answers the citations it lets be placed.
    pub(super) fn push(&mut self, text: &str) -> Vec<(Citation, Source)> {
        for line in text.split_inclusive('\n') {
            self.read += chars(line);
            let split = self.splitter.push(line.as_bytes());
            self.lay_out(split);
        }
        self.citations.ready()
    }

    /// Takes a citation; answers the citations placed now.
    pub(super) fn cite(&mut self, citation: ProviderCitation) -> Vec<(Citation, Source)> {
        self.citations.cite(citation);
        self.citations.ready()
    }

    /// The output is complete: answers the citations placed last, then every
    /// citation with the numbered sources.
    pub(super) fn finish(mut self) -> (Vec<(Citation, Source)>, PlacedCitations) {
        let split = std::mem::take(&mut self.splitter).finish();
        self.lay_out(split);
        let last = self.citations.finish();
        (last, self.citations.into_placed())
    }

    /// Lays out the prose of `split`, which ends what was read.
    fn lay_out(&mut self, split: Vec<SplitEvent>) {
        let prose: Vec<(u32, u32)> = split
            .into_iter()
            .filter_map(|event| match event {
                SplitEvent::Prose { block, text } => Some((block, chars(&text))),
                _ => None,
            })
            .collect();
        let mut start = self.read - prose.iter().map(|(_, chars)| chars).sum::<u32>();
        for (block, chars) in prose {
            self.citations.prose(start, block, chars);
            start += chars;
        }
    }
}

fn chars(text: &str) -> u32 {
    text.chars().count() as u32
}

/// Whether the route refused the hosted search tool: the unsupported
/// capability is `tools`, one of its entries, or the search tool itself.
/// Side-chat replies are grounded the same way.
pub(crate) fn refuses_search(error: &ProviderError) -> bool {
    matches!(
        error,
        ProviderError::Unsupported { param: Some(param), .. }
            if param == "tools"
                || param.starts_with("tools[")
                || param.starts_with("tools.")
                || param.contains("web_search")
    )
}
