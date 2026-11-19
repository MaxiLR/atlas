//! The figure sanitiser on fixture files: a hostile corpus comes out inert,
//! allowed figures come out unchanged up to serialisation, and sanitising
//! twice changes nothing.
//!
//! The inertness check below is written independently of the sanitiser: it
//! re-parses the output and holds it against the ticket's allow-lists.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use atlas_core::sanitise::{sanitise, SanitisedSvg};
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// The figure survives with something drawable left.
    Kept,
    /// The figure is refused as a whole.
    Rejected,
}

const HOSTILE: &[(&str, Outcome)] = &[
    ("script-element.svg", Outcome::Kept),
    ("script-mixed-case.svg", Outcome::Kept),
    ("event-attributes.svg", Outcome::Kept),
    ("javascript-links.svg", Outcome::Kept),
    ("data-links.svg", Outcome::Kept),
    ("foreign-object.svg", Outcome::Kept),
    ("external-use-image.svg", Outcome::Kept),
    ("url-in-fill.svg", Outcome::Kept),
    ("url-in-style.svg", Outcome::Kept),
    ("css-import.svg", Outcome::Kept),
    ("css-expression.svg", Outcome::Kept),
    ("nested-svg.svg", Outcome::Kept),
    ("entity-expansion.svg", Outcome::Kept),
    ("namespaced-script.svg", Outcome::Kept),
    ("animation-href.svg", Outcome::Kept),
    ("processing-instructions.svg", Outcome::Kept),
    ("cdata-tricks.svg", Outcome::Kept),
    ("colours.svg", Outcome::Kept),
    ("nothing-drawable.svg", Outcome::Rejected),
    ("not-svg.svg", Outcome::Rejected),
    ("oversized.svg", Outcome::Rejected),
    ("too-deep.svg", Outcome::Rejected),
    ("too-many-elements.svg", Outcome::Rejected),
];

const ELEMENTS: &[&str] = &[
    "svg",
    "g",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "text",
    "tspan",
    "title",
    "desc",
    "defs",
    "marker",
    "clipPath",
    "linearGradient",
    "stop",
];

const DRAWABLE: &[&str] = &[
    "path", "rect", "circle", "ellipse", "line", "polyline", "polygon",
];

const ATTRIBUTES: &[&str] = &[
    "xmlns",
    "id",
    "viewBox",
    "width",
    "height",
    "x",
    "y",
    "dx",
    "dy",
    "x1",
    "y1",
    "x2",
    "y2",
    "cx",
    "cy",
    "r",
    "rx",
    "ry",
    "d",
    "points",
    "transform",
    "role",
    "aria-label",
    "class",
    "style",
    "offset",
    "refX",
    "refY",
    "markerWidth",
    "markerHeight",
    "markerUnits",
    "orient",
    "gradientUnits",
    "gradientTransform",
    "clipPathUnits",
];

/// Presentation properties, allowed as attributes and inside `style`.
const PROPERTIES: &[&str] = &[
    "fill",
    "fill-opacity",
    "fill-rule",
    "stroke",
    "stroke-width",
    "stroke-opacity",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-dasharray",
    "stroke-dashoffset",
    "opacity",
    "font-size",
    "text-anchor",
    "stop-color",
    "stop-opacity",
    "clip-path",
    "clip-rule",
    "marker-start",
    "marker-mid",
    "marker-end",
];

/// The theme's colour tokens as CSS variables (`ui/src/theme/generate.ts`).
const TOKENS: &[&str] = &[
    "canvas",
    "panel",
    "raised",
    "heading",
    "body",
    "muted",
    "faint",
    "edge",
    "edge-strong",
    "wash",
    "wash-strong",
    "accent",
    "accent-wash",
    "primary-fill",
    "primary-text",
    "focus",
    "green",
    "amber",
    "red",
];

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Element {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<Node>,
}

impl Element {
    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn text(&self) -> String {
        self.children
            .iter()
            .map(|child| match child {
                Node::Text(text) => text.clone(),
                Node::Element(element) => element.text(),
            })
            .collect()
    }

    fn descendants(&self) -> Vec<&Element> {
        let mut found = vec![self];
        for child in &self.children {
            if let Node::Element(element) = child {
                found.extend(element.descendants());
            }
        }
        found
    }
}

fn fixtures(folder: &str) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/svg")
        .join(folder);
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "svg"))
        .collect();
    paths.sort();
    paths
}

fn file_name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Parses a document strictly: one root element, no doctype, declaration,
/// processing instruction, comment, CDATA section or entity other than the
/// predefined ones and character references.
fn parse(svg: &str) -> Result<Element, String> {
    let mut reader = Reader::from_str(svg);
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    loop {
        let event = reader.read_event().map_err(|error| error.to_string())?;
        match event {
            Event::Start(start) | Event::Empty(start) if root.is_some() => {
                return Err(format!(
                    "a second root element {}",
                    start.name().as_ref() as &str
                ));
            }
            Event::Start(start) => {
                stack.push(element(&start)?);
            }
            Event::Empty(start) => {
                let element = element(&start)?;
                close(&mut stack, &mut root, element);
            }
            Event::End(_) => {
                let element = stack.pop().ok_or("an unmatched end tag")?;
                close(&mut stack, &mut root, element);
            }
            Event::Text(text) => push_text(&mut stack, &text.xml10_content())?,
            Event::GeneralRef(reference) => {
                let text = match reference.resolve_char_ref() {
                    Ok(Some(ch)) => ch.to_string(),
                    _ => match &*reference as &str {
                        "lt" => "<".to_owned(),
                        "gt" => ">".to_owned(),
                        "amp" => "&".to_owned(),
                        "quot" => "\"".to_owned(),
                        "apos" => "'".to_owned(),
                        other => return Err(format!("the entity &{other};")),
                    },
                };
                push_text(&mut stack, &text)?;
            }
            Event::CData(_) => return Err("a CDATA section".to_owned()),
            Event::Comment(_) => return Err("a comment".to_owned()),
            Event::Decl(_) => return Err("an XML declaration".to_owned()),
            Event::PI(_) => return Err("a processing instruction".to_owned()),
            Event::DocType(_) => return Err("a doctype".to_owned()),
            Event::Eof => break,
        }
    }
    if !stack.is_empty() {
        return Err("an unclosed element".to_owned());
    }
    root.ok_or_else(|| "no root element".to_owned())
}

fn element(start: &quick_xml::events::BytesStart<'_>) -> Result<Element, String> {
    let name = (start.name().as_ref() as &str).to_owned();
    let mut attributes = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| error.to_string())?;
        let key = (attribute.key.as_ref() as &str).to_owned();
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|error| format!("{key}: {error}"))?
            .into_owned();
        attributes.push((key, value));
    }
    Ok(Element {
        name,
        attributes,
        children: Vec::new(),
    })
}

fn close(stack: &mut [Element], root: &mut Option<Element>, element: Element) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(Node::Element(element)),
        None => *root = Some(element),
    }
}

fn push_text(stack: &mut [Element], text: &str) -> Result<(), String> {
    let Some(parent) = stack.last_mut() else {
        return if text.trim().is_empty() {
            Ok(())
        } else {
            Err("text outside the root".to_owned())
        };
    };
    match parent.children.last_mut() {
        Some(Node::Text(previous)) => previous.push_str(text),
        _ => parent.children.push(Node::Text(text.to_owned())),
    }
    Ok(())
}

/// The tree with attributes in name order and whitespace-only text dropped:
/// what is left once serialisation choices are taken out.
fn canonical(mut element: Element) -> Element {
    element.attributes.sort();
    element.children = element
        .children
        .into_iter()
        .filter_map(|child| match child {
            Node::Text(text) if text.trim().is_empty() => None,
            Node::Text(text) => Some(Node::Text(text)),
            Node::Element(child) => Some(Node::Element(canonical(child))),
        })
        .collect();
    element
}

fn is_token(value: &str) -> bool {
    value
        .strip_prefix("var(--atlas-")
        .and_then(|rest| rest.strip_suffix(')'))
        .is_some_and(|token| TOKENS.contains(&token))
}

/// The id a value references as exactly `url(#id)`.
fn fragment(value: &str) -> Option<&str> {
    value.strip_prefix("url(#")?.strip_suffix(')')
}

/// Checks one presentation value; `ids` maps each defined id to its element.
fn check_property(
    context: &str,
    name: &str,
    value: &str,
    ids: &HashMap<String, String>,
) -> Result<(), String> {
    let compact: String = value
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_lowercase();
    let referenced = |kind: &str| {
        fragment(value)
            .and_then(|id| ids.get(id))
            .is_some_and(|element| element == kind)
    };
    let ok = match name {
        "fill" | "stroke" => {
            value == "none"
                || value == "currentColor"
                || is_token(value)
                || referenced("linearGradient")
        }
        "stop-color" => value == "currentColor" || is_token(value),
        "clip-path" => value == "none" || referenced("clipPath"),
        "marker-start" | "marker-mid" | "marker-end" => value == "none" || referenced("marker"),
        _ => !compact.contains("url(") && !compact.contains("var("),
    };
    if ok {
        Ok(())
    } else {
        Err(format!("{context}: disallowed {name} value {value:?}"))
    }
}

/// Fails unless the output re-parses to nothing but allowed elements,
/// attributes, references and colours.
fn check_inert(svg: &str) -> Result<(), String> {
    let root = parse(svg)?;
    if root.name != "svg" {
        return Err(format!("the root is {}", root.name));
    }
    let all = root.descendants();
    let ids: HashMap<String, String> = all
        .iter()
        .filter_map(|element| {
            let id = element.attribute("id")?;
            ["linearGradient", "clipPath", "marker"]
                .contains(&element.name.as_str())
                .then(|| (id.to_owned(), element.name.clone()))
        })
        .collect();
    for (index, element) in all.iter().enumerate() {
        let context = format!("<{}>", element.name);
        if !ELEMENTS.contains(&element.name.as_str()) {
            return Err(format!("disallowed element {context}"));
        }
        if element.name == "svg" && index != 0 {
            return Err("a nested <svg>".to_owned());
        }
        for (key, value) in &element.attributes {
            if key.to_lowercase().starts_with("on") || key.contains(':') || key.contains("href") {
                return Err(format!("{context}: disallowed attribute {key}"));
            }
            if !ATTRIBUTES.contains(&key.as_str()) && !PROPERTIES.contains(&key.as_str()) {
                return Err(format!("{context}: disallowed attribute {key}"));
            }
            let compact: String = value
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>()
                .to_lowercase();
            if key == "xmlns" {
                if value != SVG_NAMESPACE || index != 0 {
                    return Err(format!("{context}: namespace {value}"));
                }
                continue;
            }
            for needle in [
                "javascript:",
                "data:",
                "http:",
                "https:",
                "file:",
                "//",
                "expression",
                "@import",
                "\\",
                "/*",
                "<",
                ">",
            ] {
                if compact.contains(needle) {
                    return Err(format!("{context}: {key}={value:?} contains {needle}"));
                }
            }
            if key == "class"
                && !value.split_whitespace().all(|class| {
                    class.strip_prefix("face-").is_some_and(|face| {
                        face.len() == 1 && face.chars().all(|c| c.is_ascii_lowercase())
                    })
                })
            {
                return Err(format!("{context}: class {value:?}"));
            }
            if key == "style" {
                for declaration in value.split(';').filter(|d| !d.trim().is_empty()) {
                    let (name, property) = declaration
                        .split_once(':')
                        .ok_or_else(|| format!("{context}: style declaration {declaration:?}"))?;
                    let name = name.trim();
                    if !PROPERTIES.contains(&name) {
                        return Err(format!("{context}: style property {name}"));
                    }
                    check_property(&context, name, property.trim(), &ids)?;
                }
            } else if PROPERTIES.contains(&key.as_str()) {
                check_property(&context, key, value, &ids)?;
            } else if compact.contains("url(") || compact.contains("var(") {
                return Err(format!("{context}: {key}={value:?} references"));
            }
        }
    }
    Ok(())
}

fn has_drawable(element: &Element) -> bool {
    if ["defs", "clipPath", "marker"].contains(&element.name.as_str()) {
        return false;
    }
    if DRAWABLE.contains(&element.name.as_str())
        || (element.name == "text" && !element.text().trim().is_empty())
    {
        return true;
    }
    element.children.iter().any(|child| match child {
        Node::Element(child) => has_drawable(child),
        Node::Text(_) => false,
    })
}

#[test]
fn every_hostile_fixture_has_an_expected_outcome() {
    let mut files: Vec<String> = fixtures("hostile").iter().map(|p| file_name(p)).collect();
    let mut expected: Vec<String> = HOSTILE.iter().map(|(name, _)| (*name).to_owned()).collect();
    files.sort();
    expected.sort();
    assert_eq!(files, expected);
}

#[test]
fn the_hostile_corpus_comes_out_inert() {
    let mut failures = Vec::new();
    for (name, outcome) in HOSTILE {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/svg/hostile")
            .join(name);
        let result = sanitise(&read(&path));
        match (outcome, result) {
            (Outcome::Rejected, Err(_)) => {}
            (Outcome::Rejected, Ok(svg)) => {
                failures.push(format!(
                    "{name}: expected a rejection, got {}",
                    svg.as_str()
                ));
            }
            (Outcome::Kept, Err(rejected)) => {
                failures.push(format!("{name}: expected a figure, got {rejected}"));
            }
            (Outcome::Kept, Ok(svg)) => {
                if let Err(problem) = check_inert(svg.as_str()) {
                    failures.push(format!("{name}: {problem}\n{}", svg.as_str()));
                } else if !has_drawable(&parse(svg.as_str()).unwrap()) {
                    failures.push(format!("{name}: nothing drawable kept\n{}", svg.as_str()));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn allowed_figures_come_out_unchanged_up_to_serialisation() {
    let paths = fixtures("allowed");
    assert!(paths.iter().any(|p| file_name(p) == "cross-section.svg"));
    for path in paths {
        let name = file_name(&path);
        let input = read(&path);
        let output = sanitise(&input).unwrap_or_else(|rejected| panic!("{name}: {rejected}"));
        check_inert(output.as_str())
            .unwrap_or_else(|problem| panic!("{name}: {problem}\n{}", output.as_str()));
        let expected = canonical(parse(&input).unwrap_or_else(|e| panic!("{name}: {e}")));
        let actual = canonical(parse(output.as_str()).unwrap());
        assert_eq!(actual, expected, "{name}:\n{}", output.as_str());
    }
}

#[test]
fn the_inertness_check_refuses_every_raw_hostile_figure() {
    for (name, _) in HOSTILE
        .iter()
        .filter(|(_, outcome)| *outcome == Outcome::Kept)
    {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/svg/hostile")
            .join(name);
        assert!(
            check_inert(&read(&path)).is_err(),
            "{name} passes the check unsanitised"
        );
    }
}

#[test]
fn sanitising_is_idempotent() {
    for path in fixtures("allowed").into_iter().chain(fixtures("hostile")) {
        let name = file_name(&path);
        if let Ok(first) = sanitise(&read(&path)) {
            let second = sanitise(first.as_str())
                .unwrap_or_else(|rejected| panic!("{name}: second pass {rejected}"));
            assert_eq!(second, first, "{name}");
        }
    }
}

#[test]
fn cdata_in_text_is_kept_as_text() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/svg/hostile/cdata-tricks.svg");
    let svg = sanitise(&read(&path)).unwrap();
    let root = parse(svg.as_str()).unwrap();
    let text = root
        .descendants()
        .into_iter()
        .find(|element| element.name == "text")
        .map(Element::text);
    assert_eq!(
        text.as_deref(),
        Some("</text><script>alert(1)</script><text>"),
        "{}",
        svg.as_str()
    );
}

#[test]
fn colours_of_their_own_become_current_color_and_failing_gradients_go() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/svg/hostile/colours.svg");
    let svg = sanitise(&read(&path)).unwrap();
    let root = parse(svg.as_str()).unwrap();
    let all = root.descendants();
    let rects: Vec<&&Element> = all
        .iter()
        .filter(|element| element.name == "rect")
        .collect();
    assert_eq!(rects.len(), 8, "{}", svg.as_str());
    for rect in rects {
        assert_eq!(
            rect.attribute("fill"),
            Some("currentColor"),
            "{}",
            svg.as_str()
        );
        if let Some(stroke) = rect.attribute("stroke") {
            assert_eq!(stroke, "currentColor", "{}", svg.as_str());
        }
        if let Some(style) = rect.attribute("style") {
            for declaration in style.split(';').filter(|d| !d.trim().is_empty()) {
                let (name, value) = declaration.split_once(':').unwrap();
                if ["fill", "stroke"].contains(&name.trim()) {
                    assert_eq!(value.trim(), "currentColor", "{}", svg.as_str());
                }
            }
        }
    }
    assert!(
        all.iter().all(|element| element.name != "linearGradient"),
        "{}",
        svg.as_str()
    );
    let path = all.iter().find(|element| element.name == "path").unwrap();
    assert_eq!(path.attribute("stroke"), Some("currentColor"));
    assert!(root.attribute("color").is_none());
}

#[test]
fn whitespace_character_references_survive_a_second_pass() {
    for input in [
        r#"<svg xmlns="http://www.w3.org/2000/svg" aria-label="a&#10;b"><rect width="1" height="1"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg" aria-label="a&#9;b&#13;c"><rect width="1" height="1"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="1&#10;" height="1"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><polyline points="0,0&#10;1,1&#9;2,2"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text>a&#13;b&#13;&#10;c</text></svg>"#,
    ] {
        let first = sanitise(input).unwrap_or_else(|rejected| panic!("{input}: {rejected}"));
        let second = sanitise(first.as_str())
            .unwrap_or_else(|rejected| panic!("{input}: second pass {rejected}"));
        assert_eq!(second, first, "{input}");
    }
}

#[test]
fn a_sanitised_figure_read_back_from_json_is_sanitised_again() {
    let hostile = r#"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"><script>alert(2)</script><rect width="1" height="1" fill="red"/></svg>"#;
    let json = serde_json::to_string(hostile).unwrap();
    let svg: SanitisedSvg = serde_json::from_str(&json).unwrap();
    check_inert(svg.as_str()).unwrap();
    assert_eq!(svg, sanitise(hostile).unwrap());

    let nothing_drawable = r#"<svg onload="alert(1)"><script>alert(2)</script></svg>"#;
    let json = serde_json::to_string(nothing_drawable).unwrap();
    assert!(serde_json::from_str::<SanitisedSvg>(&json).is_err());
}
