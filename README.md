# ModuleEngine

[English](README.md) | [Русский](README.ru.md) | [Documentation](docs/README.md)

ModuleEngine is a modular Rust engine with a native editor and a standalone Player. Engine defines the available modules and authoring rules. Project sets up a game within those rules. Core owns ECS and the shared contracts. Gameplay packages supply specific rules, and the game renderer uses Vulkan.

The [Cargo build version](Cargo.toml) is `0.4.0`. The owner closed milestone 0.4.2 and made the [R engine rebuild](docs/product-roadmap.md#rebuild) the current priority. R1.8 completed the Bevy ECS storage change and R2.4 established the stock GPU culling path. See [current status](docs/current-status.md) for the implemented scope and verification limits, and the [archive](docs/status-archive.md) for earlier reports.

![Scene, Hierarchy, Inspector, and Assets in the editor](docs/media/editor-scene-assets.png)

<a id="запуск"></a>

## Running

From the source checkout on Windows, use the pinned tools and Cargo to run the Editor. The doctor reports missing host tools. Follow the [toolchain](docs/toolchain.md) guide to restore project-local tools and dependencies. For a Linux host see [native prerequisites and display support](docs/linux-support.md).

~~~powershell
.\scripts\bootstrap.ps1 -Json
cargo run --locked -p module-engine-editor
~~~

For a packaged Windows Editor, run `.\ModuleEngine.exe` from the package directory. Pass a project JSON to open a specific project. The editor can migrate older formats into a separate file with `--migrate-copy <source.json> <new-project.json> <neutral|arpg|puzzle|g1>`. See [Engine profiles](docs/engine-profiles.md).

<a id="engine-и-project"></a>

## Engine and Project

| Distribution | Purpose |
| --- | --- |
| Full | Change Engine architecture and sources with a complete compatible source manifest. Edit Project data. |
| Team | Edit exposed Project settings, scenes, and permitted game Rust within a frozen Engine. For an unfamiliar Team project, compilation waits for [project permission](docs/distribution.md#team-custom-rust-permission). |
| Player | Run the packaged game using its prebuilt logic and shaders. |

<a id="области-редактора"></a>

## Editor areas

Start with **File → New Project → Empty scene** or choose an example. **Scene**, **Hierarchy**, **Inspector** and **Assets** share the authored project and object selection. **Game** runs a separate Play copy. Crate Garden supplies a ten-level puzzle game. Two Arenas demonstrates ARPG gameplay and the Player route. See [editor workflow](docs/editor-workflow.md), [Scene and Assets](docs/scene-assets.md), [Crate Garden](docs/g1-game.md) and [game authoring](docs/editor-game.md).

<a id="cli-и-экспорт"></a>

## CLI and export

~~~powershell
.\ModuleEngine.exe --check project.json
.\ModuleEngine.exe --render project.json scene.png
.\ModuleEngine.exe --export-game project.json player-build
~~~

Run these examples from an Editor package with the project JSON, or replace the paths with the executable and project you use. For Player export, choose a new or empty output directory. The delivered Player opens `player.json` with its prebuilt Rust workers and SPIR-V. Full and Team Editor exports require compatible Full sources. See [distributions](docs/distribution.md) and [Player export](docs/player.md).

<a id="проверка"></a>

## Verification

Start with the [verification profile plan](docs/verification.md#profiles). The default Editor includes the project-facing `--check` and `--render` commands. Build with `--features verification` for Editor-specific checks such as `--self-check`. Use `bench` for [native scene benchmarks](docs/bench.md).

<a id="документация"></a>

## Documentation

- [Architecture and Core contracts](docs/architecture.md) · [ECS](docs/ecs-world.md) · [GameHost](docs/shared-game-host.md)
- [Executable modules](docs/module-execution.md) · [Component schemas](docs/component-schemas.md) · [Custom Rust](docs/custom-rust.md) · [Custom state](docs/custom-state.md)
- [Product roadmap](docs/product-roadmap.md) · [Working on ModuleEngine](AGENTS.md) · [Documentation index](docs/README.md)

<a id="лицензия"></a>

## License

ModuleEngine is licensed under either the [MIT License](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE), at your option. Exports include both licenses and [third-party notices](THIRD-PARTY-LICENSES.txt). See [licensing and attribution](docs/licensing.md) for assets and bundled packages.
