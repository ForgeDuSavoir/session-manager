# Documentation

This directory contains the authoritative design documentation for Session Manager.

## Repository sources

- `README.md` owns the project purpose, high-level scope, integration direction, and current status.
- `ToDo.md` tracks the active staged work and points to `ToDo-legacy.md`, which preserves the previous central-profile task history without treating its checkmarks as evidence for the replacement.
- `AGENTS.md` contains instructions for agents working in this repository only.
- This directory owns specifications, implementation planning, and durable design decisions.

## Structure

### `functional-specification.md`

Defines user-visible directory-backed project and application behavior, generic session lifecycle guarantees, adapter-visible outcomes, edge cases, and non-goals. Project application discovery follows Linux XDG conventions. It describes what the core does without naming a compositor, layout, shell, shortcut scheme, or process manager as a requirement.

### `technical-specification.md`

Owns the replacement project/application contract and the legacy version 1 core data models, public API, persistence, adapter contract, opaque checkpoint envelope, error handling, recovery guarantees, and test boundaries. The old API sections are not the revised project's final wire schema. The version 2 section is the accepted C0 contract. It is the authoritative source for the adapter/checkpoint contract.

The generic core-to-adapter protocol, including transport framing, generic lifecycle methods, compatibility, status recovery, and opaque payload handoff, is part of the core technical specification. Platform-specific protocols used internally by a concrete adapter, adapter-owned payload schemas, compositor commands, UI composition, and shortcut configuration belong to the project that owns that adapter or downstream integration. This repository may link to them or provide clearly non-normative examples after the core contract is stable.

### `implementation-plan.md`

Orders the replacement project/session/application delivery and its acceptance gates; tasks and pause/resume are independent later stages. Historical central-profile milestones and their evidence are preserved in `legacy-implementation-plan.md` and the current plan's historical records. Concrete desktop integrations are not initial-release prerequisites.

### `usage.md`

Owns installation, operation, safe upgrade/rollback/recovery procedures, developer-check commands and downstream-client examples. It links to the technical contract rather than redefining API schemas. The optional executable menu example lives in `../examples/launcher-client.py` and is not a core dependency.

### `reviews/`

Contains scoped review evidence, reuse audits and approval gates. These records link to authoritative specifications and do not replace them. `reviews/c0-contract-review.md` records C0 approval and the clean-break legacy policy; `reviews/c1-acceptance.md`, `reviews/c2-acceptance.md` and `reviews/c3-acceptance.md` record isolated implementation evidence and the initial release gate.

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
