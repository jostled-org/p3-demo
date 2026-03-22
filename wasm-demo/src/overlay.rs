use serde::Serialize;

use crate::types::BaseRect;

#[derive(Serialize)]
pub struct OverlayRect {
    #[serde(flatten)]
    pub base: BaseRect,
}
