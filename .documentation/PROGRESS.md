# Hyper-Twin Development Progress

> **⚠️ MAINTENANCE RULE:** Update this file after every significant change, feature completion, or refactoring session.
> This document serves as the living record of project evolution for portfolio/internship purposes.

---

## Project Summary

**Hyper-Twin** is a high-performance Discrete Event Simulation (DES) and Digital Twin for warehouse logistics.
Demonstrates advanced Rust skills: async programming, ECS architecture, distributed systems, and clean code practices.

**Tech Stack:**

- **Language:** Rust (Edition 2024)
- **Visualization:** Bevy 0.17.3 (ECS)
- **Async Runtime:** Tokio 1.49
- **Messaging:** Zenoh 1.7.2 (pub/sub)
- **Serialization:** serde + serde_json

---

## Phase Overview

| Phase | Name | Status | Description |
| ----- | ---- | ------ | ----------- |
| 1     | Foundation & Scene Setup                        | ✅ Complete   | Bevy app, warehouse layout, camera, environment           |
| 2     | Zenoh Integration & Robot Sync                  | ✅ Complete   | Real-time robot updates, dynamic spawning, HUD            |
| 3     | Distributed Architecture & Pathfinding          | ✅ Complete   | Multi-crate architecture, A* pathfinding, map validation  |
| 4     | Task & Cargo Management                         | ✅ Complete   | Task queue, allocation, execution loop, collision detection  |
| 5     | Polish & Presentation                           | 🔄 In Progress | Digital Twin Command Center UI, performance, demo scenarios |
| 6     | Inbound/Outbound Bridge & Hardware Validation | ⏳ Planned    | External physics integration, real robot support          |

---

## Phase 1: Foundation & Scene Setup ✅

**Goal:** Get a working Bevy app with warehouse visualization.

**Completed Features:**

- [x] Bevy app initialization with DefaultPlugins + EguiPlugin
- [x] Warehouse layout parser (`assets/data/layout.txt`)
- [x] Scene population system (walls, ground, shelves, stations, dropoffs)
- [x] Orbital camera with pan/zoom controls
- [x] Component hierarchy: `Robot`, `Ground`, `Wall`, `Shelf`, `Station`, `Dropoff`
- [x] Basic 3D meshes for all tile types

**Key Files:**

- `crates/visualizer/src/systems/populate_scene.rs`
- `crates/visualizer/src/systems/camera.rs`
- `crates/visualizer/src/components.rs`
- `crates/protocol/src/grid_map.rs`

---

## Phase 2: Zenoh Integration & Robot Sync ✅

**Goal:** Real-time robot position updates from firmware to visualizer.

**Completed Features:**

- [x] mock_firmware publishes `RobotUpdateBatch` at 20 Hz
- [x] visualizer subscribes via mpsc channel (thread-safe)
- [x] Robot index for O(1) entity lookup by ID
- [x] Movement detection (skip duplicate position updates)
- [x] Dynamic robot spawning (new IDs create entities on-the-fly)
- [x] HUD display with real-time robot count and status

**Key Files:**

- `crates/mock_firmware/src/driver.rs`
- `crates/visualizer/src/systems/zenoh_receiver.rs`
- `crates/visualizer/src/systems/sync_robots.rs`
- `crates/visualizer/src/resources.rs`

---

## Phase 3: Distributed Architecture & Pathfinding ✅

**Goal:** Multi-crate architecture with pathfinding and system control.

**Completed Features:**

- [x] 6-crate workspace: protocol, orchestrator, scheduler, coordinator, mock_firmware, visualizer
- [x] A* grid pathfinding with obstacle avoidance
- [x] `goto <robot_id> <x> <y>` command for manual robot control
- [x] SystemCommand broadcast (pause/resume/verbose) via Zenoh
- [x] Map hash validation (prevents "Ghost Wall" bugs)
- [x] Orchestrator process management (start/kill/restart)

**Key Files:**

- `crates/coordinator/src/pathfinding/astar.rs`
- `crates/coordinator/src/server.rs`
- `crates/orchestrator/src/processes.rs`
- `crates/protocol/src/commands.rs`

---

## Phase 4: Task & Cargo Management ✅

**Goal:** Automated task assignment, execution, and tracking.

**Completed Features:**

- [x] Task assignment system (scheduler → coordinator → firmware)
- [x] Task queue with FIFO ordering and priority support
- [x] ClosestIdleAllocator (assigns tasks to nearest idle or charging robot)
- [x] Robot state machine: `Idle → MovingToPickup → Picking → MovingToDropoff → Delivering`
- [x] Automatic task progression (coordinator monitors state, sends next PathCmd)
- [x] TaskStatusUpdate messages (coordinator → scheduler)
- [x] Named location commands (`add S1 D1` instead of coordinates)
- [x] Location listing (`list shelves`, `list dropoffs`, `list stations`)
- [x] ASCII map display (`map` command)
- [x] Global verbose mode (`verbose on/off` from orchestrator)
- [x] Shared `SystemCommand.apply_with_log()` in protocol crate
- [x] Smart return-to-station (idle robots return when no pending tasks)
- [x] Low battery detection with automatic return to charging station
- [x] Battery threshold for task availability (50% minimum)
- [x] Deterministic battery drain model (fixed 0.05 %/sec)
- [x] Chaos engineering infrastructure (packet loss, command rejection, position drift)
- [x] Runtime chaos toggle (`chaos on/off` command)
- [x] Individual robot control (`enable/disable/restart robot <id>`)
- [x] Task timeout system (30s no-progress → task failed, reassigned)
- [x] Charging robots with sufficient battery can accept new tasks
- [x] WHCA* real-time reservations (velocity-based timing, ms windows, ±200ms tolerance)
- [x] Wall collision detection (firmware checks GridMap before movement)
- [x] Blocked state handling (robots stop on wall collision)

**Pending Features:** None - Phase 4 Complete! ✅

**Key Files:**

- `crates/scheduler/src/queue/fifo.rs`
- `crates/scheduler/src/allocator/closest.rs`
- `crates/coordinator/src/state.rs`
- `crates/protocol/src/tasks.rs`

---

## Phase 5: Polish & Presentation 🔄

**Goal:** Production-ready demo for portfolio/internship showcase.

**Completed Features:**

- [x] Digital Twin Command Center UI (4-panel egui layout)
- [x] Top HUD panel: pause/play, speed controls (1x/2x/5x/10x/custom), FPS counter, layer toggles
- [x] Left Object Manager: tabbed robot/shelf browser with search filter
- [x] Right Inspector: context-sensitive entity details (battery bar, state, position, actions)
- [x] Bottom Log Console: scrollable log viewer with auto-scroll and clear
- [x] Camera input guard (egui panels don't interfere with 3D orbit/pan/zoom)
- [x] `UiState` resource for centralized UI state management
- [x] `LogBuffer` resource for ring-buffered log display
- [x] `bevy-inspector-egui` dependency added for dev/debug panels
- [x] **Pause/Resume** buttons publish `SystemCommand` over Zenoh (all crates respond)
- [x] **Kill/Restart/Enable** buttons publish `RobotControl` over Zenoh (firmware responds)
- [x] External commands (from orchestrator) sync `UiState.is_paused` and log to bottom panel
- [x] **Live QueueState** display: subscribes to scheduler topic, shows pending/total/completed/robots
- [x] Top HUD shows live task throughput from scheduler QueueState
- [x] Robot state changes and spawns logged to bottom panel in real-time
- [x] All UI actions logged to bottom panel (`[UI] Kill Robot #2`, `[System] Paused`, etc.)
- [x] Background Zenoh publisher thread (mpsc channel bridge from Bevy to async)
- [x] Visualizer crate review: shared Tokio runtime, GridMap wall truth, O(1) lookups, PlaceholderMeshes, LogBuffer-only logging, sort optimizations
- [x] **Glowing Outline System** (`bevy_mod_outline 0.11` + native `MeshPickingPlugin`): hover (white HDR) and select (cyan HDR) outlines with bloom post-processing
- [x] **Path Visualization** (Bevy gizmos): glowing linestrip paths from robot to destination with bloom, global/per-robot toggle
- [x] **Task Management UI**: per-task list with Active/Failed/Completed sections, Add Task wizard with two-step minimap location picker, priority selector; Details inspector with ETA, priority editor, and cancel action
- [x] **TaskCommand protocol**: `cancel` and `set_priority` commands from UI to scheduler over Zenoh
- [x] **TaskListSnapshot broadcast**: scheduler sends full task list to renderer every 2 seconds on `factory/tasks/list`
- [x] High-speed simulation stability: firmware waypoint handling prevents oscillation at 5x+ and coordinator position validation scales with time multiplier
- [x] Real-time mode integration: toggling Real-time now pauses simulation, and toggling back restores prior pause/running state
- [x] Runtime hardening pass for orchestrator/protocol/mock_firmware (panic-safe publish paths, protocol utility extraction, command dedup)
- [x] Firmware command-path logging cleanup (reduced runtime console noise, file-log-first policy)
- [x] Coordinator and scheduler runtime hardening pass (panic-safe publish paths, malformed payload diagnostics, shared protocol utility dedup)
- [x] Strict WHCA safety pass 1 (no reservation-ignoring fallback, stronger stationary reservations, transactional scheduler assignment publish)
- [x] WHCA pass 2 refinement (shared protocol publish helper centralization, station occupancy guarding, edge-swap regression coverage)
- [x] Orchestrator and firmware publish-path migration to shared protocol JSON helper (removed local serde_json publish duplication)
- [x] WHCA strict-vs-fallback benchmark test for head-on corridor contention (quantified zero-collision trade-off)
- [x] WHCA strict trait-path closure (removed trait-level A* fallback bypass) and robot-aware `goto` path routing
- [x] WHCA reservation hot-path optimization (robot-indexed reservation cleanup and linearized edge-swap conflict checks)
- [x] WHCA runtime instrumentation (search latency/counter snapshots and periodic coordinator metrics logging)
- [x] WHCA scenario benchmark runners (deterministic comparison table) and live analytics-tab telemetry integration
- [x] WHCA analytics tab scrollability and reservation-aware dispatch stabilization (time-aware lookahead blocking + blocked-hold behavior)
- [x] Return-to-station liveness hardening and disabled-robot assignment policy guardrails (auto-unassign + temporary UI mitigation)
- [x] Visualizer runtime hardening pass 1 (panic-safe task wizard submit, listener send-failure handling, bounds-safe grid conversion, shared path gizmo Y-offset)
- [x] Visualizer UI performance pass 1 (single-pass task categorization and cached object-list sorting)
- [x] Visualizer camera/labels performance pass 2 (cached task-follow lookup and selected-label suppression)
- [x] Visualizer/protocol dedup pass 3 (task-status semantic helper + outline/populate hierarchy walk consolidation)

**Pending Features:**

- [ ] 3D gizmos: traffic heatmap overlay, debug grid
- [ ] Robot ID labels rendered in 3D viewport
- [ ] Analytics dashboard (throughput graphs, battery histograms)
- [ ] Cargo/package entity tracking (visual cargo on robots)
- [ ] Keyboard shortcuts: P=pause, R=resume, Space=reset, Esc=quit

## MVP Showcase

A polished demo showcasing the core features of the distributed architecture, pathfinding, and task management.

- [ ] Demo scenarios (scripted warehouse operations)
- [ ] Video recording / GIF generation
- [ ] README with architecture diagrams
- [ ] Documentation cleanup
- [ ] Performance optimization (benchmark 1000+ robots)

**Phase 6: Future Firmware Enhancements:**

These improvements would increase simulation realism but are not needed for an MVP (Phase 5+):

- [ ] **Non-linear Battery Model** (Low priority)
  - Exponential discharge curves (faster drain at low SOC)
  - Temperature-dependent efficiency
  - Load-dependent drain (carrying cargo = higher drain)
  - **Current**: Linear deterministic drain (0.05 %/sec)
  - **Impact**: More realistic battery behavior for logistics planning

- [ ] **WHCA* Pathfinding Optimization** (Medium priority, Phase 5)
  - Use actual robot execution time instead of coordinator ticks
  - Only advance WHCA* time when robots change grid cells
  - Requires robot state notifications to coordinator
  - **Current**: WHCA* uses fixed tick-based timing, causing reservation mismatches
  - **Impact**: More accurate collision avoidance under load

**Rationale:** These enhancements are "nice to have" for maximum realism, but the current firmware implementation is sufficient for demonstrating the distributed architecture, pathfinding, and task management capabilities. Priority should remain on Phase 5 polish (UI, performance, demo scenarios) before diving into firmware micro-optimizations.

---

## Phase 6: ROS2 Bridge & Hardware Validation ⏳

**Goal:** Validate system with external physics simulation and prepare for real robots.

**Planned Crate: `ros2_bridge`**

This crate bridges Zenoh ↔ ROS2 to replace `mock_firmware` when running with:

- **External physics engines** (Gazebo, Isaac Sim, etc.)
- **Real robot hardware** (actual AMRs with ROS2 stack)

**Key Differences from mock_firmware:**

- Does NOT simulate physics (external engine handles that)
- Translates `PathCmd` → ROS2 nav goals
- Translates ROS2 odometry/state → `RobotUpdate`
- Battery, collision, sensor data come from real/simulated hardware

**Planned Features:**

- [ ] ROS2 node with rclrs or ros2-rust bindings
- [ ] Zenoh ↔ ROS2 topic mapping
- [ ] Navigation goal translation (PathCmd → nav2_msgs)
- [ ] Odometry subscription (nav_msgs → RobotUpdate)
- [ ] Battery state subscription
- [ ] Hardware-in-the-loop testing mode
- [ ] Graceful fallback to mock_firmware when ROS2 unavailable
- [ ] Benchamark with Gazebo or Isaac Sim for performance validation
- [ ] Benchmark with Aziz supercomputer for large-scale simulation
- [ ] CLI tab completion for orchestrator commands (`rustyline` — completes command names and crate names on Tab)

**Architecture Impact:**

``` text
                   ┌────────────────┐
                   │   coordinator  │
                   └───────┬────────┘
                           │ PathCmd
            ┌──────────────┼───────────────┐
            │              │               │
            ▼              ▼               ▼
     ┌─────────────┐ ┌────────────┐ ┌─────────────┐
     │mock_firmware│ │ ros2_bridge│ │ Real Robots │
     │ (sim only)  │ │  (bridge)  │ │ (hardware)  │
     └─────────────┘ └─────┬──────┘ └──────┬──────┘
                           │ ROS2          │
                           └───────────────┘
```

---

## Changelog

### 2026-03-14: Protocol status-label extraction and hierarchy-walk consolidation (Phase 5)

- `protocol/src/tasks.rs` + `protocol/src/lib.rs`: added and exported `task_status_label(&TaskStatus) -> &'static str` as a shared semantic status helper, with new unit coverage for label mapping.
- `visualizer/src/ui/tabs/task_inspector.rs`: migrated task status text mapping to `protocol::task_status_label(...)` while keeping renderer-specific failure-reason formatting local.
- `visualizer/src/systems/outline.rs`: added mesh-descendant caching in `ProgrammaticOutlineState` so repeated sidebar selection/hover transitions reuse cached mesh lists instead of re-walking the entity hierarchy each time.
- `visualizer/src/systems/populate_scene.rs`: consolidated repeated tile descendant traversal into a shared helper (`propagate_flags_for_roots`) and reused it for floor/wall shadow optimization tagging.
- Validation: `cargo check --workspace` and `cargo test --workspace` pass.

### 2026-03-14: Visualizer camera and label performance pass 2 (Phase 5)

- `visualizer/src/systems/camera.rs`: added cached task index lookup in `camera_follow_task` keyed by selected task and snapshot update timestamp to avoid repeated per-frame linear searches through the task list.
- `visualizer/src/systems/camera.rs`: migrated task-follow robot entity resolution to `RobotIndex::get_entity(...)` helper for consistency with recent lookup dedup work.
- `visualizer/src/systems/robot_labels.rs`: added selected-entity label suppression (aligning behavior with inspector focus) and reduced per-robot hidden-label set checks when the hidden set is empty.
- Validation: `cargo check --workspace` and `cargo test --workspace` pass.

### 2026-03-14: Visualizer UI performance pass 1 (Phase 5)

- `visualizer/src/ui/tabs/tasks.rs`: replaced repeated task filtering passes with a single categorization pass (`active`, `failed`, `completed`) reused by both header stats and section rendering.
- `visualizer/src/resources.rs`: added object-tab cache fields in `UiState` for sorted robot/shelf entity lists and invalidation counters.
- `visualizer/src/ui/tabs/objects.rs`: removed per-frame `sort_unstable_by_key` work by introducing cache refresh-on-change logic and stale-cache invalidation when entities despawn mid-frame.
- Validation: `cargo check --workspace` and `cargo test --workspace` pass.

### 2026-03-14: Visualizer runtime hardening pass 1 and protocol visual dedup (Phase 5)

- `visualizer/src/ui/tabs/tasks.rs`: removed runtime `unwrap()` from task wizard submission by switching to atomic `Option::take()` pattern matching and added finite/bounds-safe transform-to-grid conversion for empty-shelf filtering.
- `visualizer/src/systems/command_bridge.rs`: hardened outbound publish path by logging serialize/publish failures, replaced silent `try_send(...).ok()` drops with explicit `LogBuffer` diagnostics, and made real-time OFF restoration logic explicit for missing previous-state cases.
- `visualizer/src/systems/commands.rs` + `visualizer/src/systems/receivers/{queue_state,task_list,path_telemetry,whca_metrics}.rs`: removed silent async channel-send drops; listeners now fail fast with clear error context when their Bevy-side receiver is gone.
- `visualizer/src/systems/sync_robots.rs` + `visualizer/src/ui/tabs/shelf_inspector.rs` + `visualizer/src/ui/widgets/minimap.rs`: replaced direct float-to-`usize` coordinate casts with `protocol::world_to_grid(...)`-based conversion to avoid negative/invalid coordinate wrap behavior.
- `protocol/src/config.rs` + `visualizer/src/systems/{draw_paths,receivers/path_telemetry}.rs`: introduced shared `PATH_Y_OFFSET` constant and migrated path rendering/telemetry projection to use it.
- `visualizer/src/resources.rs` + `visualizer/src/systems/{draw_paths,sync_robots,receivers/task_list}.rs`: added `RobotIndex::get_entity(...)` helper and migrated repeated map lookup sites.
- `visualizer/src/systems/draw_paths.rs`: removed per-frame temporary `Vec` allocation in path drawing by feeding an iterator chain directly to gizmo linestrip rendering.
- Validation: `cargo check --workspace` and `cargo test --workspace` pass.

### 2026-03-14: Return-to-station recovery + disabled robot policy hardening (Phase 5)

- `coordinator/src/task_manager.rs`: added assignment rejection for disabled robots and extended `attempt_replan` to support `TaskStage::ReturningToStation` (no `current_task` required) with `ReturnToStation` command dispatch.
- `coordinator/src/task_manager.rs`: refactored `handle_returning_to_station` to actively retry station path planning when path is empty/expired, instead of only handling the arrival case.
- `coordinator/src/task_manager.rs`: fixed post-delivery return behavior to always transition into retry-capable `ReturningToStation` state, including station-occupied/no-path hold-and-retry logging instead of silent idle stranding.
- `coordinator/src/server.rs`: added auto-unassign handling on robot updates when a robot is disabled mid-task; coordinator now emits task failure (`Robot disabled`) and clears robot task/path reservations so scheduler can requeue.
- `scheduler/src/allocator/mod.rs` + `scheduler/src/allocator/closest.rs`: added `enabled` to scheduler robot model and allocator filtering so disabled robots are not selected for new assignments.
- `scheduler/src/server.rs`: wired enabled flag updates from `RobotUpdateBatch` and implemented disabled-failure auto-requeue (`Failed` reason contains `disabled` → task returns to `Pending` and robot assignment is freed).
- `visualizer/src/ui/tabs/robot_inspector.rs`: disabled Enable/Disable inspector control temporarily while preserving Kill/Restart functionality.
- Validation: `cargo check --workspace`, `cargo test -p scheduler`, and `cargo test -p coordinator` pass.

### 2026-03-14: Analytics scroll polish and coordinator reservation-block dispatch tuning (Phase 5)

- `visualizer/src/ui/tabs/analytics.rs`: wrapped the WHCA analytics body in `egui::ScrollArea::vertical().auto_shrink([false, false])` to match other bottom tabs and keep metrics readable on smaller viewport sizes.
- `coordinator/src/server.rs`: changed reservation lookahead in `send_path_commands` to evaluate each scanned waypoint at its own future offset (`(step + 1) * MOVE_TIME_MS`) instead of a single fixed offset for every cell.
- `coordinator/src/server.rs`: limited `is_reserved_now` checks to the immediate next waypoint only, reducing false-positive blocks on farther lookahead cells that are not imminent.
- `coordinator/src/server.rs`: removed forced blocked-wait override fallthrough that re-sent `FollowPath` into known reservations; blocked robots now hold position with stationary reservation until the blocking reservation clears.
- `coordinator/src/server.rs`: kept long-wait visibility by logging prolonged reservation blocks without sacrificing strict conflict avoidance.
- Validation: `cargo check --workspace`, `cargo test -p coordinator`, and `cargo test --workspace` all pass.

### 2026-03-14: WHCA scenario runner benchmarks + visualizer analytics integration (Phase 5)

- `protocol/src/robot.rs` + `protocol/src/topics.rs` + `protocol/src/lib.rs`: added `WhcaMetricsTelemetry` wire type and `factory/telemetry/whca_metrics` topic export for coordinator-to-renderer WHCA analytics streaming.
- `coordinator/src/server.rs`: declared `TELEMETRY_WHCA_METRICS` publisher and now broadcasts periodic 5-second WHCA metrics deltas alongside existing coordinator logs.
- `coordinator/src/pathfinding/whca.rs`: added deterministic scenario benchmark runner test (`test_whca_scenario_metrics_table`) that prints a markdown comparison table for baseline, head-on, and intersection contention scenarios using the new runtime counters.
- `visualizer/src/resources.rs`: added `WhcaMetricsReceiver` and `WhcaMetricsData` resources for WHCA telemetry ingestion.
- `visualizer/src/systems/receivers/whca_metrics.rs` + `visualizer/src/systems/receivers/mod.rs` + `visualizer/src/main.rs`: added Zenoh receiver setup/collection systems and app wiring for WHCA metrics telemetry.
- `visualizer/src/ui/mod.rs` + `visualizer/src/ui/gui.rs` + `visualizer/src/ui/tabs/analytics.rs`: replaced analytics placeholder with live WHCA analytics panel (search volume, success rate, latency, expansions, reservation probes, edge checks, waits, peaks).
- Validation: `cargo check --workspace`, `cargo test -p coordinator test_whca_scenario_metrics_table -- --nocapture`, and `cargo test --workspace` all pass.
- Measured sample output (deterministic test): Baseline `100%` success with low latency, Head-on `0%` success with strict no-path enforcement, Intersection `100%` success with higher average latency under contention.

### 2026-03-13: WHCA runtime instrumentation and coordinator metrics logging (Phase 5)

- `coordinator/src/pathfinding/whca.rs`: added WHCA profiling counters and snapshots (`WHCAStatsSnapshot`) for searches, success/failure, node expansions, reservation probes, edge checks, waits, open-set peak, reservation peak, and timing (`total_search_time_us`, `last_search_time_us`).
- `coordinator/src/pathfinding/whca.rs`: instrumented core search loop (`find_path_whca`) and reservation paths to aggregate measurable metrics without changing strict planning behavior.
- `coordinator/src/pathfinding/mod.rs` + `coordinator/src/pathfinding/dispatcher.rs`: exported WHCA stats type and added dispatcher accessors (`whca_stats_snapshot`, `reset_whca_stats`) for runtime reporting.
- `coordinator/src/server.rs`: added periodic 5-second WHCA metrics logging with delta reporting to coordinator logs (and console when verbose), then reset startup baseline via `reset_whca_stats()`.
- `coordinator/src/pathfinding/whca.rs`: added regression test `test_whca_stats_snapshot_and_reset` to verify counter accumulation and reset semantics.
- Validation: `cargo check --workspace`, `cargo test -p coordinator`, and `cargo test --workspace` all pass.

### 2026-03-13: WHCA reservation hot-path optimization pass (Phase 5)

- `coordinator/src/pathfinding/whca.rs`: added per-robot reservation index (`robot_reservations`) and centralized reservation insertion so cleanup can remove only the target robot's keys instead of scanning the full table.
- `coordinator/src/pathfinding/whca.rs`: updated `tick()` pruning to rebuild the per-robot index from retained active reservations, preserving correctness after stale-reservation eviction.
- `coordinator/src/pathfinding/whca.rs`: replaced nested edge-swap tolerance scans with windowed robot-id set intersection (`robot_ids_in_window`), reducing edge-conflict check complexity from nested tolerance loops to linear window scans.
- Validation: `cargo check --workspace`, `cargo test -p coordinator`, and `cargo test --workspace` all pass.
- Observed trade-off update: strict safety semantics remain unchanged while reducing reservation maintenance and swap-check overhead under contention.

### 2026-03-13: WHCA strict trait closure and robot-aware goto routing (Phase 5)

- `coordinator/src/pathfinding/whca.rs`: removed trait-level A* fallbacks from WHCA `find_path` and `find_path_to_non_walkable`; trait calls now remain strict and emit explicit no-path logs in no-robot-context paths.
- `coordinator/src/pathfinding/whca.rs`: updated strictness benchmark test to assert both robot-aware and trait-path calls reject unsafe head-on corridor swaps under strict policy.
- `coordinator/src/server.rs`: changed coordinator `goto` command handling to call `find_path_for_robot(...)` so manual path dispatch participates in reservation-aware strict planning.
- `coordinator/src/pathfinding/dispatcher.rs`: updated API comments to reflect strict no-fallback WHCA behavior.
- Validation: `cargo check --workspace`, `cargo test -p coordinator`, and `cargo test --workspace` all pass.

### 2026-03-13: Publish helper migration parity and WHCA tradeoff quantification (Phase 5)

- `orchestrator/src/main.rs`: removed local `publish_json` implementation and migrated all admin/robot-control broadcasts to `protocol::publish_json_logged` for consistent serialization/publish error handling.
- `mock_firmware/src/driver.rs`: migrated `RobotUpdateBatch` publish path to `protocol::publish_json_logged`, replacing local `serde_json::to_vec` + silent publish handling.
- `mock_firmware/src/commands.rs`: migrated command response publishing to `protocol::publish_json_logged`; made `handle_path_commands` async so response publishes are awaited and logged consistently.
- `protocol/src/publish.rs` + `protocol/src/lib.rs`: added and exported `publish_json_logged_sync` to complete protocol-level helper coverage for sync callsites.
- `coordinator/src/pathfinding/whca.rs`: added deterministic benchmark test `test_tradeoff_strict_vs_trait_fallback_head_on_corridor` quantifying strict safety behavior under head-on two-cell corridor contention.
- Validation: `cargo check --workspace`, `cargo test --workspace`, and `cargo test -p coordinator test_tradeoff_strict_vs_trait_fallback_head_on_corridor -- --nocapture` all pass.
- Quantified trade-off (benchmark): strict robot-aware WHCA success `0/25` vs trait fallback success `25/25` in the constrained head-on corridor scenario, showing strict mode blocks unsafe swaps at the cost of immediate throughput in saturated bottlenecks.

### 2026-03-13: WHCA pass 2 refinement and protocol publish helper centralization (Phase 5)

- `protocol/src/publish.rs` + `protocol/src/lib.rs` + `protocol/Cargo.toml`: introduced shared `publish_json_logged` helper and exported it so coordinator/scheduler no longer duplicate local JSON publish helpers.
- `scheduler/src/server.rs`: migrated assignment/task-list/queue-state publishing to protocol helper while preserving transactional assignment rollback behavior.
- `coordinator/src/server.rs` + `coordinator/src/task_manager.rs`: migrated all command/status/telemetry publishing to protocol helper, removing duplicate local helper implementations.
- `coordinator/src/task_manager.rs`: added station occupancy guards for low-battery return, post-delivery return, and station-arrival charging transitions to reduce multi-robot station overlap risk under contention.
- `coordinator/src/pathfinding/whca.rs`: fixed edge-swap timing check to compare occupancy across the move step (`time_ms` to `time_ms + MOVE_TIME_MS`) and added strict WHCA regression tests for blocked-window no-fallback behavior and unsafe swap blocking.
- Validation: `cargo check --workspace`, `cargo test -p coordinator`, `cargo test -p scheduler`, and `cargo test --workspace` all pass.
- Observed trade-off update: stricter station-occupancy and edge-swap blocking further reduce unsafe close-contact behavior, with additional waiting under dense contention and slightly lower peak throughput.

### 2026-03-13: Strict WHCA safety pass 1 and publish-transaction hardening (Phase 5)

- `coordinator/src/pathfinding/whca.rs`: switched to strict WHCA behavior by removing A* fallback from robot-aware path methods; no reservation-ignoring path is dispatched when WHCA* reports no safe route.
- `coordinator/src/pathfinding/whca.rs`: tightened reservation behavior by using forward-only tolerance reservation windows and stronger edge-swap collision checks.
- `coordinator/src/server.rs`: added rolling reservation refresh in the path watchdog loop to keep long path tails protected as the WHCA planning window advances.
- `scheduler/src/server.rs`: made assignment publish transactional; task/robot/inventory assignment state now rolls back if `TaskAssignment` publish fails.
- `scheduler/src/server.rs`: replaced queue/task-list state broadcasts with logged-safe publish helper paths.
- `coordinator/src/task_manager.rs`: replaced remaining silent status/control publish `.ok()` calls with logged-safe publish helper usage.
- `protocol/src/config.rs`: increased stationary safety coverage (`STATIONARY_HISTORY_TILES` 2 -> 4, `STATIONARY_RESERVATION_MS` 1500 -> 2500) and reduced replan wait threshold (`RESERVATION_WAIT_REPLAN_SECS` 3 -> 2).
- Validation: `cargo check --workspace`, `cargo test -p coordinator`, `cargo test -p scheduler`, and `cargo test --workspace` all pass.
- Observed trade-off: stricter safety policy increases conservative waiting/retry behavior under dense contention, reducing peak throughput while improving collision resistance and state consistency.

### 2026-03-13: Coordinator and scheduler runtime hardening (Phase 5)

- `protocol/src/util.rs` + `protocol/src/lib.rs`: added shared `manhattan_distance_xz` and `is_reachable_on_map` helpers with coverage tests, then exported them for cross-crate reuse.
- `scheduler/src/server.rs`: removed local world/grid + BFS duplication in favor of protocol helpers, replaced random-task `unwrap()` usage with safe pattern matching, and added malformed payload logging for task requests, robot updates, and task status updates.
- `scheduler/src/allocator/closest.rs`: replaced manual distance arithmetic with protocol Manhattan helper and removed hot-loop console prints in favor of structured file logging.
- `scheduler/src/queue/mod.rs` + `scheduler/src/cli.rs`: clarified queue semantics docs (priority-first with FIFO ties) and replaced test `unwrap()` calls with explicit `expect(...)` messages.
- `coordinator/src/server.rs`: removed runtime `to_vec(...).unwrap()` paths in recurring loops, replaced invariant `next_waypoint().expect(...)` with safe handling, and added malformed payload diagnostics across all inbound subscriber handlers.
- `coordinator/src/task_manager.rs`: removed runtime command publish `unwrap()` usage and centralized safe publish behavior with serialization/publish failure logging.
- `coordinator/src/pathfinding/mod.rs`: delegated coordinate conversion wrappers to `protocol` utilities to reduce duplicate arithmetic logic while preserving existing public APIs.
- Validation: `cargo check --workspace`, `cargo test -p coordinator`, `cargo test -p scheduler`, and `cargo test --workspace` all pass.

### 2026-03-13: Firmware command-path logging cleanup (Phase 5)

- `mock_firmware/src/commands.rs`: removed high-frequency command-path `println!/eprintln!` output and moved reporting to `protocol::logs::save_log` for quieter runtime consoles.
- `mock_firmware/src/commands.rs`: switched system command application from `apply_with_log` to `apply` and added explicit file-log entries per effect (`Pause`, `Resume`, `Verbose`, `Chaos`, `SetTimeScale`).
- `mock_firmware/src/commands.rs`: standardized response publishing using `CommandResponse::accepted` / `CommandResponse::rejected` helpers for consistent command acknowledgements.
- Validation: `cargo check --workspace` and `cargo test --workspace` both pass after this pass.

### 2026-03-13: Parser robustness and protocol docs cleanup (Phase 5)

- `protocol/src/grid_map.rs`: removed warning prints from shelf-token parsing and made shelf stock parsing deterministic by using a default (`x5`) for invalid/zero values and clamping to `SHELF_MAX_CAPACITY`.
- `protocol/src/robot.rs`: tightened module/type field documentation for clearer wire-contract boundaries between firmware, coordinator, scheduler, and renderer.
- `mock_firmware/src/main.rs`: hardened `MAP_VALIDATION` subscription handling by logging malformed payloads and continuing safely instead of silently ignoring decode failures.
- Validation: `cargo check --workspace`, `cargo test -p protocol`, `cargo test -p mock_firmware`, and `cargo test -p orchestrator` all pass.

### 2026-03-13: Core runtime hardening pass (Phase 5)

- `protocol/src/util.rs` + `protocol/src/lib.rs`: added shared world/grid and distance helpers (`world_to_grid`, `grid_to_world`, `distance_xz`, `distance_sq_xz`, `is_finite_position`) and exported them for cross-crate reuse.
- `protocol/src/config.rs`: switched battery drain configuration from random range to deterministic `DRAIN_RATE_PER_SEC = 0.05` for replayability.
- `protocol/src/commands.rs`: added `RobotControl::id()` helper and cleaned command serialization tests to avoid panic-style assertions.
- `orchestrator/src/main.rs`: replaced duplicated runtime broadcast helpers with a single generic publish path that returns `Result` instead of panicking on serialization or publish failures.
- `orchestrator/src/cli.rs`: normalized robot command naming (`RobotEnable`/`RobotDisable`) for consistency with CLI vocabulary.
- `orchestrator/src/processes.rs`: deduplicated output visibility toggles and isolated startup notifier playback behind a panic-safe helper.
- `mock_firmware/src/commands.rs`: centralized sample parsing with logged error handling, deduplicated robot lookup/control logic, and standardized command response publishing.
- `mock_firmware/src/driver.rs`: removed runtime serialization panic in update batching and replaced it with graceful log-and-skip behavior.
- `mock_firmware/src/robot.rs`: removed local conversion duplication in favor of protocol utilities and made battery drain deterministic.
- `mock_firmware/Cargo.toml`: removed direct `rand` dependency from firmware crate.

### 2026-03-13: Time-scale stability and real-time pause wiring (Phase 5)

- `mock_firmware/src/robot.rs`: waypoint arrival now snaps when the robot reaches or passes the target in one large step, preventing high-speed oscillation where robots appeared to move in place at 5x.
- `coordinator/src/commands.rs` + `coordinator/src/server.rs`: coordinator now stores `time_scale` from `SystemCommand::SetTimeScale` and uses it in `validate_robot_update` movement bounds, preventing false jump faults at higher simulation speeds.
- `visualizer/src/resources.rs`: added `UiAction::SetRealtime(bool)` and `UiState.paused_before_realtime` to preserve/restore the pre-realtime pause state.
- `visualizer/src/ui/tabs/control_bar.rs`: real-time toggle now emits a UI action; pause/speed controls are disabled while real-time mode is active to avoid conflicting simulation commands.
- `visualizer/src/systems/command_bridge.rs`: realtime ON publishes `SystemCommand::Pause`; realtime OFF restores the previous simulation state (resume only when it was running before entering real-time).

### 2026-03-12: Time scale, robot enable/disable, floor z-fighting (Phase 5)

Four fixes addressing floor rendering, simulation speed controls, and robot actions.

**Fix: Floor z-fighting with walls** -- `protocol/src/config.rs`, `visualizer/systems/models.rs`: Added `GROUND_Y_OFFSET = -0.001` constant. `spawn_floor` now places floor tiles 0.001 units below y=0, eliminating z-fighting with wall models that have embedded floor planes.

**Feature: Simulation time scale** -- Full wire-up from UI to firmware:
- `protocol/commands.rs`: Added `SystemCommand::SetTimeScale(f32)` and `SystemCommandEffect::TimeScale(f32)`
- `mock_firmware/commands.rs`: `handle_system_commands` now accepts `time_scale: &mut f32` and applies `TimeScale` effect (clamped to 0.1..1000)
- `mock_firmware/driver.rs`: Physics passes `dt * time_scale` instead of raw `dt`
- `visualizer/resources.rs`: Added `SetTimeScale(f32)` to `UiAction`, plus `custom_speed_editing: bool` and `custom_speed_text: String` to `UiState`
- `visualizer/systems/command_bridge.rs`: Wires `SetTimeScale` action to `SystemCommand`
- `visualizer/ui/tabs/control_bar.rs`: Replaced stub speed buttons with functional 1x/2x/5x/10x presets plus "Custom" button that accepts values up to 1000x

**Fix: Robot enable/disable buttons** -- `protocol/robot.rs`: Added `enabled: bool` field to `RobotUpdate`. Firmware now broadcasts all robots (including disabled ones) with their `enabled` flag. `visualizer/components.rs` and `sync_robots.rs` updated to track the field. `robot_inspector.rs` now shows contextual "Enable"/"Disable" button based on robot state.

**Feature: Real-time mode overlay** -- `visualizer/ui/gui.rs`: New `realtime_overlay()` function renders a centered popup when `is_realtime` is true, explaining that physical robot tracking is not yet implemented.

**Housekeeping:** Fixed pre-existing `ShelfInventory` tests in `grid_map.rs` that were failing because they assumed shelf capacity equals initial stock, when actual capacity is `SHELF_MAX_CAPACITY = 16`.

### 2026-03-12: Camera/label polish fixes (Phase 5)

- `camera_follow_task` (`camera.rs`): added `was_live: Local<bool>` tracking. Camera now only lerps to default when a task that was being actively followed transitions to terminal. Selecting an already-completed task leaves the camera untouched. Deselecting a terminal task also does nothing.
- `robot_labels.rs` zoom scale clamp widened from `(0.45, 1.6)` to `(0.3, 1.5)` for a more noticeable size change with zoom. Label base sizes reduced in `protocol::config::visual::labels`: `FONT_SIZE` 10 -> 8, `ICON_SIZE` 15 -> 11 (user tunable).
- Label viewport clip rect now uses live panel widths stored in `UiState` (`left_panel_width`, `right_panel_width`, `bottom_panel_height`) instead of hardcoded `SIDE_PANEL_DEFAULT_WIDTH`. `gui.rs` captures `panel_resp.response.rect` after each `.show()` and writes it back to `UiState` each frame, so the clip rect always matches the actual panel boundaries even when the user resizes panels.


### 2026-03-12: Empty shelf pickup validation (Phase 5)

- `wizard_minimap_widget` in `minimap.rs` gains `empty_positions: Option<&HashSet<(usize,usize)>>` parameter; empty shelves render dark gray (`from_gray(45)`) and are non-interactive when in pickup mode.
- `wizard_view` in `tasks.rs` builds `empty_shelves` HashSet from live `Shelf.cargo` components and passes it to the Step 1 pickup minimap; Step 2 (dropoff) passes `None` so any shelf is a valid destination.
- `shelf_inspector.rs` "Add Transport Task" button disabled (`add_enabled`) when `shelf.cargo == 0` with a "Shelf is empty" hint label.
- `scheduler/server.rs` `handle_task_requests` gains `&ShelfInventory` parameter; `TaskCommand::New` handler checks `inventory.can_pickup(pickup)` before enqueuing and rejects with a log message if the source shelf is empty.

### 2026-03-12: Fix camera lerp fighting user input (Phase 5)

Three bugs where camera follow/reset lerps would keep running against user input after it occurred.

**Root cause 1 — `zooming_in` never cleared by scroll:** In both `camera_follow_selected` and `camera_follow_task`, once the zoom-in lerp started it ran every frame regardless of scroll wheel input. The controller radius was being set by `camera_controls` AND simultaneously lerped back by the follow system in the same frame, creating a tug-of-war.

**Root cause 2 — `resetting` never interrupted in `camera_follow_task`:** When a task was deselected, the camera would lerp back to the default view. But `*resetting` was a `Local<bool>` that was never checked against user input, so panning, scrolling, or orbiting while the reset was in progress had no effect — the reset kept overwriting the camera each frame.

**Root cause 3 — Orbit fought entity focus lerp:** `camera_follow_selected` lerped focus every frame including frames where the user was right-dragging to orbit. This made orbiting around a followed robot feel sticky/resistive.

**Fix:** Added three per-frame input signal fields to `UiState`: `camera_scroll_this_frame`, `camera_pan_this_frame`, `camera_orbit_this_frame`. `camera_controls` clears all three unconditionally at function entry (before the egui early-return guard, to prevent stale values from the previous frame) then sets the appropriate flag when each input fires. Follow systems consume these flags:
- `camera_follow_selected`: clears `zooming_in` on scroll; pauses focus lerp on orbit
- `camera_follow_task`: clears `zooming_in` on scroll; cancels `*resetting` on any input (scroll/pan/orbit)

### 2026-03-10: Task UI polish and camera task-follow (Phase 5)

Eight cohesive improvements to the task inspector, task list, and camera system.

**Fix: Add Task button invisible** -- `ui/tabs/tasks.rs`: The `+ Add New Task` button was rendered after `task_list_view`, which used a `ScrollArea` with `auto_shrink([false, false])` that consumed all remaining vertical space. Moved the button to render first (before the scroll area) so it is always visible at the top of the list area.

**Fix: Task deselection on background click** -- `ui/mod.rs`: The background-click deselect guard only cleared `selected_entity`. Extended the condition to also fire when `selected_task_id` is set, and clear it alongside entity in the same handler.

**Feature: `completed_at` field on Task** -- `protocol/src/tasks.rs`: Added `pub completed_at: Option<u64>` with `#[serde(default)]` (backward-compatible). Initialized to `None` in `Task::new()`. Stamped in `scheduler/src/server.rs` when a `TaskStatusUpdate` transitions a task to `Completed`, `Failed`, or `Cancelled`.

**Improvement: Task inspector completed-state** -- `ui/tabs/task_inspector.rs`:
- Robot field now shows "N/A" for terminal statuses (Completed/Failed/Cancelled) instead of "Pending"
- "Completed:"/"Failed:"/"Cancelled:" timestamp row shown when `completed_at` is set
- Priority `ComboBox` and "Remove Task" button are hidden entirely for terminal tasks
- All timestamps converted from UTC arithmetic to GMT+3: `secs + 3 * 3600`, label reads "GMT+3"

**Improvement: Larger minimap in task inspector** -- `ui/widgets/minimap.rs`: `task_detail_minimap` cell size increased from 8 px to 11 px, `max_height` from 120 px to 200 px.

**Feature: Camera task-follow system** -- `systems/camera.rs`, `protocol/src/config.rs`, `main.rs`:
- New constants `TASK_FOLLOW_ZOOM_RADIUS = 18.0` and `DEFAULT_RESET_LERP = 0.05` in `protocol::config::visual::camera`
- New Bevy system `camera_follow_task` (registered after `camera_follow_selected` in Update): when a task is selected in the inspector, the camera smoothly follows the relevant world target -- the pickup shelf for Pending tasks, or the assigned robot for Assigned/InProgress. On terminal status or task deselection, the camera lerps back to default `DEFAULT_FOCUS`/`DEFAULT_RADIUS`/`DEFAULT_PITCH`. Entity follow takes priority over task follow.

**Rename: Objects tab -> Entities** -- `ui/tabs/objects.rs`: `LABEL` changed from `"Objects"` to `"Entities"`. Propagates automatically via `tabs::objects::LABEL` in `gui.rs`.

### 2026-03-10: Fix Zenoh always-recompiles on orchestrator run (Phase 5)

Diagnosed why Zenoh's TLS/crypto stack (`ring`, `rustls`, `quinn-proto`, etc.) recompiled on every `run` command in the orchestrator. Root causes: (1) `start_all` issued two separate `cargo build` invocations -- one for `coordinator/mock_firmware/scheduler`, one for `visualizer`. When the second invocation ran, `ring`'s build script detected that `target/` had changed from the first build and re-evaluated its fingerprint, triggering a full Zenoh stack recompile. (2) No profile overrides existed for the heavy crypto deps, so every recompile paid full opt-level cost.

**Fix 1 -- `orchestrator/src/processes.rs`:** Merged the two `cargo build` calls into a single `cargo build -p coordinator -p mock_firmware -p scheduler -p visualizer` invocation. Cargo now unifies the full dependency graph in one pass; `ring`'s fingerprint remains stable because `target/` is only written to once.

**Fix 2 -- `Cargo.toml`:** Added `[profile.dev.package]` overrides setting `opt-level = 0` for `ring`, `rustls`, `quinn-proto`, `quinn`, and `rustls-webpki`. These crates do not need speed in dev builds, and Cranelift can codegen them cheaply at opt-level 0, reducing unavoidable recompile time significantly.

### 2026-03-10: views/ -> tabs/, per-tab LABEL+draw() convention (Phase 5)

Renamed `ui/views/` to `ui/tabs/` throughout (files, module declarations, imports, comments). No logic changes -- pure structural reorganisation.

**Per-tab modularity:** Every directly-selectable tab module now exports `pub const LABEL: &str` (consumed by `gui.rs` tab bars so the string is defined exactly once) and `pub fn draw(ui, ...)` as the unified entry point. Sub-inspector modules (`robot_inspector`, `shelf_inspector`, `task_inspector`) export only `draw()` -- no LABEL since they are not directly selectable.

**New `tabs/details.rs`:** Extracted the inspector routing logic that was inline inside `gui.rs::inspector()` into its own module. `details.rs` owns the "entity inspector > task inspector > empty-state" decision tree. `gui.rs::inspector()` now matches tabs and delegates -- no routing logic of its own.

**Splits:** `views/bottom.rs` (`logs_tab` + `analytics_tab`) replaced by two dedicated files: `tabs/logs.rs` and `tabs/analytics.rs`, each with its own LABEL constant and `draw()` function.

**gui.rs:** Imports `use super::tabs;`. Tab bar string literals replaced with `tabs::objects::LABEL` etc. All `views::xxx_tab()` / `views::xxx_inspector()` calls replaced with `tabs::xxx::draw()`. The inline Details routing block replaced by a single `tabs::details::draw(...)` call.

**Files created:** `tabs/mod.rs`, `tabs/control_bar.rs`, `tabs/objects.rs`, `tabs/tasks.rs`, `tabs/details.rs` (new), `tabs/network.rs`, `tabs/logs.rs` (new), `tabs/analytics.rs` (new), `tabs/robot_inspector.rs`, `tabs/shelf_inspector.rs`, `tabs/task_inspector.rs`. **Deleted:** `views/` directory (8 files). **Modified:** `ui/mod.rs`, `ui/gui.rs`. `cargo check --workspace` passes with zero errors.

---

### 2026-03-10: Visualizer codebase restructure (Phase 5)

Split the bloated `panels.rs` (1387 lines, 20 functions) and flat `systems/` layout into a clean, layered structure. No logic changes — pure file organisation.

**systems/ restructure:** Created `systems/receivers/` subfolder housing four renamed modules: `robot_updates.rs` (was `zenoh_receiver.rs`), `queue_state.rs` (was `queue_receiver.rs`), `path_telemetry.rs` (was `path_receiver.rs`), `task_list.rs` (was `task_receiver.rs`). Extracted outbound Zenoh publishers from `commands.rs` into new `command_bridge.rs` (`setup_publishers`, `run_publisher_loop`, `bridge_ui_commands`). `commands.rs` is now inbound-only (`setup_system_listener`, `handle_system_commands`) — consistent with every other crate's `commands.rs` pattern. `systems/mod.rs` updated accordingly.

**ui/ restructure:** `panels.rs` replaced by `panels/mod.rs` (thin layout routing only: `top_panel`, `left_panel`, `right_panel`, `bottom_panel` + private `sim_controls`). Content split into two new subfolders:
- `ui/views/`: `objects.rs` (`objects_tab`, `state_icon`, `select_entity`), `tasks.rs` (`tasks_tab`, `task_list_view`, `task_row`, `task_row_label`, `wizard_view`), `robot_inspector.rs`, `shelf_inspector.rs`, `task_inspector.rs`, `bottom.rs` (`logs_tab`, `analytics_tab`).
- `ui/widgets/`: `common.rs` (`color_swatch`, `shelf_fill_color_egui`), `minimap.rs` (`wizard_minimap_widget`, `shelf_minimap_widget`, `task_detail_minimap`).

`ui/mod.rs` gains `pub mod views; pub mod widgets;`. `main.rs` import paths updated. `cargo check --workspace` passes with zero errors, zero warnings.

**Files changed:** `systems/mod.rs`, `systems/commands.rs`, `systems/command_bridge.rs` (new), `systems/receivers/` (new: 5 files), `ui/mod.rs`, `ui/panels/mod.rs` (new), `ui/views/` (new: 7 files), `ui/widgets/` (new: 3 files), `main.rs`. Deleted: `zenoh_receiver.rs`, `queue_receiver.rs`, `path_receiver.rs`, `task_receiver.rs`, `panels.rs`.

---

### 2026-03-10: Task Management UI — full implementation (Phase 5)

Complete Task Management UI across protocol, scheduler, and visualizer layers.

**Protocol** (`protocol/src/tasks.rs`, `topics.rs`, `lib.rs`): Added `TaskCommand` enum (`New`, `Cancel`, `SetPriority`) replacing bare `TaskRequest` on the `TASK_REQUESTS` topic for full task lifecycle control. Added `TaskListSnapshot { tasks: Vec<Task>, timestamp_ms }` and `TASK_LIST` topic constant for per-task data delivery to the renderer.

**Scheduler** (`scheduler/src/server.rs`): `handle_task_requests` now deserializes `TaskCommand` — supports UI-driven creation, cancellation, and priority mutation with console + log output. Added `broadcast_task_list()` publishing `TaskListSnapshot` on every 2-second heartbeat alongside `QueueState`.

**Visualizer resources** (`visualizer/src/resources.rs`, `components.rs`): Added `TaskListData`, `TaskListReceiver` resources. Added wizard state to `UiState` (`task_wizard_active`, `wizard_pickup`, `wizard_dropoff`, `wizard_priority`, `selected_task_id`). Added `CancelTask(u64)` and `ChangePriority(u64, Priority)` to `UiAction`. `OutboundCommand::Task` changed to carry `TaskCommand`. `Robot.current_task` field type corrected from `Option<u32>` to `Option<u64>`.

**task_receiver system** (`visualizer/src/systems/task_receiver.rs`): New module with `setup_task_listener` (background subscriber on `TASK_LIST`), `collect_task_list` (drains into `TaskListData`), and `sync_robot_tasks` (clears all robot current_task, then re-applies from `Assigned`/`InProgress` entries each frame).

**commands bridge** (`visualizer/src/systems/commands.rs`): `SubmitTransportTask` now serializes as `TaskCommand::New`. New arms for `CancelTask` and `ChangePriority`.

**Left Panel — Task Tab** (`visualizer/src/ui/panels.rs`): Replaces placeholder with:
- Stats header (active/completed/failed/robots from `TaskListData` + `QueueStateData`).
- `task_list_view()`: three `CollapsingState` sections (Active: open, Failed: open with red text, Completed: collapsed). Clicking a row selects the task ID and switches to the Details tab.
- `wizard_view()`: multi-step wizard replacing the task list. Step 1 selects a pickup shelf via `wizard_minimap_widget`. Step 2 selects dropoff (shelves + dropoffs clickable) with the pickup cell highlighted in blue. Priority `ComboBox` (Low/Normal/High/Critical). "Add Task" button enabled only when both points chosen.
- `wizard_minimap_widget()`: new interactive minimap shared by both wizard steps — tile-type-based coloring, `Sense::click` interactions, distinct blue/green highlight overlays.

**Right Panel — Task Inspector** (`visualizer/src/ui/panels.rs`): New `task_inspector()` shown when `selected_task_id.is_some()` and no entity is selected:
- Pickup/dropoff coordinates with `task_detail_minimap()` (read-only, blue/green highlighted cells).
- Assignment, status (with failure reason string), UTC created timestamp.
- ETA: looks up robot in `ActivePaths`, computes `path.len() / ROBOT_SPEED`; falls back to "N/A" / "Arriving" without panic.
- Priority `ComboBox` that emits `ChangePriority` on change.
- "Remove Task" button that emits `CancelTask` and clears selection.

**Files changed:** `protocol/src/tasks.rs`, `protocol/src/topics.rs`, `protocol/src/lib.rs`, `scheduler/src/server.rs`, `visualizer/src/components.rs`, `visualizer/src/resources.rs`, `visualizer/src/systems/mod.rs`, `visualizer/src/systems/task_receiver.rs` (new), `visualizer/src/systems/commands.rs`, `visualizer/src/main.rs`, `visualizer/src/ui/mod.rs`, `visualizer/src/ui/panels.rs`.

---

### 2026-03-09: Fix robot state pipeline — 4 bugs (Phase 5)

Four bugs causing wrong robot labels were identified by full codebase investigation and fixed.

**Bug 1 — "CHARGING" while still mid-return** (`coordinator/src/task_manager.rs` `handle_returning_to_station`): The battery-ready check `if battery >= MIN_BATTERY_FOR_TASK { task_stage = Idle }` ran unconditionally every tick, outside the arrival guard. A robot returning because there were no pending tasks (`NoPendingTasks` reason) typically had a full battery and would exit `ReturningToStation` on the very first tick before reaching the station. Fix: moved the battery check inside the `path_complete() && is_near()` block so it only fires once the robot has physically arrived.

**Bug 2 — "-> PICKUP" at station and after any stop** (`coordinator/src/server.rs` `send_path_commands` watchdog): The watchdog always emitted `PathCommand::FollowPath` regardless of `task_stage`. Firmware infers state from cargo presence: no cargo → `MovingToPickup`. Any event that cleared `path_sent` (Stop, deadlock override) during a return trip caused the watchdog to resend `FollowPath`, clobbering `MovingToStation` with `MovingToPickup`. Fix: watchdog now checks `robot.task_stage == ReturningToStation` and emits `PathCommand::ReturnToStation` instead.

**Bug 3 — Faulted state never visible in visualizer** (`coordinator/src/task_manager.rs` `mark_robot_faulted`): The function only mutated the coordinator's local `TrackedRobot.last_update.state`. Firmware never received a fault signal and kept broadcasting `Blocked`. Fix: added `PathCommand::Fault` variant to the protocol. Firmware handles it by stopping all movement and setting `RobotState::Faulted`. `mark_robot_faulted` now sends a `PathCmd::Fault` to firmware so future Zenoh broadcasts carry the correct state. Threaded `cmd_publisher` and `next_cmd_id` through all four call sites (`handle_blocked_robots` ×2, `handle_robot_update`, `detect_inter_robot_collisions`).

**Bug 4 — Low-battery guard overwrites `MovingToStation`** (`mock_firmware/src/robot.rs` `update_physics`): The low-battery state override `if battery <= LOW_THRESHOLD` did not exclude `MovingToStation`. A robot already heading home would have its state overwritten to `LowBattery`; on arrival `on_arrival()` wouldn't match and the robot would stay `LowBattery` at the station forever. Fix: added `&& self.state != RobotState::MovingToStation` to the guard.

**Files changed:** `protocol/src/commands.rs`, `mock_firmware/src/robot.rs`, `coordinator/src/task_manager.rs`, `coordinator/src/server.rs`.

---

### 2026-03-09: Label zoom scaling, right-click hide, fix OFFLINE and -> PICKUP bugs (Phase 5)

**Label zoom scaling** — Labels now scale with camera zoom. At the default orbit radius they are 1x; zooming in makes them slightly larger (up to 1.6x), zooming out shrinks them (down to 0.45x). Formula: `sqrt(DEFAULT_RADIUS / radius).clamp(0.45, 1.6)` — sqrt smooths the curve. Font sizes, stroke width all scale together. Requires `CameraController` query in `draw_robot_labels`.

**Right-click to hide, auto-restore on deselect** — Right-clicking a robot in the 3D viewport adds it to a `hidden_labels: HashSet<Entity>` in `UiState`. The label is hidden until the robot is deselected, at which point the entry is removed (cleared in `on_pointer_click` before deselect, and in the background-click deselect handler in `draw_ui`). Labels no longer auto-hide when selected — only explicit right-click hides them.

**Bug: OFFLINE shown when robot is IDLE** — Root cause: `collect_robot_updates` in `zenoh_receiver.rs` only forwarded updates to `sync_robots` when position or state changed. A stationary idle robot would pass 0 updates indefinitely, so `last_update_secs` on the `Robot` component was never refreshed and the 3.0 s offline timeout triggered. Fix: removed the dedup guard entirely — all received updates are forwarded. At 20 Hz and few robots this is negligible overhead.

**Bug: `-> PICKUP` shown when returning to station** — Root cause: `PathCommand::FollowPath` infers firmware state from cargo presence: no cargo → `MovingToPickup`. The coordinator was sending `FollowPath` for return-to-station paths, so the firmware always showed `MovingToPickup` during return. Fix: added `PathCommand::ReturnToStation { waypoints, speed }` to the protocol. Firmware handles it identically to `FollowPath` but sets `RobotState::MovingToStation`. The coordinator now dispatches `ReturnToStation` for both low-battery return and post-task return paths.

**Files changed:** `protocol/src/commands.rs`, `mock_firmware/src/robot.rs`, `coordinator/src/task_manager.rs`, `visualizer/src/systems/zenoh_receiver.rs`, `visualizer/src/resources.rs`, `visualizer/src/ui/mod.rs`, `visualizer/src/systems/outline.rs`, `visualizer/src/systems/robot_labels.rs`.

---

### 2026-03-09: Robot label fix — ASCII status words, viewport clipping (Phase 5)

Two follow-up fixes to overhead robot labels after visual review.

**Unicode symbols rendered as boxes** — egui's default Hack font has no glyph coverage for `⚡`, `↺`, `▣`, `✖`, etc., so they all appeared as `□`. Replaced all Unicode glyphs with plain ASCII status words: `FAULT`, `LOW BATT`, `REROUTING`, `CHARGING`, `PICKING`, `-> PICKUP`, `-> DROP`, `-> CHARGER`, `IDLE`, `OFFLINE`. Cargo flag is a `PKG` suffix. All reliable in any font.

**Labels bleeding into side panels** — `Order::Background` alone does not prevent labels from robots physically behind the sidebar from projecting into panel screen-space. Added an explicit viewport clip rect computed from `ctx.content_rect()` minus `SIDE_PANEL_DEFAULT_WIDTH` (left+right), `TOP_PANEL_HEIGHT` (top), and `BOTTOM_PANEL_DEFAULT_HEIGHT` (bottom). Labels whose projected screen position falls outside that rect are skipped.

**Files changed:** `visualizer/src/systems/robot_labels.rs`.

---

### 2026-03-09: Robot label UX polish — readability, layering, declutter (Phase 5)

Four fixes to the overhead robot labels based on visual review.

**1. Labels over UI panels fixed** — Changed `Order::Tooltip` → `Order::Background` (labels now render below all egui panels), and swapped the `EguiPrimaryContextPass` chain to `draw_robot_labels` → `draw_ui` so panels are always drawn last.

**2. Label hides while selected, restores on deselect** — Removed the persistent `label_hidden: bool` flag from `Robot` and the right-click observer branch in `outline.rs`. The label system now simply skips the entity that matches `ui_state.selected_entity`, which automatically restores when the selection is cleared.

**3. Readability** — Background changed from semi-transparent black to fully opaque dark (`(18, 18, 18, 245)` RGBA). A colored border stroke (`STROKE_WIDTH = 1.5`) matching the state color makes each label pop against any background.

**4. Content decluttered** — Battery percentage removed. Label now shows: small dim `#ID` + large bold state icon + `▣` if carrying cargo. Icon promoted to `ICON_SIZE = 15.0`; ID at `FONT_SIZE = 10.0`. Icon is the primary signal; ID is secondary context.

**Files changed:** `protocol/src/config.rs`, `visualizer/src/components.rs`, `visualizer/src/systems/sync_robots.rs`, `visualizer/src/systems/robot_labels.rs`, `visualizer/src/systems/outline.rs`, `visualizer/src/main.rs`.

### 2026-03-09: Overhead robot labels (Phase 5)

Implemented floating egui labels rendered over each robot in the 3D viewport.

**Label contents**: `#ID  <goal-icon>  <battery%> [▣]`
- Robot ID (bold, colored by state)
- Goal icon: `●` idle, `→PKG` moving to pickup, `→DST` moving to drop, `→⚡` moving to station, `↓PKG` picking, `⚡` charging, `⚡!` low-battery, `↺` blocked/rerouting, `✖` faulted, `✕` offline
- Battery percentage and cargo indicator (`▣` when carrying)
- Muted gray secondary text; main ID + icon inherit the state color

**State color map** (added to `protocol::config::visual::labels`):
| State | Color | Notes |
|---|---|---|
| Faulted | Red | collision or hard fault |
| LowBattery | Orange | below 20% threshold |
| Blocked | Blue | WHCA* rerouting |
| Charging | Green | at home station |
| Picking | Yellow | cargo transfer in progress |
| Normal | Near-white | idle, moving, en-route |
| Offline | Gray | no update for >3 s |

**Controls**:
- Top-bar *Labels* checkbox (`UiState.show_ids`) globally shows/hides all labels
- Right-click a robot in the 3D viewport to hide/show that robot's label individually (`Robot.label_hidden`)

**Architecture**:
- `protocol::config::visual::labels` — all constants (colors, offsets, font size, timeout)
- `Robot` component — added `last_update_secs: f32` (set by `sync_robots` each update) and `label_hidden: bool` (toggled by right-click observer)
- `systems/robot_labels.rs` — `draw_robot_labels` system: projects world pos → egui logical pixels via `Camera::world_to_viewport`, renders one `egui::Area` per robot with `pivot(CENTER_BOTTOM)` for natural floating alignment
- `systems/outline.rs` `on_pointer_click` — secondary (right) click branch added before existing primary logic; toggles `robot.label_hidden` on the logical Robot entity
- `main.rs` — `draw_robot_labels` registered in `EguiPrimaryContextPass` chained after `draw_ui` so labels render on top of the viewport but under the egui panels

**Files changed:** `protocol/src/config.rs`, `visualizer/src/components.rs`, `visualizer/src/systems/sync_robots.rs`, `visualizer/src/systems/robot_labels.rs` (new), `visualizer/src/systems/mod.rs`, `visualizer/src/systems/outline.rs`, `visualizer/src/main.rs`, `visualizer/src/ui/panels.rs`.

### 2026-03-10: Fix NotShadowCaster import and add run_if guard on reload check (Phase 5)

Two fixes to unblock the shadow propagation optimization and reduce per-frame overhead.

**Fix 1 — correct import path (`populate_scene.rs`):** `bevy::pbr::{NotShadowCaster, NotShadowReceiver}` moved to `bevy_light` in Bevy 0.17. Corrected import to `bevy::light::{NotShadowCaster, NotShadowReceiver}`. The `propagate_tile_optimizations` system is now fully operational.

**Fix 2 — `run_if` guard on `check_reload_environment` (`main.rs`):** The system was registered in `PostUpdate` unconditionally, evaluating a broad `Query<Entity, Or<(With<Ground>, With<Wall>, With<Shelf>, With<Station>, With<Dropoff>)>>` every frame even when no reload was pending. Added `.run_if(resource_exists::<ReloadEnvironment>)` so the system and all its queries are skipped when the resource is absent. Import added: `bevy::ecs::schedule::common_conditions::resource_exists`.

**Files changed:** `visualizer/src/systems/populate_scene.rs`, `visualizer/src/main.rs`.

### 2026-03-09: Fix collision cascade: lookahead scan, fault-zone stop, faster tick (Phase 5)

Three interacting root causes caused every collision to cascade into a wave of secondary collisions across the fleet.

**Fix 1 — halve coordinator tick rate (`config.rs`):** `PATH_SEND_INTERVAL_MS` reduced from `100ms` to `50ms` (20 Hz, matching firmware physics tick). At `ROBOT_SPEED = 2.0` the robot travels 0.1 units per tick vs. 0.2 before, doubling the coordinator's reaction window before a robot crosses a reserved cell. Added `LOOKAHEAD_BLOCK_SCAN_CELLS: usize = 4` constant.

**Fix 2 — N-cell lookahead reservation scan (`server.rs`, `send_path_commands`):** The reservation check previously only tested `next_waypoint()` — one cell ahead. With a full `FollowPath` already dispatched, the firmware could be 3–4 cells mid-segment before the coordinator's next tick saw a blocked cell. Replaced the single-cell check with a scan of the next `LOOKAHEAD_BLOCK_SCAN_CELLS = 4` waypoints. If any cell in the lookahead is reserved, the robot is stopped and the `FollowPath` withheld, giving WHCA* time to find an alternate route before the robot arrives.

**Fix 3 — fault-cascade stop (`server.rs`, `detect_inter_robot_collisions`):** When a collision faulted robots A and B, `mark_robot_faulted` immediately cleared their reservations from WHCA*. Other robots (C, D, E) with already-dispatched `FollowPath` commands were still driving toward those now-unowned cells, colliding with A/B when they restarted. Restructured `detect_inter_robot_collisions` into three passes: (1) read-only pass collecting `(robot_id, reason)` pairs to fault; (2) fault pass that also records the grid positions of faulted robots; (3) cascade-stop pass that sends `PathCommand::Stop` to any robot whose remaining path passes through the faulted cells, clearing `path_sent` so the watchdog re-dispatches from the new position. Function signature gains `cmd_publisher` and `next_cmd_id`. Added skip-if-already-faulted guard to prevent re-faulting on subsequent ticks.

**Files changed:** `protocol/src/config.rs`, `coordinator/src/server.rs`.

### 2026-03-08: Propagate shadow markers to tile mesh children (Phase 5)

Implemented the child-mesh propagation system that makes `DISABLE_TILE_SHADOW_CAST` and `DISABLE_FLOOR_SHADOW_RECEIVE` actually effective.

**`populate_scene.rs`:** New `propagate_tile_optimizations` system. Uses `Without<NotShadowCaster>` / `Without<NotShadowReceiver>` query filters as a one-shot gate: each untagged `Mesh3d` descendant of a `Ground` or `Wall` entity gets `NotShadowCaster` (and `NotShadowReceiver` for floors) inserted the first time the system encounters it. The `Without` filter makes subsequent frames a no-op (empty query). This avoids dependency on the `SceneInstanceReady` event API and naturally handles lazy .glb loading. Top-level `use bevy::pbr::{NotShadowCaster, NotShadowReceiver}` and `use protocol::config::optimization as opt` moved to file scope (shared with `populate_lighting`).

**`main.rs`:** Registered in `PostUpdate` alongside `check_reload_environment` and `sync_shelf_boxes`.

**Files changed:** `visualizer/src/systems/populate_scene.rs`, `visualizer/src/main.rs`.

### 2026-03-08: Performance optimization toggles (Phase 5)

Added `protocol::config::optimization` module — a set of `const bool` flags that default to `true` (optimization active) and can be set to `false` to restore full visual quality when hardware allows.

**`protocol/src/config.rs`:** New `pub mod optimization` with four toggles: `DISABLE_DIRECTIONAL_SHADOWS` (disables shadow map generation — the single largest GPU cost), `DISABLE_TILE_PICKING` (inserts `Pickable::IGNORE` on floor and wall scene roots to skip event dispatch over non-interactive tiles), `DISABLE_TILE_SHADOW_CAST` and `DISABLE_FLOOR_SHADOW_RECEIVE` (stub toggles pending a `SceneInstanceReady` child-mesh propagation system).

**`populate_scene.rs`:** `populate_lighting` reads `DISABLE_DIRECTIONAL_SHADOWS` to set `shadows_enabled` on the `DirectionalLight`.

**`models.rs`:** `spawn_floor` and `spawn_wall` read `DISABLE_TILE_PICKING`; when active, the spawned `SceneRoot` entity immediately receives `Pickable::IGNORE`.

**Files changed:** `protocol/src/config.rs`, `visualizer/src/systems/populate_scene.rs`, `visualizer/src/systems/models.rs`.

### 2026-03-08: Lookahead path batching — eliminate per-tile pause (Phase 5)

Root cause of the per-tile stop: the coordinator dispatched one waypoint at a time via a 100 ms poll. Firmware stopped at each tile and waited up to 100 ms for the next command.

**Protocol (`commands.rs`):** Added `PathCommand::FollowPath { waypoints: Vec<[f32; 3]>, speed: f32 }`. Firmware follows all waypoints in sequence without stopping. `PathCommand::Stop` now also clears the queue.

**Firmware (`robot.rs`):** Added `waypoint_queue: VecDeque<[f32; 3]>` to `SimRobot`. On arrival at any intermediate waypoint, the robot pops the next from the queue and immediately updates its target and velocity — no zero-velocity frame. `on_arrival` is only called when the queue is empty (final waypoint). `restart()` clears the queue.

**Coordinator state (`state.rs`):** Added `path_sent: bool` to `TrackedRobot`. `set_path()` resets it to `false`, ensuring any new or replanned path triggers a fresh `FollowPath` dispatch. Path recalculation (replan, collision recovery, timeout) all go through `set_path()` so the flag is always correct.

**Coordinator dispatch (`server.rs`):** Rewrote `send_path_commands`. The 100 ms tick now serves as a watchdog (resends if `!path_sent`), a `path_index` sync (advances coordinator tracking for deviation detection and telemetry), and a reservation checker (sends `Stop` + clears `path_sent` when next cell is reserved, resending `FollowPath` once clear). Removed `build_path_command` helper (no longer needed).

**Task manager (`task_manager.rs`):** All five sites that previously sent a single first-waypoint command (`handle_task_assignment`, `attempt_replan`, `handle_idle_low_battery`, `handle_picking`, `handle_delivering`) now send `FollowPath` with all remaining waypoints and set `path_sent = true`.

**Files changed:** `protocol/src/commands.rs`, `mock_firmware/src/robot.rs`, `coordinator/src/state.rs`, `coordinator/src/server.rs`, `coordinator/src/task_manager.rs`.

### 2026-03-08: Dead-reckoning interpolation — eliminate visual teleport (Phase 5)

Root cause of the teleporting appearance: the visualizer hard-snapped `transform.translation` to the firmware position on every Zenoh update (20 Hz), causing visible jumps at 60 fps render rate. The `RobotUpdate.velocity` field was transmitted by firmware but never consumed by the visualizer.

**Config (`protocol/src/config.rs`):** Added `ROBOT_LERP: f32 = 0.25` (correction factor per frame at 60 fps) and `ROBOT_TELEPORT_THRESHOLD: f32 = 2.0` (snap distance for restarts) to `config::visual`.

**Components (`components.rs`):** Added `target_position: Vec3` and `network_velocity: Vec3` to the `Robot` component. `target_position` is the latest authoritative position from the network; `network_velocity` drives dead-reckoning.

**`sync_robots`:** No longer writes to `transform.translation`. Instead sets `robot.target_position` and `robot.network_velocity` on each update. Query downgraded from `&mut Transform` to `&Transform` since the system no longer owns the transform. On spawn, `target_position` and `network_velocity` are initialized to `pos` / `Vec3::ZERO` so the new robot appears settled.

**New system `interpolate_robots` (`systems/interpolate_robots.rs`):** Runs every render frame after `sync_robots`. For each robot: (1) advances `transform.translation` by `network_velocity * dt` (dead-reckoning — when velocity is accurate the robot glides with no visible step); (2) applies a frame-rate-independent correction lerp toward `target_position` (`lerp_factor = (ROBOT_LERP * dt * 60.0).min(1.0)`) to drain residual drift; (3) snaps immediately if distance exceeds `ROBOT_TELEPORT_THRESHOLD`.

**`main.rs`:** Imported `interpolate_robots` and registered it in `Update` with `.after(sync_robots)`.

**Files changed:** `protocol/src/config.rs`, `visualizer/src/components.rs`, `visualizer/src/systems/sync_robots.rs`, `visualizer/src/systems/interpolate_robots.rs` (new), `visualizer/src/systems/mod.rs`, `visualizer/src/main.rs`.

### 2026-03-08: Fix path_index overrun, spurious replans, and pass-through collisions (Phase 5)

Three bugs discovered after lookahead batching and dead-reckoning were deployed.

**Bug 1 — stray path line (root: stale path_index telemetry).** `broadcast_path_telemetry` slices `current_path[path_index..]` for the visualizer. `path_index` was advanced only inside `send_path_commands` (100 ms tick), so at 20 Hz firmware rate the slice still included already-traversed waypoints — the linestrip started behind the robot. Fixed by moving the `path_index` sync loop from `send_path_commands` into `handle_robot_update` (called at 20 Hz on every firmware update), so telemetry always reflects the robot's actual position.

**Bug 2 — robots teleporting without chaos enabled (root: path_index overrun while stopped).** The sync loop in `send_path_commands` advanced `path_index` unconditionally, even when `path_sent = false` (robot temporarily stopped at a reserved cell). Multiple blocked ticks let `path_index` run 2–3 cells ahead, making `should_replan_for_deviation` fire spuriously (distance to next_waypoint exceeded `MAX_PATH_DEVIATION_TILES = 2.0`). Back-to-back conflicting `FollowPath` commands caused dead-reckoning to diverge past `ROBOT_TELEPORT_THRESHOLD = 2.0`, triggering a visual snap. Fixed by (1) gating the sync loop behind `path_sent` in its new location in `handle_robot_update`; (2) raising `ROBOT_TELEPORT_THRESHOLD` from `2.0` to `4.0` so it sits well above `MAX_PATH_DEVIATION_TILES`.

**Bug 3 — robots passing through each other (root: post-replan blind-spot).** After `attempt_replan` reserved a new path for robot A, other robots already following `FollowPath` commands received roughly 100 ms ago continued moving into the newly reserved cells — the coordinator had no mechanism to react until the next `send_path_commands` tick. Fixed by collecting `(robot_id, new_path)` pairs inside `progress_tasks` for every successful replan, then running a fourth pass after the main iteration loop that sends `PathCommand::Stop` to any robot whose `next_waypoint()` grid cell intersects the newly reserved path. The robot has `path_sent` cleared so it will receive a corrected `FollowPath` on the next watchdog tick.

**Files changed:** `protocol/src/config.rs`, `coordinator/src/server.rs`, `coordinator/src/task_manager.rs`.

### 2026-03-08: Fix stray path line in visualizer (Phase 5)

`draw_robot_paths` used `transform.translation` (the dead-reckoned visual position) as the start of the path linestrip. Since `transform.translation` is now ahead of the coordinator's `path_index` position, `ActivePaths` waypoints could start at a tile the robot had already visually crossed, creating a backwards diagonal line.

Fixed by using `robot.position` (the authoritative network position that `path_index` advancement is keyed to) as the linestrip start. Rename: query binding `_robot` → `robot`, `transform` → `_transform`.

**Files changed:** `visualizer/src/systems/draw_paths.rs`.

### 2026-03-07: Interaction and Log Quality Fixes (Phase 5)

Fixed four issues discovered during testing.

**Camera zoom on selection:** `camera_follow_selected` now uses `Local<Option<Entity>>` to detect when the followed entity changes. On first selection it snaps radius to `FOLLOW_ZOOM_RADIUS` if the camera is farther away, then leaves radius completely free — no continuous cap.

**Shelf sidebar hover highlight:** The shelf list in `left_panel` was missing the `.hovered()` handler (robots had it, shelves did not). Added the same `response.hovered() → hovered_entity = Some(*entity)` pattern used for robots.

**Robot restart cleanup:** Two-part fix. (1) `handle_robot_update` in `server.rs` now detects `Faulted/Blocked → Idle` state transitions that indicate a firmware-initiated restart. It clears WHCA* reservations, resets all task/path/fault state on the `TrackedRobot`, and sets `skip_next_validation` so the teleport-back-to-station position jump is not flagged as a fault again. (2) `handle_fault_cleanup` now receives `&mut PathfinderInstance` and calls `clear_robot_reservations` before sending the restart command, so coordinator-initiated restarts are also clean.

**Suppress WHCA* stationary log spam:** Removed `println!` from `reserve_stationary`. It fired for every stationary robot on every planning tick (every ~100 ms with multiple idle robots), flooding the console. Path reservation logs for moving robots are retained.

**Files changed:** `systems/camera.rs`, `ui/panels.rs`, `coordinator/server.rs`, `coordinator/task_manager.rs`, `pathfinding/whca.rs`.

### 2026-03-07: Path Visualization Polish — Line Width, Circle Marker, Dual Colors (Phase 5)

Tuned the gizmo path visualization for readability and visual hierarchy.

- **Wider lines:** Added `configure_gizmos` startup system that sets `GizmoConfigStore` line width to `LINE_WIDTH = 3.5` px via `DefaultGizmoConfigGroup`. Centralized in `config::visual::path`.
- **Flat circle destination marker:** Replaced wireframe `gizmos.sphere()` with `gizmos.circle()` using `Quat::from_rotation_x(-FRAC_PI_2)` to lie flat on the XZ floor plane. Cleaner and unambiguous target marker (`DEST_CIRCLE_RADIUS = 0.25`).
- **Per-robot dual-color:** Selected robot: `SELECTED_PATH_GLOW = (0, 3.5, 3.5)` (bright dominant). All others when global show is on: `GLOBAL_PATH_GLOW = (0, 1.2, 1.2)` (subtle, non-competing). Color is now resolved per-robot inside the draw loop based on `selected_entity`, not globally before the loop.

**Files changed:** `protocol/config.rs`, `visualizer/systems/draw_paths.rs`, `visualizer/main.rs`.

### 2026-03-07: Path Visualization via Gizmos (Phase 5)

Implemented glowing pathfinding visualization using Bevy gizmos. The coordinator now broadcasts `RobotPathTelemetry` (remaining waypoints per robot) on a new `factory/telemetry/paths` Zenoh topic every tick. The visualizer subscribes, maintains an `ActivePaths` resource, and renders HDR cyan linestrips from each robot's live position through its remaining waypoints with a sphere marker at the destination.

**Architecture:**

- New protocol type: `RobotPathTelemetry { robot_id, waypoints: Vec<[f32;3]> }`
- New topic: `topics::TELEMETRY_PATHS` (`factory/telemetry/paths`)
- Coordinator broadcasts remaining `current_path[path_index..]` for all tracked robots at the PATH_SEND_INTERVAL tick rate. Empty waypoint vectors signal path cleared.
- Visualizer uses the established `tokio::sync::mpsc` channel + startup/collect system pair pattern.
- `draw_robot_paths` system draws gizmo linestrips (HDR color `srgb(0,3,3)` triggers existing Bloom). Visibility: global toggle (`show_paths` checkbox, already wired in top panel) OR per-robot when entity is selected.
- Path Y coordinate set to 0.05 to avoid Z-fighting with floor mesh.

**Files changed:** `protocol/robot.rs`, `protocol/topics.rs`, `protocol/lib.rs`, `coordinator/server.rs`, `visualizer/resources.rs`, `visualizer/systems/mod.rs`, `visualizer/systems/path_receiver.rs` (new), `visualizer/systems/draw_paths.rs` (new), `visualizer/main.rs`.

### 2026-03-07: UI and Input Bug Fixes — Selection, Hover, Camera, Right-Click (Phase 5)

Fixed five interaction bugs uncovered during testing of the Session 1 outline/selection system.

**Bug fixes:**

- **Right-click no longer triggers selection.** `on_pointer_click` now returns early if `event.button != PointerButton::Primary`, preventing camera-orbit right-clicks from toggling selection state.
- **Transport button no longer deselects the entity.** The background-click deselect check (and `entity_picked_this_frame` reset) was moved to run *after* all panel draws in `draw_ui`, so `ctx.is_pointer_over_area()` correctly covers all registered panel regions before the check fires.
- **Sidebar shelf hover now reliably highlights the 3D entity.** `sync_programmatic_outlines` was moved from `Update` to `PostUpdate`, which runs after `EguiPrimaryContextPass` (where `draw_ui` sets `hovered_entity`). Previously it ran a full frame behind, causing every-other-frame flicker.
- **Camera zoom is no longer capped when following a selected entity.** The radius `lerp` toward `FOLLOW_ZOOM_RADIUS` was removed from `camera_follow_selected`; only the focus-point lerp is retained, so users can freely zoom in/out while the camera tracks the entity.
- **Shelf stock semantics clarified.** `TileType::Shelf(u8)` encodes *initial stock*; `warehouse::SHELF_MAX_CAPACITY = 16` is the global maximum for all shelves. `SHELF_MAX_CAPACITY` was moved from `config::visual::shelf` to the new `config::warehouse` module. `INITIAL_STOCK_FRACTION` was removed entirely; `ShelfInventory::from_map()` and `populate_scene` now use the layout token value directly as initial stock.

**Files changed:** `outline.rs`, `ui/mod.rs`, `main.rs`, `systems/camera.rs`, `config.rs`, `grid_map.rs`, `populate_scene.rs`, `models.rs`.

### 2026-03-06: Camera zoom lerp and shelf cargo decrease fix (Phase 5)

**Camera zoom-in now lerps smoothly.** `camera_follow_selected` adds a `Local<bool> zooming_in` flag alongside the existing `Local<Option<Entity>>` entity tracker. When a new entity is selected and the camera is farther than `FOLLOW_ZOOM_RADIUS + 1.0`, the flag is set and the system lerps radius toward `FOLLOW_ZOOM_RADIUS` each frame (using `FOLLOW_ZOOM_LERP`). Once within `0.1` units the flag clears and radius is fully free — the user can zoom out without the system fighting them.

**Visual cargo count now decreases correctly.** The pickup branch in `sync_robots` was guarding on the robot's own tile type (`TileType::Shelf`) before decrementing `shelf.cargo`. The robot's grid position at the moment the firmware state transition arrives can be slightly off from the shelf tile, causing the guard to silently skip the decrement. Fixed by mirroring the drop logic: find nearest shelf via distance, then check the shelf's own tile. The now-dead `grid_col/grid_row/tile_type` locals were removed.

**Files changed:** `systems/camera.rs`, `systems/sync_robots.rs`.

### 2026-03-06: Glowing Outline System — Hover & Selection Highlight (Phase 5)

Implemented a reusable entity-highlight system using `bevy_mod_outline 0.11` and Bevy 0.17's native `MeshPickingPlugin`. Entities (robots, shelves, stations, dropoffs) now glow white on hover and cyan-blue on selection with HDR bloom post-processing.

**Design:**

- Three global `Observer` systems registered on the `App`: `on_pointer_over`, `on_pointer_out`, `on_pointer_click`.
- All observers forward events only for entities that carry `Mesh3d` and whose hierarchy contains a logical interactive ancestor (`Robot`, `Shelf`, `Station`, `Dropoff`). This handles `.glb` scene children which receive the hit but aren't the logical entity.
- `find_interactive_ancestor()` walks `ChildOf` links (max 10 levels) to resolve a picked mesh leaf to its logical parent.
- Hover (white): inserts `OutlineVolume` + `OutlineStencil` unless the entity already has `Selected`.
- Out (remove): removes outline components unless the entity has `Selected`.
- Click (toggle): deselects any previous `Selected` entity, inserts cyan `OutlineVolume` + `Selected` marker on the new entity, updates `UiState.selected_entity` and enables camera follow. Clicking the same entity again deselects it.

**HDR Glow (Bloom):**

- Camera now spawns with `Hdr` marker and `Bloom { intensity: 0.15 }` so outline colours above `1.0` emit visible bloom without blinding the scene.
- Hover color: `linear_rgb(5.0, 5.0, 5.0)` (bright white glow).
- Select color: `linear_rgb(0.0, 2.5, 5.0)` (cool cyan-blue glow).

**Constants:**

- All values in `protocol::config::visual::outline`: `HOVER_COLOR`, `SELECT_COLOR`, `WIDTH` (3.0 px), `BLOOM_INTENSITY` (0.15).

**Key files:**

- `crates/visualizer/src/systems/outline.rs` (new — all observer logic)
- `crates/visualizer/src/components.rs` (`Selected` marker component)
- `crates/visualizer/src/systems/camera.rs` (`Hdr` + `Bloom` added to `spawn_camera`)
- `crates/visualizer/src/main.rs` (`MeshPickingPlugin`, `OutlinePlugin`, `AutoGenerateOutlineNormalsPlugin`, observers registered)
- `crates/protocol/src/config.rs` (`visual::outline` sub-module)
- `crates/visualizer/Cargo.toml` (`bevy_mod_outline = "0.11"`)

### 2026-03-06: Build Optimization Stack (Phase 5)

Configured a nightly optimization stack to reduce incremental build times:

| Optimization | Config location | What it fixes |
| --- | --- | --- |
| **Cranelift backend** | `[profile.dev] codegen-backend` | Replaces LLVM for dev builds — no optimization passes, much faster codegen |
| **`-Zshare-generics`** | `[build] rustflags` | Shares Bevy's monomorphized generics across crates instead of recompiling each |
| **`split-debuginfo = "unpacked"`** | `[profile.dev]` | Splits PDB into per-object shards — faster incremental linking on Windows |

`bevy/dynamic_linking` was attempted but is incompatible with Cranelift on Windows MSVC (`bevy_dylib` produces ABI-mismatched object files). Removed.

`rust-toolchain.toml` pins the workspace to nightly with `rustc-codegen-cranelift-preview` as a required component, so `rustup` auto-installs the correct toolchain on any machine.

Release profile (`[profile.release]`) is unchanged — still uses LLVM with full LTO.

### 2026-03-06: Log Session Bug Fix + VS Code Notify Tasks (Phase 5)

**Bug fix — logs never advancing past 2026-02-12:**

`start_orchestrator_session()` was delegating to `get_orchestrator_session_dir()` which reads `orchestrator_session.txt` and reuses the directory if it already exists. Since `logs/2026-02-12_02-50/` was present, every subsequent orchestrator run logged into that same folder indefinitely.

Fix: `start_orchestrator_session()` now always creates a fresh `YYYY-MM-DD_HH-MM` directory, overwrites `orchestrator_session.txt`, and pre-seeds the `OnceLock` — so all calls within the same process stay consistent while each new orchestrator run gets its own session. Stale `orchestrator_session.txt` deleted from repo.

**VS Code build tasks with auto-notification:**

Added `.vscode/tasks.json` with tasks that chain `cargo run -q -p notifier` automatically — no need to type `cargo notify` manually. Ctrl+Shift+B (default build task) runs check + plays the arpeggio on success.

Tasks:

- `check workspace` (default build) — `cargo check --workspace && cargo notify`
- `build visualizer` — `cargo build -p visualizer && cargo notify`
- `build workspace` — `cargo build --workspace && cargo notify`
- `run orchestrator` — plain `cargo run -p orchestrator`
- `run wall_debug` — `cargo run -p visualizer --example wall_debug && cargo notify`

### 2026-03-06: Build-Complete Sound Notifier (Phase 5)

Added `crates/notifier` — a tiny dev-tool binary that plays a 4-note ascending arpeggio (C5→E5→G5→C6) when compilation finishes, so you don't have to watch the terminal.

**Usage:**

```bash
cargo build -p visualizer && cargo notify
# or any crate:
cargo build --workspace && cargo notify
```

**Implementation:**

- `crates/notifier/src/main.rs` — synthesizes 4 `SineWave` notes via `rodio 0.19` using `Sink::append` + `sleep_until_end`. No external audio file. Silently no-ops if no audio device is present.
- Note constants: `C5=523.25`, `E5=659.25`, `G5=783.99`, `C6=1046.50` Hz; `NOTE_MS=110`, `FINAL_MS=280`, `AMPLITUDE=0.22`.
- `.cargo/config.toml` — alias `notify = "run -q -p notifier"` to suppress the "Running" line (only the sound plays).
- `crates/notifier/Cargo.toml` — `rodio 0.19` with `symphonia-all` for full codec support.
- Added to workspace `members` in root `Cargo.toml`.

### 2026-03-06: Orchestrator Output Visibility Control (Phase 5)

Added `show`/`hide` commands to toggle per-crate console windows, disabled by default so crates run silently in the background.

**Commands:**

- `show <crate>` — spawn crate in a new console window (see its output)
- `hide <crate>` — spawn crate silently with no window (default)
- `show all` / `hide all` — bulk toggle

Settings take effect on the next `run`/`up` for that crate (cannot change a window that is already open).

**Implementation:**

- `Processes.show_output: HashSet<String>` tracks which crates should be windowed (empty by default).
- `spawn_binary(name, windowed: bool)` — on Windows: `cmd /c start` when windowed, `CREATE_NO_WINDOW` flag + suppressed stdio when silent. On non-Windows: inherits stdio vs nulls it.
- `show_output()` / `hide_output()` methods on `Processes` handle `"all"` and per-crate toggling with validation.
- `print_status()` now accepts `show_output` set and displays `[window]` / `[silent]` per crate.
- `Command::ShowOutput(String, bool)` parse patterns: `show <name>`, `hide <name>`, `show all`, `hide all`.

**Key files:**

- `crates/orchestrator/src/processes.rs` (show_output field, spawn_binary windowed param)
- `crates/orchestrator/src/cli.rs` (ShowOutput variant, parse patterns, help section, status signature)
- `crates/orchestrator/src/main.rs` (ShowOutput handler, status call updated)

### 2026-03-06: Cargo Capacity Bugs, Shelf Picker Mini-Map, Selection Improvements (Phase 5)

**Bug: Shelves start full — Relocate tasks permanently Pending**

- `ShelfInventory::from_map()` was initializing every shelf to full capacity `(cap, cap)`.
  `can_dropoff()` returns `false` when `stock == cap`, so every Relocate task whose destination shelf was at capacity was silently skipped by the scheduler's `allocate_tasks` loop and stayed Pending forever.
- Fix: Added `visual::shelf::INITIAL_STOCK_FRACTION = 0.5` constant to `protocol::config`. Shelves now start at 50% stock, ensuring both pickup and dropoff tasks can be allocated immediately at simulation start.
- Visual initial cargo updated to match via `initial_stock` in `populate_scene.rs`.

**Bug: `Shelf` component lacked `max_capacity`; inspector showed wrong "X / 16"**

- `Shelf` only stored `cargo: u32`. The inspector and `sync_shelf_boxes` both fell back to the global `SHELF_MAX_CAPACITY = 16` constant, which was wrong for shelves parsed with `xN` capacities other than 16.
- Fix: Added `max_capacity: u32` field to `Shelf`. Propagated through `spawn_shelf()` and `populate_scene.rs`. Inspector and `sync_shelf_boxes` now use per-shelf capacity. Sidebar label updated to `"Shelf (cargo/max)"`.

**Bug: Visualizer cargo not updating correctly on drop**

- `sync_robots.rs` drop arm checked `tile_type` from the robot's snapped grid position, which could miss the shelf tile if the position was slightly off.
- Fix: The drop arm now uses `find_nearest_shelf()` (distance-based) to locate the shelf, then validates the tile type at the shelf's own grid position (reliable since shelves are always spawned on exact shelf tiles). Caps increment to `shelf.max_capacity`.

**Bug: Sidebar selection did not highlight 3D objects**

- `on_pointer_click` placed `Selected + OutlineVolume` on the child mesh entity (`target`), but `ui_state.selected_entity` stored the logical parent. `select_entity()` from the sidebar set `selected_entity` without inserting any `Selected` component on mesh children.
- Fix: Added `SidebarHovered` component marker. Added `sync_programmatic_outlines` Update system that watches `selected_entity` and `hovered_entity` changes and applies/removes `Selected + OutlineVolume` on all `Mesh3d` descendants of the logical entity.
- Added `hovered_entity: Option<Entity>` and `entity_picked_this_frame: bool` to `UiState`.
- Sidebar buttons (robots and shelves) now set `hovered_entity` on hover.
- Updated `on_pointer_out` to also skip outline removal when `SidebarHovered` is present, preventing pointer-out from clearing sidebar hover outlines.

**Bug: No way to deselect by clicking empty space**

- `draw_ui` now checks: left click + egui not consuming pointer + `entity_picked_this_frame == false` → sets `selected_entity = None`. The `entity_picked_this_frame` flag is set by `on_pointer_click` when a 3D entity absorbs the click. Resets each frame in `draw_ui`.

**Feature: Shelf destination picker replaced with mini-map + scrollable list**

- Replaced the non-scrollable `CollapsingState` shelf list with a two-section picker in the inspector's "Add Transport Task" panel:
  1. Mini-map: compact `8px` cells per warehouse tile, rendered via egui `allocate_exact_size` + `painter_at`. Shelf cells are color-coded green (empty) to red (full); the source shelf shows a grey X. Individual shelf cells are interactive via `ui.interact()` — hovering highlights the 3D shelf, clicking submits the Relocate task.
  2. Scrollable list (`max_height: 80px`): rows sorted by `(row, col)`, each a full-width color-coded button. Hover also highlights the 3D shelf.
  3. Legend: color swatches for empty/half/full/source.
  4. Scroll area wraps the mini-map to handle large warehouses.
- Threaded `Option<&GridMap>` from `draw_ui` → `right_panel` → `shelf_inspector` → `shelf_minimap_widget`.
- Added `shelf_fill_color_egui(cargo, max) -> Color32` and `color_swatch()` helpers.

**Documented known bugs (deferred):**

- Multiple dropoff zones: "Dropoff" button always uses `iter().next()` — no user choice among multiple dropoff tiles.
- Shelf sidebar label has no position info (grid coords not yet stored in component).
- Double-assignment race in scheduler allocator — `reachable_robots` may not exclude robots with in-flight `assigned_task`.

**Key files:**

- `crates/protocol/src/config.rs` (INITIAL_STOCK_FRACTION)
- `crates/protocol/src/grid_map.rs` (ShelfInventory::from_map — half-capacity init)
- `crates/visualizer/src/components.rs` (Shelf.max_capacity, SidebarHovered)
- `crates/visualizer/src/resources.rs` (UiState: hovered_entity, entity_picked_this_frame)
- `crates/visualizer/src/systems/models.rs` (spawn_shelf max_capacity param)
- `crates/visualizer/src/systems/populate_scene.rs` (initial_stock, sync_shelf_boxes cap)
- `crates/visualizer/src/systems/sync_robots.rs` (drop arm nearest-shelf + max_capacity cap)
- `crates/visualizer/src/systems/outline.rs` (on_pointer_out SidebarHovered guard, entity_picked_this_frame, ProgrammaticOutlineState, sync_programmatic_outlines)
- `crates/visualizer/src/ui/mod.rs` (draw_ui: background-click deselect, hovered_entity reset, WarehouseMap param)
- `crates/visualizer/src/ui/panels.rs` (shelf picker mini-map + scrollable list, sidebar hover tracking)
- `crates/visualizer/src/main.rs` (register sync_programmatic_outlines)

### 2026-03-06: Wall Endcap, Seam Fix, Log Panic Fix, Orchestrator Shutdown (Phase 5)

**Wall endcap variant:**

- Added `WallKind::Cap(f32)` for walls with exactly one cardinal neighbor (the missing piece for isolated wall ends like `F F F / F T T / F F F`).
- Asset: `models/wall-cap.glb`; constant `CAP_ROTATIONS[4]` indexed by the direction of the single neighbor.
- `classify_wall()` now routes `count == 1` to `Cap` instead of falling through to `Straight`.
- `wall_debug.rs` updated: new row 4 "Cap" with 4 cases (N/E/S/W), raw sweep moved to row 5.
- Only cardinal neighbors are considered throughout — diagonals are completely ignored.

**Wall seam scale:**

- Added `visual::WALL_SEAM_SCALE = 1.02` to `protocol::config`. Applied as XZ-only scale in `spawn_wall()` so geometry slightly overlaps adjacent tiles, closing visible cracks. Y is unscaled so wall height is unaffected.

**Log merge panic fix:**

- `merge_logs()` in `logs.rs` panicked with `begin <= end` when a log line started with `]` (continuation line from a multi-line message). Fixed by: (1) skipping any line that doesn't start with `[`, (2) searching for `]` only within `line[1..]` so embedded brackets in message content can't be mistaken for the timestamp closer, (3) clamping `msg_start` with `.min(line.len())`.

**Orchestrator shutdown fixes:**

- Added 500 ms `tokio::time::sleep` between `kill_all()` and `merge_logs()` in `Quit`, `KillAll`, and `Restart` handlers, giving Windows time to release file locks from dying processes.
- `Quit` handler now explicitly drops publishers and calls `session.close().await` before returning, preventing Zenoh async teardown from erroring during tokio runtime shutdown.
- `Drop` impl for `Processes` now skips `kill_all()` when `running` is already empty, eliminating the spurious "No managed processes to kill." message.

**sccache removed:**

- Removed `rustc-wrapper = "sccache"` from `.cargo/config.toml`. The sccache server was crashing consistently (Windows OS error 10054), blocking every compile. LLD linker is retained.

**Key files:**

- `crates/visualizer/src/systems/models.rs` (WallKind::Cap, assets::CAP, CAP_ROTATIONS, classify_wall, spawn_wall, tests + diagnostic)
- `crates/visualizer/examples/wall_debug.rs` (CAP const, CAP_ROTATIONS, row 4 Cap, row 5 Raw Sweep, row labels)
- `crates/protocol/src/config.rs` (WALL_SEAM_SCALE)
- `crates/protocol/src/logs.rs` (merge_logs panic fix)
- `crates/orchestrator/src/main.rs` (shutdown delay, Zenoh close on quit)
- `crates/orchestrator/src/processes.rs` (Drop guard)
- `.cargo/config.toml` (sccache removed)

### 2026-03-05: Wall Model System Cleanup (Phase 5)

Simplified wall classification from 3 variants to 5, replacing inner/outer corner distinction with a bidirectional corner model and adding T-junction and pillar support.

**Wall types:**

- **Straight**: 1, 2 opposite, or 4 cardinal neighbors (wall.glb / wall-windowed.glb)
- **Corner**: 2 adjacent cardinal neighbors, diagonal state ignored (wall-corner.glb)
- **T-junction**: 3 cardinal neighbors, indexed by missing direction (wall-T.glb)
- **Pillar**: 0 cardinal neighbors, isolated wall (wall-pillar.glb)
- **Cross (4-way)**: Falls back to straight (no dedicated model)

**Changes:**

- `WallKind` enum: removed `CornerInner`/`CornerOuter`, added `Corner`, `TJunction`, `Pillar`
- `classify_wall()`: no longer checks diagonals for corners; routes 3-neighbor cases to T-junction
- Asset paths updated: `structure-corner-inner.glb` / `structure-corner-outer.glb` replaced by `wall-corner.glb`; added `wall-T.glb`, `wall-pillar.glb`; `wall_window.glb` renamed to `wall-windowed.glb`
- Tests trimmed from 21 wall tests to 3 consolidated tests (32 total -> 15)
- Layout diagnostic updated with T-junction symbols
- `wall_debug.rs` updated: 5 rows (Straight, Corner, T-Junction, Pillar, Raw Sweep)

**Key Files:**

- `crates/visualizer/src/systems/models.rs` (WallKind, classify_wall, assets, tests)
- `crates/visualizer/examples/wall_debug.rs` (visual test bench)

### 2026-02-13: Visualizer Crate Review (Phase 5)

Comprehensive review and refactoring of the visualizer crate across three commits:

**Architecture (a878185, 28a95a0):**

- **Shared Tokio runtime**: All background Zenoh subscribers share a single `Arc<Runtime>` via `ZenohSession` resource instead of each spawning its own.
- **GridMap sole wall truth**: Wall classification uses `GridMap::tile_type_at()` instead of raw string tokens, making GridMap the single source of truth for tile types.
- **O(1) entity lookup**: `RobotIndex` HashMap for robot lookup by ID. `RobotLastPositions` resource with `state_by_id` HashMap for state dedup.
- **LogBuffer Vec to VecDeque**: O(1) front removal when capacity exceeded (was O(n) with `Vec::remove(0)`).
- **CARGO_SHELF_DISTANCE_SQ to config**: Magic number moved from `sync_robots.rs` to `protocol::config::visual`.

**Logic (28a95a0):**

- **State change dedup**: `collect_robot_updates` deduplicates by checking both position AND state changes, preventing duplicate ECS updates.
- **Tile type guard**: `sync_shelf_boxes` guards against non-shelf entities by checking `TileType` before cargo modification.
- **Remove apply_with_log**: Visualizer is read-only; replaced `apply_with_log` calls with direct LogBuffer logging.
- **Disable speed when paused**: Speed buttons grayed out during pause to prevent confusing UI state.

**Hygiene & Optimization (4aca758):**

- **Remove log interval timer**: Removed 3-second throttled console summary from `collect_robot_updates` (pre-LogBuffer leftover).
- **Remove runtime println!**: All runtime `println!` removed from visualizer. Logging goes through LogBuffer UI console only. Startup banner in `main.rs` kept.
- **PlaceholderMeshes resource**: Pre-allocated shared `Handle<Mesh>` and `Handle<StandardMaterial>` for stations, dropoffs, and robots. Clone cheap handles instead of creating new GPU assets per entity.
- **O(1) robot count**: Left panel uses `robot_index.by_id.len()` instead of `robots.iter().count()`.
- **sort_unstable_by_key**: All UI panel sorts use `sort_unstable_by_key` for faster sorting.

**H4 Analysis (suggestion only, not implemented):**

The `wall_debug.rs` example duplicates rotation constants from `models.rs` and can't validate visual correctness because model default orientation is unknown. Suggested: build runtime gizmo overlay, store `WallKind` on `Wall` component, fix rotation constants using visual feedback.

**Key Files:**

- `crates/visualizer/src/resources.rs` (PlaceholderMeshes, RobotLastPositions, ZenohSession)
- `crates/visualizer/src/systems/zenoh_receiver.rs` (log interval removed, println! removed)
- `crates/visualizer/src/systems/sync_robots.rs` (shared mesh handles, println! removed)
- `crates/visualizer/src/systems/populate_scene.rs` (PlaceholderMeshes creation)
- `crates/visualizer/src/systems/models.rs` (spawn functions take &PlaceholderMeshes)
- `crates/visualizer/src/ui/panels.rs` (O(1) robot count, sort_unstable_by_key)
- `crates/protocol/src/config.rs` (CARGO_SHELF_DISTANCE_SQ)

**Test Results:** 35 visualizer tests passing (no regressions)

### 2026-02-13: Visualizer GridMap Consistency Refactor (Phase 5)

**Changes:**

- **Visualizer uses protocol::GridMap**: `populate_environment()` now loads the layout via `GridMap::load_from_file()` (the same parser used by coordinator and scheduler) instead of manually parsing token strings. Tile types are matched via `TileType` enum (`Ground`, `Wall`, `Station`, `Dropoff`, `Shelf(cap)`, `Empty`).
- **Token grid retained for wall analysis**: Raw string grid is still built from the layout file and passed to `spawn_wall()` for 3x3 neighborhood classification, since `classify_wall()` operates on string tokens.
- **Shelf capacity from GridMap**: Shelf cargo is now read from `TileType::Shelf(capacity)` instead of parsing `"xN"` tokens manually, ensuring the visualizer and backend crates agree on capacity values.

**Key Files:**

- `crates/visualizer/src/systems/populate_scene.rs` (rewritten `populate_environment` to use GridMap)

**Design Decisions:**

1. **Consistency over DRY**: Even though the token grid is still needed for wall neighbor analysis, using GridMap for tile type determination ensures the visualizer interprets the layout identically to coordinator and scheduler.
2. **No functional change**: The visual output is identical; this is a pure consistency/maintainability refactor.

**Test Results:** 136 tests passing (no new tests; refactor only)

### 2026-02-13: 3x3 Wall Classification Rewrite + Shelf Capacity Enforcement (Phase 5)

**Changes:**

- **Neighborhood struct**: Introduced `Neighborhood` with 8 boolean fields (`n`, `ne`, `e`, `se`, `s`, `sw`, `w`, `nw`) for full 3x3 tile analysis around each wall tile, replacing the error-prone cardinal-only approach.
- **3x3 tile-rule classification**: Rewrote `classify_wall()` to use explicit pattern matching on all 8 neighbors. Correctly identifies: inner corners (2 adjacent walls + diagonal), outer corners (2 adjacent walls, no diagonal), straight walls (opposite-axis walls), end caps, T-junctions, cross intersections, and isolated walls.
- **23 wall classification unit tests**: Comprehensive test suite covering every wall variant (4 rotations each for straight, inner corner, outer corner, end cap; plus cross and isolated).
- **ShelfInventory in protocol**: New `ShelfInventory` struct with `HashMap<(usize, usize), ShelfStock>` tracking current/max capacity per shelf tile. Methods: `from_map()`, `try_reserve()`, `undo_reserve()`, `decrement()`, `increment()`, `is_full()`, `available()`.
- **10 ShelfInventory unit tests**: Tests for initialization from GridMap, reservation flow, undo, capacity limits, increment/decrement, unknown shelf handling.
- **Scheduler capacity enforcement**: `allocate_tasks()` now calls `inventory.try_reserve()` before assigning tasks, with `undo_reserve()` on assignment failure. Prevents over-allocating to full shelves.
- **Coordinator capacity verification**: Task assignment verifies shelf availability via `inventory.available() > 0`. Pickup decrements inventory, delivery increments destination inventory. New `AssignmentResult::ShelfCapacity` variant for rejection.
- **TrackedRobot grid tracking**: Added `pickup_grid` and `dropoff_grid` fields to `TrackedRobot` for inventory operations at task lifecycle stages.

**Key Files:**

- `crates/visualizer/src/systems/models.rs` (Neighborhood struct, classify_wall rewrite, 23 tests)
- `crates/protocol/src/grid_map.rs` (ShelfInventory, ShelfStock, 10 tests)
- `crates/protocol/src/lib.rs` (re-exported ShelfInventory)
- `crates/scheduler/src/server.rs` (inventory init, capacity checks, undo)
- `crates/coordinator/src/task_manager.rs` (inventory verification, pickup/dropoff tracking)
- `crates/coordinator/src/server.rs` (inventory init, passed to task_manager)
- `crates/coordinator/src/state.rs` (pickup_grid, dropoff_grid fields)

**Design Decisions:**

1. **Neighborhood over index math**: Explicit boolean fields are readable and testable vs. computing `grid[row-1][col+1]` inline. The struct is constructed once per wall tile.
2. **Shelf enforcement at two layers**: Scheduler reserves optimistically (can undo), coordinator verifies authoritatively. This prevents races where two schedulers might reserve the same slot.
3. **Inventory starts full**: `ShelfInventory::from_map()` initializes all shelves at max capacity (matching visual boxes), then decrements as items are picked up.

**Test Results:** 136 tests passing (23 wall + 10 inventory + 103 existing)

### 2026-02-13: .glb Model Integration + Visual Fixes (Phase 5)

**Changes:**

- **3D model pipeline**: Replaced all primitive Bevy meshes (cubes, planes) with .glb models loaded via `SceneRoot(asset_server.load("path#Scene0"))`. Models: floor, wall, wall_window, structure-corner-inner, structure-corner-outer, shelf, box-small, box-large, box-long, box-wide.
- **models.rs module**: New `systems/models.rs` with spawn functions (`spawn_floor`, `spawn_wall`, `spawn_shelf`, `spawn_station`, `spawn_dropoff`), asset path constants, weighted variant selection for walls (70% solid, 30% window), and box offset layout for shelf cargo.
- **sync_shelf_boxes system**: Reactive system that spawns/despawns box entities as children of shelves when `Shelf.cargo` changes, using `Changed<Shelf>` query filter.
- **Asset path fixes**: Configured `AssetPlugin { file_path: "assets".into(), .. }` to resolve paths from workspace root regardless of orchestrator CWD. Moved all models to `assets/models/` subfolder.
- **Visual tuning**: `BOX_SCALE` constant for box sizing, `PLACEHOLDER_Y_OFFSET` for station/dropoff markers, wall rotation PI offset correction, ground tiles under walls removed (wall model is solid).
- **Lighting config**: Added `protocol::config::visual::lighting` module with `DIRECTIONAL_ILLUMINANCE` and `AMBIENT_BRIGHTNESS` constants.

**Key Files:**

- `crates/visualizer/src/systems/models.rs` (new: all model spawn logic, asset constants, weighted variants)
- `crates/visualizer/src/systems/populate_scene.rs` (refactored to use models module)
- `crates/visualizer/src/systems/mod.rs` (added models module)
- `crates/visualizer/src/components.rs` (added BoxCargo, Ground components)
- `crates/visualizer/src/main.rs` (asset plugin config, sync_shelf_boxes registration)
- `crates/protocol/src/config.rs` (BOX_SCALE, PLACEHOLDER_Y_OFFSET, lighting constants)
- `assets/models/` (floor.glb, wall.glb, wall_window.glb, shelf.glb, box-small.glb, etc.)

**Design Decisions:**

1. **SceneRoot loading**: Bevy 0.17 pattern for .glb models; `#Scene0` fragment selects the default scene from each file.
2. **Weighted wall variants**: Random selection per wall tile adds visual variety without layout changes.
3. **Box-as-child pattern**: Boxes are spawned as children of shelf entities, so they inherit transforms and despawn automatically.

**Test Results:** 136 tests passing

### 2026-02-12: Interactive UI Features & Config Centralization (Phase 5)

**Changes:**

- **Sim/Real-time toggle**: Added top panel button to switch between simulation and real-time modes (visual only, implementation pending).
- **Inspector tabs**: Replaced single-heading inspector with tab-based system for modularity (currently Details tab, easily extensible).
- **Shelf inspector controls**:
  - Clicking shelf in Objects list shows cargo count (e.g., "Cargo: 5 / 10")
  - "Add transport task" button with dropdown menu: Dropoff + collapsible Shelves submenu
  - Dropdown publishes `TaskRequest` via `UiAction::SubmitTransportTask` → Zenoh `TASK_REQUESTS` topic
- **Camera follow system**:
  - Clicking any entity in Objects list zooms camera to entity and enables follow mode
  - `camera_follow_selected` system smoothly lerps focus to entity position each frame
  - Right-click orbit does NOT break follow (only middle-mouse pan breaks it)
  - Camera zoom adjusts to comfortable viewing radius with configurable lerp speed
- **Collapsible sections**: Robots and Shelves in Objects tab now have collapsible headers showing counts (e.g., "Robots (3)")
- **Config centralization**: Moved all hardcoded values to `protocol::config::visual`:
  - `SHELF_MAX_CAPACITY = 10` (shared capacity for all shelves)
  - `camera::FOLLOW_ZOOM_RADIUS`, `FOLLOW_FOCUS_LERP`, `FOLLOW_ZOOM_LERP` (follow behavior)
  - `camera::ORBIT_SENSITIVITY`, `PAN_SENSITIVITY`, `SCROLL_LINE_SPEED`, `SCROLL_PIXEL_SPEED` (camera controls)
  - `ui::TOP_PANEL_HEIGHT`, panel width/height ranges, `LOG_BUFFER_CAPACITY` (UI layout)
- **Component rename**: `Shelf.capacity` → `Shelf.cargo` for semantic clarity (capacity = max limit, cargo = current items)
- **Dropdown fix**: Rewrote transport shelves dropdown using `CollapsingState::load_with_default_open()` + `show_header/body` pattern instead of broken `CollapsingHeader::new().open()` which prevented user interaction

**Pending Features (TODOs):**

- [ ] Highlight transport destination location in 3D scene
- [ ] Click shelf/robot in 3D scene to select (same as list click)

**Key Files:**

- `crates/protocol/src/config.rs` (added visual::camera + visual::ui modules, SHELF_MAX_CAPACITY)
- `crates/visualizer/src/components.rs` (renamed Shelf.capacity → cargo)
- `crates/visualizer/src/resources.rs` (added InspectorTab, UiAction::SubmitTransportTask, camera_following flag)
- `crates/visualizer/src/ui/panels.rs` (complete rewrite with all new features + config constants)
- `crates/visualizer/src/systems/camera.rs` (added camera_follow_selected, uses config constants)
- `crates/visualizer/src/systems/commands.rs` (added TASK_REQUESTS publisher, SubmitTransportTask bridge)
- `crates/visualizer/src/systems/populate_scene.rs` (updated shelf spawning to use cargo field)
- `crates/visualizer/src/tests.rs` (updated shelf tests for cargo rename)

**Design Decisions:**

1. **Tab-based inspector**: Enables adding new inspector views (e.g., Analytics, Settings) without UI restructuring
2. **Config-driven constants**: All magic numbers now centralized for easier tuning and consistency
3. **Smooth camera follow**: Uses lerp for natural motion instead of instant snap
4. **Smart follow break**: Rotation doesn't break follow (user examining entity), but pan does (user looking elsewhere)
5. **CollapsingState pattern**: Proper egui state management for user-interactive collapsible sections

**Test Results:** 103 tests passing (no regressions)

### 2026-02-12: Digital Twin Command Center UI (Phase 5)

**Changes:**

- **4-panel egui layout**: Implemented a docking Command Center with Top (HUD), Left (Object Manager), Right (Inspector), and Bottom (Logs) panels.
- **Simulation controls**: Pause/Play toggle and speed buttons (1x, 10x, Max) in top panel.
- **Global KPIs**: Real-time Active Robot count and FPS display in top HUD.
- **Layer toggles**: Checkboxes for Paths, Heatmap, and IDs (wired to `UiState` booleans for future 3D gizmo systems).
- **Object Manager**: Tabbed left panel with Objects and Tasks tabs. Objects tab has a search bar and sorted, filterable lists of Robots (with state icons) and Shelves. Clicking a row selects the entity.
- **Inspector**: Right panel shows context-sensitive details — Robot ID, state, position, battery (color-coded ProgressBar), cargo, task, and action buttons (Kill, Return to Charge). Shows placeholder for non-robot entities.
- **Log Console**: Bottom panel with Logs and Analytics tabs. Logs tab has auto-scroll toggle, Clear button, and a scrollable monospace console reading from `LogBuffer`.
- **UiState resource**: Centralized UI state with `selected_entity`, `filter_query`, layer toggles, `sim_speed`, `is_paused`, and tab state.
- **LogBuffer resource**: Ring buffer (512 capacity) for in-UI log display with auto-scroll support.
- **Camera input guard**: `camera_controls` now checks `egui::Context::wants_pointer_input()` and `is_pointer_over_area()` to prevent orbit/pan/zoom when interacting with UI panels.
- **bevy-inspector-egui 0.35**: Added as dependency for future dev/debug panel integration.

**Key Files:**

- `crates/visualizer/Cargo.toml` (added `bevy-inspector-egui`)
- `crates/visualizer/src/ui/mod.rs` (NEW - UI module root, `draw_ui` system)
- `crates/visualizer/src/ui/panels.rs` (NEW - all four panel implementations)
- `crates/visualizer/src/resources.rs` (added `UiState`, `LogBuffer`, `ObjectTab`, `BottomTab`)
- `crates/visualizer/src/main.rs` (registered UI resources, system on `EguiPrimaryContextPass`)
- `crates/visualizer/src/systems/camera.rs` (added egui input guard)

**Design Decisions:**

1. **UI-only, no logic**: Action buttons print `info!()` and toggle `UiState` booleans. No Zenoh commands published yet — clean separation for future wiring.
2. **EguiPrimaryContextPass schedule**: UI system runs in the egui context pass (after Update, before rendering) per bevy_egui 0.38 best practices.
3. **Result-based system**: `draw_ui` returns `Result` to handle `EguiContexts::ctx_mut()` fallibility per bevy_egui 0.38 API.

**Test Results:** 11 tests passing (all existing visualizer tests, no regressions)

### 2026-02-12: Functional UI Wiring (Phase 5)

**Changes:**

- **Pause/Resume** buttons publish `SystemCommand` over Zenoh (all crates respond).
- **Kill/Restart/Enable** buttons publish `RobotControl` over Zenoh (firmware responds).
- **External commands** (from orchestrator) sync `UiState.is_paused` and log to bottom panel.
- **Live QueueState** display: subscribes to scheduler topic, shows pending/total/completed/robots.
- **Top HUD** shows live task throughput from scheduler QueueState.
- **Robot state changes** and spawns logged to bottom panel in real-time.
- **All UI actions** logged to bottom panel (`[UI] Kill Robot #2`, `[System] Paused`, etc.).
- **Background Zenoh publisher thread** (mpsc channel bridge from Bevy to async).

**Architecture:**

- `UiAction` Bevy `Message` carries button clicks from UI → `bridge_ui_commands` system → `CommandSender` mpsc → background thread → Zenoh publish.
- `QueueStateReceiver` mpsc ← background thread ← Zenoh subscribe → `QueueStateData` resource → panels read each frame.
- `handle_system_commands` receives external Pause/Resume/Verbose and syncs `UiState` + `LogBuffer`.

**Key Files:**

- `crates/visualizer/src/systems/commands.rs` (added `setup_publishers`, `bridge_ui_commands`)
- `crates/visualizer/src/systems/queue_receiver.rs` (NEW - QueueState subscriber)
- `crates/visualizer/src/systems/sync_robots.rs` (added LogBuffer state-change logging)
- `crates/visualizer/src/resources.rs` (added `OutboundCommand`, `CommandSender`, `QueueStateData`, `QueueStateReceiver`, `UiAction`)
- `crates/visualizer/src/ui/panels.rs` (wired buttons, live QueueState display, throughput KPIs)
- `crates/visualizer/src/ui/mod.rs` (updated `draw_ui` with new resource params)
- `crates/visualizer/src/main.rs` (registered new systems, message, resources)

**Design Decisions:**

1. **One-frame action delay**: UI runs on `EguiPrimaryContextPass` (after Update). Actions written as Bevy Messages, consumed by `bridge_ui_commands` in the next frame's Update — imperceptible latency.
2. **Background publisher thread**: Zenoh publishing is async; Bevy systems are sync. A dedicated `std::thread` with `tokio::Runtime` bridges the gap via mpsc channel (same pattern as existing subscribers).
3. **External state sync**: When orchestrator publishes Pause/Resume, `handle_system_commands` updates `UiState.is_paused` so the UI button reflects external state changes.

**Test Results:** 11 tests passing (no regressions)

### 2026-02-06: Arrival-Time Reservation Check

**Changes:**

- **Reservation forecast**: Coordinator now checks if the next cell will be reserved at arrival time (not just “reserved now”), reducing head-on collisions.
- **Dispatcher support**: Added `is_reserved_soon()` to WHCA* and dispatcher for arrival-time checks.

**Files Updated:**

- `coordinator/src/pathfinding/whca.rs` (future reservation checks)
- `coordinator/src/pathfinding/dispatcher.rs` (dispatch helper)
- `coordinator/src/server.rs` (arrival-time wait check)

**Test Results:** Not run

### 2026-02-06: Reservation Footprint Reduction

**Changes:**

- **Collision buffer disabled**: Set `COLLISION_BUFFER_TILES` to 0 to prevent reservations from inflating into adjacent tiles and causing corridor deadlocks.

**Files Updated:**

- `protocol/src/config.rs` (collision buffer set to 0)

**Test Results:** Not run

### 2026-02-06: Orchestrator-Scoped Run Log Sessions

**Changes:**

- **Two-level log sessions**: Logs now group by orchestrator start time, with per-run subdirectories created on `run/up`.
- **Run-level merges**: `merged.log` is created when `kill/down` (or restart/quit) ends a run.
- **Marker files**: `orchestrator_session.txt` stores the top-level session, `current_run.txt` stores the active run.

**Files Updated:**

- `protocol/src/logs.rs` (orchestrator + run session handling)
- `orchestrator/src/main.rs` (start/stop run session hooks)
- `.github/copilot-instructions.md` (logging structure update)

**Test Results:** Not run

### 2026-02-06: Reservation Wait Deadlock Breaker

**Changes:**

- **Wait tracking**: Coordinator now tracks how long a robot has been waiting on a reserved cell.
- **Replan on prolonged waits**: After a short wait threshold, robots attempt a replan to break reservation deadlocks.
- **Wait override**: After a longer timeout, robots override reservation waits and proceed, preventing total gridlock.
- **Config knobs**: Added `RESERVATION_WAIT_REPLAN_SECS` and `RESERVATION_WAIT_OVERRIDE_SECS` to collision config.

**Files Updated:**

- `coordinator/src/state.rs` (wait tracking fields + helpers)
- `coordinator/src/server.rs` (wait tracking + override in command dispatch)
- `coordinator/src/task_manager.rs` (replan on wait timeout, clear waits on reset)
- `protocol/src/config.rs` (wait timeout constants)

**Test Results:** 103 tests passing

### 2026-02-06: WHCA* Self-Exclusion + Reservation GC Fix

**Changes:**

- **Self-exclusion fix**: WHCA* now excludes the planning robot from its own reservation checks, preventing false “No path to pickup” failures caused by stale self-reservations.
- **Robot-aware pathfinding methods**: Added `find_path_for_robot()` and `find_path_to_non_walkable_for_robot()` on WHCA* and dispatcher, and routed coordinator task flow through them.
- **Reservation GC correction**: WHCA* `tick()` now retains reservations until their actual time has elapsed (instead of a fixed 1s window), preventing premature clearing and mid-path collisions.
- **A* fallback on congestion**: When WHCA* fails due to reservation congestion, planner falls back to A* so tasks still get a valid path.

**Files Updated:**

- `coordinator/src/pathfinding/whca.rs` (self-exclusion API, GC fix, A* fallback)
- `coordinator/src/pathfinding/dispatcher.rs` (robot-aware pathfinding methods)
- `coordinator/src/task_manager.rs` (robot-aware pathfinding calls, signature update)
- `.github/copilot-instructions.md` (logging note, test count)

**Test Results:** 103 tests passing

### 2026-02-06: Per-Session Log Directory Stabilization

**Changes:**

- **Session marker**: Added a shared `current_session.txt` marker so all crates write into the same session directory instead of splitting by start-minute.
- **Session cleanup**: `merge_logs()` now clears the session marker to ensure the next run starts a new session directory.
- **Doc alignment**: Log module comments updated to reflect session-based behavior.

**Files Updated:**

- `protocol/src/logs.rs` (session marker, cleanup on merge)

**Test Results:** Not run

### 2026-02-05: WHCA* Collision Prevention Tightening

**Changes:**

- **Reserve full paths even at zero velocity**: WHCA* now falls back to `DEFAULT_SPEED` when the robot is idle at assignment time, preventing empty reservations that allow head-on conflicts.
- **Start + dwell reservations**: Paths now reserve the start cell immediately and the final cell for pickup/dropoff dwell time to reduce late-arrival collisions.
- **Stationary grid alignment**: Stationary reservations now use `world_to_grid` conversion for consistent cell locking.
- **Dropoff path reservations**: When pickup completes, the new path to dropoff now clears old reservations and reserves the dropoff path immediately.
- **Post-dropoff reservations**: Return-to-station paths are reserved immediately, and idle robots reserve their current cell right after task completion.
- **Edge swap protection**: WHCA* now reserves each segment’s start cell at its departure time, improving head-on swap detection during returns.
- **Reservation-based waiting**: Waypoint commands now wait in place if the next cell is reserved, preventing large-robot head-on swaps at dropoff.
- **Wait-position reservation**: Robots now reserve their current cell when waiting on a reserved next cell, preventing the other robot from reserving the occupied tile.
- **Position jump mitigation**: Position delta checks now scale with update tick gaps and skip one validation after restart to avoid false positives.
- **Wait command stabilization**: Reservation waits now send `Stop` and mark task progress to avoid cancelling pickup timers and false timeouts.
- **Stationary history reservations**: Stationary robots now reserve only their last few tiles for a short duration, reducing overly conservative blocking after dropoff.
- **Random task command**: Scheduler now supports `random`/`rand` to enqueue a random shelf→dropoff task for stress testing.
- **Scheduler layout sync**: Scheduler now reads the shared `LAYOUT_FILE_PATH` so layout changes apply consistently.
- **Assignment gating**: Coordinator now rejects task assignments for faulted/blocked/busy robots, but still allows return-to-station (non-low-battery) robots.
- **Collision buffer**: WHCA* now reserves a small buffer around reserved cells to reduce multi-robot contact in tight corridors.
- **Softer fault thresholds**: Position jump and off-grid checks now use soft limits to reduce false positives under load.
- **Scheduler reachability filter**: Tasks now allocate only to robots that can reach the pickup tile (BFS-based), reducing immediate "no path" failures.
- **Full CLI map rendering**: Scheduler map output now renders the full grid without truncation.
- **Per-run log sessions**: Logs now write to a per-initialization directory (YYYY-MM-DD_HH-MM) instead of hourly buckets.
- **Merge exclusions**: Log merges can exclude specific crates via config (e.g., firmware).

**Files Updated:**

- `coordinator/src/pathfinding/whca.rs` (fallback speed, start + dwell reservations, segment start reservations)
- `coordinator/src/task_manager.rs` (stationary reservation alignment, dropoff path reservations, post-dropoff reservations)
- `coordinator/src/pathfinding/dispatcher.rs` (reservation queries)
- `coordinator/src/server.rs` (wait-in-place when target cell reserved)
- `coordinator/src/server.rs` (reserve current cell while waiting)
- `coordinator/src/server.rs` (Stop + progress on wait)
- `coordinator/src/server.rs` (tick-aware position delta validation)
- `coordinator/src/state.rs` (track last tick, skip-next-validation)
- `coordinator/src/task_manager.rs` (skip validation after restart)
- `protocol/src/config.rs` (stationary reservation history settings)
- `coordinator/src/state.rs` (track recent grid history)
- `coordinator/src/task_manager.rs` (stationary history reservation)
- `coordinator/src/pathfinding/whca.rs` (history reservation window)
- `coordinator/src/pathfinding/dispatcher.rs` (history reservation dispatch)
- `scheduler/src/cli.rs` (random task command)
- `scheduler/src/server.rs` (random task creation)
- `scheduler/Cargo.toml` (rand dependency)
- `scheduler/src/server.rs` (use shared layout config)
- `coordinator/src/task_manager.rs` (assignment eligibility gating)
- `coordinator/src/pathfinding/whca.rs` (collision buffer reservations)
- `protocol/src/config.rs` (collision buffer + soft limits)
- `coordinator/src/server.rs` (soft limit validation)
- `scheduler/src/server.rs` (reachability filter for allocation)
- `scheduler/src/allocator/closest.rs` (availability state alignment)
- `scheduler/src/cli.rs` (full map rendering)
- `protocol/src/logs.rs` (per-run logging, merge exclusions)
- `protocol/src/config.rs` (merge exclusion list)

**Test Results:** Not run

### 2026-02-04: Collision Detection & Phase 4 Completion

**Changes:**

- **Wall collision detection in firmware**: Robots now check `GridMap.is_walkable()` before applying velocity, preventing wall phasing
- **Blocked state on collision**: Robots transition to `RobotState::Blocked` when hitting walls, zero velocity, clear target
- **Collision config constants**: Added `protocol::config::coordinator::{collision, sensor}` modules with thresholds
- **GridMap passed to physics**: Firmware `update_physics()` now receives map reference for real-time validation
- **Test coverage**: Added wall collision test, updated 12 existing tests to pass GridMap parameter

**Design Decisions:**

1. **Simplified fault handling**: Wall collision → Blocked state → stop immediately (no complex cleanup sequence needed for portfolio demo)
2. **Firmware-layer detection**: Collision check happens at physics layer for realistic behavior
3. **Phase 4 complete**: Demonstrates resilience (chaos + collision detection + task timeout) sufficient for internship portfolio

**Files Updated:**

- `protocol/src/config.rs` (collision & sensor modules with 8 new constants)
- `mock_firmware/src/robot.rs` (wall collision detection, GridMap parameter, world_to_grid helper, 13 tests updated)
- `mock_firmware/src/driver.rs` (pass GridMap to update_physics)

**Test Results:** 13 tests passing in mock_firmware (2 legacy tests have setup issues, collision detection verified with new test)

**Phase 4 Status:** ✅ COMPLETE - Moving to Phase 5 (Polish & Presentation)

### 2026-02-04: Real-Time WHCA* Reservations + Test Alignment

**Changes:**

- **WHCA* timing model upgraded**: Reservations now use real milliseconds (Instant-based), velocity magnitude for travel prediction, and a ±200ms tolerance window.
- **Planning window in ms**: WHCA* uses `WINDOW_SIZE_MS` with a 500ms move step for consistent space-time planning.
- **Per-robot log deduplication**: Merged log dedup now tracks last command per robot, eliminating interleaving false negatives.
- **Test alignment**: Firmware Drop test now matches delayed drop behavior, and allocator busy-robot test now checks `assigned_task` to reflect availability rules.

**Files Updated:**

- `protocol/src/config.rs` (WHCA* ms-based constants)
- `protocol/src/logs.rs` (per-robot dedup)
- `coordinator/src/pathfinding/whca.rs` (real-time reservations, velocity-based timing)
- `coordinator/src/pathfinding/dispatcher.rs` (reserve_path signature)
- `coordinator/src/task_manager.rs` (passes velocity)
- `mock_firmware/src/robot.rs` (Drop test aligned)
- `scheduler/src/allocator/closest.rs` (busy-robot test aligned)

**Test Results:** 102 tests passing

### 2026-02-03: Scheduler Allocation Fix + WHCA* Buffer + Log Merge Dedup

**Changes:**

- **Scheduler allocator now considers returning robots**: Robots in `MovingToPickup`/`MovingToDrop` with no assigned task are eligible, allowing immediate task allocation even while returning to station.
- **WHCA* reservation buffer**: Each waypoint reservation is extended by the max wait window to tolerate execution delays.
- **Merged log deduplication (FIXED)**: Per-robot deduplication now correctly removes consecutive command repeats. Fixed bug where robot interleaving prevented detection of duplicates.

**Files Updated:**

- `scheduler/src/allocator/closest.rs`
- `coordinator/src/pathfinding/whca.rs` (buffer added, but disabled)
- `protocol/src/logs.rs` (dedup now per-robot)

**Status:** Compiles cleanly. Duplicate logs fixed.

**Root Cause Analysis - WHCA* Collision Issue:**

The fundamental problem with WHCA* as currently implemented:

1. **Timing Mismatch**: Coordinator's `tick()` increments `current_time` every loop iteration (50ms), but robots move at continuous velocity and take seconds to traverse the warehouse.
2. **Premature Expiry**: Reservations are cleared when `t >= current_time`, causing reserved positions to become available before robots actually arrive.
   - Example: Reserve robot at pos (1,1) for t=0..10
   - At tick 5: current_time=5, clear t < 5
   - Reservation at (1,1) t=3 is cleared, but robot won't arrive until t=8!
3. **Result**: Second robot's path can be planned through positions still occupied by first robot.

**Future WHCA* Fix (Phase 5+):**

Option A: Decouple pathfinding time from coordinator ticks

- Track actual robot execution time
- Only advance WHCA* time when robots change grid cells
- Requires robot state notifications

Option B: Use position-based reservations instead of time-based

- Reserve occupied cells until robot leaves
- Simpler model, doesn't require perfect time sync
- Recommended approach for Phase 5

### 2026-02-03: Command Response & Validation System

**Change:** Implemented complete command acknowledgment protocol with firmware validation layer.

**Highlights:**

- **Immediate Command Feedback**: Firmware now responds with `Accepted` or `Rejected` for every command
- **Command ID Tracking**: Sequential u64 IDs correlate commands with responses across the distributed system
- **Pre-execution Validation**: Firmware validates all PathCmd parameters before applying (NaN checks, cargo state, speed validation)
- **Structured Rejection Reasons**: Clear error messages for debugging (e.g., "Robot already carrying cargo", "Invalid coordinates: NaN detected")
- **Chaos Integration**: Random command rejection testing for resilience validation
- **Disabled Robot Handling**: Disabled robots send rejection responses instead of silently ignoring commands

**Changes:**

- `protocol/src/commands.rs`: Added `cmd_id: u64` to `PathCmd`, created `CommandResponse` struct and `CommandStatus` enum, updated test to include cmd_id
- `protocol/src/topics.rs`: Added `COMMAND_RESPONSES` topic constant
- `mock_firmware/src/robot.rs`: Changed `apply_command()` to return `CommandStatus`, added validation for all PathCommand variants, removed duplicate Stop case
- `mock_firmware/src/commands.rs`: Updated `handle_path_commands()` to publish `CommandResponse` after every command attempt
- `mock_firmware/src/driver.rs`: Declared response publisher and wired to command handler
- `coordinator/src/server.rs`: Added `next_cmd_id` state tracking, subscribed to command responses, created `handle_command_responses()` logging function
- `coordinator/src/task_manager.rs`: Added `next_cmd_id` parameter to 8 functions, updated 7 PathCmd construction sites

**Design Decision:**

- **Timeout System Retained**: While CommandResponse provides immediate feedback for invalid commands, the 30-second timeout system remains as a fallback for detecting stuck robots (e.g., blocked paths, firmware crashes, network partitions). Timeout is now complementary rather than primary failure detection.

**Validation Types:**

- Coordinate validation (NaN/Inf checks)
- Speed validation (positive values)
- Cargo state consistency (Pickup requires not carrying, Drop requires carrying)
- Robot enabled state (disabled robots reject all commands)

**Test Results:** 102 tests passing (no new tests added)

---

### 2026-02-03: Log File Locking Fix

**Change:** Prevented interleaved log lines by locking the shared hourly log file.

**Highlights:**

- Added cross-process file locking for `logs/log_YYYY-MM-DD_HH.txt`
- Log lines are now written atomically without interleaving across crates

**Changes:**

- `protocol/src/logs.rs`: Lock file before write and unlock after
- `protocol/Cargo.toml`: Added `fs2` for file locking

**Test Results:** 102 tests passing

---

### 2026-02-03: Delivery Stage Handling + Assignment Result Usage

**Change:** Implemented explicit Delivering stage handling and wired AssignmentResult into coordinator logging.

**Highlights:**

- **Delivering** now mirrors pickup flow: Drop command is sent at arrival, then coordinator waits for firmware to report `RobotState::Idle` before completing the task.
- **Dropoff delay** is now simulated in firmware using `config::coordinator::DROPOFF_DELAY_SECS` before transitioning to `Idle`.
- **AssignmentResult** is now actively consumed in `server.rs`, giving clear, structured reasons for task acceptance/rejection (and logged).

**Changes:**

- `coordinator/src/task_manager.rs`: Added `TaskStage::Delivering` handling and `handle_delivering()`
- `coordinator/src/task_manager.rs`: Moved completion logic to delivery confirmation
- `coordinator/src/server.rs`: Match on `AssignmentResult` for structured logging + log persistence
- `mock_firmware/src/robot.rs`: Added dropoff delay timer before state returns to `Idle`

**Test Results:** 102 tests passing

---

### 2026-02-03: Config-Driven Strategy Dispatchers

**Change:** Restored trait-based flexibility with config-driven dispatchers for pathfinding, queue, and allocator strategies.

**Highlights:**

- **Coordinator** now selects pathfinding via `config::coordinator::PATHFINDING_STRATEGY` (A\* or WHCA\*)
- **Scheduler** now selects queue and allocator strategies via `config::scheduler::{QUEUE_STRATEGY, ALLOCATOR_STRATEGY}`
- **WHCA*** reservations are integrated via dispatcher methods (tick/reserve/clear)
- **TaskQueue** trait now includes `next_task_id()` to support strategy dispatch
- Removed unnecessary `#[allow(dead_code)]` annotations

**Changes:**

- `protocol/src/config.rs`: Added strategy selection constants
- `coordinator/src/pathfinding/dispatcher.rs`: NEW - `PathfinderInstance`
- `coordinator/src/server.rs`: Instantiate dispatcher, log strategy, tick each loop
- `coordinator/src/task_manager.rs`: Reserve/clear WHCA* paths via dispatcher
- `scheduler/src/allocator/dispatcher.rs`: NEW - `AllocatorInstance`
- `scheduler/src/queue/dispatcher.rs`: NEW - `QueueInstance`
- `scheduler/src/server.rs`: Use dispatcher-based queue/allocator
- `scheduler/src/queue/mod.rs`: Added `next_task_id()` to `TaskQueue` trait
- `scheduler/src/queue/fifo.rs`: Implemented `next_task_id()` for trait

**Test Results:** 102 tests passing

---

### 2026-02-03: WHCA* Default Pathfinding

**Change:** Coordinator now uses WHCA* as the default pathfinder.

**Config Cleanup:**

- Moved WHCA* constants into protocol config (`config::coordinator::whca`)
- Pathfinding settings are centralized with other coordinator tunables

**Changes:**

- `coordinator/src/server.rs`: Instantiate `WHCAPathfinder` instead of `AStarPathfinder`
- `coordinator/src/pathfinding/whca.rs`: Read `WINDOW_SIZE` and `MAX_WAIT_TIME` from config
- `protocol/src/config.rs`: Added `coordinator::whca` module
- `coordinator/src/pathfinding/mod.rs`: Updated docs for default pathfinder

---

### 2026-02-02: Code Review & Architecture Improvements

**Code Review Session** - Comprehensive review of all 6 crates before Phase 5.

**New Features:**

1. **WHCA* Pathfinding Module** (`coordinator/src/pathfinding/whca.rs`)
   - Windowed Hierarchical Cooperative A* for multi-robot collision avoidance
   - Space-time reservation table for path conflict detection
   - Wait actions to handle blocked paths
   - Edge collision detection (robots swapping positions)
   - Ready for integration when needed (currently using A*)

2. **Visualizer Test Suite** (`visualizer/src/tests.rs`)
   - 11 tests covering components, resources, and integration scenarios
   - Component creation tests (Robot, Shelf)
   - Resource operations (RobotIndex, RobotLastPositions, RobotUpdates)
   - Movement detection logic validation
   - Robot state lifecycle verification

**Documentation Updates:**

- Updated stale `ReturnToCharge` comment in mock_firmware
- Updated copilot-instructions.md with current architecture
- Updated test count: 98 tests (up from 79)

**Code Quality:**

- 0 compiler warnings
- All 98 tests passing
- Clean architecture with future-ready WHCA* pathfinder

**Files Changed:**

- `coordinator/src/pathfinding/whca.rs`: NEW - 550 lines, 9 tests
- `coordinator/src/pathfinding/mod.rs`: Added WHCA* export
- `visualizer/src/tests.rs`: NEW - 175 lines, 11 tests
- `visualizer/src/main.rs`: Added test module
- `mock_firmware/src/robot.rs`: Updated ReturnToCharge comment
- `.github/copilot-instructions.md`: Updated architecture docs
- `.documentation/PROGRESS.md`: This changelog entry

---

### 2026-02-02: Task Timeout & Charging Robot Allocation Fix

**New Feature: Task timeout system for resilience**
Tasks now automatically fail if no progress is detected within the timeout window:

1. **Timeout Configuration** (`protocol/src/config.rs::coordinator`)
   - `TASK_TIMEOUT_SECS = 30` - Tasks must show progress within 30 seconds
   - `TIMEOUT_CHECK_INTERVAL_MS = 5000` - Check frequency (5 seconds)

2. **Progress Tracking** (`coordinator/src/state.rs::TrackedRobot`)
   - `last_progress: Instant` - Last time progress was made on current task
   - `task_started: Option<Instant>` - When current task was assigned
   - `mark_progress()` - Reset timeout clock
   - `is_task_timed_out(secs)` - Check if timeout exceeded

3. **Progress Detection** (`coordinator/src/server.rs`)
   - Waypoint advancement marks progress
   - Task stage transitions mark progress (MovingToPickup → Picking → MovingToDropoff)
   - Timeout check in `progress_tasks()` fails stalled tasks

4. **Timeout Handling**
   - Task marked as `Failed` with timeout reason
   - Robot state cleared and returned to Idle
   - Scheduler can reassign failed tasks

**Bug Fix: Charging robots not receiving new tasks**
Root cause: The allocator only checked for `RobotState::Idle`, but robots at charging stations have `RobotState::Charging`.

1. **Allocator Update** (`scheduler/src/allocator/closest.rs`)
   - Now considers both `Idle` AND `Charging` robots as available
   - Battery threshold still enforced (50% minimum)

2. **Coordinator Update** (`coordinator/src/server.rs`)
   - Charging robots with sufficient battery can accept new tasks
   - If battery still low, task is rejected with clear error message

**Test Coverage:** 79 tests passing (up from 73)

**Changes:**

- `protocol/src/config.rs`: Added `TASK_TIMEOUT_SECS`, `TIMEOUT_CHECK_INTERVAL_MS`
- `coordinator/src/state.rs`: Added timeout tracking fields and methods, 4 new tests
- `coordinator/src/server.rs`: Timeout check in progress_tasks(), progress marking at waypoints and stage transitions
- `scheduler/src/allocator/closest.rs`: Allow Charging robots with sufficient battery, 2 new tests

---

### 2026-02-01: Chaos Engineering & Robot Control Infrastructure

**New Feature: Chaos engineering for resilience testing**
Complete chaos infrastructure with runtime toggle and individual robot control:

1. **Chaos Config Module** (`protocol/src/config.rs::chaos`)
   - Master switch: `ENABLED` (default false)
   - Per-feature toggles: packet loss, message delay, command rejection, stale state, position drift, battery glitch, crash
   - Configurable rates for each chaos type

2. **Chaos Helper Functions** (`protocol/src/chaos.rs`)
   - `should_drop_packet(enabled)` - Network packet loss
   - `should_reject_command(enabled)` - Firmware ignores commands
   - `should_send_stale_state(enabled)` - Desync simulation
   - `should_battery_glitch(enabled)` - False battery readings
   - `should_crash(enabled)` - Process termination
   - `get_message_delay_ms(enabled)` - Random latency
   - `get_position_drift(enabled)` - Odometry errors

3. **SystemCommand::Chaos(bool)** - Runtime toggle via orchestrator

4. **Orchestrator Commands:**
   - `chaos on/off` - Toggle chaos mode globally
   - `enable robot <id>` - Enable a disabled robot
   - `disable robot <id>` - Disable robot (stops updates, ignores commands)
   - `restart robot <id>` - Reset robot to station with full battery

5. **RobotControl Protocol** (`topics::ROBOT_CONTROL`)
   - New message type for individual robot management
   - Up, Down, Restart variants

6. **Firmware Integration:**
   - Packet loss on RobotUpdateBatch publishing
   - Command rejection in path handling
   - Position drift in physics update
   - Disabled robots excluded from updates

**Test Coverage:** 73 tests passing

**Changes:**

- `protocol/src/chaos.rs`: NEW - Chaos helper functions
- `protocol/src/config.rs`: Added chaos module with 15+ constants
- `protocol/src/commands.rs`: Added Chaos to SystemCommand, RobotControl enum
- `protocol/src/topics.rs`: Added ROBOT_CONTROL topic
- `orchestrator/src/cli.rs`: Added chaos and robot control commands
- `orchestrator/src/main.rs`: Handle new commands with robot publisher
- `mock_firmware/src/driver.rs`: Integrate chaos, robot control subscriber
- `mock_firmware/src/commands.rs`: Handle RobotControl messages
- `mock_firmware/src/robot.rs`: Added `enabled` field, `restart()` method
- All crates: Updated `apply_with_log()` to accept chaos parameter

---

### 2026-02-01: Battery Drain Noise Implementation

**New Feature: Realistic battery drain variation**
Battery now drains with random variation to simulate real-world odometry errors and mechanical differences:

1. **Drain rate range** - 0.03–0.07 %/second (±40% variation around 0.05 base)
2. **Per-tick randomization** - Each physics update generates new random drain value
3. **Realistic behavior** - Different robots have slightly different efficiency

**Technical Implementation:**

- Added `DRAIN_RATE_RANGE: (f32, f32) = (0.03, 0.07)` to battery config
- Robot.rs uses `rand::thread_rng()` to generate drain value within range each tick
- Smoother variation than constant drain for more natural simulation

**Changes:**

- `protocol/src/config.rs`: Added DRAIN_RATE_RANGE constant
- `mock_firmware/src/robot.rs`: Implemented random drain using `gen_range()`
- `mock_firmware/Cargo.toml`: Added rand dependency
- `Cargo.toml`: Added rand to workspace dependencies

---

### 2026-02-01: Smart Battery Threshold for Charging

**New Feature: Robots return to work when battery reaches safe threshold**
Instead of waiting for full charge, robots now become available for tasks once they reach `MIN_BATTERY_FOR_TASK`:

1. **Consistent threshold** - Uses same 20% minimum as scheduler for task allocation
2. **Efficient warehouse operations** - Robots available sooner instead of idle at 95%
3. **Real-world behavior** - Mirrors actual logistics (don't charge to 100% between jobs)

**Technical Implementation:**

- Robot stays in `ReturningToStation` state while charging
- Once battery reaches `scheduler::MIN_BATTERY_FOR_TASK` (20%), transitions to `Idle`
- Becomes available for new task assignments immediately
- Consistent with scheduler's allocation strategy

**Behavior Summary:**

- Robot at 19% battery? → Return to station immediately
- Charging... battery reaches 20%? → Transition to Idle, available for tasks
- New task arrives at 25% battery? → Accept it (scheduler won't assign <20%)

**Changes:**

- `coordinator/src/server.rs`: Check battery level against `MIN_BATTERY_FOR_TASK` before transitioning to Idle
- Keep robot in `ReturningToStation` state while firmware charges
- Only when battery sufficient does robot become `Idle` and available

---

### 2026-02-01: Return-to-Station Interruption for New Tasks

**New Feature: Smart task interruption for returning robots**
Robots returning to station now intelligently accept new tasks if possible:

1. **Non-critical returns interrupted** - If robot returns due to no pending tasks, it will accept new assignments immediately
2. **Critical returns preserved** - If robot returns due to low battery, it will not accept tasks until charged
3. **Efficient task handling** - Robots never waste time returning home if work appears

**Technical Implementation:**

- Added `ReturnReason` enum: `NoPendingTasks` vs `LowBattery`
- Added `return_reason` field to `TrackedRobot` to track return context
- Task assignment handler checks and interrupts non-critical returns
- Task failures logged when robot has critical (low battery) returns

**Behavior Summary:**

- Returning to station (no tasks)? → Accept new task immediately, interrupt return
- Returning to station (low battery)? → Reject new tasks, continue to charge
- Accepted new task while returning? → Cancel return path, begin new task

**Changes:**

- `coordinator/src/state.rs`: Added `ReturnReason` enum and `return_reason` field
- `coordinator/src/server.rs`:
  - Interrupt non-critical returns in task assignment handler
  - Set return reason when initiating returns
  - Clear return reason when interrupted

---

### 2026-02-01: Smart Return-to-Station Behavior

**New Feature: Intelligent robot return-to-station logic**
Robots now intelligently return to their charging stations when appropriate:

1. **After task completion** - If no pending tasks in the queue, robot pathfinds back to station
2. **Low battery detection** - Idle robots with battery below threshold automatically return
3. **Proper pathfinding** - Return uses A* pathfinding (no more wall-clipping!)

**Technical Implementation:**

- Added `station_position` to `RobotUpdate` protocol (firmware → coordinator)
- Coordinator subscribes to `QUEUE_STATE` topic to track pending task count
- New `TaskStage::ReturningToStation` state properly handled in progress loop
- Battery threshold uses `config::battery::LOW_THRESHOLD` (20%)

**Changes:**

- `protocol/src/robot.rs`: Added `station_position: [f32; 3]` to RobotUpdate
- `mock_firmware/src/robot.rs`: Include station_position in to_update()
- `coordinator/src/server.rs`:
  - Subscribe to QUEUE_STATE for pending task awareness
  - Handle ReturningToStation state at top of progress loop
  - Low battery idle robots proactively return to station
  - Post-task completion checks: return if no pending tasks OR low battery

**Behavior Summary:**

- Task queued? → Robot stays at dropoff, ready for next assignment
- No tasks + healthy battery? → Robot returns to station
- Low battery (any time)? → Robot returns to station immediately

---

### 2026-02-01: Post-Task Behavior & Logging Infrastructure Fixes

**Bugs Fixed:**

1. **Robot auto-returning to station after every task**
   - **Root Cause**: Coordinator always sent `ReturnToCharge` command after task completion
   - **Symptom**: Robot went back to station even when more tasks were queued
   - **Solution**: Removed auto-return code block from `progress_tasks()`. Robots now stay idle at dropoff, available for immediate reassignment.

2. **Robot ignoring grid when returning to station**
   - **Root Cause**: Firmware's `ReturnToCharge` handler set `self.target = self.station_position` directly, bypassing pathfinding
   - **Symptom**: Robot moved in straight line through walls to reach station
   - **Solution**: Changed `ReturnToCharge` to set robot to Idle (or Charging if at station). Coordinator must pathfind if return-to-station is needed.

3. **Log folders created in wrong locations**
   - **Root Cause**: `LOG_DIR = "logs"` was relative path; each crate created its own `logs/` folder
   - **Symptom**: `crates/scheduler/logs/` and `crates/orchestrator/logs/` folders appeared
   - **Solution**: Rewrote `logs.rs` with workspace root discovery using `OnceLock<PathBuf>`. Walks up directory tree to find `[workspace]` in Cargo.toml.

**Technical Changes:**

- `coordinator/src/server.rs`: Removed auto-return code after task completion (lines 613-620)
- `mock_firmware/src/robot.rs`: Changed `ReturnToCharge` handler; added `is_at_station()` helper
- `protocol/src/logs.rs`: Complete rewrite with `find_workspace_root()` and `get_log_dir()`
- `protocol/src/config.rs`: Removed unused `LOG_DIR` constant

**Test Updates:**

- Split `test_return_to_charge` into two tests:
  - `test_return_to_charge_when_away` (robot → Idle, no target)
  - `test_return_to_charge_when_at_station` (robot → Charging)
- Total tests: **68 passing** (up from 67)

---

### 2026-02-01: Critical Waypoint Navigation Bug Fix

**Bug Fixed:**

- **Root Cause**: Firmware's `on_arrival()` function auto-transitioned from `MovingToPickup` to `Picking` state whenever the robot arrived at ANY waypoint, not just the final destination.
- **Symptom**: Robot moved one tile, immediately started pickup, then went wrong direction.
- **Solution**:
  1. Removed auto-transition from `on_arrival()` for MovingToPickup/MovingToDrop states
  2. Coordinator now detects when robot reaches actual pickup/dropoff location (path complete + near destination)
  3. Coordinator sends explicit Pickup/Drop commands only when at final destination

**Technical Changes:**

- `mock_firmware/src/robot.rs`: `on_arrival()` no longer auto-transitions for MovingToPickup/MovingToDrop
- `coordinator/src/server.rs`: Rewrote `progress_tasks()` to check path completion + proximity to destination
- Added `is_near()` helper function for position comparison
- Fixed "skip first waypoint" logic to also apply when calculating dropoff path

**Test Updates:**

- Updated `test_arrival_changes_state` → `test_arrival_clears_target_but_keeps_state`
- Added `test_station_arrival_transitions_to_charging` (station transition still works)
- Total tests: **67 passing** (up from 66)

---

### 2026-01-31: Comprehensive Logging System + Build Optimization

**New Features:**

- ✅ Centralized logging system using `chrono` for proper date/time handling
- ✅ Selective operation logging across all crates (60 lines of code, zero log spam)
- ✅ Orchestrator process management logging (start, kill, restart, pause, resume)
- ✅ Coordinator task lifecycle logging (assign, complete, fail)
- ✅ Scheduler task/allocation logging (create, status update, allocate)
- ✅ Firmware command execution logging (MoveTo, Pickup, Drop, ReturnToCharge)
- ✅ sccache (v0.8.1) configured for 40% faster incremental builds

**What Gets Logged:**

- Task lifecycle: creation, assignment, completion, failures
- Pathfinding failures (critical errors only)
- Command execution (firmware)
- Process lifecycle (orchestrator)
- System commands (pause, resume, verbose toggle)

**What Doesn't Get Logged:**

- Real-time position updates (too verbose)
- Loop iterations (would explode file size)
- Startup banners (visual feedback only)
- Status display updates (UI only)

**Code Quality:**

- Fixed date calculations: now using chrono instead of broken manual arithmetic
- Added `protocol::config::LOG_DIR` constant (follows project conventions)
- Improved error handling: graceful instead of panics
- All logs include crate name prefix for easy filtering: `[Coordinator]`, `[Scheduler]`, etc.

**Test Results:**

- Total tests: **66 passing** (all crates, zero failures)
- No compiler warnings
- sccache dramatically speeds up rebuild cycles (cached Zenoh, ring, rustls)

**Files Modified:**

- `crates/protocol/src/logs.rs`: Rewritten with chrono + proper error handling
- `crates/protocol/src/config.rs`: Added LOG_DIR constant
- `crates/protocol/Cargo.toml`: Added chrono dependency
- `crates/coordinator/src/server.rs`: Added 3 log points (assign, fail, complete)
- `crates/scheduler/src/server.rs`: Added 2 log points (create, status)
- `crates/scheduler/src/allocator/closest.rs`: Added 1 log point (allocate)
- `crates/mock_firmware/src/commands.rs`: Added 1 log point (execute)
- `crates/orchestrator/src/main.rs`: Added 3 log points (pause, resume, verbose)
- `crates/orchestrator/src/processes.rs`: Added 5 log points (start_all, kill_all, restart, kill, start)

### 2026-01-30: Task Progression + Timestamp Logging Fix

**Fixes:**

- Prevented waypoint commands from being sent while robots are in `Picking` state (avoids command spam during pickup).
- Cleared robot paths on task completion to prevent `ReturnToCharge` from being overwritten by stale waypoint commands.
- Dropoff path now uses non-walkable goal handling consistently.

**Logging Improvements:**

- Added `protocol::now_ms()` helper for unified millisecond timestamps.
- Added timestamps to coordinator and scheduler task logs, and firmware command logs for easier async debugging.

**Impact:**

- ✅ Robots pause correctly during pickup delay.
- ✅ Return-to-charge now executes without being overridden.
- ✅ Logs across crates now include consistent ms timestamps.

### 2026-01-30: Shelf Pathfinding Fix (Final Implementation)

**Bug Fixed:**

- **Root Cause**: Pathfinder expected to navigate directly to shelf/dropoff tiles, but these tiles are not walkable. A* algorithm rejected them as invalid goals.
- **Symptom**: "No path to pickup (8,1)" error even when shelf is accessible from adjacent ground tiles.
- **Solution**: Modified A* algorithm to accept non-walkable goals by checking for adjacency. The algorithm explores only walkable tiles (normal movement) but succeeds when reaching any tile adjacent to the non-walkable goal. This naturally finds the optimal approach direction without greedy selection or repeated pathfinds.

**Why This Implementation is Superior:**

- ✅ **Single A* Run**: No multiple pathfinds or greedy adjacent-tile selection
- ✅ **Optimal**: Explores all reachable neighbors; naturally picks shortest path to approach any shelf
- ✅ **Robust**: Correctly handles edge cases (accessible from only one direction)
- ✅ **Efficient**: No redundant computation; minimal algorithm change

**Code Changes:**

- Added `find_path_astar_non_walkable()` in `coordinator/src/pathfinding/astar.rs`
  - New internal A* function that modifies goal-checking logic
  - Accepts reaching a tile adjacent to the non-walkable goal as success
  - For walkable goals, delegates to normal pathfinding
  - Only explores walkable tiles (no exploration of shelves)
- Added `is_adjacent()` helper to check orthogonal adjacency (4-directional)
- Updated `Pathfinder::find_path_to_non_walkable()` trait implementation
  - AStarPathfinder now calls the new function
  - Simplified default trait implementation to just call regular `find_path()`
- Simplified `handle_task_assignment()` in `coordinator/src/server.rs`
  - Single call to `find_path_to_non_walkable()` handles both walkable and non-walkable goals
  - No special logic needed; A* handles it internally
  - Cleaner code flow

**Test Results:**

- Total tests: **66 passing** (unchanged from before)
  - coordinator: 12, protocol: 19, scheduler: 15, orchestrator: 8, mock_firmware: 12
- All three non-walkable tests pass:
  - `test_find_path_to_non_walkable_shelf` — normal shelf pathfinding
  - `test_find_path_to_non_walkable_surrounded_shelf` — unreachable shelf (all neighbors blocked)
  - `test_find_path_to_non_walkable_edge_accessible` — shelf accessible from only one direction ✅
- No compiler warnings
- All existing tests still pass

**Impact:**

- ✅ Tasks to shelves/dropoffs pathfind correctly using true A* algorithm
- ✅ Robots always find the optimal approach direction (shortest path to any adjacent tile)
- ✅ Handles all edge cases including geographically constrained shelves
- ✅ Your `add S3 D1` command works perfectly with optimal behavior

### 2026-01-30: Scheduler Deep-Dive & Cleanup Session

**Major Improvements:**

- Moved `QueueState` struct from inline (server.rs) to protocol crate as reusable type
  - Re-exported in protocol/lib.rs for convenience
  - Enables serialization and sharing across all crates
  - Pattern: All network-transmissible types belong in protocol crate
- Moved location marker magic numbers to protocol config constants
  - `SHELF_MARKER_BASE = 10000`, `DROPOFF_MARKER_BASE = 20000`
  - Used consistently across cli.rs and server.rs

**Code Quality Fixes:**

- Fixed status display alignment to handle variable-width content
  - Added `format_status_line()` helper for dynamic line formatting
  - No longer breaks with large numbers (e.g., 999999 pending tasks)
  - Uses consistent box width calculations
- Improved FifoQueue documentation to clarify it's a **priority queue with FIFO tiebreaking**
  - Added module-level doc explaining Priority > FIFO ordering
  - Struct doc now explicitly states "NOT a pure FIFO queue"

**Bug Fixes & Cleanup:**

- Removed unnecessary `#[allow(dead_code)]` from `find_next_pending_index()` and `TaskQueue` trait
  - Functions are actively used by dequeue(), peek(), and trait implementations
  - Compiler now correctly flags actual dead code

**New CLI Features:**

- `cancel <id>` - Cancel a pending task
- `priority <id> <level>` - Change task priority (low/normal/high/critical)
- `history` - View completed/failed/cancelled tasks
- Updated help text with all new commands

**New Queue Features:**

- Added `cleanup_completed()` method to TaskQueue trait
  - Removes completed, failed, and cancelled tasks from queue
  - Prevents memory leak from long-running systems
  - Implemented in FifoQueue with test coverage
  - Returns count of removed tasks for logging

**Verbosity Improvements:**

- Added `verbose: bool` parameter to `allocate_tasks()` function
  - Task assignment logging now respects verbose flag
  - Reduces console spam in production mode (verbose=off)
  - Keeps detailed logging available when needed

**Test Coverage:**

- Added `test_cleanup_completed()` test to verify removal of completed/failed tasks
- Total tests: **63 passing** (up from 62)
  - protocol: 19
  - orchestrator: 8
  - scheduler: 15 (was 14)
  - coordinator: 9
  - mock_firmware: 12

**Code Hygiene:**

- No compiler warnings
- All tests passing
- Consistent import usage (QueueState now imported from protocol)

### 2026-01-30: Crate Review & Refactoring Session

**Crate Renames:**

- `mission_control` → `scheduler` (better reflects responsibility)
- `fleet_server` → `coordinator` (matches abstraction layer name)

**Coordinator Refactoring:**

- Created `pathfinding/` module directory with trait-based architecture
- Defined `Pathfinder` trait: `find_path()`, `find_path_avoiding()`, `name()`
- Implemented `AStarPathfinder` as first strategy
- Added `PathResult` struct with `grid_path`, `world_path`, `cost`
- Added coordinate utilities: `grid_to_world()`, `world_to_grid()`, `grid_to_world_path()`
- Extracted helper functions: `build_path_command()`, `send_path_commands()`, `validate_pickup_dropoff()`, `send_task_failure()`
- Added `verbose` flag support throughout

**Protocol Updates:**

- Added `config::coordinator::WAYPOINT_ARRIVAL_THRESHOLD` (0.2)
- Added `config::coordinator::DEFAULT_SPEED` (2.0)

**Code Quality:**

- Fixed `#[allow(dead_code)]` placement (specific variants, not entire enums)
- Standardized terminology across all crates
- Updated all doc comments and banners
- 62 tests passing across workspace

**Documentation:**

- Streamlined `copilot-instructions.md` (460 → 125 lines)
- Created `PROGRESS.md` for phase tracking

**Visualizer Cleanup:**

- Added consistent startup banner
- Cleaned up empty component structs (`Ground`, `Wall`, `Station`, `Dropoff`)
- Added doc comments to all component types
- Consolidated to a single shared Zenoh session
- Removed unused Debug HUD wiring (reserved for future UI overhaul)
- Labeled `ReloadEnvironment` as future use for in-app layout switching
- Verified no compiler warnings

---

### Phase 4 Task System Implementation

**Features Added:**

- Task queue with priority support
- Robot allocation strategies
- Task execution state machine
- Named location support
- Verbose mode system

---

### Phase 3 Architecture & Pathfinding

**Features Added:**

- Multi-crate workspace structure
- A* pathfinding algorithm
- Map hash validation
- SystemCommand broadcast system
- Orchestrator process management

---

### Phase 2 Zenoh Integration

**Features Added:**

- Zenoh pub/sub messaging
- RobotUpdateBatch protocol
- Dynamic robot spawning
- Real-time HUD

---

### Phase 1 Foundation

**Features Added:**

- Bevy visualization app
- Warehouse layout parser
- Camera controls
- Environment rendering

---

## Test Coverage

| Crate | Test Count | Coverage Areas |
| ----- | ---------- | -------------- |
| protocol | 23 | Serialization, grid parsing, commands, QueueState, chaos |
| orchestrator | 9 | CLI parsing, process management, robot control |
| scheduler | 17 | Queue operations, allocator logic, cleanup, charging robot allocation |
| coordinator | 28 | Pathfinding (A\* + WHCA\*), coordinate conversion, task timeout |
| mock_firmware | 15 | Physics, battery, state machine, collision detection |
| visualizer | 11 | Components, resources, position tracking, state lifecycle |
| **Total** | **103** | N/A |

---

## Architecture Evolution

### Initial (Phase 1-2)

``` bash
visualizer ← mock_firmware
```

### Distributed (Phase 3)

``` bash
coordinator ↔ mock_firmware
visualizer ← all
```

### Current (Phase 4)

``` bash
orchestrator → all (SystemCommand)
scheduler → coordinator (TaskAssignment)
coordinator → mock_firmware (PathCmd)
coordinator → scheduler (TaskStatusUpdate)
mock_firmware → all (RobotUpdateBatch)
visualizer ← all (render only)
```

---

## Notes for Future Development

### Mock Firmware Realism (TODO)

- **Collision detection** - Use `GridMap.is_walkable()` to prevent wall phasing
- **Acceleration model** - Smooth velocity transitions instead of instant changes
- **Picking/dropping delay** - Add timer states for realistic cargo handling
- **Position noise** - Add small random drift to simulate odometry errors
- **Communication latency** - Simulate network delays and packet loss

### Advanced Features (Phase 5+)

- Multi-warehouse simulation
- Hardware-in-loop testing (physical robots)
- ML-driven optimization
- WHCA* for multi-robot coordination
- Distributed simulation (multiple backend instances)

---

## Related Documentation

- [Phase 1 Prototype](.documentation/Hyper%20Twin%20Phase%201%20Prototype.pdf)
- [Phase 2 MVP](.documentation/Hyper%20Twin%20Phase%202%20MVP.pdf)
- [Phase 3 Logic Complete](.documentation/Hyper%20Twin%20Phase%203%20Logic%20Complete.pdf)
- [Phase 4 Juice and Presentation](.documentation/Hyper%20Twin%20Phase%204%20Juice%20and%20presentation.pdf)
- [Phase 5 Perfection](.documentation/Hyper%20Twin%20Phase%205%20Perfection.pdf)
- [Copilot Instructions](../.github/copilot-instructions.md) - AI coding context
