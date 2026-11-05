# Functional Specification

## Purpose

This document is the authoritative source for the user-visible behavior of Session Manager. Project and application discovery follows Linux XDG conventions; session lifecycle and checkpoint contracts remain independent of a particular compositor or launcher.

Session Manager discovers project folders, creates projects from templates, supplies current application lists, and manages independent logical sessions. Optional task presets and pause/resume are later additions. It does not implement a compositor, desktop shell, launcher, layout, keyboard shortcuts, window manager, or process manager.

Concrete integrations may create workspaces, launch applications, preserve windows, suspend processes, or present the user interface. They do so as clients or adapters of Session Manager's generic contracts and remain responsible for their platform-specific behavior.

## Status

The project and application contract follows [ADR 0012](decisions/0012-discover-project-profiles-and-resolve-xdg-applications.md). The initial delivery covers projects, applications, and running sessions; tasks and pause/resume are later work. C1 implements project discovery/creation and running sessions, C2 implements current application resolution, and C3 exposes them through the owner-only API and CLI. The [initial release gate is accepted](reviews/c3-acceptance.md). The version 1 technical/API inventory describes the former central-profile model. M1/M2 acceptance records do not validate this revised contract.

## Product terminology

- **Project/profile:** an immediate subdirectory of a configured project-source directory, named by that directory. The project directory itself is the source of current configuration; no configuration file is required for it to be recognized.
- **Profile task:** a named optional preset defined under the project's `.session-manager/tasks/`, containing an ordered, unique selection of application desktop-entry IDs available to that project. It contains no window or workspace placement.
- **Desktop-entry ID:** an application identifier used to merge installed and project-local `.desktop` entries. Session Manager resolves the list; clients decide whether and how to launch an entry.
- **Logical workspace group:** the stable UUID-backed group represented by one session. The group has no core-visible members, slots, count, roles, or global/special variants. A client or adapter maps the session UUID to any concrete workspaces it owns.
- **Launch source:** a currently discovered project or, when task support is delivered, a task in that project.
- **Session:** one numbered runtime instance opened from a project or, later, a task. It keeps its identity and source name independently of the project directory; later contextual requests look up the current directory by name.
- **Active session:** the single session selected as the current logical context. Activity is a core role, not proof that a desktop integration is currently displaying it.
- **Running session:** a live logical session that has not committed a checkpoint as paused and has not stopped.
- **Paused session:** a resumable session whose checkpoint has been durably accepted by Session Manager.
- **Stopped session:** a completed session with no retained checkpoint or runtime record.
- **Adapter:** an independently implemented component that performs optional platform-specific lifecycle work through the generic adapter contract.
- **Checkpoint envelope:** core-owned metadata identifying the producing adapter and payload format and protecting the stored payload.
- **Checkpoint payload:** opaque bytes owned exclusively by the producing adapter.
- **Client:** a shell, launcher, compositor configuration, command-line tool, automation process, or other consumer of the public API.
- **Recovery:** reconciliation of core state, pending operations, checkpoint integrity, and adapter-reported status after interruption.

Running, paused, active, and stopped are not interchangeable. Switching the active session does not pause or stop the previous session.

## Projects and templates

One configurable project-source directory contains profiles as immediate subdirectories. A project requires no metadata file; absent `.session-manager/`, `apps/`, or `tasks/` means no local overrides. Project names derive from directory names, not a central profile registry. The project directory is contextual data for clients; Session Manager does not change other processes' working directories. Discovery happens when listing projects for project creation and when starting a session, not through continuous watching. A start resolves the current directory and rejects a project name that no longer exists. Project creation copies a chosen template from `$XDG_CONFIG_HOME/session-manager/profile-template/<template-name>/` into a **new** directory in the project source. The initial shipped template is an empty `Default Profile`. If the requested base name is `Name`, creation chooses the first available `Name-1`, `Name-2`, etc.; it never merges with or overwrites an existing directory. Users rename, edit, and remove project directories themselves; Session Manager provides no profile rename/delete/edit operation. Template contents, including README.md, Notes.md, ToDo.md, AGENTS.md, Obsidian metadata, and `.session-manager/`, are user choices, not required schema or automatically created session fields. The project directory must not be confused with the template or with a fallback profile.

## Applications

At each application-list request, Session Manager discovers installed applications according to the Linux XDG desktop-entry conventions, removes IDs excluded by the project's `.session-manager/apps/` exclusion list, then applies `.desktop` files in `.session-manager/apps/` by desktop-entry ID, replacing an installed entry of the same ID and allowing a local entry to reintroduce an excluded ID. A project without local application settings sees all eligible installed applications. An entry explicitly marked `Hidden=true` or `NoDisplay=true` does not appear and may omit `Exec`; malformed fields other than an absent `Exec` still cause an error. The result, not a frozen list from session start, is exposed through a launcher-independent API; a client chooses how to display or launch it. A Firefox-specific command or profile can be expressed by the project's own `.desktop` file; the core has no Firefox field. Installed-app discovery is part of Session Manager's Linux integration, not a launcher-specific plugin. If the active session's recorded project name does not resolve to a current directory, the application list falls back to eligible installed applications with no project overrides. If a new directory later takes the same name, subsequent lookups use that new directory.

## Tasks (later delivery)

Tasks retain their existing meaning: optional named presets selecting ordered unique desktop-entry IDs available to their parent project, without window placement. When task support is delivered, task definitions are loaded from `.session-manager/tasks/` when starting a task, rather than kept in a central profile configuration. Multiple independent sessions may be opened from the same project or task. The initial project/session delivery omits task functionality and ignores task-definition files; a later delivery adds task discovery, launch, and validation. Switching sessions does not reload project or task definitions.

## Independence of open sessions

An open session retains its UUID, name, recorded project/task names, number, lifecycle, and any checkpoint when a project directory is renamed, removed, or replaced. The old name ceases to be a launchable project; the new name becomes launchable independently. No rename propagation, deletion guard, or strict live link to the filesystem exists. A new directory with the old name may supply settings for that existing session's future context-dependent requests. A missing project does not prevent switch or core-only stop. A paused session resumes from its checkpoint via the producing adapter, never by reconstructing a project from its current directory; project-specific behavior after resume can use the current name lookup or report missing context. Project edits never rewrite an accepted checkpoint.

Every session remains one logical workspace group. Fixed counts, numbered slots, special workspaces, concrete membership, focus, and window placement remain downstream desktop policy.

## Session identity and naming

A project and (in the later task delivery) each of its tasks are distinct named launch sources. Multiple running or paused sessions may have the same source name; this is not a persistent link to the current project directory.

Names are generated automatically:

- profile session: `<profile-name>-<number>`;
- task session: `<profile-name>-<task-name>-<number>`.

The number is calculated independently per launch source in the range 1 through 4,294,967,295. Running and paused sessions reserve their numbers; stopped instances do not. A new session uses `1` when no number is reserved; otherwise it uses one more than the highest live number. Exhaustion fails without creating a session; lower gaps are not reused while a higher live number remains. Generated names are globally unique among live sessions. If distinct profile/task name combinations produce the same generated text, start fails without creating a session instead of changing the allocation rule or silently allowing duplicate names. A start never exposes a partially created session or a separately visible reservation.

Examples include `FDS-1`, `FDS-MontageVidéo-1`, `FDS-MontageVidéo-2`, `FDS-MiniaMaking-1`, `QMK-1`, and `QMK-2`.

If a session stops, its number no longer participates. If `QMK-1` is the only live `QMK` session and stops, the next `QMK` start is again `QMK-1`. If `QMK-2` remains running or paused, the next is `QMK-3`.

Renaming a project or task definition on disk does not rename any running or paused session. Existing live numbers and generated names remain reserved; a start under a newly discovered name must still reject any generated live-name collision without rewriting existing sessions.

## Session lifecycle

The initial delivery supports **start**, **switch**, and **stop**. Starting a currently existing project records its name without storing a project-path link or copying project settings; it creates a running session and makes it active. Switching changes the active logical context; it does not pause or stop the previous session. Stopping removes the logical session but does not close windows, terminate applications, or delete a project directory. At most one running session is active; having none is valid. These operations do not invoke an adapter or manipulate the desktop. Clients decide how to present the active session.

A subsequent additive operation, **deactivate**, clears only the active role. All open sessions remain running and can be selected again; repeating deactivate when already neutral changes nothing. Application listing in the neutral context returns installed applications only. The core does not create a default session on login or infer whether an open session has desktop windows. A downstream graphical integration may refuse to stop a session whose windows or scratchpads are nonempty; direct API/CLI stop remains a logical-only operation without that desktop guard.

Later pause/resume support may add **paused** sessions with adapter-owned checkpoints. A session becomes paused only after a valid checkpoint is stored. Resume gives the unchanged checkpoint only to its compatible producing adapter and makes the session running and active only after a verified completion; it never reconstructs the checkpoint from current project files. A failed or uncertain adapter action cannot be reported as successful. Stopping a paused session discards its checkpoint only after any required adapter cleanup succeeds. Missing or incompatible adapters, uncertain outcomes, and interrupted operations remain visible rather than being guessed away. The core stores checkpoint bytes without interpreting them; concrete restoration belongs to the adapter. The exact compatibility, recovery, integrity, and privacy rules belong to the [technical specification](technical-specification.md) and must be revised for the later delivery before they are implemented.

Neither switching nor resuming requires the original project directory to exist. Project-dependent requests use the current directory matching the recorded name; application listing falls back to installed applications when it is absent.

## Public integration behavior

The Session Manager API lets clients discover available projects and templates, create a project, start/switch/deactivate/stop sessions, inspect the active session, and request the **current** application list for it. There is no profile edit, rename, or delete API and no automatic profile synchronization. The list is computed when requested, including when there is no active session (installed applications only). Clients may use it to present a launcher; Session Manager does not display a launcher, launch applications, control windows, or choose keyboard shortcuts.

The later task delivery adds task discovery and task-based start. A later pause/resume delivery adds checkpoint and adapter status inspection with truthful outcomes. Those features are not required to use projects, sessions, or the application list.

The API exposes project names and paths, session UUIDs, generated names and active state, and resolved applications; it does not expose a frozen project configuration inside a session. It stays independent of any particular compositor, launcher UI, browser, or workspace scheme. Session Manager remains useful without a graphical client.

## Delivery scope

The initial project/session delivery includes:

- configured project source, project discovery and template-based creation (not profile edit/rename/delete);
- Linux XDG application discovery, per-project exclusions and `.desktop` overrides, and a current application-list API;
- session naming, numbering, active role, core-only start, switch, and stop;
- generic local CLI and API for those operations and current application listing;
- crash-safe core persistence and privacy-safe diagnostics.

A later task delivery adds task discovery and task-based starts; task files are ignored in the initial delivery. A separate later lifecycle delivery may add pause/resume, explicitly configured subprocess-adapter discovery, opaque checkpoint capture/storage/handoff/discard/recovery, and the corresponding fake-port, fake-subprocess, sample-adapter, and conformance testing. The lifecycle and checkpoint sections above describe that target, not initial-delivery acceptance.

Neither the initial delivery nor the later generic lifecycle target requires:

- a built-in concrete platform, shell, launcher, layout, process, or service-manager integration;
- a built-in compositor, shell, launcher, or process adapter;
- fixed keyboard shortcuts or a fixed workspace count;
- a core-visible workspace slot, standard/special/global workspace role, or task window destination;
- window discovery, ownership, correlation, movement, placement, focus, or closure in the core;
- process launch, attachment, suspension, signalling, or termination in the core;
- a universal semantic window or layout checkpoint schema;
- checkpoint portability between adapters;
- security isolation between sessions;
- remote or multi-user operation.

## Functional acceptance

The initial delivery is functionally accepted when:

1. projects can be discovered and created from templates without a launcher or compositor;
2. an application query reflects the installed and current project entries, exclusions, overrides, and missing-project fallback without requiring a graphical client;
3. multiple sessions follow the documented naming, numbering, and independence from project-directory edits, while start, switch, and stop report truthful logical outcomes;
4. a client can obtain the active session and application list without compositor- or launcher-specific core methods;
5. default tests cannot discover or mutate the live desktop.

Later task support must preserve the specified preset semantics. Later pause/resume support must preserve opaque checkpoint bytes across storage and adapter handoff, reject incompatible adapters before handoff, and report uncertain outcomes truthfully; those features are not conditions for accepting the initial delivery.
