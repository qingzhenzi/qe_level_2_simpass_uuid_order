use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

impl<T: Serialize> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, total: i64, page: i64, page_size: i64) -> Self {
        Self { data, total, page, page_size }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: String,
    pub message: String,
    pub data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self { code: "SUCCESS".into(), message: "ok".into(), data: Some(data) }
    }

    pub fn success_msg(message: &str) -> Self {
        Self { code: "SUCCESS".into(), message: message.into(), data: None }
    }
}
