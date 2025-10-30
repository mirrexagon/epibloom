pub mod backend_proxy;
pub mod llm_session;

/// Responder that sets the status code and just responds with a string, which
/// should be the error message.
#[derive(Debug, Clone, Responder)]
#[allow(dead_code)]
pub enum ApiErrorResponse {
    #[response(status = 404)]
    NotFound(String),

    #[response(status = 500)]
    InternalServerError(String),

    #[response(status = 502)]
    BadGateway(String),
}
