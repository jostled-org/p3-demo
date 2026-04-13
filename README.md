# p3-demo

Demo showcase for [panes](https://github.com/jostled-org/panes) and [palette-core](https://github.com/jostled-org/palette-core).

**panes** is a renderer-agnostic layout engine with declarative ergonomics. **palette-core** is a structured color palette library with 30+ built-in themes. Both are available on [crates.io](https://crates.io).

## Demos

| Demo | Description | Link |
|------|-------------|------|
| Terminal | Interactive ratatui layout explorer | `cargo run --bin panes-demo` |
| CSS Showcase | All palette-core themes as CSS custom properties and gradients | [Live](https://jostled-org.github.io/p3-demo/) |
| WASM | Interactive panes layout engine in the browser | [Live](https://jostled-org.github.io/p3-demo/wasm/) |
| egui | Native egui layout explorer | `cargo run -p panes-egui-demo` |

## Terminal demo

The terminal demo uses ratatui to render an interactive layout explorer.

- Browse built-in layout presets plus a custom grid showcase
- Cycle through 30+ color themes
- Add and remove panels at runtime
- Tab through focus with decoration-aware highlighting
- Spatial focus navigation (HJKL)
- Swap and resize panels live
- Smooth animated transitions between layouts
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
| `H` `J` `K` `L` | Spatial focus (left/down/up/right) |
| `a` | Add panel |
| `d` | Remove focused panel |
| `[` `]` | Swap focused panel backward/forward |
| `+` `-` | Resize focused panel |
| `scroll` | Scroll layout |
| `?` | Toggle help |
| `q` / `Esc` | Quit |
