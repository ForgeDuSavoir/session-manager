# 0012: Discover project profiles and resolve XDG applications on demand

- Status: Accepted
- Date: 2026-11-04

## Context

The earlier core design stored editable profiles, task subsets, roots, and application lists centrally in TOML, kept a live source association and immutable start snapshot, and forbade installed-application discovery in the core. The desired workflow instead uses project folders as the source of configuration and gives launchers an application list that reflects current files. M1 and M2 were accepted against the older contract; that evidence remains historical, not evidence of this replacement.

## Decision

Configure one project-source directory; each immediate subdirectory is a profile even when empty. Read the project directory when starting a session, enumerate projects when creating one, and resolve installed and project-local applications when a launcher client requests the list. Do not continuously watch project files or maintain a central profile registry. Create projects by copying a selected template from `profile-template/<template-name>/` under Session Manager's configuration directory into a new `Name-N` project directory, choosing the first available positive suffix and never merging or overwriting. Ship one empty `Default Profile` template. All Markdown, Obsidian, application, and task files in templates are optional user-controlled contents.

Resolve the installed application list internally using Linux XDG desktop-entry conventions. Remove project-local excluded IDs and then overlay project-local `.desktop` entries from `.session-manager/apps/` by ID; local entries can reintroduce exclusions. Expose the resulting list through a launcher-independent Session Manager API, evaluated on demand. Entries explicitly marked `Hidden=true` or `NoDisplay=true` may omit `Exec` because they are never launch candidates; all other fields remain validated, and visible entries still require a nonempty `Exec`. This narrow clarification avoids rejecting installed menu-suppression entries that conform to ordinary desktop practice. Firefox and launcher presentation have no core-specific schema. The task definitions in `.session-manager/tasks/` retain their existing preset/subset meaning but are ignored in the initial delivery; task support and pause/resume may be delivered later.

A live session keeps stable UUID, recorded source name, generated name, number, lifecycle, and checkpoint without a durable reference to the current project directory or frozen application configuration. Rename/removal of the directory does not mutate the session. A new directory with the recorded name supplies future contextual lookups; absence falls back to eligible installed apps for application listing. Checkpoint restoration continues to use the producing adapter's retained payload, not current project files. Session naming and numbering remain per named launch source and must still reject live collisions.

This decision supersedes the central-profile configuration, source-rename/delete guards, start application snapshots, and no-installed-app-discovery parts of [ADR 0002](0002-use-toml-configuration-and-json-runtime-state.md) and [ADR 0010](0010-keep-the-core-platform-independent-with-opaque-adapter-checkpoints.md). Their crash-safe runtime/checkpoint and core-versus-desktop-lifecycle boundaries remain applicable. A new API/schema version is required; the old version 1 method shapes are not silently redefined.

## Consequences

Projects can be renamed or edited with ordinary filesystem tools, and launchers receive current application choices without storing copies in sessions. Linux XDG discovery becomes an explicit core responsibility, while windowing, launching, Firefox behavior, and launcher UI remain downstream. A same-name directory may supply different context to a previously opened session by design. The configuration/API/schema, adapter capture context, and milestone acceptance plan must be redesigned before implementation; existing M1/M2 implementation and acceptance records must not be retroactively rewritten.
