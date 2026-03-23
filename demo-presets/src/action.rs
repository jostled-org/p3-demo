use panes::FocusDirection;

/// Renderer-agnostic input action for the demo state machine.
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
    DragStart(f32, f32),
    DragMove(f32, f32),
    DragEnd,
    ToggleHelp,
}

impl Action {
    /// Whether this action may change the layout geometry.
    pub fn changes_layout(&self) -> bool {
        !matches!(self, Self::NextTheme | Self::PrevTheme)
    }
}
