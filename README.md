# Session Manager

Session Manager is a platform-independent work-context service for Linux.

It manages reusable profiles, optional tasks, desktop-entry lists, logical workspace groups, and named session lifecycles. Concrete desktop state can be saved and resumed through adapters. Session Manager stores a generic checkpoint envelope and an opaque adapter-owned payload; it does not interpret compositor windows, layouts, focus, shortcuts, or processes.

The project aims to:

- let users create, inspect, modify, and delete session profiles;
- associate each profile with a root directory, desktop-entry list, and optional tasks;
- create multiple independently numbered sessions from the same profile or task;
- expose running, paused, and stopped lifecycle operations through a generic local API;
- retain adapter-produced checkpoints for paused sessions and discard them after a completed stop;
- allow shell, launcher, compositor, and automation integrations to consume the public API;
- support independently developed desktop adapters without coupling the core to one platform.

Session Manager does not implement a desktop shell, keyboard shortcuts, workspace navigation, window placement, layout management, or process suspension. Platform behavior belongs to independently developed downstream clients or adapters and is not a core dependency.

## Project documentation

- [`ToDo.md`](ToDo.md) tracks planned and completed work.
- [`AGENTS.md`](AGENTS.md) contains working instructions for AI agents.
- [`docs/documentation-structure.md`](docs/documentation-structure.md) defines the documentation structure and sources of truth.
- [`docs/decisions/0010-keep-the-core-platform-independent-with-opaque-adapter-checkpoints.md`](docs/decisions/0010-keep-the-core-platform-independent-with-opaque-adapter-checkpoints.md) records the current architectural direction.

## Project status

The platform-independent functional design, architecture, adapter contract, and implementation plan are sufficiently stable to begin M0 of the core MVP. Completed-capture reconciliation, explicit adapter-registry reload, and terminal-status release are settled prerequisites for M3. Concrete desktop adapters and clients remain optional independent integrations.
