# Documentation

This directory contains the authoritative design documentation for Session Manager.

## Repository sources

- `README.md` owns the project purpose, high-level scope, integrations, and current status.
- `ToDo.md` tracks planned, active, and completed work only.
- `AGENTS.md` contains instructions for agents working in the repository only.
- This directory owns specifications, implementation planning, and durable design decisions.

## Structure

### `functional-specification.md`

Defines product terminology, user-visible behavior, session lifecycle, requirements, edge cases, and non-goals. It must describe what the system does without depending on a particular implementation.

### `technical-specification.md`

Defines the architecture, components, data models, interfaces, persistence, external integrations, error handling, and recovery guarantees. It must implement the functional specification rather than redefine it.

### `implementation-plan.md`

Turns the specifications into ordered milestones with acceptance criteria, dependencies, testing requirements, and rollout steps.

### `decisions/`

Contains Architecture Decision Records for durable choices that need their own rationale and history. The format and naming convention are defined in `decisions/decision-records.md`.

Decision records complement the specifications. Once accepted, their outcome must be reflected in the relevant authoritative specification.

## Documentation rules

- Store each fact in the document responsible for it and link to that source elsewhere.
- Do not use `ToDo.md` as a specification or decision log.
- Keep functional behavior separate from technical implementation.
- Update affected documents when a decision changes an existing contract.
- Write all documentation in English.
