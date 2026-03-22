pub struct HelpBinding {
    pub key: &'static str,
    pub action: &'static str,
}

/// Build a `&'static [HelpBinding]` from shared head entries,
/// renderer-specific entries, shared tail entries, and optional extras.
macro_rules! help_bindings {
    (specific: [$($spec:expr),* $(,)?] $(, extra: [$($ext:expr),* $(,)?])?) => {
        &[
            // --- shared head ---
            HelpBinding { key: "←/→",      action: "cycle preset" },
            HelpBinding { key: "↑/↓",      action: "cycle theme" },
            HelpBinding { key: "Tab/S-Tab", action: "focus next/prev" },
            // --- renderer-specific ---
            $($spec,)*
            // --- shared tail ---
            HelpBinding { key: "[/]", action: "swap panel" },
            HelpBinding { key: "+/-", action: "resize horiz" },
            HelpBinding { key: "S+/S-", action: "resize vert" },
            HelpBinding { key: "scroll", action: "scroll layout" },
            HelpBinding { key: "?", action: "toggle help" },
            // --- optional extras ---
            $($($ext,)*)?
        ]
    };
}

pub const HELP_BINDINGS_TUI: &[HelpBinding] = help_bindings!(
    specific: [
        HelpBinding { key: "HJKL", action: "spatial focus" },
        HelpBinding { key: "a/d",  action: "add/remove panel" },
        HelpBinding { key: "c",    action: "collapse panel" },
    ],
    extra: [
        HelpBinding { key: "q/Esc", action: "quit" },
    ]
);

pub const HELP_BINDINGS_GUI: &[HelpBinding] = help_bindings!(
    specific: [
        HelpBinding { key: "Shift+HJKL", action: "spatial focus" },
        HelpBinding { key: "A/D",        action: "add/remove panel" },
        HelpBinding { key: "C",          action: "collapse panel" },
    ]
);
