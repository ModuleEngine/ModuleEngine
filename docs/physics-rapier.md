# Rapier physics

[English](physics-rapier.md) | [Русский](ru/physics-rapier.md) | [Documentation](README.md)

R6.1 supplies 3D rigid bodies, colliders, joints, contact events and queries.
In Full, add one **Rapier physics** system (`physics.rapier`) to the authored
execution graph. Bind its **world** and **events** ports to the existing
`GameWorld` and `GameEvents` resources. A node's optional `params.gravity`
is a three-element vector. The default is `[0, -9.81, 0]`. Put systems
that drive body transforms before physics. Put consumers of current-tick
queries and collisions after physics, with an `after` instance dependency
where the graph must enforce that order.

The Player export selects the `rapier-physics` native feature from the active
composition and verifies its module and schema pins. Team uses its frozen
graph. A compiled-in capability alone never advances physics. The minimal
Player does not depend on Rapier. G1 and Two Arenas use their existing
gameplay collision rules. Migration of those rules belongs to R6.2.

## Authoring

Attach the components in Inspector once the system is active. An object can
have one body, one collider and one joint. A body or collider belongs on a
scene root. Visual descendants can follow the root transform. Shape sizes,
collider offsets and joint anchors use world units. Changing a visual
`Transform.scale` does not resize a collision shape. Vector fields accept XYZ
values. `Transform.rotation` uses the same radians and rotation order as
the rest of Scene.

| Component | Fields and behavior |
| --- | --- |
| `physics.rigid_body` | `kind` is `dynamic` (default), `fixed` or position-based `kinematic`. `mass` is additional mass besides the attached collider's density contribution. The default is 1. `linear_velocity` and `angular_velocity` seed a new body, `gravity_scale` defaults to 1, and `ccd` extends the continuous sweep to moving targets. Rapier also sweeps fast ordinary bodies against fixed targets. Systems can move kinematic bodies by writing their ECS transform and teleport dynamic bodies through the same transform. Editing a body specification creates a fresh body with its configured velocities. |
| `physics.collider` | `kind` is `box` (default), `ball` or `capsule_y`. Use `half_extents` for a box, `radius` for a ball and `half_height` plus `radius` for a capsule. Capsule half height excludes its caps. `offset` is local to the object's pose. `sensor` reports intersections without a contact force. `density`, `friction` and `restitution` configure contact and mass properties. A collider on an object without a body is static. |
| `physics.joint` | The object must have a body. `target` names another body by its stable scene-local object ID. Null leaves the joint inactive until a target is selected. `kind` is `fixed` (default), `spherical`, `revolute` or `prismatic`. `anchor` and `target_anchor` are local to the two bodies. Revolute and prismatic joints use the normalized `axis` in each body's local frame. `contacts_enabled` defaults to false for the linked pair. |

Scale and offsets should be set on the shapes the system actually simulates.
Shape dimensions range from 0.001 to 10,000. Invalid components and missing
joint targets reject the candidate through the shared schema contract.
Direct runtime attachments must carry effective values for all schema fields.
Native code can use `ComponentRegistry::resolve_component` to apply defaults
before inserting a payload into the ECS. Inspect or open authored data with the capability inactive to retain its
archived values, including fields unknown to the installed schema.

**Physics** in Scene toggles the authored preview wireframe. In narrow panes
it is in the **Scene** menu. The preview shows root body axes, colliders,
sensors and joint anchors. It follows the accepted author-preview position
and camera while editing, including while Game has a separate Play copy.

## Fixed tick, queries and events

Bevy and the Core journal retain ownership of entity identity, components and
transforms. Each session has its own Rapier solver, stepped once per accepted
60 Hz tick in the authored order. Dynamic poses are written through the ECS
journal. Rapier handles and contact history stay in the solver. Rejection at
any later system, including a commit hook, restores the previous solver,
world and events. A scene transition, Restart or Stop ends the previous
solver's lifetime. Rapier is built with `enhanced-determinism`. Stable
insertion order and equal recorded inputs matter for replay. Cross-platform
agreement of the whole Host requires additional evidence.
[Rapier determinism](https://rapier.rs/docs/user_guides/rust/determinism/)

Native systems use `SystemContext::query_physics`. Custom Rust uses
`ScriptContext::query_physics`. Both accept the Core `PhysicsQuery` enum.
Its `Ray` and `Cast` variants return the first collider as a stable object
ID, the distance in world units and the query origin advanced to that
distance. Cast sweeps a ball, box or Y-up capsule at the specified orientation.
`Overlap` returns sorted object IDs. A filter may exclude one object and
include sensors. Directions can have any finite nonzero length. An initial
overlap can hit at distance zero. Misses return `Hit(None)` or an empty
overlap. Queries reject invalid input and overlaps beyond 4096 objects.

The available geometry is the completed physics step in the current tick.
ECS operations after that step enter the solver on the next tick. An object
removed or replaced between physics and a consumer cannot be hit as its old
incarnation. Without a preceding physics step, queries return `Unavailable`.
Use `physics_queries_available()` to check. Custom queries go through the
current invocation's worker exchange and share its deadline, up to 64 calls
per update.

The named `GameEvent` values `physics.collision_started` and
`physics.collision_stopped` carry `{"a": <u64>, "b": <u64>, "sensor": <bool>}`.
IDs are ascending. The physics step emits pair edges in ascending pair order
and keeps Rapier order within a pair. They are facts about the simulated
contact and can include a removed collider. The same tick's later systems and
scripts read these events through the existing event stream. A rejected step
publishes no events from the rejected tick. GameHost exposes one-shot events
from the ticks committed in its current step invocation.

<a id="verify"></a>

## Verification

```powershell
cargo test --locked -p module-engine-physics-rapier --features debug-draw
cargo test --locked --release -p module-engine-physics-rapier acceptance::custom_rust_queries_share_the_physics_step_and_rollback -- --exact --ignored --nocapture --test-threads=1
cargo test --locked --release -p module-engine-physics-rapier acceptance::release_s2_with_projectiles_stays_within_one_ms_per_tick -- --exact --ignored --nocapture --test-threads=1
```

Replay feeds identical recorded input to two fresh sessions and compares full
ordered traces of poses, events and query results. The GameHost rollback
check rejects a collision tick at a later commit, compares the accepted state
and retries against a fresh reference. The explicitly run Custom variant
uses two actual workers, one before and one after physics.

The S2 physics measure uses the stock 20-enemy Small monsters encounter and
its compiled controller in a headless GameHost. Only the fixture adds seven
static environment colliders, kinematic actor colliders and 32 active CCD
projectiles, recycling one stable ID per tick. Every sampled tick must
retain Playing, 20 living enemies and the projectile load. Each of three
Release runs emits all 120 scored durations after 15 warmup ticks and checks
the maximum against 1 ms. Timing covers the real physics step, including
checkpointing, ECS synchronization, solver work, event generation and staged
writes. The fixture does not measure native viewport FPS.
