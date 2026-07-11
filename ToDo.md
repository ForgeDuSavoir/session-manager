# To Do

## Project foundation

- [x] Write the project README.
- [x] Add the MIT license.
- [x] Add instructions for AI agents.
- [x] Define the documentation structure.
- [x] Create git repo
- [x] Publish repo to GitHub and add ssh key

## Functional design

- [ ] Write the functional specification.
- [ ] Define the session vocabulary and lifecycle.
- [ ] Specify start, switch, pause, resume, stop, and recovery behavior.
- [ ] Define how sessions own workspaces, windows, applications, browser profiles, and processes.
- [ ] Specify workspace, window navigation, launcher, and system bar behavior.
- [ ] Define expected failure cases, recovery behavior, and user-facing feedback.
- [ ] Confirm the scope and non-goals of the first version.

## Technical design

- [ ] Write the technical specification.
- [ ] Choose the implementation language and project structure.
- [ ] Define the session configuration and persisted state formats.
- [ ] Define the CLI and IPC interfaces.
- [ ] Define the Hyprland integration boundary.
- [ ] Define the Quickshell integration boundary.
- [ ] Decide how applications and windows are identified reliably.
- [ ] Decide how session-specific processes and browser profiles are managed.
- [ ] Define logging, error handling, and recovery guarantees.
- [ ] Define a safe testing strategy that does not disrupt the active desktop session.

## Implementation plan

- [ ] Write the implementation plan.
- [ ] Split the MVP into independently testable milestones.
- [ ] Define acceptance criteria for each milestone.
- [ ] Identify dependencies and required changes in `linux-setup`.

## MVP implementation

- [ ] Create the project skeleton and automated test setup.
- [ ] Implement session definitions and active-session state.
- [ ] Implement session listing, status, creation, switching, and stopping commands.
- [ ] Implement state persistence and startup recovery.
- [ ] Implement the Hyprland adapter.
- [ ] Associate workspaces and tracked windows with sessions.
- [ ] Restore the last active workspace when switching sessions.
- [ ] Expose session state and actions through IPC.
- [ ] Integrate session display and switching into Quickshell.
- [ ] Filter displayed workspaces for the active session.
- [ ] Filter keyboard navigation between workspaces and windows.
- [ ] Filter launcher entries according to the active session.
- [ ] Add focused unit, integration, and recovery tests.

## Release

- [ ] Document installation, configuration, usage, and troubleshooting.
- [ ] Add continuous integration checks.
- [ ] Validate the MVP in the real Hyprland and Quickshell environment.
- [ ] Prepare the first tagged release.
