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
