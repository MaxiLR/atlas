//! Checks and extends the course map ([`crate::domain::course_map`]) and
//! merges conventions so the first fixing holds (ADR-0013).
//!
//! Every function is pure: index order comes in as section ids, in the order
//! [`crate::index_tree::in_index_order`] gives them.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;

use uuid::Uuid;

use crate::domain::conventions::Convention;
use crate::domain::course_map::{CourseMap, MapEntry};
use crate::domain::ladder::{Ladder, RungId};
use crate::domain::record::{RecordStatus, SectionRecord, StoredRecord, TaughtConcept};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// The section has no map entry.
    MissingEntry,
    /// A concept relied on is introduced by no earlier section.
    NotIntroducedEarlier,
    /// A concept names a rung the ladder does not have.
    UnknownRung,
    /// A concept's name is empty or only space.
    EmptyName,
    /// An entry names a section that is not in the index.
    UnknownSection,
    /// The section has more than one entry.
    DuplicateEntry,
}

/// One fault of a map, in a form a repair request can quote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapFault {
    pub section: Uuid,
    pub concept: Option<String>,
    pub kind: FaultKind,
}

impl fmt::Display for MapFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let section = self.section;
        let concept = self.concept.as_deref().unwrap_or_default();
        match self.kind {
            FaultKind::MissingEntry => write!(f, "section {section} has no map entry"),
            FaultKind::NotIntroducedEarlier => write!(
                f,
                "section {section} relies on \"{concept}\", which no earlier section introduces"
            ),
            FaultKind::UnknownRung => write!(
                f,
                "section {section} introduces \"{concept}\", whose rung is not on the ladder"
            ),
            FaultKind::EmptyName => write!(
                f,
                "section {section} names a concept with an empty name, \"{concept}\""
            ),
            FaultKind::UnknownSection => {
                write!(
                    f,
                    "the map has an entry for {section}, which is not in the index"
                )
            }
            FaultKind::DuplicateEntry => {
                write!(f, "section {section} has more than one map entry")
            }
        }
    }
}

/// An incoming convention whose key is already fixed with another meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionConflict {
    pub fixed: Convention,
    pub incoming: Convention,
}

/// What [`merge_conventions`] makes of the incoming conventions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConventionMerge {
    /// Newly fixed, in the order they came.
    pub accepted: Vec<Convention>,
    /// Not accepted: the key already holds another meaning.
    pub conflicts: Vec<ConventionConflict>,
}

/// Every fault of `map` for the index in `index_order`: a section without an
/// entry, a relied-on concept no earlier section introduces, a rung the
/// ladder lacks, an empty concept name, and an entry for no section or a
/// second one. Faults of the index's sections come first, in index order;
/// empty when the map is valid. A concept introduced by several sections is
/// introduced by the earliest.
pub fn validate(map: &CourseMap, index_order: &[Uuid], ladder: &Ladder) -> Vec<MapFault> {
    faults(map, 0, index_order, ladder)
}

/// The map with the entries of a deep dive's proposals added, given the
/// index order once the children are in it. The proposals' sections must be
/// the sections of the index without an entry; each is checked by the rules
/// of [`validate`], so it may rely on what the parent, the sections before it
/// and its earlier siblings introduce, and never on a later section. Existing
/// entries and concepts are returned unchanged, followed by the proposals' and
/// by the proposed concepts not already named. On a fault, every fault of the
/// proposals.
pub fn extend(
    map: &CourseMap,
    index_order: &[Uuid],
    proposals: &CourseMap,
    ladder: &Ladder,
) -> Result<CourseMap, Vec<MapFault>> {
    let mut extended = map.clone();
    for concept in &proposals.concepts {
        if !extended.concepts.iter().any(|c| c.name == concept.name) {
            extended.concepts.push(concept.clone());
        }
    }
    extended.entries.extend(proposals.entries.iter().cloned());
    let faults = faults(&extended, map.entries.len(), index_order, ladder);
    if faults.is_empty() {
        Ok(extended)
    } else {
        Err(faults)
    }
}

/// The section's entry.
pub fn entry(map: &CourseMap, section: Uuid) -> Option<&MapEntry> {
    map.entries.iter().find(|entry| entry.section == section)
}

/// The section that introduced `concept`: the earliest in index order of
/// those introducing it.
pub fn introducer(map: &CourseMap, index_order: &[Uuid], concept: &str) -> Option<Uuid> {
    Order::new(index_order)
        .first_introductions(map)
        .get(concept)
        .map(|&at| index_order[at])
}

/// The sections that introduced the concepts `section` relies on, each once,
/// in index order.
pub fn prerequisite_sections(map: &CourseMap, index_order: &[Uuid], section: Uuid) -> Vec<Uuid> {
    let Some(entry) = entry(map, section) else {
        return Vec::new();
    };
    let introduced = Order::new(index_order).first_introductions(map);
    entry
        .relies_on
        .iter()
        .filter_map(|concept| introduced.get(concept.as_str()).copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|at| index_order[at])
        .collect()
}

/// Merges `incoming` into the course's conventions: the first fixing of a
/// key holds. Keys are compared per kind, ignoring case and surrounding space;
/// a key already fixed with the same meaning is neither new nor a conflict,
/// one fixed with another meaning is a conflict and not accepted. Earlier
/// incoming entries count as fixed for later ones.
pub fn merge_conventions(existing: &[Convention], incoming: &[Convention]) -> ConventionMerge {
    let mut merge = ConventionMerge::default();
    for convention in incoming {
        let key = normalise(&convention.key);
        let fixed = existing
            .iter()
            .chain(&merge.accepted)
            .find(|fixed| fixed.kind == convention.kind && normalise(&fixed.key) == key);
        match fixed {
            None => merge.accepted.push(convention.clone()),
            Some(fixed) if normalise(&fixed.meaning) == normalise(&convention.meaning) => {}
            Some(fixed) => {
                let conflict = ConventionConflict {
                    fixed: fixed.clone(),
                    incoming: convention.clone(),
                };
                merge.conflicts.push(conflict);
            }
        }
    }
    merge
}

/// The record a section's map entry predicts, used when the model's own
/// record cannot be confirmed: its planned concepts without lines, nothing
/// else, marked predicted.
pub fn predicted_record(entry: &MapEntry) -> StoredRecord {
    StoredRecord {
        record: SectionRecord {
            summary: None,
            concepts: entry
                .introduces
                .iter()
                .map(|name| TaughtConcept {
                    name: name.clone(),
                    line: None,
                })
                .collect(),
            notation: Vec::new(),
            terms: Vec::new(),
            examples: Vec::new(),
            promises: Vec::new(),
        },
        status: RecordStatus::Predicted,
    }
}

/// Positions in index order.
struct Order<'a> {
    sections: &'a [Uuid],
    position: HashMap<Uuid, usize>,
}

impl<'a> Order<'a> {
    fn new(sections: &'a [Uuid]) -> Self {
        let mut position = HashMap::with_capacity(sections.len());
        for (at, &section) in sections.iter().enumerate() {
            position.entry(section).or_insert(at);
        }
        Self { sections, position }
    }

    /// For each concept, the earliest position of a section introducing it.
    fn first_introductions<'m>(&self, map: &'m CourseMap) -> HashMap<&'m str, usize> {
        let mut first = HashMap::new();
        for entry in &map.entries {
            let Some(&at) = self.position.get(&entry.section) else {
                continue;
            };
            for concept in &entry.introduces {
                first
                    .entry(concept.as_str())
                    .and_modify(|earliest: &mut usize| *earliest = (*earliest).min(at))
                    .or_insert(at);
            }
        }
        first
    }
}

/// The faults of the entries from `checked_from` on, and of the sections of
/// the index that no earlier entry covers.
fn faults(
    map: &CourseMap,
    checked_from: usize,
    index_order: &[Uuid],
    ladder: &Ladder,
) -> Vec<MapFault> {
    let order = Order::new(index_order);
    let introduced = order.first_introductions(map);
    let mut rungs: HashMap<&str, Option<RungId>> = HashMap::new();
    for concept in &map.concepts {
        rungs.entry(concept.name.as_str()).or_insert(concept.rung);
    }
    let on_ladder: HashSet<RungId> = ladder.rungs.iter().map(|rung| rung.id).collect();
    let (settled, checked) = map.entries.split_at(checked_from);
    let settled: HashSet<Uuid> = settled.iter().map(|entry| entry.section).collect();

    let fault = |section, concept: &str, kind| MapFault {
        section,
        concept: Some(concept.to_owned()),
        kind,
    };
    let mut faults = Vec::new();
    for (at, &section) in order.sections.iter().enumerate() {
        if settled.contains(&section) {
            continue;
        }
        let Some(entry) = entry(map, section) else {
            faults.push(MapFault {
                section,
                concept: None,
                kind: FaultKind::MissingEntry,
            });
            continue;
        };
        for concept in &entry.introduces {
            if is_blank(concept) {
                faults.push(fault(section, concept, FaultKind::EmptyName));
            } else if introduced.get(concept.as_str()) == Some(&at)
                && let Some(Some(rung)) = rungs.get(concept.as_str())
                && !on_ladder.contains(rung)
            {
                faults.push(fault(section, concept, FaultKind::UnknownRung));
            }
        }
        for concept in &entry.relies_on {
            if is_blank(concept) {
                faults.push(fault(section, concept, FaultKind::EmptyName));
            } else if introduced
                .get(concept.as_str())
                .is_none_or(|&first| first >= at)
            {
                faults.push(fault(section, concept, FaultKind::NotIntroducedEarlier));
            }
        }
    }

    let mut seen = settled;
    for entry in checked {
        let kind = if !order.position.contains_key(&entry.section) {
            FaultKind::UnknownSection
        } else if !seen.insert(entry.section) {
            FaultKind::DuplicateEntry
        } else {
            continue;
        };
        faults.push(MapFault {
            section: entry.section,
            concept: None,
            kind,
        });
    }
    faults
}

fn is_blank(name: &str) -> bool {
    name.trim().is_empty()
}

/// Case and surrounding space set aside, with no language-specific rule.
fn normalise(text: &str) -> String {
    text.trim().to_lowercase()
}
