//! The Responses stream: recorded `.sse` fixtures fed to the parser in any
//! chunking, and replayed through the fake provider.

use atlas_core::provider::chatgpt::{ResponseFailure, ResponsesEvent, ResponsesStreamParser};
use atlas_core::provider::{
    FakeProvider, FakeScript, ModelProvider, ProviderCitation, ProviderError, ProviderEvent,
    ProviderRequest, Task, complete_text,
};
use futures::StreamExt;

use ResponsesEvent::{Broken, Completed, Failed, Incomplete, Searching, TextDelta};

const FIXTURES: &[&str] = &[
    "completed.sse",
    "web-search-citations.sse",
    "failed-usage-limit.sse",
    "failed-usage-unavailable.sse",
    "incomplete.sse",
    "broken-mid-event.sse",
    "unknown-events.sse",
];

const REQUEST_ID: &str = "req_5d1e0c7a9b";

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/fixtures/responses/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn delta(text: &str) -> ResponsesEvent {
    TextDelta(text.to_owned())
}

fn citation(url: &str, title: &str, start: u32, end: u32) -> ResponsesEvent {
    ResponsesEvent::Citation(ProviderCitation {
        url: url.to_owned(),
        title: title.to_owned(),
        start,
        end,
    })
}

fn failed(code: &str, message: &str) -> ResponsesEvent {
    Failed(ResponseFailure {
        code: Some(code.to_owned()),
        message: Some(message.to_owned()),
        param: None,
        resets_at: None,
        request_id: Some(REQUEST_ID.to_owned()),
    })
}

/// The events each fixture must yield, in order.
fn expected(name: &str) -> Vec<ResponsesEvent> {
    match name {
        "completed.sse" => vec![delta("Hello"), delta(", world"), delta("!"), Completed],
        "web-search-citations.sse" => vec![
            delta("Let me check "),
            delta("current figures."),
            Searching,
            delta(" Tectonic plates move a few "),
            delta("centimetres a year — about as fast "),
            delta("as fingernails grow (usgs.gov). "),
            delta("Their motion is driven by mantle "),
            delta("convection and slab pull (britannica.com)."),
            citation(
                "https://www.usgs.gov/faqs/how-fast-do-tectonic-plates-move",
                "How fast do tectonic plates move? | U.S. Geological Survey",
                30,
                111,
            ),
            citation(
                "https://www.britannica.com/science/plate-tectonics",
                "Plate tectonics | Britannica",
                124,
                181,
            ),
            Completed,
        ],
        "failed-usage-limit.sse" => vec![
            delta("The crust "),
            delta("floats on "),
            failed(
                "usage_limit_reached",
                "You have hit your ChatGPT usage limit.",
            ),
        ],
        "failed-usage-unavailable.sse" => vec![failed(
            "server_is_overloaded",
            "Our servers are currently overloaded. Please try again later.",
        )],
        "incomplete.sse" => vec![
            delta("Convection carries "),
            delta("heat from the core"),
            Incomplete {
                reason: Some("max_output_tokens".to_owned()),
            },
        ],
        "broken-mid-event.sse" => vec![delta("Subduction "), delta("recycles "), Broken],
        "unknown-events.sse" => vec![
            delta("Mountains rise "),
            delta("where plates meet."),
            Completed,
        ],
        other => panic!("no expectation for {other}"),
    }
}

fn parse_chunks<'a>(chunks: impl IntoIterator<Item = &'a [u8]>) -> Vec<ResponsesEvent> {
    let mut parser = ResponsesStreamParser::new(Some(REQUEST_ID.to_owned()));
    let mut events = Vec::new();
    for chunk in chunks {
        events.extend(parser.feed(chunk));
    }
    events.extend(parser.finish());
    events
}

fn parse(bytes: &[u8]) -> Vec<ResponsesEvent> {
    parse_chunks([bytes])
}

fn is_outcome(event: &ResponsesEvent) -> bool {
    matches!(event, Completed | Failed(_) | Incomplete { .. } | Broken)
}

fn output_text(events: &[ResponsesEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            TextDelta(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn crlf(bytes: &[u8], line_end: &[u8]) -> Vec<u8> {
    bytes.iter().fold(Vec::new(), |mut out, &byte| {
        if byte == b'\n' {
            out.extend_from_slice(line_end);
        } else {
            out.push(byte);
        }
        out
    })
}

#[test]
fn each_fixture_yields_its_events_in_order() {
    for name in FIXTURES {
        assert_eq!(parse(&fixture(name)), expected(name), "{name}");
    }
}

#[test]
fn each_fixture_ends_with_exactly_one_outcome() {
    for name in FIXTURES {
        let events = parse(&fixture(name));
        let outcomes = events.iter().filter(|event| is_outcome(event)).count();
        assert_eq!(outcomes, 1, "{name}: {events:?}");
        assert!(is_outcome(events.last().unwrap()), "{name}: {events:?}");
    }
}

#[test]
fn splitting_a_fixture_at_any_byte_yields_the_same_events() {
    for name in FIXTURES {
        let bytes = fixture(name);
        let want = expected(name);
        for at in 0..=bytes.len() {
            let (head, tail) = bytes.split_at(at);
            assert_eq!(parse_chunks([head, tail]), want, "{name} split at {at}");
        }
        assert_eq!(
            parse_chunks(bytes.chunks(1)),
            want,
            "{name} fed byte by byte"
        );
    }
}

#[test]
fn crlf_and_cr_line_ends_yield_the_same_events_in_any_chunking() {
    for name in FIXTURES {
        let want = expected(name);
        for line_end in [&b"\r\n"[..], &b"\r"[..]] {
            let bytes = crlf(&fixture(name), line_end);
            for at in 0..=bytes.len() {
                let (head, tail) = bytes.split_at(at);
                assert_eq!(
                    parse_chunks([head, tail]),
                    want,
                    "{name} with {line_end:?} split at {at}"
                );
            }
        }
    }
}

#[test]
fn a_final_event_without_a_blank_line_or_newline_is_delivered() {
    let mut bytes = fixture("completed.sse");
    while bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    assert_eq!(parse(&bytes), expected("completed.sse"));
}

#[test]
fn text_before_a_failure_is_delivered_before_the_failure_arrives() {
    let bytes = fixture("failed-usage-limit.sse");
    let text = String::from_utf8(bytes.clone()).unwrap();
    let failure_at = text.find("event: response.failed").unwrap();

    let mut parser = ResponsesStreamParser::new(Some(REQUEST_ID.to_owned()));
    let before = parser.feed(&bytes[..failure_at]);
    assert_eq!(before, vec![delta("The crust "), delta("floats on ")]);

    let mut after = parser.feed(&bytes[failure_at..]);
    after.extend(parser.finish());
    assert_eq!(
        after,
        vec![failed(
            "usage_limit_reached",
            "You have hit your ChatGPT usage limit.",
        )]
    );
}

#[test]
fn a_stream_cut_anywhere_before_completion_is_broken_never_completed() {
    let bytes = fixture("completed.sse");
    let text = String::from_utf8(bytes.clone()).unwrap();
    let completed_at = text.find("event: response.completed").unwrap();
    let last_brace = text.rfind('}').unwrap();
    for cut in 0..=last_brace {
        let events = parse(&bytes[..cut]);
        assert_eq!(events.last(), Some(&Broken), "cut at {cut}: {events:?}");
        assert!(!events.contains(&Completed), "cut at {cut}: {events:?}");
        if cut >= completed_at {
            assert_eq!(output_text(&events), "Hello, world!", "cut at {cut}");
        }
    }
}

#[test]
fn an_empty_stream_is_broken() {
    assert_eq!(parse(b""), vec![Broken]);
}

#[test]
fn citation_ranges_index_the_concatenated_output_text() {
    let events = parse(&fixture("web-search-citations.sse"));
    let text: Vec<char> = output_text(&events).chars().collect();
    let cited: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            ResponsesEvent::Citation(citation) => Some(
                text[citation.start as usize..citation.end as usize]
                    .iter()
                    .collect(),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(
        cited,
        [
            "Tectonic plates move a few centimetres a year — about as fast as fingernails grow",
            "Their motion is driven by mantle convection and slab pull",
        ]
    );
}

#[test]
fn a_failure_keeps_its_code_param_and_request_id() {
    let bytes = br#"event: response.failed
data: {"type":"response.failed","sequence_number":1,"response":{"id":"resp_1","object":"response","status":"failed","error":{"code":"unsupported_parameter","message":"web_search is not supported.","param":"tools"}}}

"#;
    let mut parser = ResponsesStreamParser::new(Some("req_abc".to_owned()));
    let mut events = parser.feed(bytes);
    events.extend(parser.finish());
    assert_eq!(
        events,
        vec![Failed(ResponseFailure {
            code: Some("unsupported_parameter".to_owned()),
            message: Some("web_search is not supported.".to_owned()),
            param: Some("tools".to_owned()),
            resets_at: None,
            request_id: Some("req_abc".to_owned()),
        })]
    );
}

#[test]
fn a_codex_usage_limit_named_by_its_type_keeps_its_reset_time() {
    let bytes = br#"event: response.failed
data: {"type":"response.failed","sequence_number":1,"response":{"id":"resp_1","object":"response","status":"failed","error":{"type":"usage_limit_reached","message":"You have hit your ChatGPT usage limit.","resets_at":1767229200}}}

"#;
    let mut parser = ResponsesStreamParser::new(None);
    let mut events = parser.feed(bytes);
    events.extend(parser.finish());
    assert_eq!(
        events,
        vec![Failed(ResponseFailure {
            code: Some("usage_limit_reached".to_owned()),
            message: Some("You have hit your ChatGPT usage limit.".to_owned()),
            param: None,
            resets_at: chrono::DateTime::from_timestamp(1_767_229_200, 0),
            request_id: None,
        })]
    );
}

#[test]
fn an_error_event_fails_the_response() {
    let bytes = br#"event: error
data: {"type":"error","sequence_number":3,"code":"server_error","message":"The server had an error.","param":null}

"#;
    let mut parser = ResponsesStreamParser::new(None);
    let mut events = parser.feed(bytes);
    events.extend(parser.finish());
    assert_eq!(
        events,
        vec![Failed(ResponseFailure {
            code: Some("server_error".to_owned()),
            message: Some("The server had an error.".to_owned()),
            param: None,
            resets_at: None,
            request_id: None,
        })]
    );
}

#[test]
fn nothing_after_the_outcome_is_delivered() {
    let bytes = br#"data: {"type":"response.output_text.delta","output_index":0,"content_index":0,"delta":"Done."}

data: {"type":"response.completed","response":{"status":"completed"}}

data: {"type":"response.output_text.delta","output_index":0,"content_index":0,"delta":"More."}

data: {"type":"response.failed","response":{"status":"failed","error":{"code":"server_error","message":"late"}}}

"#;
    assert_eq!(parse(bytes), vec![delta("Done."), Completed]);
}

fn replaying(name: &str) -> FakeProvider {
    FakeProvider::new(FakeScript::sample()).replay(Task::SideChatReply, fixture(name))
}

fn side_chat() -> ProviderRequest {
    ProviderRequest::new(Task::SideChatReply, "Answer the question.", "Why?")
}

async fn replay_events(name: &str) -> Vec<Result<ProviderEvent, ProviderError>> {
    let provider = replaying(name);
    let stream = provider.stream(side_chat()).await.expect("stream opens");
    let events = stream.collect::<Vec<_>>().await;
    assert_eq!(provider.requests(), vec![side_chat()], "{name}");
    events
}

#[tokio::test]
async fn the_fake_provider_replays_a_completed_fixture() {
    assert_eq!(
        replay_events("web-search-citations.sse").await,
        vec![
            Ok(ProviderEvent::TextDelta("Let me check ".to_owned())),
            Ok(ProviderEvent::TextDelta("current figures.".to_owned())),
            Ok(ProviderEvent::Searching),
            Ok(ProviderEvent::TextDelta(
                " Tectonic plates move a few ".to_owned()
            )),
            Ok(ProviderEvent::TextDelta(
                "centimetres a year — about as fast ".to_owned()
            )),
            Ok(ProviderEvent::TextDelta(
                "as fingernails grow (usgs.gov). ".to_owned()
            )),
            Ok(ProviderEvent::TextDelta(
                "Their motion is driven by mantle ".to_owned()
            )),
            Ok(ProviderEvent::TextDelta(
                "convection and slab pull (britannica.com).".to_owned()
            )),
            Ok(ProviderEvent::Citation(ProviderCitation {
                url: "https://www.usgs.gov/faqs/how-fast-do-tectonic-plates-move".to_owned(),
                title: "How fast do tectonic plates move? | U.S. Geological Survey".to_owned(),
                start: 30,
                end: 111,
            })),
            Ok(ProviderEvent::Citation(ProviderCitation {
                url: "https://www.britannica.com/science/plate-tectonics".to_owned(),
                title: "Plate tectonics | Britannica".to_owned(),
                start: 124,
                end: 181,
            })),
            Ok(ProviderEvent::Completed),
        ]
    );

    let provider = replaying("completed.sse");
    let text = complete_text(&provider, side_chat(), |_| {}).await;
    assert_eq!(text, Ok("Hello, world!".to_owned()));
}

#[tokio::test]
async fn the_fake_provider_replays_an_incomplete_fixture() {
    assert_eq!(
        replay_events("incomplete.sse").await,
        vec![
            Ok(ProviderEvent::TextDelta("Convection carries ".to_owned())),
            Ok(ProviderEvent::TextDelta("heat from the core".to_owned())),
            Ok(ProviderEvent::Incomplete {
                reason: Some("max_output_tokens".to_owned()),
            }),
        ]
    );
}

#[tokio::test]
async fn the_fake_provider_delivers_text_then_the_failure() {
    let events = replay_events("failed-usage-limit.sse").await;
    assert_eq!(events.len(), 3, "{events:?}");
    assert_eq!(
        events[..2],
        [
            Ok(ProviderEvent::TextDelta("The crust ".to_owned())),
            Ok(ProviderEvent::TextDelta("floats on ".to_owned())),
        ]
    );
    assert!(events[2].is_err(), "{events:?}");

    let events = replay_events("failed-usage-unavailable.sse").await;
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(events[0].is_err(), "{events:?}");

    let provider = replaying("failed-usage-limit.sse");
    let mut seen = String::new();
    let result = complete_text(&provider, side_chat(), |text| seen.push_str(text)).await;
    assert!(result.is_err(), "{result:?}");
    assert_eq!(seen, "The crust floats on ");
}

#[tokio::test]
async fn the_fake_provider_never_completes_a_broken_fixture() {
    let events = replay_events("broken-mid-event.sse").await;
    assert_eq!(
        events[..2],
        [
            Ok(ProviderEvent::TextDelta("Subduction ".to_owned())),
            Ok(ProviderEvent::TextDelta("recycles ".to_owned())),
        ]
    );
    assert_eq!(events.len(), 3, "{events:?}");
    assert!(events[2].is_err(), "{events:?}");

    let provider = replaying("broken-mid-event.sse");
    let result = complete_text(&provider, side_chat(), |_| {}).await;
    assert!(result.is_err(), "{result:?}");
}
