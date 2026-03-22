use demo_presets::DemoState;

#[test]
fn adaptive_preset_narrow_is_monocle() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("adaptive").unwrap();

    // Narrow viewport — monocle: only the focused panel has non-zero area
    let frame = state.resolve(60.0, 50.0).unwrap();
    let panels: Vec<_> = frame.layout().panels().collect();
    assert!(
        panels.len() >= 2,
        "need multiple panels to verify monocle behavior"
    );

    let visible_count = panels
        .iter()
        .filter(|p| p.rect.w > 0.0 && p.rect.h > 0.0)
        .count();
    assert_eq!(
        visible_count, 1,
        "monocle should show exactly one panel with non-zero area, got {visible_count}"
    );
}

#[test]
fn adaptive_preset_medium_is_master_stack() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("adaptive").unwrap();

    // Medium viewport — master-stack: master panel wider than stack panels
    let frame = state.resolve(120.0, 50.0).unwrap();
    let panels: Vec<_> = frame.layout().panels().collect();
    assert!(
        panels.len() >= 2,
        "master-stack needs at least 2 panels, got {}",
        panels.len()
    );

    let master_w = panels[0].rect.w;
    let stack_w = panels[1].rect.w;
    assert!(
        master_w > stack_w,
        "master panel ({master_w}) should be wider than stack panel ({stack_w})"
    );
}

#[test]
fn adaptive_preset_wide_is_dwindle() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("adaptive").unwrap();

    // Wide viewport — dwindle: each successive panel smaller
    let frame = state.resolve(200.0, 50.0).unwrap();
    let panels: Vec<_> = frame.layout().panels().collect();
    assert!(
        panels.len() >= 3,
        "dwindle needs at least 3 panels, got {}",
        panels.len()
    );

    // In dwindle, panel areas decrease (each split halves the remaining space)
    let area = |i: usize| panels[i].rect.w * panels[i].rect.h;
    assert!(
        area(0) > area(1),
        "first panel area ({}) should exceed second ({})",
        area(0),
        area(1)
    );
    assert!(
        area(1) > area(2),
        "second panel area ({}) should exceed third ({})",
        area(1),
        area(2)
    );
}

#[test]
fn adaptive_switches_on_resize() {
    let mut state = DemoState::new(1.0).unwrap();
    state.switch_preset("adaptive").unwrap();

    // Start wide — dwindle: panels have decreasing areas
    let frame_wide = state.resolve(200.0, 50.0).unwrap();
    let wide_panels: Vec<_> = frame_wide.layout().panels().collect();
    assert!(
        wide_panels.len() >= 3,
        "wide should show multiple panels (dwindle)"
    );
    let area0 = wide_panels[0].rect.w * wide_panels[0].rect.h;
    let area1 = wide_panels[1].rect.w * wide_panels[1].rect.h;
    assert!(
        area0 > area1,
        "dwindle: first panel should be larger than second"
    );

    // Shrink to narrow — monocle: only focused panel visible
    let frame_narrow = state.resolve(60.0, 50.0).unwrap();
    let narrow_panels: Vec<_> = frame_narrow.layout().panels().collect();
    let visible_count = narrow_panels
        .iter()
        .filter(|p| p.rect.w > 0.0 && p.rect.h > 0.0)
        .count();
    assert_eq!(
        visible_count, 1,
        "after resize to narrow, only one panel should be visible (monocle)"
    );
}
