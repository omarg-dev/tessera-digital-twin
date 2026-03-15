# Visualizer File-by-File Compliance Checklist

Date: 2026-03-15
Scope: crates/visualizer/src/** and protocol touchpoints used by visualizer.
Legend: PASS = compliant for current scope, FOLLOW-UP = valid but has deferred/non-blocking work, TEST-ONLY = non-runtime risk.

## Runtime Policy Gates

- Runtime unwrap/expect in visualizer: PASS (startup/test-only occurrences remain in resources/models test paths)
- Silent command loss in publish/send paths: PASS (publisher + receiver send paths now explicit)
- Unsafe float-to-index casts in visualizer: PASS (migrated to protocol::world_to_grid in touched UI/system paths; no direct round-as-usize matches found)
- Protocol/topics boundary discipline: PASS (topic constants and protocol types only)

## File Checklist

- PASS: crates/visualizer/src/main.rs
- PASS: crates/visualizer/src/components.rs
- PASS: crates/visualizer/src/resources.rs
- PASS: crates/visualizer/src/tests.rs

- PASS: crates/visualizer/src/systems/mod.rs
- PASS: crates/visualizer/src/systems/interpolate_robots.rs
- PASS: crates/visualizer/src/systems/draw_paths.rs
- PASS: crates/visualizer/src/systems/command_bridge.rs
- PASS: crates/visualizer/src/systems/commands.rs
- PASS: crates/visualizer/src/systems/camera.rs
- PASS: crates/visualizer/src/systems/populate_scene.rs
- PASS: crates/visualizer/src/systems/outline.rs
- FOLLOW-UP: crates/visualizer/src/systems/models.rs (test-only unwrap/panic patterns retained; runtime path acceptable)
- PASS: crates/visualizer/src/systems/robot_labels.rs
- PASS: crates/visualizer/src/systems/sync_robots.rs

- PASS: crates/visualizer/src/systems/receivers/mod.rs
- PASS: crates/visualizer/src/systems/receivers/queue_state.rs
- PASS: crates/visualizer/src/systems/receivers/path_telemetry.rs
- PASS: crates/visualizer/src/systems/receivers/task_list.rs
- PASS: crates/visualizer/src/systems/receivers/robot_updates.rs
- PASS: crates/visualizer/src/systems/receivers/whca_metrics.rs

- PASS: crates/visualizer/src/ui/mod.rs
- PASS: crates/visualizer/src/ui/gui.rs

- PASS: crates/visualizer/src/ui/tabs/mod.rs
- PASS: crates/visualizer/src/ui/tabs/control_bar.rs
- PASS: crates/visualizer/src/ui/tabs/details.rs
- PASS: crates/visualizer/src/ui/tabs/logs.rs
- PASS: crates/visualizer/src/ui/tabs/network.rs
- FOLLOW-UP: crates/visualizer/src/ui/tabs/analytics.rs (placeholder scope remains)
- PASS: crates/visualizer/src/ui/tabs/objects.rs
- PASS: crates/visualizer/src/ui/tabs/robot_inspector.rs
- PASS: crates/visualizer/src/ui/tabs/shelf_inspector.rs
- PASS: crates/visualizer/src/ui/tabs/tasks.rs
- PASS: crates/visualizer/src/ui/tabs/task_inspector.rs

- PASS: crates/visualizer/src/ui/widgets/mod.rs
- PASS: crates/visualizer/src/ui/widgets/common.rs
- PASS: crates/visualizer/src/ui/widgets/minimap.rs

## Protocol Touchpoints

- PASS: crates/protocol/src/config.rs (shared visual constants including path Y offset)
- PASS: crates/protocol/src/tasks.rs (shared task status semantic label helper)
- PASS: crates/protocol/src/lib.rs (helper re-exports)

## Residual Risks / Follow-up Backlog

- Consider replacing test-only unwrap/panic idioms in models diagnostics with Result-returning helpers for style consistency.
- Consider adding UI-log sink integration for background listener errors (currently eprintln fallback remains for async task context).
- Placeholder tabs (analytics/network) remain intentionally minimal outside current hardening scope.

## Phase 7 Validation Evidence (2026-03-15)

- Deterministic gate: PASS
	- `cargo check --workspace` succeeded.
	- `cargo test --workspace` succeeded.
	- Visualizer-focused build task (`build visualizer`) succeeded.
- Policy gate: PASS
	- unwrap/expect scan confirms remaining occurrences are startup/test-only (`resources.rs`, `models.rs`).
	- `.ok();` scan found no silent loss patterns in active send/publish paths.
	- float-to-index cast scan found no `round() as usize` matches in `crates/visualizer/src/**`.
- Runtime smoke (non-interactive CLI evidence): PASS (partial)
	- Orchestrator startup succeeded and reported Zenoh session establishment.
	- Scripted `run -> status -> quit` flow started all managed crates (`coordinator`, `mock_firmware`, `scheduler`, `visualizer`) and shut them down cleanly.
	- Log merge completed on orchestrator shutdown.
- Runtime smoke (interactive GUI behavior): FOLLOW-UP
	- UI click/gesture-driven checks (button click path, wizard interactions, pointer outlines) require a human-in-the-loop visualizer session and are not fully assertable via terminal-only automation.
