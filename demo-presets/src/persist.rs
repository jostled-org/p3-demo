use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::snapshot::DemoSnapshot;
use crate::state::DemoState;

fn snapshot_path() -> Option<&'static Path> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join(".config/p3-demo/layout.json"))
    })
    .as_deref()
}

/// Save layout state to `~/.config/p3-demo/layout.json`.
///
/// Silently does nothing if the snapshot cannot be taken or written.
pub fn save_snapshot(state: &DemoState) {
    let Some(snap) = state.snapshot() else { return };
    let Some(path) = snapshot_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(file) = std::fs::File::create(path) else {
        return;
    };
    let _ = serde_json::to_writer(BufWriter::new(file), &snap);
}

/// Load layout state from `~/.config/p3-demo/layout.json`.
///
/// Silently does nothing if the file doesn't exist or can't be parsed.
pub fn load_snapshot(state: &mut DemoState) {
    let Some(path) = snapshot_path() else { return };
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    let Ok(snap) = serde_json::from_reader::<_, DemoSnapshot>(BufReader::new(file)) else {
        return;
    };
    let _ = state.restore(snap);
}
