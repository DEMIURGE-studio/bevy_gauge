# bevy_gauge_macros

Procedural macro crate for [bevy_gauge](https://github.com/DEMIURGE-studio/bevy_gauge) - a flexible stats system for Bevy.

This crate provides the procedural macros used by `bevy_gauge` to generate stat components, modifiers, and other compile-time functionality.

## Usage

This crate is typically used indirectly through `bevy_gauge`. You generally don't need to add this as a direct dependency unless you're extending the macro functionality.

```toml
[dependencies]
bevy_gauge = "0.1"
```

## Version Compatibility

| bevy_gauge_macros | bevy_gauge | bevy   |
|-------------------|------------|--------|
| 0.1.1             | 0.1.1      | 0.16   |

## License
`bevy_gauge` is dual-licensed under either
*   MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
*   Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
at your option.