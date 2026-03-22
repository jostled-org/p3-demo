use panes::FocusDirection;

/// Renderer-agnostic input action for the demo state machine.
///
/// Each renderer maps its own key events to `Action` values, then calls
/// `DemoState::apply`. This eliminates duplicated dispatch logic.
pub enum Action {
    NextPreset,
    PrevPreset,
    NextTheme,
    PrevTheme,
    FocusNext,
    FocusPrev,
    FocusDirection(FocusDirection),
    AddPanel,
    RemovePanel,
    ToggleCollapsed,
    SwapNext,
    SwapPrev,
    ResizeHorizontal(f32),
    ResizeVertical(f32),
    ScrollBy(f32),
    FocusAt(f32, f32),
    ToggleHelp,
}

impl Action {
    /// Whether this action may change the layout geometry.
    ///
    /// Renderers that animate layout transitions use this to decide whether
    /// to snapshot before applying the action. Theme cycling is visual-only.
    pub fn changes_layout(&self) -> bool {
        !matches!(self, Self::NextTheme | Self::PrevTheme)
    }
}
