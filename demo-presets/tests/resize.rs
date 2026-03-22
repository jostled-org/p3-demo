use demo_presets::DemoState;

#[test]
fn horizontal_resize_master_stack_master_focused() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();

    let frame_before = state.resolve(100.0, 50.0).unwrap();
    let master_w_before = frame_before.layout().panels().next().unwrap().rect.w;

    state.resize_horizontal(0.1);

    let frame_after = state.resolve(100.0, 50.0).unwrap();
    let master_w_after = frame_after.layout().panels().next().unwrap().rect.w;

    assert!(
        master_w_after > master_w_before,
        "master panel should be wider after resize_horizontal(0.1): before={master_w_before}, after={master_w_after}"
    );
}

#[test]
fn vertical_resize_master_stack_stack_focused() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();

    // Focus a stack panel (move off master)
    state.focus_next();

    let frame_before = state.resolve(100.0, 50.0).unwrap();
    let focused_pid = state.focused_pid().unwrap();
    let h_before = frame_before
        .layout()
        .panels()
        .find(|p| p.id == focused_pid)
        .unwrap()
        .rect
        .h;

    state.resize_vertical(0.1);

    let frame_after = state.resolve(100.0, 50.0).unwrap();
    let h_after = frame_after
        .layout()
        .panels()
        .find(|p| p.id == focused_pid)
        .unwrap()
        .rect
        .h;

    assert!(
        h_after > h_before,
        "focused stack panel should be taller after resize_vertical(0.1): before={h_before}, after={h_after}"
    );
}

#[test]
fn vertical_resize_noop_on_flat_row() {
    let mut state = DemoState::new(1.0).unwrap();
    // Default preset (index 0) is a flat row

    let frame_before = state.resolve(100.0, 50.0).unwrap();
    let widths_before: Vec<f32> = frame_before.layout().panels().map(|p| p.rect.w).collect();

    state.resize_vertical(0.1);

    let frame_after = state.resolve(100.0, 50.0).unwrap();
    let widths_after: Vec<f32> = frame_after.layout().panels().map(|p| p.rect.w).collect();

    assert_eq!(
        widths_before, widths_after,
        "panel widths should be unchanged after vertical resize on flat row"
    );
}

#[test]
fn help_bindings_include_both_resize_directions() {
    use demo_presets::{HELP_BINDINGS_GUI, HELP_BINDINGS_TUI};

    let tui_actions: Vec<&str> = HELP_BINDINGS_TUI.iter().map(|b| b.action).collect();
    assert!(
        tui_actions.iter().any(|a| a.contains("resize horiz")),
        "TUI bindings missing horizontal resize: {tui_actions:?}"
    );
    assert!(
        tui_actions.iter().any(|a| a.contains("resize vert")),
        "TUI bindings missing vertical resize: {tui_actions:?}"
    );

    let gui_actions: Vec<&str> = HELP_BINDINGS_GUI.iter().map(|b| b.action).collect();
    assert!(
        gui_actions.iter().any(|a| a.contains("resize horiz")),
        "GUI bindings missing horizontal resize: {gui_actions:?}"
    );
    assert!(
        gui_actions.iter().any(|a| a.contains("resize vert")),
        "GUI bindings missing vertical resize: {gui_actions:?}"
    );
}
