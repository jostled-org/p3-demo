use wasm_bindgen::prelude::*;

#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    #[error("unknown preset: {0}")]
    UnknownPreset(String),
    #[error("unknown direction: {0} (use left/right/up/down)")]
    UnknownDirection(String),
    #[error("preset is not dynamic")]
    NotDynamic,
    #[error("layout error: {0}")]
    Panes(#[from] panes::PaneError),
}

impl From<EngineError> for JsValue {
    fn from(e: EngineError) -> Self {
        JsValue::from_str(&e.to_string())
    }
}
