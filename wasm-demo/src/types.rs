use serde::Serialize;

#[derive(Serialize)]
pub struct PanelRect {
    pub id: u32,
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub focused: bool,
    pub kind_index: usize,
}
