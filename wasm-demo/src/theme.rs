use palette_core::registry::Registry;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct ThemeDesc {
    id: String,
    name: String,
    style: String,
}

/// Returns JSON: [{id, name, style}]
#[wasm_bindgen(js_name = "themeList")]
pub fn theme_list() -> String {
    let registry = Registry::new();
    let themes: Vec<ThemeDesc> = registry
        .list()
        .map(|t| ThemeDesc {
            id: t.id.to_string(),
            name: t.name.to_string(),
            style: t.style.to_string(),
        })
        .collect();
    serde_json::to_string(&themes).unwrap_or_default()
}

/// Returns a CSS block (`:root { --bg: ...; ... }`) for the given theme ID.
#[wasm_bindgen(js_name = "themeCss")]
pub fn theme_css(id: &str) -> Result<String, JsValue> {
    let registry = Registry::new();
    let palette = registry
        .load(id)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(palette.to_css())
}
