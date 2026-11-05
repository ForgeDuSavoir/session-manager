# 0014: Deactivate the active session without stopping it

- Status: Accepted
- Date: 2026-11-05

## Context

A downstream desktop integration needs to return to ordinary workspaces that do not belong to any session, including at login when no session was selected. `session.stop` is not suitable: it deletes the logical session and releases its name/number. Simulating an unselected session in the shell alone would leave application queries bound to the last active project.

## Decision

Add `session.deactivate` to the existing version 2 owner-only API and `session deactivate` to the CLI. It takes an empty params object, clears `active_session_id`, and returns `{revision, active_session_id: null}`. It leaves every running session unchanged. If already inactive, it succeeds without committing a new revision. The installed-only application fallback applies immediately. The core does not select a default session at login or infer desktop window membership.

A graphical client may guard stop by checking its own session workspaces and special workspaces first. The generic core deliberately does not enforce a compositor-specific guard: direct `session.stop` remains a logical-only operation. The downstream integration must disclose that boundary and must not claim that the check is atomic with window creation. No window is automatically adopted from an ordinary neutral workspace by starting a session.

## Consequences

Users can return to neutral context while retaining open sessions for later switching. The additive method does not change the version 2 runtime schema or the existing method shapes. Clients that require a desktop window guard must implement it independently; they cannot rely on direct CLI/API stop to protect those windows.
