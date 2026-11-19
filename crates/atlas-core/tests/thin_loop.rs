//! The thin loop through the core's request handler: topic → index → section
//! → Next, with a scripted provider and a database in a temp dir.

use std::path::Path;
use std::sync::{Arc, Mutex};

use atlas_core::harness::index::{ChapterDraft, IndexDraft, SectionDraft};
use atlas_core::ports::EventSink;
use atlas_core::protocol::{CoreEvent, CoreRequest, CoreResponse, CourseView, SectionView};
use atlas_core::provider::{FakeProvider, FakeScript, Task};
use atlas_core::{Core, CoreDeps};
use uuid::Uuid;

#[derive(Default)]
struct RecordedEvents(Mutex<Vec<CoreEvent>>);

impl RecordedEvents {
    fn take(&self) -> Vec<CoreEvent> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

impl EventSink for RecordedEvents {
    fn emit(&self, event: CoreEvent) {
        self.0.lock().unwrap().push(event);
    }
}

struct Harness {
    core: Core,
    provider: Arc<FakeProvider>,
    events: Arc<RecordedEvents>,
}

fn script() -> FakeScript {
    let section = |title: &str| SectionDraft {
        title: title.to_owned(),
        summary: format!("What {title} covers."),
    };
    FakeScript {
        index: IndexDraft {
            chapters: vec![
                ChapterDraft {
                    title: "Plates".to_owned(),
                    sections: vec![section("Crust and mantle"), section("Plate boundaries")],
                },
                ChapterDraft {
                    title: "Motion".to_owned(),
                    sections: vec![section("Convection"), section("Subduction")],
                },
            ],
        },
        section_deltas: vec![
            "The crust ".to_owned(),
            "floats on ".to_owned(),
            "the mantle.".to_owned(),
        ],
    }
}

fn start(database: &Path) -> Harness {
    let provider = Arc::new(FakeProvider::new(script()));
    let events = Arc::new(RecordedEvents::default());
    let core = Core::new(CoreDeps {
        database_path: database.to_owned(),
        provider: provider.clone(),
        events: events.clone(),
    })
    .expect("core starts");
    Harness {
        core,
        provider,
        events,
    }
}

impl Harness {
    async fn request(&self, request: CoreRequest) -> CoreResponse {
        self.core.handle(request).await.expect("request succeeds")
    }

    async fn create_course(&self, topic: &str) -> CourseView {
        match self
            .request(CoreRequest::CreateCourse {
                topic: topic.to_owned(),
            })
            .await
        {
            CoreResponse::Course { course } => course,
            other => panic!("expected a course, got {other:?}"),
        }
    }

    async fn open_section(&self, course: &CourseView, section_id: Uuid) -> SectionView {
        match self
            .request(CoreRequest::OpenSection {
                course_id: course.id,
                section_id,
            })
            .await
        {
            CoreResponse::Section { section } => section,
            other => panic!("expected a section, got {other:?}"),
        }
    }
}

fn section_ids(course: &CourseView) -> Vec<Uuid> {
    course
        .chapters
        .iter()
        .flat_map(|chapter| chapter.sections.iter().map(|section| section.id))
        .collect()
}

#[tokio::test]
async fn creating_a_course_stores_an_index_of_at_least_two_chapters() {
    let dir = tempfile::tempdir().unwrap();
    let harness = start(&dir.path().join("atlas.sqlite3"));

    let course = harness.create_course("Plate tectonics").await;

    assert_eq!(course.topic, "Plate tectonics");
    let chapter_titles: Vec<_> = course.chapters.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(chapter_titles, ["Plates", "Motion"]);
    let section_titles: Vec<_> = course.chapters[0]
        .sections
        .iter()
        .map(|s| s.title.as_str())
        .collect();
    assert_eq!(section_titles, ["Crust and mantle", "Plate boundaries"]);
    assert!(
        course
            .chapters
            .iter()
            .flat_map(|c| &c.sections)
            .all(|s| !s.generated)
    );

    let requests = harness.provider.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].task, Task::BuildIndex);
    assert!(requests[0].input.contains("Plate tectonics"));

    let stored = harness
        .request(CoreRequest::OpenCourse {
            course_id: course.id,
        })
        .await;
    assert_eq!(
        stored,
        CoreResponse::Course {
            course: course.clone()
        }
    );

    match harness.request(CoreRequest::ListCourses).await {
        CoreResponse::Courses { courses } => {
            assert_eq!(courses.len(), 1);
            assert_eq!(courses[0].id, course.id);
            assert_eq!(courses[0].topic, "Plate tectonics");
        }
        other => panic!("expected courses, got {other:?}"),
    }
}

#[tokio::test]
async fn opening_a_section_streams_its_deltas_in_order_then_completes() {
    let dir = tempfile::tempdir().unwrap();
    let harness = start(&dir.path().join("atlas.sqlite3"));
    let course = harness.create_course("Plate tectonics").await;
    let first = course.chapters[0].sections[0].id;
    harness.events.take();

    let section = harness.open_section(&course, first).await;

    assert_eq!(section.id, first);
    assert_eq!(section.title, "Crust and mantle");
    assert_eq!(section.content, "The crust floats on the mantle.");
    let delta = |text: &str| CoreEvent::SectionDelta {
        course_id: course.id,
        section_id: first,
        text: text.to_owned(),
    };
    assert_eq!(
        harness.events.take(),
        [
            delta("The crust "),
            delta("floats on "),
            delta("the mantle."),
            CoreEvent::SectionCompleted {
                course_id: course.id,
                section_id: first,
            },
        ]
    );
    let requests = harness.provider.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].task, Task::WriteSection);
    assert!(requests[1].input.contains("Crust and mantle"));
}

#[tokio::test]
async fn opening_a_section_again_answers_from_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let harness = start(&dir.path().join("atlas.sqlite3"));
    let course = harness.create_course("Plate tectonics").await;
    let first = course.chapters[0].sections[0].id;
    let generated = harness.open_section(&course, first).await;
    let requests_before = harness.provider.requests().len();
    harness.events.take();

    let reopened = harness.open_section(&course, first).await;

    assert_eq!(reopened, generated);
    assert_eq!(harness.provider.requests().len(), requests_before);
    assert_eq!(harness.events.take(), []);
    match harness
        .request(CoreRequest::OpenCourse {
            course_id: course.id,
        })
        .await
    {
        CoreResponse::Course { course } => assert!(course.chapters[0].sections[0].generated),
        other => panic!("expected a course, got {other:?}"),
    }
}

#[tokio::test]
async fn next_from_the_first_section_returns_the_second() {
    let dir = tempfile::tempdir().unwrap();
    let harness = start(&dir.path().join("atlas.sqlite3"));
    let course = harness.create_course("Plate tectonics").await;
    let ids = section_ids(&course);
    harness.open_section(&course, ids[0]).await;

    let next = harness
        .request(CoreRequest::Next {
            course_id: course.id,
            section_id: ids[0],
        })
        .await;

    match next {
        CoreResponse::Section { section } => {
            assert_eq!(section.id, ids[1]);
            assert_eq!(section.title, "Plate boundaries");
            assert_eq!(section.content, "The crust floats on the mantle.");
        }
        other => panic!("expected the second section, got {other:?}"),
    }
}

#[tokio::test]
async fn next_crosses_into_the_following_chapter_and_ends_after_the_last_section() {
    let dir = tempfile::tempdir().unwrap();
    let harness = start(&dir.path().join("atlas.sqlite3"));
    let course = harness.create_course("Plate tectonics").await;
    let ids = section_ids(&course);

    match harness
        .request(CoreRequest::Next {
            course_id: course.id,
            section_id: ids[1],
        })
        .await
    {
        CoreResponse::Section { section } => assert_eq!(section.title, "Convection"),
        other => panic!("expected the next chapter's first section, got {other:?}"),
    }
    assert_eq!(
        harness
            .request(CoreRequest::Next {
                course_id: course.id,
                section_id: ids[3],
            })
            .await,
        CoreResponse::EndOfCourse {
            course_id: course.id
        }
    );
}

#[tokio::test]
async fn the_data_survives_reopening_the_database_file() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("atlas.sqlite3");
    let (course, section) = {
        let harness = start(&database);
        let course = harness.create_course("Plate tectonics").await;
        let section = harness
            .open_section(&course, course.chapters[0].sections[0].id)
            .await;
        (course, section)
    };

    let reopened = start(&database);

    match reopened.request(CoreRequest::ListCourses).await {
        CoreResponse::Courses { courses } => {
            let ids: Vec<_> = courses.iter().map(|c| c.id).collect();
            assert_eq!(ids, [course.id]);
        }
        other => panic!("expected courses, got {other:?}"),
    }
    let mut stored = course.clone();
    stored.chapters[0].sections[0].generated = true;
    assert_eq!(
        reopened
            .request(CoreRequest::OpenCourse {
                course_id: course.id
            })
            .await,
        CoreResponse::Course { course: stored }
    );
    assert_eq!(reopened.open_section(&course, section.id).await, section);
    assert!(reopened.provider.requests().is_empty());
    assert_eq!(reopened.events.take(), []);
}
