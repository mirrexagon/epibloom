use std::sync::Arc;

use futures::StreamExt;
use rocket::{
    get,
    response::stream::{Event, EventStream},
    State,
};
use utoipa::OpenApi;

use crate::state::{StateEvent, StateStore};

use super::ApiErrorResponse;

#[derive(OpenApi)]
#[openapi(paths(stream_state_objects))]
pub struct StateApiDoc;

/// Subscribe to one or more state objects.
/// Returns SSE stream with snapshots of current values, then updates as they change.
#[utoipa::path(
    responses(
        (
            status = 200,
            description = "SSE stream of state initial values, then updates as JSON Patches.",
            content_type = "text/event-stream",
            body = StateEvent,
        ),
    ),
    params(
        (
            "objects" = String,
            Query,
            description = "Comma-separated list of state object names.",
            example = "chat:123:messages,player:456:stats",
        ),
    ),
)]
#[get("/?<objects>")]
pub async fn stream_state_objects(
    objects: String,
    store: &State<Arc<StateStore>>,
) -> Result<EventStream![], ApiErrorResponse> {
    let object_names: Vec<&str> = objects.split(',').collect();

    // Get or create all requested state objects and subscribe.
    let mut streams = Vec::new();
    for name in &object_names {
        let obj = store.get_or_create(name).await;
        let stream = obj.subscribe().await;
        streams.push(Box::pin(stream));
    }

    Ok(EventStream! {
        // Merge all streams and forward updates.
        let mut merged = futures::stream::select_all(streams);

        while let Some(event) = merged.next().await {
            yield Event::json(&event);
        }
    })
}
