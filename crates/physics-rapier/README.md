# module-engine-physics-rapier

[English](README.md) | [Русский](README.ru.md) | [Documentation](../../docs/README.md)

An optional source-installed 60 Hz physics step. It registers the
`physics.rapier` system and its body, collider and joint schemas. Core
owns the ECS and portable query API. Rapier supplies the scene-local
solver and debug geometry for the Editor's `debug-draw` feature.
See the [physics contract and verification commands](../../docs/physics-rapier.md).
