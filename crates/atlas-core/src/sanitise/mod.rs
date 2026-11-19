//! The allow-list sanitiser for figures the model writes: SVG parsed and
//! re-serialised with nothing that can run, fetch or escape the theme.
//!
//! The input is parsed into a tree of the allowed elements only (anything
//! else goes with its whole subtree); doctypes, entities other than the
//! predefined ones, processing instructions and comments never reach the
//! tree. Definitions (gradients, clip paths, markers) are collected next, so
//! a `url(#id)` survives only when it names one that is kept; then each
//! attribute is checked against its element's allow-list and the tree is
//! written out again.

mod rules;

use quick_xml::escape::escape;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use rules::{Definitions, Kind};

/// SVG that passed the sanitiser. Inside the core it is made only by the
/// sanitiser; the interface receives it as a string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(transparent)]
pub struct SanitisedSvg(String);

impl SanitisedSvg {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a figure was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the figure was rejected: {reason}")]
pub struct Rejected {
    pub reason: String,
}

/// The largest figure accepted, in bytes.
pub const MAX_BYTES: usize = 64 * 1024;
/// The deepest element nesting accepted, the root counting as one.
pub const MAX_DEPTH: usize = 32;
/// The most elements accepted, counting those that are removed.
pub const MAX_ELEMENTS: usize = 2000;

/// Parses `input` as SVG and re-serialises what the allow-list keeps.
///
/// Refuses input that is not well-formed XML, whose root is not `svg`, that
/// exceeds [`MAX_BYTES`], [`MAX_DEPTH`] or [`MAX_ELEMENTS`], or that has
/// nothing drawable left once sanitised.
pub fn sanitise(input: &str) -> Result<SanitisedSvg, Rejected> {
    if input.len() > MAX_BYTES {
        return Err(rejected(format!("it is larger than {MAX_BYTES} bytes")));
    }
    let mut root = parse(input)?;
    let definitions = definitions(&mut root);
    clean(&mut root, &definitions);
    if !drawable(&root) {
        return Err(rejected("nothing drawable is left".to_owned()));
    }
    let mut output = String::with_capacity(input.len());
    write(&root, true, &mut output);
    Ok(SanitisedSvg(output))
}

fn rejected(reason: String) -> Rejected {
    Rejected { reason }
}

#[derive(Debug)]
enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug)]
struct Element {
    name: &'static str,
    attributes: Vec<(String, String)>,
    children: Vec<Node>,
}

/// One open element while parsing: kept, or skipped with its subtree.
enum Frame {
    Kept(Element),
    Skipped,
}

fn parse(input: &str) -> Result<Element, Rejected> {
    let mut reader = Reader::from_str(input);
    let mut stack: Vec<Frame> = Vec::new();
    let mut root: Option<Element> = None;
    let mut elements = 0;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| rejected(format!("it is not well-formed XML: {error}")))?;
        match event {
            Event::Start(_) | Event::Empty(_) if root.is_some() => {
                return Err(rejected("it has more than one root element".to_owned()));
            }
            Event::Start(_) | Event::Empty(_) if stack.len() == MAX_DEPTH => {
                return Err(rejected(format!(
                    "it nests elements deeper than {MAX_DEPTH}"
                )));
            }
            Event::Start(start) => {
                elements += 1;
                let frame = open(&start, &stack)?;
                stack.push(frame);
            }
            Event::Empty(start) => {
                elements += 1;
                let frame = open(&start, &stack)?;
                close(&mut stack, &mut root, frame);
            }
            Event::End(_) => {
                let frame = stack
                    .pop()
                    .ok_or_else(|| rejected("it has an unmatched end tag".to_owned()))?;
                close(&mut stack, &mut root, frame);
            }
            Event::Text(text) => push_text(&mut stack, &text.xml10_content()),
            Event::CData(text) => push_text(&mut stack, &text.xml10_content()),
            Event::GeneralRef(reference) => {
                let resolved = match reference.resolve_char_ref() {
                    Ok(Some(ch)) => Some(ch),
                    Ok(None) => quick_xml::escape::resolve_predefined_entity(&reference)
                        .and_then(|text| text.chars().next()),
                    Err(_) => None,
                };
                if let Some(ch) = resolved {
                    push_text(&mut stack, ch.encode_utf8(&mut [0; 4]));
                }
            }
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
            Event::Eof => break,
        }
        if elements > MAX_ELEMENTS {
            return Err(rejected(format!(
                "it has more than {MAX_ELEMENTS} elements"
            )));
        }
    }
    if !stack.is_empty() {
        return Err(rejected("it has an unclosed element".to_owned()));
    }
    root.ok_or_else(|| rejected("it has no root element".to_owned()))
}

/// Decides whether an element is kept; inside a skipped one, nothing is.
fn open(start: &BytesStart<'_>, stack: &[Frame]) -> Result<Frame, Rejected> {
    let parent = match stack.last() {
        None => None,
        Some(Frame::Kept(parent)) => Some(parent.name),
        Some(Frame::Skipped) => return Ok(Frame::Skipped),
    };
    let qname = start.name();
    let raw: &str = qname.as_ref();
    let Some(name) = rules::element_name(raw).filter(|name| rules::element_allowed(name, parent))
    else {
        if parent.is_none() {
            return Err(rejected(format!("its root is <{raw}>, not <svg>")));
        }
        return Ok(Frame::Skipped);
    };
    let mut attributes = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute
            .map_err(|error| rejected(format!("it has a malformed attribute: {error}")))?;
        let key: &str = attribute.key.as_ref();
        // A value naming an entity other than the predefined ones is dropped.
        let Ok(value) = attribute.normalized_value(XmlVersion::Implicit1_0) else {
            continue;
        };
        if key == "xmlns" && value != rules::SVG_NAMESPACE {
            // The element and its subtree are not SVG.
            if parent.is_none() {
                return Err(rejected(format!("its root is in the namespace {value}")));
            }
            return Ok(Frame::Skipped);
        }
        attributes.push((key.to_owned(), value.into_owned()));
    }
    Ok(Frame::Kept(Element {
        name,
        attributes,
        children: Vec::new(),
    }))
}

fn close(stack: &mut [Frame], root: &mut Option<Element>, frame: Frame) {
    let Frame::Kept(element) = frame else {
        return;
    };
    match stack.last_mut() {
        Some(Frame::Kept(parent)) => parent.children.push(Node::Element(element)),
        Some(Frame::Skipped) => {}
        None => *root = Some(element),
    }
}

/// Keeps text only inside the elements that show it.
fn push_text(stack: &mut [Frame], text: &str) {
    let Some(Frame::Kept(parent)) = stack.last_mut() else {
        return;
    };
    if !rules::TEXT_ELEMENTS.contains(&parent.name) {
        return;
    }
    match parent.children.last_mut() {
        Some(Node::Text(previous)) => previous.push_str(text),
        _ => parent.children.push(Node::Text(text.to_owned())),
    }
}

fn attribute<'e>(element: &'e Element, name: &str) -> Option<&'e str> {
    element
        .attributes
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// Collects the definitions a `url(#id)` may name and removes the ones that
/// cannot be kept: gradients whose stops carry a colour of their own, and
/// definitions without a valid id or repeating an earlier one.
fn definitions(root: &mut Element) -> Definitions {
    let mut found = Definitions::new();
    collect(root, &mut found);
    found
}

fn collect(element: &mut Element, found: &mut Definitions) {
    element.children.retain_mut(|child| {
        let Node::Element(child) = child else {
            return true;
        };
        if !rules::DEFINITIONS.contains(&child.name) {
            collect(child, found);
            return true;
        }
        let Some(id) = attribute(child, "id").filter(|id| rules::is_id(id)) else {
            return false;
        };
        if found.contains_key(id) || (child.name == "linearGradient" && !stops_pass(child)) {
            return false;
        }
        found.insert(id.to_owned(), child.name);
        collect(child, found);
        true
    });
}

/// A gradient is kept only when it has stops and every stop's colour passes.
fn stops_pass(gradient: &Element) -> bool {
    let empty = Definitions::new();
    let mut stops = 0;
    for child in &gradient.children {
        if let Node::Element(stop) = child {
            stops += 1;
            let colour = attribute(stop, "stop-color");
            if colour
                .and_then(|colour| rules::value(Kind::StopColor, colour, &empty))
                .is_none()
            {
                return false;
            }
        }
    }
    stops > 0
}

/// Keeps each attribute its element allows, with its value checked.
fn clean(element: &mut Element, definitions: &Definitions) {
    let name = element.name;
    element.attributes = std::mem::take(&mut element.attributes)
        .into_iter()
        .filter_map(|(key, value)| {
            let kind = rules::attribute(name, &key)?;
            rules::value(kind, &value, definitions).map(|value| (key, value))
        })
        .collect();
    for child in &mut element.children {
        if let Node::Element(child) = child {
            clean(child, definitions);
        }
    }
}

/// Whether anything is drawn outside definitions: a shape, or text.
fn drawable(element: &Element) -> bool {
    if element.name == "defs" || rules::DEFINITIONS.contains(&element.name) {
        return false;
    }
    if rules::SHAPES.contains(&element.name) {
        return true;
    }
    element.children.iter().any(|child| match child {
        Node::Element(child) => drawable(child),
        Node::Text(text) => matches!(element.name, "text" | "tspan") && !text.trim().is_empty(),
    })
}

fn write(element: &Element, root: bool, output: &mut String) {
    output.push('<');
    output.push_str(element.name);
    if root {
        output.push_str(" xmlns=\"");
        output.push_str(rules::SVG_NAMESPACE);
        output.push('"');
    }
    for (key, value) in &element.attributes {
        output.push(' ');
        output.push_str(key);
        output.push_str("=\"");
        output.push_str(&escape(value.as_str()));
        output.push('"');
    }
    if element.children.is_empty() {
        output.push_str("/>");
        return;
    }
    output.push('>');
    for child in &element.children {
        match child {
            Node::Element(child) => write(child, false, output),
            Node::Text(text) => output.push_str(&escape(text.as_str())),
        }
    }
    output.push_str("</");
    output.push_str(element.name);
    output.push('>');
}
