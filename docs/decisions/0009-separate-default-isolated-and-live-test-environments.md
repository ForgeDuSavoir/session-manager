# 0009: Separate default, adapter-conformance, and live test environments

- Status: Accepted
- Date: 2026-07-15

## Context

Session Manager rewrites persisted state and may invoke adapters capable of changing external desktop resources. Running an installed concrete adapter during ordinary core tests could damage unrelated work.

Pure fakes establish the core contract but cannot prove compatibility between a concrete adapter and the platform it controls.

## Decision

Make the core default suite use fake adapters and structurally prevent it from discovering or invoking installed concrete adapters. Use deterministic clocks, synthetic fixtures, temporary XDG roots, guarded fixture subprocesses, and network-disabled CI where practical.

Define an adapter-conformance suite against the generic contract. Run concrete adapter and platform tests in the adapter-owning project or a disposable environment with separate runtime, configuration, and home data. Keep live checks manual, read-only by default, and mutation-gated by explicit approval of an exact scoped checklist.

Test core sagas through deterministic fault injection at persistence and adapter-protocol boundaries. Never fall back from a missing fake, conformance, or isolated backend to an installed live adapter.

## Consequences

Ordinary core tests remain safe during normal desktop use and cover domain, API, persistence, opaque payload round-trips, and recovery behavior quickly.

Concrete integration evidence belongs to each adapter and is not a core MVP gate unless that adapter is separately selected as a release deliverable. Live validation cannot replace missing fake or conformance coverage.
