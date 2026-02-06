use std::sync::Arc;

use futures::StreamExt;
use rocket::{
    get,
    response::stream::{Event, EventStream},
    State,
};
use utoipa::OpenApi;
use uuid::Uuid;

use crate::trace::{self, TraceEvent, TraceStore, TraceStoreEvent};

use super::ApiErrorResponse;

impl From<trace::Error> for ApiErrorResponse {
    fn from(value: trace::Error) -> Self {
        match value {
            trace::Error::TraceNotFound(_)
            | trace::Error::TraceHasNoMessages(_)
            | trace::Error::MessageNotFound { .. } => ApiErrorResponse::NotFound(value.to_string()),
            trace::Error::TraceAlreadyGenerating => ApiErrorResponse::Conflict(value.to_string()),
            trace::Error::ThinkingSuppliedForNonAssistantMessage => {
                ApiErrorResponse::BadRequest(value.to_string())
            }
            trace::Error::Io(_) | trace::Error::Json(_) => {
                ApiErrorResponse::InternalServerError(value.to_string())
            }
            trace::Error::Backend(err) => ApiErrorResponse::BadGateway(err),
        }
    }
}

#[derive(OpenApi)]
#[openapi(paths(stream_trace_store_events, stream_trace_by_id))]
pub struct TraceApiDoc;

/// Stream all current debug traces as additions, then stream any subsequent
/// traces being added or removed.
///
/// Just includes metadata, no messages.
#[utoipa::path(
    responses(
        (status = 200, description = "An SSE stream of trace additions or removals.", body = TraceStoreEvent),
        (status = 500, description = "An error occurred starting the stream.", body = String)
    )
)]
#[get("/")]
pub async fn stream_trace_store_events(
    store: &State<Arc<TraceStore>>,
) -> Result<EventStream![], ApiErrorResponse> {
    let trace_store_stream = store.get_trace_store_event_stream().await.map_err(|err| {
        ApiErrorResponse::InternalServerError(format!(
            "Could not get trace store event stream: {}",
            err
        ))
    })?;

    Ok(EventStream! {
        futures::pin_mut!(trace_store_stream);

        while let Some(trace_store_event) = trace_store_stream.next().await {
            yield Event::json(&trace_store_event);
        }
    })
}

/// Stream the current state of the trace and any future message deltas.
#[utoipa::path(
    responses(
        (status = 200, description = "An SSE stream of message events.", body = TraceEvent),
        (status = 404, description = "The specified trace does not exist.", body = String),
        (status = 500, description = "An error occurred starting the stream.", body = String)
    )
)]
#[get("/<trace_id>")]
pub async fn stream_trace_by_id(
    trace_id: Uuid,
    store: &State<Arc<TraceStore>>,
) -> Result<EventStream![], ApiErrorResponse> {
    let trace_stream = {
        let session = store
            .get_session(trace_id)
            .await
            .map_err(|err| ApiErrorResponse::NotFound(err.to_string()))?;

        let session = session.read().await;

        session
            .get_trace_stream()
            .await
            .map_err(|err| ApiErrorResponse::InternalServerError(err.to_string()))?
    };

    Ok(EventStream! {
        futures::pin_mut!(trace_stream);

        while let Some(trace_event) = trace_stream.next().await {
            yield Event::json(&trace_event);
        }
    })
}
