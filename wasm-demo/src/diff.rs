use serde::Serialize;

#[derive(Serialize)]
pub struct DiffCounts {
    pub added: usize,
    pub removed: usize,
    pub resized: usize,
    pub moved: usize,
    pub unchanged: usize,
}
