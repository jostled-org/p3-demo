# p3-demo

Demo showcase for [panes](https://github.com/jostled-org/panes) and [palette-core](https://github.com/jostled-org/palette-core).

**panes** is a renderer-agnostic layout engine with declarative ergonomics. **palette-core** is a structured color palette library with 30+ built-in themes. Both are available on [crates.io](https://crates.io).

## Terminal demo

The terminal demo uses ratatui to render an interactive layout explorer.

- Browse all 15 built-in layout presets
- Cycle through 30+ color themes
- Add and remove panels at runtime
- Tab through focus with decoration-aware highlighting
- See layout diff stats (added, removed, resized, moved, unchanged)

### Run it

```
cargo run
```

### Controls

| Key | Action |
|-----|--------|
| `←` `→` / `h` `l` | Switch preset |
| `↑` `↓` / `j` `k` | Switch theme |
| `Tab` / `Shift+Tab` | Cycle focus |
| `a` | Add panel |
| `d` | Remove focused panel |
| `q` / `Esc` | Quit |
