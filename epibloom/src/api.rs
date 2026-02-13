pub mod script;
pub mod state;
pub mod trace;

/// Responder that sets the status code and just responds with a string, which
/// should be the error message.
#[derive(Debug, Clone, Responder)]
#[allow(dead_code)]
pub enum ApiErrorResponse {
    #[response(status = 400)]
    BadRequest(String),

    #[response(status = 403)]
    Forbidden(String),

    #[response(status = 404)]
    NotFound(String),

    #[response(status = 409)]
    Conflict(String),

    #[response(status = 500)]
    InternalServerError(String),

    #[response(status = 502)]
    BadGateway(String),
}
