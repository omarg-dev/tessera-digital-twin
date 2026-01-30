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
|-------|-------------------------------------------|---------------|-----------------------------------------------------------|
| 1     | Foundation & Scene Setup                  | ✅ Complete   | Bevy app, warehouse layout, camera, environment           |
| 2     | Zenoh Integration & Robot Sync            | ✅ Complete   | Real-time robot updates, dynamic spawning, HUD            |
| 3     | Distributed Architecture & Pathfinding    | ✅ Complete   | Multi-crate architecture, A* pathfinding, map validation  |
| 4     | Task & Cargo Management                   | 🔄 InProgress | Task queue, allocation, execution loop, cargo tracking    |
| 5     | Polish & Presentation                     | ⏳ Planned    | Performance optimization, UI polish, demo scenarios       |

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
- [x] ClosestIdleAllocator (assigns tasks to nearest idle robot)
- [x] Robot state machine: `Idle → MovingToPickup → Picking → MovingToDropoff → Delivering`
- [x] Automatic task progression (coordinator monitors state, sends next PathCmd)
- [x] TaskStatusUpdate messages (coordinator → scheduler)
- [x] Named location commands (`add S1 D1` instead of coordinates)
- [x] Location listing (`list shelves`, `list dropoffs`, `list stations`)
- [x] ASCII map display (`map` command)
- [x] Global verbose mode (`verbose on/off` from orchestrator)
- [x] Shared `SystemCommand.apply_with_log()` in protocol crate

**Pending Features:**
- [ ] Cargo/package entity tracking (visual cargo on robots)
- [ ] Order completion status in scheduler UI
- [ ] Performance metrics dashboard
- [ ] Multi-robot collision avoidance (WHCA*)

**Key Files:**
- `crates/scheduler/src/queue/fifo.rs`
- `crates/scheduler/src/allocator/closest.rs`
- `crates/coordinator/src/state.rs`
- `crates/protocol/src/tasks.rs`

---

## Phase 5: Polish & Presentation ⏳

**Goal:** Production-ready demo for portfolio/internship showcase.

**Planned Features:**
- [ ] Performance optimization (benchmark 1000+ robots)
- [ ] UI polish (better HUD, status panels)
- [ ] Demo scenarios (scripted warehouse operations)
- [ ] Video recording / GIF generation
- [ ] README with architecture diagrams
- [ ] Documentation cleanup

---

## Changelog

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
|-------|------------|----------------|
| protocol | 19 | Serialization, grid parsing, commands |
| orchestrator | 8 | CLI parsing, process management |
| scheduler | 14 | Queue operations, allocator logic |
| coordinator | 9 | Pathfinding, coordinate conversion |
| mock_firmware | 12 | Physics, battery, state machine |
| visualizer | 0 | (Bevy systems, manual testing) |
| **Total** | **62** | |

---

## Architecture Evolution

### Initial (Phase 1-2)
```
visualizer ← mock_firmware
```

### Distributed (Phase 3)
```
coordinator ↔ mock_firmware
visualizer ← all
```

### Current (Phase 4)
```
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
