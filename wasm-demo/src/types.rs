use serde::Serialize;

#[derive(Serialize)]
pub struct BaseRect {
    pub id: u32,
    pub kind: Box<str>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Serialize)]
pub struct PanelRect {
    #[serde(flatten)]
    pub base: BaseRect,
    pub focused: bool,
    pub kind_index: usize,
}
