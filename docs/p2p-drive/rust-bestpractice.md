Foundation (Beginner → Competent)

The fundamentals that separate “I can write Rust” from “I understand Rust.”

- Core Language
- Ownership, Borrowing, Lifetimes (the real entry exam)
- Result, Option, pattern matching
- Slices, strings, references, iterators
- Traits & generics (parametric polymorphism)
- Error handling (thiserror, anyhow)
- Cargo workspaces & dependency management

Mental Models

- Move semantics vs Copy semantics
- Zero-cost abstractions
- Stack vs heap & how Rust allocates
- Why Rust is not like C++, Go, or Python

Must-Learn Tools

- cargo test, cargo bench, cargo fmt, clippy
- rust-analyzer

---

Architecture (Intermediate → Solid Engineer)

- Systems Patterns
- Traits as interfaces
- ZST strategy types & compile-time polymorphism
- Phantom types for type-level invariants
- Smart pointers (Arc, Rc, Weak, RefCell)
- Concurrency primitives (Mutex, RwLock, channels)
- Async Rust (tokio, async-std, executors)

Memory & Performance

- Profiling (perf, flamegraph, cargo profiler)
- Allocation patterns (Box, arenas, bump allocators)
- Minimizing clones
- Struct-of-arrays vs Array-of-structs
- Cache friendliness

Async & Networking

- tokio::spawn, cancellation, backpressure
- Streams, pinning, async lifetimes
- Building servers with axum, warp, hyper

---

Advanced (Production-Grade Rust Engineer)

This is where Rust becomes a superpower.

- Advanced Type System
- Higher-ranked trait bounds (HRTBs)
- GATs (generic associated types)
- Advanced lifetimes & variance
- Type-state programming
- Custom derives & procedural macros

Unsafe & FFI

- Safe wrappers around unsafe code
- Writing bindings to C/C++, Python, Zig
- Understanding UB, aliasing rules, and memory models

Data Structures

- Lock-free structures
- Custom allocators
- Building your own iterator adaptors
- Arena allocators for high-performance workloads

Concurrency & Parallelism

- Work-stealing runtimes
- Custom executors
- Actor models (e.g., building your own mini-Actix)

---

Production (Deploying Rust in Real Systems)

Most engineers fail here — Rust doesn’t forgive sloppy production design.

- Engineering Practices
- Deterministic error handling
- Observability (OpenTelemetry, tracing spans)
- Feature flags
- CI/CD checks (fmt, clippy, tests, benches, docs)
- Load testing & regression benchmarking

System Design with Rust

- Building high-throughput services (100k–1M RPS)
- Understanding Linux syscalls & epoll
- Backpressure handling
- Graceful shutdown & cancellation
- Horizontal scaling architecture

Security & Safety

- Memory-safe FFI boundaries
- Sandboxing & capability-based design
- Auditing unsafe blocks

---
