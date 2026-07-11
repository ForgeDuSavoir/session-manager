# Agent Instructions

## Required context

Before working on the project:

1. Read `README.md` for the project overview.
2. Read `ToDo.md` for current priorities and completed work.
3. Read `docs/functional-specification.md` for product behavior when it exists.
4. Read `docs/technical-specification.md` and relevant records under `docs/decisions/` before making architectural or implementation changes.
5. Inspect the existing implementation and tests within the scope of the task.

Do not infer settled behavior or architecture from `ToDo.md`, examples, old discussions, or this file. If an authoritative document does not exist yet, treat the subject as unresolved.

## Sources of truth

Documentation ownership and structure are defined in `docs/documentation-structure.md`. Follow that index and link to authoritative documents instead of copying their content elsewhere.

## Language

All project content must be written in English. This includes documentation, source code comments, configuration examples, test descriptions, commit messages, and user-facing text.

## Working rules

- Follow documented functional and technical decisions; do not silently replace them.
- If required behavior is unresolved, document the question and request clarification before making a permanent choice.
- Keep functional intent separate from implementation details.
- Record durable decisions in the appropriate specification or decision record, not only in code or conversation.
- Prefer small, reviewable changes and preserve existing conventions.
- Add tests in proportion to the behavioral risk of a change.
- Avoid unrelated refactoring and premature abstractions.
- Update `ToDo.md` when tracked work is completed or when new required work is discovered.

## Safety

Development and tests may run inside the user's active graphical session. Never close windows, terminate processes, switch workspaces, stop Hyprland, overwrite session state, or alter the live desktop unless the user explicitly authorizes that action.

Use mocks, fixtures, temporary state directories, dry-run modes, or isolated IPC adapters for automated tests. Real-environment tests must be explicit, reversible, and documented.

Never commit credentials, browser profile data, private session state, machine-specific secrets, or generated runtime files.
