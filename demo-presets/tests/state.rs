use demo_presets::{Action, DemoError, DemoState};

#[test]
fn construction_succeeds() {
    let state = DemoState::new(1.0).unwrap();
    assert!(state.preset_count() > 0);
    assert!(state.theme_count() > 0);
    assert_eq!(state.preset_idx(), 0);
    assert_eq!(state.theme_idx(), 0);
}

#[test]
fn preset_cycling_wraps_forward() {
    let mut state = DemoState::new(1.0).unwrap();
    let count = state.preset_count();
    for _ in 0..count {
        state.next_preset();
    }
    assert_eq!(state.preset_idx(), 0);
}

#[test]
fn preset_cycling_wraps_backward() {
    let mut state = DemoState::new(1.0).unwrap();
    state.prev_preset();
    assert_eq!(state.preset_idx(), state.preset_count() - 1);
}

#[test]
fn theme_cycling_wraps_forward() {
    let mut state = DemoState::new(1.0).unwrap();
    let count = state.theme_count();
    for _ in 0..count {
        state.next_theme();
    }
    assert_eq!(state.theme_idx(), 0);
}

#[test]
fn theme_cycling_wraps_backward() {
    let mut state = DemoState::new(1.0).unwrap();
    state.prev_theme();
    assert_eq!(state.theme_idx(), state.theme_count() - 1);
}

#[test]
fn add_panel_on_dynamic_preset() {
    let mut state = DemoState::new(1.0).unwrap();
    // Find a dynamic preset
    let dynamic_idx = state
        .presets()
        .iter()
        .position(|p| p.input == panes::PanelInputKind::DynamicList)
        .unwrap();
    for _ in 0..dynamic_idx {
        state.next_preset();
    }
    assert!(state.is_dynamic());

    let before = state.panel_count();
    state.add_panel().unwrap();
    assert_eq!(state.panel_count(), before + 1);
}

#[test]
fn add_panel_on_fixed_preset_returns_not_dynamic() {
    let mut state = DemoState::new(1.0).unwrap();
    // Find a fixed preset
    let fixed_idx = state
        .presets()
        .iter()
        .position(|p| p.input == panes::PanelInputKind::FixedSlots)
        .unwrap();
    for _ in 0..fixed_idx {
        state.next_preset();
    }
    assert!(!state.is_dynamic());

    let result = state.add_panel();
    assert!(matches!(result, Err(DemoError::NotDynamic)));
}

#[test]
fn resolve_produces_frame() {
    let mut state = DemoState::new(1.0).unwrap();
    let frame = state.resolve(100.0, 50.0).unwrap();
    let layout = frame.layout();
    // Should have at least one panel
    assert!(layout.panels().count() > 0);
}

#[test]
fn load_current_palette_succeeds() {
    let state = DemoState::new(1.0).unwrap();
    let palette = state.load_current_palette().unwrap();
    // Base background should be populated
    assert!(palette.base.background.is_some());
}

#[test]
fn switch_preset_by_name() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("monocle").unwrap();
    assert_eq!(state.preset_name(), "monocle");
}

#[test]
fn switch_preset_unknown_returns_error() {
    let mut state = DemoState::new(1.0).unwrap();
    let result = state.switch_preset("nonexistent");
    assert!(matches!(result, Err(DemoError::UnknownPreset(_))));
}

/// Helper: extract the effective layout from resolve_lerped result.
fn effective_layout(
    result: &(panes::runtime::Frame, Option<panes::ResolvedLayout>),
) -> &panes::ResolvedLayout {
    match &result.1 {
        Some(lerped) => lerped,
        None => result.0.layout(),
    }
}

#[test]
fn resolve_lerped_at_zero_returns_previous_layout() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();
    let before = state.resolve(100.0, 50.0).unwrap();
    let before_rects: Vec<_> = before.layout().panels().map(|e| *e.rect).collect();

    state.apply(Action::FocusNext);
    let result = state.resolve_lerped(100.0, 50.0, 0.0).unwrap();
    let lerped_rects: Vec<_> = effective_layout(&result)
        .panels()
        .map(|e| *e.rect)
        .collect();

    assert_eq!(before_rects, lerped_rects);
}

#[test]
fn resolve_lerped_at_one_returns_current_layout() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();
    state.resolve(100.0, 50.0).unwrap();

    state.resize_horizontal(0.1);
    let result = state.resolve_lerped(100.0, 50.0, 1.0).unwrap();
    let lerped_rects: Vec<_> = effective_layout(&result)
        .panels()
        .map(|e| *e.rect)
        .collect();

    let current = state.resolve(100.0, 50.0).unwrap();
    let current_rects: Vec<_> = current.layout().panels().map(|e| *e.rect).collect();

    assert_eq!(lerped_rects, current_rects);
}

#[test]
fn resolve_lerped_at_half_interpolates() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();
    let before = state.resolve(100.0, 50.0).unwrap();
    let before_width = before.layout().panels().next().unwrap().rect.w;

    state.resize_horizontal(0.1);
    let result = state.resolve_lerped(100.0, 50.0, 0.5).unwrap();
    let lerped_width = effective_layout(&result).panels().next().unwrap().rect.w;

    let after = state.resolve(100.0, 50.0).unwrap();
    let after_width = after.layout().panels().next().unwrap().rect.w;

    // Lerped width should be strictly between before and after
    let (lo, hi) = match before_width < after_width {
        true => (before_width, after_width),
        false => (after_width, before_width),
    };
    assert!(
        lerped_width > lo && lerped_width < hi,
        "lerped {lerped_width} should be between {lo} and {hi}"
    );
}

#[test]
fn resolve_lerped_without_prior_frame_returns_current() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("master-stack").unwrap();
    // No prior resolve — resolve_lerped should fall back to direct resolve
    let result = state.resolve_lerped(100.0, 50.0, 0.5).unwrap();
    assert!(result.1.is_none(), "should fall back to direct resolve");
    assert!(result.0.layout().panels().count() > 0);

    // Should match a direct resolve
    let current = state.resolve(100.0, 50.0).unwrap();
    let lerped_rects: Vec<_> = result.0.layout().panels().map(|e| *e.rect).collect();
    let current_rects: Vec<_> = current.layout().panels().map(|e| *e.rect).collect();
    assert_eq!(lerped_rects, current_rects);
}
