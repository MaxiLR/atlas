//! "Explain differently": new versions of a section in another style, and the
//! choice of the current version.
//!
//! A new version is a section generation with the explain-differently
//! prompt's instructions for the style added and the current version's record
//! as the account of the explanation that did not land. Once complete it is
//! stored with its own record and becomes current; the earlier versions stay
//! as they are. Selecting a version only moves the current pointer.

use std::sync::Arc;

use uuid::Uuid;

use super::Features;
use super::sections::Rewrite;
use crate::CoreDeps;
use crate::harness::prompts;
use crate::protocol::{CoreError, ExplainStyle, SectionView, VersionStyle};
use crate::store::{NodeKind, NodeRecord, Store};

pub(crate) struct Feature {
    store: Arc<Store>,
}

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
        })
    }

    /// Writes a new version of the section in `style` in place of its
    /// current one, streamed as events; once complete it is stored with its
    /// own record and becomes current. Stopped or failed, nothing changes.
    pub(crate) async fn explain_differently(
        &self,
        cx: &Features,
        course_id: Uuid,
        section_id: Uuid,
        style: ExplainStyle,
    ) -> Result<SectionView, CoreError> {
        let node = self.section(course_id, section_id)?;
        let current =
            self.store
                .current_version(node.id)?
                .ok_or_else(|| CoreError::InvalidRequest {
                    message: format!("section {section_id} has no version to explain differently"),
                })?;
        let rewrite = Rewrite {
            parent: current.id,
            instructions: instructions(style),
            prompt: prompts::EXPLAIN_DIFFERENTLY,
        };
        cx.sections
            .generate(cx, node.id, version_style(style), Some(rewrite))
            .await
    }

    /// Makes one of the section's own versions current, with no request,
    /// and answers the section with that version.
    pub(crate) async fn select_version(
        &self,
        cx: &Features,
        course_id: Uuid,
        section_id: Uuid,
        version_id: Uuid,
    ) -> Result<SectionView, CoreError> {
        self.section(course_id, section_id)?;
        self.store.set_current_version(section_id, version_id)?;
        cx.sections.open(cx, course_id, section_id).await
    }

    /// The section `section_id` of the course `course_id`.
    fn section(&self, course_id: Uuid, section_id: Uuid) -> Result<NodeRecord, CoreError> {
        self.store
            .index_node(section_id)?
            .filter(|node| node.kind == NodeKind::Section && node.course_id == course_id)
            .ok_or_else(|| CoreError::NotFound {
                what: format!("section {section_id}"),
            })
    }
}

fn version_style(style: ExplainStyle) -> VersionStyle {
    match style {
        ExplainStyle::Simpler => VersionStyle::Simpler,
        ExplainStyle::MoreTechnical => VersionStyle::MoreTechnical,
        ExplainStyle::MoreExamples => VersionStyle::MoreExamples,
    }
}

/// The heading of the style's part of the explain-differently prompt.
fn heading(style: ExplainStyle) -> &'static str {
    match style {
        ExplainStyle::Simpler => "Simpler",
        ExplainStyle::MoreTechnical => "More technical",
        ExplainStyle::MoreExamples => "More examples",
    }
}

/// The explain-differently prompt for `style`: its opening, which every
/// style shares, and the part under the style's own heading.
fn instructions(style: ExplainStyle) -> String {
    let text = prompts::EXPLAIN_DIFFERENTLY.text();
    let mut parts = text.split("\n## ");
    let opening = parts.next().unwrap_or_default().trim();
    let heading = heading(style);
    let part = parts
        .find_map(|part| part.strip_prefix(heading)?.strip_prefix('\n'))
        .unwrap_or_else(|| {
            panic!(
                "prompt {} has no part headed {heading}",
                prompts::EXPLAIN_DIFFERENTLY.name()
            )
        });
    format!(
        "## Explaining differently\n\n{opening}\n\n### {heading}\n\n{}",
        part.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLES: [ExplainStyle; 3] = [
        ExplainStyle::Simpler,
        ExplainStyle::MoreTechnical,
        ExplainStyle::MoreExamples,
    ];

    #[test]
    fn each_style_gets_the_shared_opening_and_its_own_part_alone() {
        let opening = "The candidate has read this section once";
        let parts = [
            "Explain it more simply.",
            "Explain it more technically",
            "Explain it through more examples.",
        ];
        for (style, own) in STYLES.into_iter().zip(parts) {
            let text = instructions(style);
            assert!(text.contains(opening), "{style:?}: {text}");
            assert!(text.contains(own), "{style:?}: {text}");
            for other in parts.iter().filter(|other| **other != own) {
                assert!(!text.contains(other), "{style:?}: {text}");
            }
        }
    }
}
