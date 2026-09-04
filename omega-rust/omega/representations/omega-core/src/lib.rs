//! The dependency floor of the Omega half: four small things everything above
//! may use, which themselves depend on nothing.
//!
//! `Cargo.toml` here has no `[dependencies]` section at all, and that is the
//! only property the four modules share. They are not a subsystem:
//!
//! - `allocations` — a `GlobalAlloc` wrapper counting calls and bytes, so a
//!   phase can report what it allocated. Installed by the `omega` binary.
//! - `operations` — the nine-way `OperationDomain` taxonomy: which kind of
//!   operation touches runtime storage, which crosses the host boundary.
//! - `parallel` — a fixed worker pool with `map_ordered`, `join2`, `join3`.
//! - `runtime_storage` — the two native storage roots, `Machine` and
//!   `RuntimeFrame`.
//!
//! Four crates would read better on the dependency graph and we are not making
//! them, because a crate does not get added until a module boundary has stopped
//! moving, and two of these four have no caller yet to move against.

//! Only `allocations` reaches production. `omega/src/command.rs` installs
//! `CountingAllocator` as the global allocator, and
//! `omega-compiler/src/pipeline/timing.rs` and `omega-artifacts` read
//! `AllocationDelta` off it.
//!
//! @Incomplete: the other three are reached from nowhere. `OperationDomain`
//! predates the Terminal Psi cut and no lowering stage asks it anything;
//! `WorkerPool` was written for a parallel pipeline that has not landed. Do not
//! read their presence here as evidence the pipeline uses them.
//!
//! `runtime_storage` is the one worth explaining, because it looks live and is
//! not. `omega-object-file/src/names.rs` does import `RuntimeStorageRegion` and
//! match on both variants — but it does so inside `storage_region_symbol_name`,
//! and that function has no caller anywhere in the workspace, its own tests
//! included. An earlier version of this header called the module live on the
//! strength of that import. Liveness is a property of the call chain, not of
//! the `use` line, and one grep short of the answer is how it gets written down
//! wrong.

pub mod allocations;
pub mod operations;
pub mod parallel;
pub mod runtime_storage;
