use demo_presets::DemoState;

#[test]
fn snapshot_roundtrip_preserves_focus() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();

    let frame = state.resolve(100.0, 50.0).unwrap();
    let panels: Vec<_> = frame.layout().panels().collect();
    assert!(panels.len() >= 2, "need at least 2 panels");

    // Focus the second panel
    let center_x = panels[1].rect.x + panels[1].rect.w / 2.0;
    let center_y = panels[1].rect.y + panels[1].rect.h / 2.0;
    state.focus_at(center_x, center_y);
    let focused_before = state.focused_pid();
    assert!(focused_before.is_some(), "should have a focused panel");

    // Take snapshot
    let snap = state.snapshot().expect("snapshot should succeed");

    // Restore into a fresh DemoState
    let mut state2 = DemoState::new(1.0).unwrap();
    state2.restore(snap).unwrap();

    // Resolve to populate runtime state
    state2.resolve(100.0, 50.0).unwrap();

    assert_eq!(
        state2.focused_pid(),
        focused_before,
        "focused panel should survive snapshot roundtrip"
    );
}

#[test]
fn snapshot_roundtrip_preserves_preset() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("dwindle").unwrap();
    let _ = state.resolve(100.0, 50.0).unwrap();

    let snap = state.snapshot().expect("snapshot should succeed");

    let mut state2 = DemoState::new(1.0).unwrap();
    state2.restore(snap).unwrap();

    assert_eq!(
        state2.preset_name(),
        "dwindle",
        "preset name should survive snapshot roundtrip"
    );
}

#[test]
fn snapshot_returns_none_without_runtime() {
    let mut state = DemoState::new(1.0).unwrap();
    // Switch to adaptive but don't resolve — runtime is None (built lazily)
    state.switch_preset("adaptive").unwrap();

    assert!(
        state.snapshot().is_none(),
        "snapshot should return None when no runtime"
    );
}
