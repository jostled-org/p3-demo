use demo_presets::{Action, DemoState, HELP_BINDINGS_GUI, HELP_BINDINGS_TUI};
use panes::BoundaryAxis;

/// Helper: resolve master-stack at 100x50, return the boundary x position.
fn master_stack_boundary_x(state: &mut DemoState) -> f32 {
    let frame = state.resolve(100.0, 50.0).unwrap();
    let panels: Vec<_> = frame.layout().panels().collect();
    // Master is ~60% of width. Boundary is at master's right edge.
    let master = &panels[0];
    master.rect.x + master.rect.w
}

#[test]
fn boundary_hover_detects_boundary() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();
    state.resolve(100.0, 50.0).unwrap();

    let boundary_x = master_stack_boundary_x(&mut state);
    let result = state.boundary_hover(boundary_x, 25.0);
    assert_eq!(
        result,
        Some(BoundaryAxis::Vertical),
        "should detect vertical boundary at master/stack edge"
    );
}

#[test]
fn boundary_hover_returns_none_in_panel() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();
    state.resolve(100.0, 50.0).unwrap();

    let result = state.boundary_hover(10.0, 25.0);
    assert_eq!(result, None, "should return None in the center of a panel");
}

#[test]
fn drag_start_move_end_resizes_panels() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();

    let frame_before = state.resolve(100.0, 50.0).unwrap();
    let master_w_before = frame_before.layout().panels().next().unwrap().rect.w;

    let boundary_x = master_stack_boundary_x(&mut state);
    assert!(
        state.drag_start(boundary_x, 25.0),
        "drag should start at boundary"
    );
    state.drag_move(boundary_x + 10.0, 25.0);
    state.drag_end();

    let frame_after = state.resolve(100.0, 50.0).unwrap();
    let master_w_after = frame_after.layout().panels().next().unwrap().rect.w;

    assert!(
        master_w_after > master_w_before,
        "master panel should be wider after dragging boundary right: before={master_w_before}, after={master_w_after}"
    );
}

#[test]
fn drag_start_returns_false_away_from_boundary() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();
    state.resolve(100.0, 50.0).unwrap();

    assert!(
        !state.drag_start(10.0, 25.0),
        "drag_start should return false in center of panel"
    );
}

#[test]
fn drag_click_disambiguation() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();

    let frame_before = state.resolve(100.0, 50.0).unwrap();
    let widths_before: Vec<f32> = frame_before.layout().panels().map(|p| p.rect.w).collect();

    let boundary_x = master_stack_boundary_x(&mut state);
    assert!(state.drag_start(boundary_x, 25.0));
    // No drag_move — immediate end (click)
    state.drag_end();

    let frame_after = state.resolve(100.0, 50.0).unwrap();
    let widths_after: Vec<f32> = frame_after.layout().panels().map(|p| p.rect.w).collect();

    assert_eq!(
        widths_before, widths_after,
        "click (drag_start + drag_end without move) should not resize"
    );
}

#[test]
fn action_drag_dispatches() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();
    state.resolve(100.0, 50.0).unwrap();

    let boundary_x = master_stack_boundary_x(&mut state);
    let changed = state.apply(Action::DragStart(boundary_x, 25.0));
    assert!(
        changed,
        "DragStart at boundary should report layout changed"
    );
}

#[test]
fn boundary_hover_works_after_resolve() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();
    state.resolve(100.0, 50.0).unwrap();

    let boundary_x = master_stack_boundary_x(&mut state);
    let result = state.boundary_hover(boundary_x, 25.0);
    assert_eq!(
        result,
        Some(BoundaryAxis::Vertical),
        "boundaries should be collected by default after resolve"
    );
}

#[test]
fn boundary_hover_works_during_drag() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();
    state.resolve(100.0, 50.0).unwrap();

    let boundary_x = master_stack_boundary_x(&mut state);
    assert!(state.drag_start(boundary_x, 25.0));

    // Re-resolve while dragging — boundaries should still be collected
    state.resolve(100.0, 50.0).unwrap();
    let result = state.boundary_hover(boundary_x, 25.0);
    assert_eq!(
        result,
        Some(BoundaryAxis::Vertical),
        "boundaries should be collected during drag"
    );
}

#[test]
fn help_bindings_mention_drag() {
    let has_drag = |bindings: &[demo_presets::HelpBinding]| {
        bindings
            .iter()
            .any(|b| b.action.contains("drag") || b.action.contains("mouse resize"))
    };
    assert!(
        has_drag(HELP_BINDINGS_TUI),
        "TUI help bindings should mention drag/resize"
    );
    assert!(
        has_drag(HELP_BINDINGS_GUI),
        "GUI help bindings should mention drag/resize"
    );
}
