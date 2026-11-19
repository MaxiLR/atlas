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

/// Every prompt the core embeds.
pub const ALL: [Prompt; 3] = [BUILD_INDEX, WRITE_SECTION, REPAIR_STRUCTURED];

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
