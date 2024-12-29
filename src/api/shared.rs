use actix_web::Error;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
pub struct APIError {
    pub cause: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ResponseType<T = String> {
    pub data: Option<T>,
    pub error: Option<APIError>,
}

pub fn parse_error(cause: String, message: String) -> Error {
    actix_web::error::ErrorInternalServerError(json!(ResponseType::<String> {
        data: None,
        error: Some(APIError { cause, message })
    }))
}
