# MCP Rust SDK Migration Plan

This document outlines the step-by-step plan to migrate the current MCP SDK codebase to the new, modular, adapter-based architecture as described in `SDK_DESIGN.md`. Each step should be tracked and checked off as completed.

---

## Migration Steps

> **Important:**
> - **Every migration step must be completed with all relevant tests passing and at least one working example.**
> - **Each contributor is responsible for verifying and guaranteeing that all tests and examples are green before marking a step as complete.**

### 1. Preparation
- [ ] Review and freeze the current codebase (create a migration branch).
- [ ] Ensure all existing functionality is covered by tests (add missing tests).
- [ ] Communicate the migration plan to all contributors.

### 2. Project Structure Refactor
- [ ] Create new files: `adapter_client_tcp.rs`, `adapter_server_tcp.rs`, `routing.rs`, `protocol.rs`.
- [ ] Move protocol types and helpers to `types.rs`, `protocol.rs`, and `common.rs` as needed.
- [ ] Update `client.rs` and `server.rs` to serve as high-level APIs only.

### 3. Adapter Layer Implementation
- [ ] Define the `NetworkAdapter` trait (async send/recv interface).
- [ ] Implement `adapter_client_tcp.rs` and `adapter_server_tcp.rs` for TCP transport.
- [ ] Write unit and integration tests for each adapter.

### 4. Protocol Layer Isolation
- [ ] Move all message (de)serialization and validation logic into `protocol.rs`.
- [ ] Ensure all protocol errors and responses are handled in this layer.
- [ ] Add protocol roundtrip and validation tests.

### 5. Routing Layer Refactor
- [ ] Move all method dispatch and handler lookup logic to `routing.rs`.
- [ ] Ensure routing is agnostic to the transport (uses trait objects for adapters).
- [ ] Add routing unit tests and handler dispatch tests.

### 6. High-Level API Update
- [ ] Refactor `client.rs` and `server.rs` to use the new adapter, protocol, and routing modules.
- [ ] Update public API to match the design doc.
- [ ] Add/expand API documentation and examples.
- [ ] Ensure all public methods have tests.

### 7. Application Layer & Examples
- [ ] Update all examples to use the new API and architecture.
- [ ] Ensure user-facing handler registration is unchanged or improved.
- [ ] Add integration tests for all examples.

### 8. Finalization & Cleanup
- [ ] Remove obsolete code, files, and comments.
- [ ] Update documentation (`SDK_DESIGN.md`, README, etc.) to reflect the new structure.
- [ ] Run full test suite and ensure 100% pass rate.
- [ ] Perform code review and QA.

---

## Notes
- All steps must include appropriate tests before marking as complete.
- Keep this document up-to-date as migration progresses.
- Raise blockers or questions in project discussions as soon as they are discovered.

---

*This migration plan is a living document. Update as necessary to ensure a smooth and successful migration.*
