# ADR-0001: Tauri with a Rust core and a web interface

- Status: Accepted
- Date: 2026-11-18

## Context

Atlas is a desktop app whose first requirement is generated diagrams, graphics
and schemas inside lesson text. The two candidates were Tauri 2 and
[Rune](https://github.com/anomalyco/rune), an all-Rust GPU interface framework.

Rune offers streaming Markdown with selectable text, native Mermaid diagrams,
static SVG rasterised through resvg, and an app shell with a sidebar and an
inspector. It has no webview, no maths typesetting in its documentation, is
pre-release (a pinned git revision), and its maintainers take no pull requests.

## Decision

Atlas is a Tauri 2 app. The harness, sign-in, storage and every network call
live in the Rust core. The interface is React and TypeScript in the webview.

## Tradeoff

The interface layer is not Rust. In exchange the model's output can be rendered
with everything a browser draws (SVG, Mermaid, KaTeX), existing component
libraries apply, and the same core and interface can later target iPad and
iPhone, which Tauri 2 supports.
