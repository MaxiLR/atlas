//! Every prompt is a versioned text file in `prompts/`, embedded at build time.
//!
//! A prompt file starts with front matter naming its version:
//!
//! ```text
//! ---
//! version: 1
//! ---
//! ```

/// One embedded prompt file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prompt {
    name: &'static str,
    source: &'static str,
}

pub const DRAFT_LADDER: Prompt = Prompt {
    name: "draft-ladder",
    source: include_str!("../../../../prompts/draft-ladder.md"),
};

pub const PLACEMENT_QUESTION: Prompt = Prompt {
    name: "placement-question",
    source: include_str!("../../../../prompts/placement-question.md"),
};

pub const BUILD_INDEX: Prompt = Prompt {
    name: "build-index",
    source: include_str!("../../../../prompts/build-index.md"),
};

pub const WRITE_SECTION: Prompt = Prompt {
    name: "write-section",
    source: include_str!("../../../../prompts/write-section.md"),
};

pub const REPAIR_STRUCTURED: Prompt = Prompt {
    name: "repair-structured",
    source: include_str!("../../../../prompts/repair-structured.md"),
};

pub const REPAIR_BLOCK: Prompt = Prompt {
    name: "repair-block",
    source: include_str!("../../../../prompts/repair-block.md"),
};

pub const REPAIR_RECORD: Prompt = Prompt {
    name: "repair-record",
    source: include_str!("../../../../prompts/repair-record.md"),
};

pub const DEFINE: Prompt = Prompt {
    name: "define",
    source: include_str!("../../../../prompts/define.md"),
};

/// Every prompt the core embeds.
pub const ALL: [Prompt; 8] = [
    DRAFT_LADDER,
    PLACEMENT_QUESTION,
    BUILD_INDEX,
    WRITE_SECTION,
    REPAIR_STRUCTURED,
    REPAIR_BLOCK,
    REPAIR_RECORD,
    DEFINE,
];

impl Prompt {
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The version named in the front matter.
    pub fn version(&self) -> u32 {
        let (front_matter, _) = self.split();
        front_matter
            .lines()
            .find_map(|line| line.strip_prefix("version:"))
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or_else(|| panic!("prompt {} names no version", self.name))
    }

    /// `name@version`, as stored with what the prompt produced.
    pub fn id(&self) -> String {
        format!("{}@{}", self.name, self.version())
    }

    /// The prompt itself, without its front matter.
    pub fn text(&self) -> &'static str {
        self.split().1.trim()
    }

    fn split(&self) -> (&'static str, &'static str) {
        self.source
            .strip_prefix("---\n")
            .and_then(|rest| rest.split_once("\n---\n"))
            .unwrap_or_else(|| panic!("prompt {} has no front matter", self.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_prompt_names_a_version_and_has_text() {
        for prompt in ALL {
            assert!(prompt.version() >= 1, "{}", prompt.name());
            assert!(!prompt.text().is_empty(), "{}", prompt.name());
            assert!(!prompt.text().contains("version:"), "{}", prompt.name());
        }
    }

    #[test]
    fn the_id_joins_name_and_version() {
        assert_eq!(
            BUILD_INDEX.id(),
            format!("build-index@{}", BUILD_INDEX.version())
        );
    }
}
