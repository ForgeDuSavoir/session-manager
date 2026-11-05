# Session Manager

Session Manager is a Linux work-context service with a compositor-independent session lifecycle and built-in XDG application discovery.

The initial release discovers projects from one configured source directory, creates them from templates, resolves current Linux XDG application lists with project-local overrides, and manages running logical sessions through an owner-only local API and CLI. Optional tasks and adapter-backed pause/resume belong to later deliveries. The planned checkpoint model keeps platform state opaque; the core does not interpret compositor windows, layouts, focus, shortcuts, or processes.

The project aims to:

- discover each immediate project subdirectory as a profile and create new projects from selectable templates;
- resolve installed XDG applications with per-project exclusions and `.desktop` overrides on demand for launcher clients;
- create multiple independently numbered sessions from the same project or, later, a task preset;
- expose running, paused, and stopped lifecycle operations through a generic local API;
- retain adapter-produced checkpoints for paused sessions and discard them after a completed stop;
- allow shell, launcher, compositor, and automation integrations to consume the public API;
- support independently developed desktop adapters without coupling the core to one platform.

Session Manager does not implement a desktop shell, keyboard shortcuts, workspace navigation, window placement, layout management, or process suspension. Platform behavior belongs to independently developed downstream clients or adapters and is not a core dependency.

## Project documentation

- [`ToDo.md`](ToDo.md) tracks planned and completed work.
- [`docs/usage.md`](docs/usage.md) covers installation, CLI/API use, upgrade/recovery, and the generic launcher-client example.
- [`AGENTS.md`](AGENTS.md) contains working instructions for AI agents.
- [`docs/documentation-structure.md`](docs/documentation-structure.md) defines the documentation structure and sources of truth.
- [`docs/decisions/0010-keep-the-core-platform-independent-with-opaque-adapter-checkpoints.md`](docs/decisions/0010-keep-the-core-platform-independent-with-opaque-adapter-checkpoints.md) records the current architectural direction.

## Project status

[ADR 0012](docs/decisions/0012-discover-project-profiles-and-resolve-xdg-applications.md) replaces the central profile/application model. The functional contract and staged implementation plan are revised; the old version 1 technical API and milestone plan are historical. [C0 is accepted](docs/reviews/c0-contract-review.md), including a clean break without legacy compatibility or migration; [C1](docs/reviews/c1-acceptance.md), [C2](docs/reviews/c2-acceptance.md), and [C3](docs/reviews/c3-acceptance.md) are implemented and accepted for the initial release scope. The daemon/API/CLI and installation guide are available; 101 tests pass normally and under fully isolated release checks. No deployment into a live desktop was performed. M0–M2 acceptance records describe completed work against the former contract, not the replacement. Generic checkpoint safety remains intended for a later pause/resume delivery. Concrete desktop adapters and clients remain independent integrations.
