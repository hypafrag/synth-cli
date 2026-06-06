# synth-cli

Terminal front-end for the **synth** modular synthesizer platform. Exposes `synth-core`
functionality from the command line: list modules, build/load/save/run pipelines, discover
LAN instances, push pipelines to a remote headless instance.

Part of a multi-repo project:

- **synth** — dev repo: docs, architecture, and the development workspace (submodules)
- **synth-core** — core library (this depends on it)
- **synth-cli** — this terminal front-end
- **synth-ui** — visual authoring tool

> Architecture and design decisions live in the `synth` dev repo.

Status: architecture phase — no implementation yet.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed
as above, without any additional terms or conditions.
