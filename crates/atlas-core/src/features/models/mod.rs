//! The model picker: the models the plan lists, the default, and the
//! candidate's pick while it is still listed.
//!
//! The list is read from the provider once per launch, on first need, and
//! again when the session changes. The default is the first model listed. The
//! candidate's pick is stored and holds across launches; a listing that no
//! longer contains it clears it. A failed listing keeps the last known list,
//! and the next need reads again.

use std::sync::Arc;

use tokio::sync::Mutex;

use super::Features;
use crate::CoreDeps;
use crate::ports::EventSink;
use crate::protocol::{CoreError, CoreEvent, Model, ModelList};
use crate::provider::ModelProvider;
use crate::store::Store;

pub(crate) struct Feature {
    store: Arc<Store>,
    provider: Arc<dyn ModelProvider>,
    events: Arc<dyn EventSink>,
    listing: Mutex<Listing>,
}

/// What this launch knows of the plan's list.
#[derive(Default)]
struct Listing {
    /// The last list read in this launch.
    models: Option<Vec<Model>>,
    /// Whether `models` answers for the current session.
    fresh: bool,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            provider: deps.provider.clone(),
            events: deps.events.clone(),
            listing: Mutex::new(Listing::default()),
        })
    }

    /// The models in the order the plan reports them, and the one in use.
    pub(crate) async fn list(&self, _cx: &Features) -> Result<ModelList, CoreError> {
        let mut listing = self.listing.lock().await;
        let models = self.read(&mut listing).await?;
        self.view(models)
    }

    /// Stores the candidate's pick; only a listed model can be picked.
    pub(crate) async fn select(
        &self,
        _cx: &Features,
        slug: String,
    ) -> Result<ModelList, CoreError> {
        let mut listing = self.listing.lock().await;
        let models = self.read(&mut listing).await?;
        if !models.iter().any(|model| model.slug == slug) {
            return Err(CoreError::NotFound {
                what: format!("model {slug}"),
            });
        }
        self.store.set_selected_model(Some(&slug))?;
        let list = self.view(models)?;
        self.events
            .emit(CoreEvent::ModelsChanged { list: list.clone() });
        Ok(list)
    }

    /// The slug every provider request carries: the pick while it is listed,
    /// else the first model listed. When the plan cannot be listed, the last
    /// known list answers, else the stored pick; `None` leaves the choice to
    /// the provider.
    pub(crate) async fn current(&self, _cx: &Features) -> Result<Option<String>, CoreError> {
        let mut listing = self.listing.lock().await;
        let models = match self.read(&mut listing).await {
            Ok(models) => Some(models),
            Err(error @ CoreError::Store { .. }) => return Err(error),
            // The plan could not be listed: the last known list answers.
            Err(_) => listing.models.clone(),
        };
        match models {
            Some(models) => Ok(self.view(models)?.current),
            None => Ok(self.store.selected_model()?),
        }
    }

    /// Reads the list again for a new session and reports it. On failure the
    /// last known list stays in use and the error is returned.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the session feature calls it when the session changes"
        )
    )]
    pub(crate) async fn session_changed(&self, _cx: &Features) -> Result<(), CoreError> {
        let mut listing = self.listing.lock().await;
        listing.fresh = false;
        let models = self.read(&mut listing).await?;
        let list = self.view(models)?;
        self.events.emit(CoreEvent::ModelsChanged { list });
        Ok(())
    }

    /// The list for this session, read from the provider unless already read.
    /// A successful read clears a stored pick the list no longer contains.
    async fn read(&self, listing: &mut Listing) -> Result<Vec<Model>, CoreError> {
        if listing.fresh
            && let Some(models) = &listing.models
        {
            return Ok(models.clone());
        }
        let models = self.provider.list_models().await?;
        if let Some(pick) = self.store.selected_model()?
            && !models.iter().any(|model| model.slug == pick)
        {
            self.store.set_selected_model(None)?;
        }
        listing.models = Some(models.clone());
        listing.fresh = true;
        Ok(models)
    }

    /// The list with the model in use: the stored pick while listed, else
    /// the first listed.
    fn view(&self, models: Vec<Model>) -> Result<ModelList, CoreError> {
        let pick = self.store.selected_model()?;
        let current = pick
            .filter(|pick| models.iter().any(|model| &model.slug == pick))
            .or_else(|| models.first().map(|model| model.slug.clone()));
        Ok(ModelList { models, current })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use async_trait::async_trait;

    use super::*;
    use crate::provider::{ProviderError, ProviderRequest, ProviderStream};
    use crate::testing::{FakePorts, RecordingEvents};

    /// Lists whatever the test last scripted.
    struct Plan(StdMutex<Result<Vec<Model>, ProviderError>>);

    impl Plan {
        fn answer(&self, listing: Result<Vec<Model>, ProviderError>) {
            *self.0.lock().unwrap() = listing;
        }
    }

    #[async_trait]
    impl ModelProvider for Plan {
        async fn stream(&self, _: ProviderRequest) -> Result<ProviderStream, ProviderError> {
            Err(ProviderError::failed("no generation in these tests"))
        }

        async fn list_models(&self) -> Result<Vec<Model>, ProviderError> {
            self.0.lock().unwrap().clone()
        }
    }

    fn model(slug: &str) -> Model {
        Model {
            slug: slug.to_owned(),
            display_name: slug.to_uppercase(),
        }
    }

    struct Setup {
        _dir: tempfile::TempDir,
        plan: Arc<Plan>,
        events: Arc<RecordingEvents>,
        features: Features,
    }

    fn setup(models: Vec<Model>) -> Setup {
        let dir = tempfile::tempdir().unwrap();
        let plan = Arc::new(Plan(StdMutex::new(Ok(models))));
        let events = Arc::new(RecordingEvents::default());
        let deps = FakePorts::default().deps(
            dir.path().join("atlas.sqlite3"),
            plan.clone(),
            events.clone(),
        );
        let store = Arc::new(Store::open(&deps.database_path, deps.clock.clone()).unwrap());
        let features = Features::new(&deps, &store).unwrap();
        Setup {
            _dir: dir,
            plan,
            events,
            features,
        }
    }

    #[tokio::test]
    async fn the_list_is_read_once_per_session_and_again_when_it_changes() {
        let Setup {
            plan,
            events,
            features: cx,
            _dir,
        } = setup(vec![model("a"), model("b")]);
        assert_eq!(
            cx.models.list(&cx).await.unwrap().models,
            [model("a"), model("b")]
        );
        plan.answer(Ok(vec![model("c"), model("a")]));
        assert_eq!(cx.models.current(&cx).await.unwrap().as_deref(), Some("a"));
        events.take();

        cx.models.session_changed(&cx).await.unwrap();

        let list = ModelList {
            models: vec![model("c"), model("a")],
            current: Some("c".to_owned()),
        };
        assert_eq!(
            events.take(),
            [CoreEvent::ModelsChanged { list: list.clone() }]
        );
        assert_eq!(cx.models.list(&cx).await.unwrap(), list);
    }

    #[tokio::test]
    async fn a_failed_listing_keeps_the_last_known_list_and_reports_the_error() {
        let Setup {
            plan,
            events,
            features: cx,
            _dir,
        } = setup(vec![model("a"), model("b")]);
        cx.models.select(&cx, "b".to_owned()).await.unwrap();
        plan.answer(Err(ProviderError::Offline));
        events.take();

        assert_eq!(
            cx.models.session_changed(&cx).await,
            Err(CoreError::Offline)
        );

        assert_eq!(events.take(), []);
        assert_eq!(cx.models.current(&cx).await.unwrap().as_deref(), Some("b"));
        assert_eq!(cx.models.list(&cx).await, Err(CoreError::Offline));
        plan.answer(Ok(vec![model("a"), model("b")]));
        assert_eq!(
            cx.models.list(&cx).await.unwrap(),
            ModelList {
                models: vec![model("a"), model("b")],
                current: Some("b".to_owned()),
            }
        );
    }
}
