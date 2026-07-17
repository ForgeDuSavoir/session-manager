# 0001: Use Rust and a single-package modular structure

- Status: Accepted
- Date: 2026-07-15

## Context

Session Manager is a long-running local service that coordinates client requests, persisted state, adapter outcomes, and lifecycle operations. It must remain testable without a graphical session or concrete desktop adapter.

The first version needs clear boundaries between domain rules, persistence, IPC, the generic adapter contract, and user-facing interfaces. The project is still small, so multiple independently built core packages would add coordination overhead before a concrete need exists.

## Decision

Implement Session Manager in stable Rust using the Rust 2024 edition and Cargo.

Start with one Cargo package containing one executable target and one library target. Organize the library into internal modules for domain rules, application operations, persistence and transport ports, the generic adapter contract, and user-facing interfaces. Dependencies point toward the domain, and the domain performs no external I/O or platform-specific work.

Do not include a concrete desktop adapter in this package merely for convenience. Do not create a production Cargo workspace or additional production packages until an independently justified build, release, or reuse boundary appears. A standalone sample-adapter fixture may live outside the production package and build independently using only the published wire contract; this deliberate conformance fixture is not shipped in the core artifact and does not change the single-package production architecture.

## Consequences

Rust's ownership, type, and error models can encode invalid state and concurrency interactions explicitly, at the cost of a steeper learning curve and longer compile times than a scripting implementation.

A single deployable executable suits the local-service use case. Keeping reusable logic in the library enables focused tests with fake storage and adapter ports.

Module visibility and dependency tests must ensure that platform integrations depend on public contracts and cannot leak concrete platform types into the core.
