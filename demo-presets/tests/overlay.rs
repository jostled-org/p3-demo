use demo_presets::{DemoState, HELP_BINDINGS_TUI, HELP_OVERLAY_KIND};

#[test]
fn help_overlay_height_matches_binding_count() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();
    state.toggle_help();
    let frame = state.resolve(100.0, 50.0).unwrap();

    let overlay = frame
        .layout()
        .overlays()
        .find(|o| o.kind == HELP_OVERLAY_KIND)
        .expect("help overlay should be visible");

    let binding_count = HELP_BINDINGS_TUI.len() as f32;
    // Each binding takes one line (1 cell height), plus 2 lines padding (border)
    let expected_height = binding_count + 2.0;
    let actual_height = overlay.rect.h;

    assert!(
        (actual_height - expected_height).abs() < 0.5,
        "overlay height {actual_height} should be ~{expected_height} (bindings={binding_count} + 2 padding)"
    );
}

#[test]
fn overlay_diff_detects_help_toggle() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();

    // Initial resolve — help hidden
    state.resolve(100.0, 50.0).unwrap();

    // Toggle help on
    state.toggle_help();

    // Resolve again — overlay should appear in diff
    state.resolve(100.0, 50.0).unwrap();

    let diff = state.last_overlay_diff();
    assert!(
        !diff.added.is_empty(),
        "overlay diff should show help overlay as added after toggle"
    );
}
