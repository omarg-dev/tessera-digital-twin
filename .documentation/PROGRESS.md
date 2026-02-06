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

| Phase | Name                                      | Status        | Description                                               |
| ------ | ------------------------------------------ | -------------- ------------------------------------------------------------ |
| 1     | Foundation & Scene Setup                  | ✅ Complete   | Bevy app, warehouse layout, camera, environment           |
| 2     | Zenoh Integration & Robot Sync            | ✅ Complete   | Real-time robot updates, dynamic spawning, HUD            |
| 3     | Distributed Architecture & Pathfinding    | ✅ Complete   | Multi-crate architecture, A* pathfinding, map validation  |
| 4     | Task & Cargo Management                   | ✅ Complete   | Task queue, allocation, execution loop, collision detection  |
| 5     | Polish & Presentation                     | ⏳ Planned    | Performance optimization, UI polish, demo scenarios       |
| 6     | ROS2 Bridge & Hardware Validation         | ⏳ Planned    | External physics integration, real robot support          |

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

## Phase 4: Task & Cargo Management 🔄

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
- [x] Battery drain noise (realistic variation 0.03-0.07 %/sec)
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

## Phase 5: Polish & Presentation ⏳

**Goal:** Production-ready demo for portfolio/internship showcase.

**Planned Features:**

- [ ] Performance metrics dashboard
- [ ] Performance optimization (benchmark 1000+ robots)
- [ ] Cargo/package entity tracking (visual cargo on robots)
- [ ] UI polish (better HUD, status panels)
- [ ] Demo scenarios (scripted warehouse operations)
- [ ] Video recording / GIF generation
- [ ] README with architecture diagrams
- [ ] Documentation cleanup

**Future Firmware Enhancements:**

These improvements would increase simulation realism but are LOW priority (Phase 5+):

- [ ] **Non-linear Battery Model** (Low priority)
  - Exponential discharge curves (faster drain at low SOC)
  - Temperature-dependent efficiency
  - Load-dependent drain (carrying cargo = higher drain)
  - **Current**: Linear drain with random variation (0.03-0.07 %/sec)
  - **Impact**: More realistic battery behavior for logistics planning

- [ ] **Error State Recovery** (Medium priority, Phase 5)
  - Add `Blocked` state when robot cannot progress toward waypoint
  - Add `Faulted` state for simulated hardware failures
  - Automatic retry logic (backoff, alternate paths)
  - Diagnostic reporting to coordinator
  - **Current**: Robots either succeed or timeout (30s no-progress)
  - **Impact**: Better mirrors real AMR behavior with recoverable errors

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
| coordinator | 24 | Pathfinding (A\* + WHCA\*), coordinate conversion, task timeout |
| mock_firmware | 14 | Physics, battery, state machine |
| visualizer | 11 | Components, resources, position tracking, state lifecycle |
| **Total** | **98** | N/A |

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
