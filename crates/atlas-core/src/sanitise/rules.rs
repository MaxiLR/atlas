//! The allow-lists: which elements survive, which attributes each may carry,
//! and what a kept value may look like.

use std::collections::HashMap;

/// The colour tokens of the theme generator (`ui/src/theme/generate.ts`),
/// as the CSS names its stylesheet gives them after `--atlas-`.
pub(super) const TOKENS: &[&str] = &[
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

/// The classes a figure may use for its faces; the interface styles them.
pub(super) const FACES: &[&str] = &["face-a", "face-b", "face-c"];

pub(super) const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

/// Elements whose text content is kept.
pub(super) const TEXT_ELEMENTS: &[&str] = &["text", "tspan", "title", "desc"];

/// Elements that draw something on their own.
pub(super) const SHAPES: &[&str] = &[
    "path", "rect", "circle", "ellipse", "line", "polyline", "polygon",
];

/// Elements that are only drawn when referenced.
pub(super) const DEFINITIONS: &[&str] = &["linearGradient", "clipPath", "marker"];

/// Every element that may be kept, matched exactly: no other spelling, no
/// namespace prefix.
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

/// The allow-listed name `raw` spells, if any.
pub(super) fn element_name(raw: &str) -> Option<&'static str> {
    ELEMENTS.iter().copied().find(|name| *name == raw)
}

/// Whether an element named by [`element_name`] may appear under `parent`;
/// `None` is the document root.
pub(super) fn element_allowed(name: &str, parent: Option<&str>) -> bool {
    match (name, parent) {
        ("svg", None) => true,
        (_, None) | ("svg", Some(_)) => false,
        ("stop", Some(parent)) => parent == "linearGradient",
        (_, Some("linearGradient")) => false,
        ("tspan", Some(parent)) => parent == "text" || parent == "tspan",
        (_, Some(parent)) if TEXT_ELEMENTS.contains(&parent) => false,
        _ => true,
    }
}

/// What a kept attribute value must be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// Numbers with an optional unit, one or more.
    Lengths,
    /// Plain numbers, one or more (`viewBox`, `points`).
    Numbers,
    PathData,
    Transform,
    /// `none` or lengths.
    DashArray,
    /// One of a fixed set of keywords.
    Keyword(&'static [&'static str]),
    /// `auto`, `auto-start-reverse` or an angle.
    Orient,
    /// A fill or stroke: anything not allowed becomes `currentColor`.
    Paint,
    /// A gradient stop's colour; anything not allowed fails the gradient.
    StopColor,
    /// `none` or `url(#id)` naming a kept definition of this element.
    Reference(&'static str),
    /// Classes from [`FACES`].
    Class,
    /// Free text, escaped on output.
    Label,
    Id,
    /// Declarations parsed into the presentation allow-list.
    Style,
}

const ROLES: &[&str] = &["img", "presentation", "none", "group"];
const RULES: &[&str] = &["nonzero", "evenodd"];
const UNITS: &[&str] = &["userSpaceOnUse", "objectBoundingBox"];

/// The presentation properties, allowed as attributes and inside `style`.
fn property(name: &str) -> Option<Kind> {
    Some(match name {
        "fill" | "stroke" => Kind::Paint,
        "fill-opacity" | "stroke-opacity" | "opacity" | "stroke-width" | "stroke-miterlimit"
        | "stroke-dashoffset" | "font-size" => Kind::Lengths,
        "fill-rule" | "clip-rule" => Kind::Keyword(RULES),
        "stroke-linecap" => Kind::Keyword(&["butt", "round", "square"]),
        "stroke-linejoin" => Kind::Keyword(&["miter", "round", "bevel", "miter-clip", "arcs"]),
        "stroke-dasharray" => Kind::DashArray,
        "text-anchor" => Kind::Keyword(&["start", "middle", "end"]),
        "clip-path" => Kind::Reference("clipPath"),
        "marker-start" | "marker-mid" | "marker-end" => Kind::Reference("marker"),
        _ => return None,
    })
}

/// The kind of value `attribute` must hold on `element`, or `None` when the
/// attribute is not allowed there.
pub(super) fn attribute(element: &str, attribute: &str) -> Option<Kind> {
    let specific = match (element, attribute) {
        ("svg" | "marker", "viewBox") => Some(Kind::Numbers),
        ("svg", "width" | "height") => Some(Kind::Lengths),
        ("path", "d") => Some(Kind::PathData),
        ("rect", "x" | "y" | "width" | "height" | "rx" | "ry") => Some(Kind::Lengths),
        ("circle", "cx" | "cy" | "r") => Some(Kind::Lengths),
        ("ellipse", "cx" | "cy" | "rx" | "ry") => Some(Kind::Lengths),
        ("line" | "linearGradient", "x1" | "y1" | "x2" | "y2") => Some(Kind::Lengths),
        ("polyline" | "polygon", "points") => Some(Kind::Numbers),
        ("text" | "tspan", "x" | "y" | "dx" | "dy") => Some(Kind::Lengths),
        ("marker", "refX" | "refY" | "markerWidth" | "markerHeight") => Some(Kind::Lengths),
        ("marker", "markerUnits") => Some(Kind::Keyword(&["strokeWidth", "userSpaceOnUse"])),
        ("marker", "orient") => Some(Kind::Orient),
        ("clipPath", "clipPathUnits") | ("linearGradient", "gradientUnits") => {
            Some(Kind::Keyword(UNITS))
        }
        ("linearGradient", "gradientTransform") => Some(Kind::Transform),
        ("linearGradient" | "clipPath" | "marker", "id") => Some(Kind::Id),
        ("stop", "offset") => Some(Kind::Lengths),
        ("stop", "stop-color") => Some(Kind::StopColor),
        ("stop", "stop-opacity") => Some(Kind::Lengths),
        _ => None,
    };
    if specific.is_some() {
        return specific;
    }
    if matches!(element, "title" | "desc" | "stop" | "linearGradient") {
        return None;
    }
    match attribute {
        "transform" if element != "svg" => Some(Kind::Transform),
        "role" => Some(Kind::Keyword(ROLES)),
        "aria-label" => Some(Kind::Label),
        "class" => Some(Kind::Class),
        "style" => Some(Kind::Style),
        _ => property(attribute),
    }
}

/// The ids of the kept definitions, each with its element name.
pub(super) type Definitions = HashMap<String, &'static str>;

/// The value to write for an attribute of `kind`, or `None` to drop it.
pub(super) fn value(kind: Kind, value: &str, definitions: &Definitions) -> Option<String> {
    let trimmed = value.trim();
    let keep = || Some(value.to_owned());
    match kind {
        Kind::Lengths => lengths(trimmed).then(keep)?,
        Kind::Numbers => numbers(trimmed).then(keep)?,
        Kind::PathData => path_data(trimmed).then(keep)?,
        Kind::Transform => transform(trimmed).then(keep)?,
        Kind::DashArray => (trimmed == "none" || lengths(trimmed)).then(keep)?,
        Kind::Keyword(words) => words.contains(&trimmed).then(keep)?,
        Kind::Orient => (trimmed == "auto"
            || trimmed == "auto-start-reverse"
            || number(trimmed.strip_suffix("deg").unwrap_or(trimmed)))
        .then(keep)?,
        Kind::Paint => Some(
            if trimmed == "none"
                || trimmed == "currentColor"
                || is_token(trimmed)
                || references(trimmed, "linearGradient", definitions)
            {
                trimmed.to_owned()
            } else {
                "currentColor".to_owned()
            },
        ),
        Kind::StopColor => {
            (trimmed == "currentColor" || is_token(trimmed)).then(|| trimmed.to_owned())
        }
        Kind::Reference(element) => (trimmed == "none"
            || references(trimmed, element, definitions))
        .then(|| trimmed.to_owned()),
        Kind::Class => {
            let classes: Vec<&str> = trimmed
                .split_ascii_whitespace()
                .filter(|class| FACES.contains(class))
                .collect();
            (!classes.is_empty()).then(|| classes.join(" "))
        }
        Kind::Label => keep(),
        Kind::Id => is_id(trimmed).then(|| trimmed.to_owned()),
        Kind::Style => style(trimmed, definitions),
    }
}

/// Parses a `style` attribute into allowed declarations, written back as
/// `name:value` joined by `;`. CSS beyond plain declarations (escapes,
/// comments, at-rules, blocks, quoted strings) drops the whole attribute.
fn style(css: &str, definitions: &Definitions) -> Option<String> {
    if css.contains(['\\', '@', '{', '}', '<', '>', '"', '\'']) || css.contains("/*") {
        return None;
    }
    let mut declarations = Vec::new();
    for declaration in css.split(';') {
        if declaration.trim().is_empty() {
            continue;
        }
        let (name, raw) = declaration.split_once(':')?;
        let name = name.trim().to_ascii_lowercase();
        let Some(kind) = property(&name) else {
            continue;
        };
        if let Some(kept) = value(kind, raw.trim(), definitions) {
            declarations.push(format!("{name}:{kept}"));
        }
    }
    (!declarations.is_empty()).then(|| declarations.join(";"))
}

fn is_token(value: &str) -> bool {
    value
        .strip_prefix("var(--atlas-")
        .and_then(|rest| rest.strip_suffix(')'))
        .is_some_and(|token| TOKENS.contains(&token))
}

fn references(value: &str, element: &str, definitions: &Definitions) -> bool {
    value
        .strip_prefix("url(#")
        .and_then(|rest| rest.strip_suffix(')'))
        .and_then(|id| definitions.get(id))
        .is_some_and(|kind| *kind == element)
}

pub(super) fn is_id(value: &str) -> bool {
    let mut chars = value.chars();
    value.len() <= 64
        && chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

/// A plain number: `[+-]?(digits[.digits]|.digits)([eE][+-]?digits)?`.
fn number(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut at = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        at += 1;
    }
    let digits = |at: &mut usize| {
        let start = *at;
        while bytes.get(*at).is_some_and(u8::is_ascii_digit) {
            *at += 1;
        }
        *at - start
    };
    let whole = digits(&mut at);
    let mut fraction = 0;
    if bytes.get(at) == Some(&b'.') {
        at += 1;
        fraction = digits(&mut at);
    }
    if whole + fraction == 0 {
        return false;
    }
    if matches!(bytes.get(at), Some(b'e' | b'E')) {
        at += 1;
        if matches!(bytes.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        if digits(&mut at) == 0 {
            return false;
        }
    }
    at == bytes.len()
}

fn length(value: &str) -> bool {
    let unitless = ["px", "em", "%"]
        .iter()
        .find_map(|unit| value.strip_suffix(unit))
        .unwrap_or(value);
    number(unitless)
}

fn list(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(|ch: char| ch.is_ascii_whitespace() || ch == ',')
        .filter(|item| !item.is_empty())
}

fn lengths(value: &str) -> bool {
    !value.is_empty() && list(value).all(length)
}

fn numbers(value: &str) -> bool {
    !value.is_empty() && list(value).all(number)
}

/// Path data is checked by its alphabet: commands, digits and separators.
fn path_data(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| "MmLlHhVvCcSsQqTtAaZz0123456789.,+-eE \t\n\r".contains(ch))
}

/// A list of `name(numbers)` with the six SVG transform functions.
fn transform(value: &str) -> bool {
    let mut rest = value;
    let mut any = false;
    loop {
        rest = rest.trim_start_matches(|ch: char| ch.is_ascii_whitespace() || ch == ',');
        if rest.is_empty() {
            return any;
        }
        let Some(open) = rest.find('(') else {
            return false;
        };
        let name = rest[..open].trim_end();
        if !matches!(
            name,
            "matrix" | "translate" | "scale" | "rotate" | "skewX" | "skewY"
        ) {
            return false;
        }
        let Some(close) = rest[open..].find(')') else {
            return false;
        };
        if !numbers(rest[open + 1..open + close].trim()) {
            return false;
        }
        rest = &rest[open + close + 1..];
        any = true;
    }
}
