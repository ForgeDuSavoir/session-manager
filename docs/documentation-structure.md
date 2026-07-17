# Documentation

This directory contains the authoritative design documentation for Session Manager.

## Repository sources

- `README.md` owns the project purpose, high-level scope, integration direction, and current status.
- `ToDo.md` tracks planned, active, and completed work only.
- `AGENTS.md` contains instructions for agents working in this repository only.
- This directory owns specifications, implementation planning, and durable design decisions.

## Structure

### `functional-specification.md`

Defines platform-independent product terminology, user-visible profile and session behavior, lifecycle guarantees, adapter-visible outcomes, edge cases, and non-goals. It describes what the core does without naming a compositor, layout, shell, shortcut scheme, or process manager as a requirement.

### `technical-specification.md`

Owns the platform-independent architecture, core data models, generic public API, persistence, adapter contract, opaque checkpoint envelope, error handling, recovery guarantees, and test boundaries. It is the authoritative source for the adapter/checkpoint contract.

The generic core-to-adapter protocol, including transport framing, generic lifecycle methods, compatibility, status recovery, and opaque payload handoff, is part of the core technical specification. Platform-specific protocols used internally by a concrete adapter, adapter-owned payload schemas, compositor commands, UI composition, and shortcut configuration belong to the project that owns that adapter or downstream integration. This repository may link to them or provide clearly non-normative examples after the core contract is stable.

### `implementation-plan.md`

Turns the core specifications into ordered milestones with acceptance criteria, dependencies, testing requirements, and rollout steps. Concrete desktop integrations cannot be MVP prerequisites unless a later accepted scope decision explicitly adds them.

### `decisions/`

Contains Architecture Decision Records for durable choices that need their own rationale and history. The format and naming convention are defined in `decisions/decision-records.md`.

Decision records complement the specifications. Once accepted, their outcome must be reflected in the relevant authoritative specification. Superseded records remain historical and must link to the replacing decision.

## Documentation rules

- Store each fact in the document responsible for it and link to that source elsewhere.
- Do not use `ToDo.md` as a specification or decision log.
- Keep functional behavior separate from technical implementation.
- Keep core contracts separate from adapter-owned payload and integration documentation.
- Describe platform examples as non-normative unless their owning external project publishes the normative contract.
- Update affected documents and ADR statuses when a decision changes an existing contract.
- Write all documentation in English.
