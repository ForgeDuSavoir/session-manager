# 0010: Keep the core platform-independent with opaque adapter checkpoints

- Status: Accepted
- Date: 2026-07-16

## Context

Session Manager's essential purpose is to manage reusable profiles, tasks, desktop-entry lists, logical workspace groups, and the lifecycle of named sessions. A session may be saved and resumed, but the concrete state required to do that belongs to the platform integration that created it.

No universal window or workspace snapshot can faithfully represent every compositor, layout, shell, and process model without either losing platform-specific state or continually expanding the core schema. Conversely, storing an entirely anonymous blob would prevent Session Manager from selecting the correct producer, checking compatibility, or reporting why a checkpoint cannot be resumed.

## Decision

Keep the Session Manager core independent of any compositor, layout engine, desktop shell, shortcut configuration, window model, application process manager, or concrete desktop integration.

The core owns only platform-independent concepts and guarantees:

- profile, task, and desktop-entry configuration, plus logical workspace-group identity and session association;
- stable session identity, source association, generated naming, and lifecycle state;
- configuration and runtime persistence;
- atomic storage and retention of checkpoints;
- a generic public API for clients and integrations;
- adapter selection, capability and compatibility checks, operation coordination, and truthful outcomes.

Platform integrations depend on Session Manager's public contracts. Presentation and input integrations, such as a desktop shell or compositor configuration, are ordinary clients: they query core state and request supported lifecycle operations. They own their visual models, keyboard shortcuts, launcher interaction, and concrete workspace navigation. The core does not import them, generate configuration for them, or expose APIs named after them.

Saving and resuming concrete desktop state is delegated through a generic adapter contract defined by Session Manager. An adapter owns every platform-specific effect and representation required to capture, suspend when supported, restore or resume, and discard its state. This includes any compositor workspaces and windows, layout-engine state, focus, monitor placement, application relaunch behavior, or process control used by that adapter. The core treats those details as outside its domain.

Each saved checkpoint consists of a core-owned envelope and an adapter-owned opaque payload. The envelope carries enough bounded metadata to identify the producing adapter and checkpoint format, determine declared compatibility, verify payload integrity, and manage persistence and lifecycle. The core may validate the envelope and payload bytes against generic size, integrity, confidentiality, and storage rules, but it must not parse, normalize, merge, partially update, reconstruct, or infer semantics from the payload.

The adapter that defines a payload format exclusively owns its schema, validation, migration, capture, restoration, and disposal semantics. A checkpoint is offered only to a compatible adapter. Missing adapters, unsupported format versions, failed compatibility checks, unavailable capabilities, and indeterminate adapter outcomes are reported explicitly; the core never guesses a replacement adapter or claims an exact resume that the adapter did not verify.

Adapters declare platform-neutral capabilities through the generic contract. Session Manager gates lifecycle operations using those declarations and records the adapter's verified outcome. Optional capabilities do not become mandatory core dependencies. The contract must allow an adapter that only captures and restores serialized state as well as one that preserves live resources while a session is paused.

The architectural dependency direction is:

```text
desktop-shell and compositor configuration ---> Session Manager public API

Session Manager core ---> generic adapter contract <--- concrete adapters
```

The arrow from the core denotes dependence on the abstract contract, never on a concrete adapter implementation. A concrete adapter may be maintained in another package or project and may evolve its payload independently within the compatibility rules of the envelope and adapter contract. [ADR 0011](0011-use-explicit-subprocess-adapters.md) selects explicitly configured subprocess adapters for the first-version transport; the technical specification owns the exact capability vocabulary.

## Consequences

The MVP can concentrate on configuration, session lifecycle, durable generic checkpoints, adapter coordination, and a stable local API. Its default tests can use a fake reference adapter and need no graphical desktop or process manager.

Platform-specific presentation, workspace behavior, layout, process control, and input handling may be delivered independently as adapters, clients, or downstream configuration. Such integrations depend on Session Manager rather than Session Manager depending on them.

Checkpoint payloads can preserve platform-native information without forcing it into an incomplete universal window schema. The cost is that the core cannot inspect, edit, migrate, or restore a payload by itself, and checkpoints are portable only where a compatible adapter provides that guarantee.

The adapter boundary becomes a critical compatibility and safety surface. The technical specification must define envelope limits, adapter discovery and invocation, capability semantics, timeouts, cancellation, indeterminate results, atomic handoff, and privacy rules before implementation. It must do so without leaking one platform's concepts into the generic contract.
