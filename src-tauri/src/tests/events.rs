use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::Duration;

use atlas_core::ports::EventSink;
use atlas_core::protocol::{CoreEvent, ModelList};

use crate::ports::ForwardedEvents;

/// An event told apart from the others by `label`.
fn event(label: String) -> CoreEvent {
    CoreEvent::ModelsChanged {
        list: ModelList {
            models: Vec::new(),
            current: Some(label),
        },
    }
}

#[test]
fn events_reach_the_interface_in_the_order_the_core_emits_them() {
    let (delivered, received) = channel();
    let sink = ForwardedEvents::start(move |event| delivered.send(event).unwrap());
    let events: Vec<CoreEvent> = (0..500).map(|n| event(n.to_string())).collect();

    for event in &events {
        sink.emit(event.clone());
    }

    let forwarded: Vec<CoreEvent> = events
        .iter()
        .map(|_| {
            received
                .recv_timeout(Duration::from_secs(5))
                .expect("every event is forwarded")
        })
        .collect();
    assert_eq!(forwarded, events);
}

#[test]
fn events_from_several_core_threads_keep_each_thread_s_order() {
    let (delivered, received) = channel();
    let sink = Arc::new(ForwardedEvents::start(move |event| {
        delivered.send(event).unwrap()
    }));
    let threads: Vec<_> = ["a", "b", "c"]
        .into_iter()
        .map(|thread| {
            let sink = sink.clone();
            std::thread::spawn(move || {
                for n in 0..200 {
                    sink.emit(event(format!("{thread}{n}")));
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }

    let mut seen: Vec<String> = Vec::new();
    for _ in 0..600 {
        match received
            .recv_timeout(Duration::from_secs(5))
            .expect("every event is forwarded")
        {
            CoreEvent::ModelsChanged { list } => seen.push(list.current.unwrap()),
            other => panic!("unexpected {other:?}"),
        }
    }
    for thread in ["a", "b", "c"] {
        let order: Vec<&String> = seen
            .iter()
            .filter(|label| label.starts_with(thread))
            .collect();
        let expected: Vec<String> = (0..200).map(|n| format!("{thread}{n}")).collect();
        assert_eq!(
            order,
            expected.iter().collect::<Vec<_>>(),
            "{thread} in order"
        );
    }
}
