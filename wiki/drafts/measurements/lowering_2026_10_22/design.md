# Design direction after the lowering wave

The bigger play is to make lowering's work proportional to retained bodies and
changed facts: immutable shared catalogs, explicit dependency edges, and owned
output construction. Strengthen those boundaries in the existing checked plans
and lowering modules. A new IR, driver, cache layer, or parallel framework would
add responsibilities before the expensive work is understood.

This assessment follows implementation commit `0047571a3f1`. It uses the
user-requested `perfaware` skill: inspect actual emitted code, distinguish work
from hardware limits, and measure before choosing an implementation. This record
selects the next experiment; it does not claim a second optimization landed.

## What changes the priority

The first wave's two small witnesses spend only 0.12–0.33% of candidate
whole-compile median time in lowering. Their 18–21% lowering improvement cannot
establish a large improvement in developer latency.

An existing larger program, `samples/cli/collections/segmented_packet_checksum/main.omg`,
does exercise material lowering work. Three uninstrumented candidate compilations
give these milliseconds (all observations retained):

| Measurement | Minimum | Median | Maximum | Mean |
| --- | ---: | ---: | ---: | ---: |
| Lowering | 65.910 | 66.536 | 67.606 | 66.684 |
| Whole compile | 807.933 | 813.833 | 868.151 | 829.972 |

Lowering is about 8.2% of the ratio of medians. Even eliminating that stage
entirely would remove only about 8% of this command's time, assuming other work
stays constant. Checking remains the larger end-to-end opportunity. These are
three workload-selection probes on a busy shared Apple M4 host, not an A/B
comparison or a reliable latency distribution.

Reapplying the retained temporary instrumentation to the committed candidate
gave three successful compilations. The intervals, in milliseconds, are:

| Attribution interval | Minimum | Median | Maximum | Mean |
| --- | ---: | ---: | ---: | ---: |
| Leaf-copy place reconciliation plus copied-membership retention | 43.009 | 43.358 | 44.137 | 43.501 |
| Closure body emission | 23.518 | 23.771 | 24.841 | 24.043 |
| Closure discovery | 0.352 | 0.369 | 0.764 | 0.495 |

The first interval wraps **two calls**; it does not prove which call dominates,
nor how much is allocation versus traversal or semantic work. Instrumentation
overhead and changed code placement preclude comparing these timings directly
with the uninstrumented series. They prioritize investigation within this run.
The instrumentation was removed after measurement.

Two additional workload probes, `prime_counter` and `stdin_checksum`, stop at
package entry-binding admission before lowering. Their exact diagnostics remain
in the record; no package acceptance or source was changed to admit them. No
runtime claim is made for these design probes, including the successfully
compiled packet program.

## First experiment: explain the 43 ms interval

Split the two calls at `machine_lowering.rs` and count membership iterations,
operations visited, descriptor queries, and newly added qualification rows.
Compare visits with useful changes before implementing a worklist. The current
`retention/copied_memberships.rs::retain` rebuilds each machine's membership
history and walks every block/operation on every fixed-point iteration. It also
clones structural declarations and builds a descriptor graph before the loop.
The source establishes repeated-work opportunities, not their dynamic cost.

If repeated propagation dominates, retain dependency relationships and enqueue
only consumers of changed contracts. If graph/history construction dominates,
address that ownership and reuse first. If place reconciliation dominates,
optimize its lookup path instead. Do not replace a measured question with the
most aesthetically appealing rewrite.

The semantic constraints are substantial: descriptor observations name captured
backing storage at a particular operation; joins intersect possible sources;
receiving homes may deliberately weaken facts. A generic union of memberships
or an unversioned per-place cache is incorrect. Preserve these exact semantics,
including borrowed-storage changes, and keep independent Terminal validation.

Acceptance for that next wave is a repeatable reduction of this interval and
whole lowering on the frozen packet program, with no regression on the small
witnesses; equal canonical output where identity order remains unchanged;
actual native/Terminal execution agreement; and relevant invalid custody and
membership controls still rejecting. Counted work must explain the timing gain.
Finish or reject that bounded experiment before broadening the rewrite.

## Second experiment: separate shared input from mutable emission

Body emission is the next measured interval. Several ownership choices deserve
allocation counts rather than a blanket removal of `clone`:

- `ordinary_machine.rs:303` clones `PreparedScalarQualifications` into each
  `Evaluation`. This contains a catalog of vectors and parameter rows; it is
  not an `Arc` clone. Nested evaluations clone it again. The prepared namespace
  is documented as immutable, while newly minted coercions already have their
  own output vector. Borrowing the namespace is a concrete seam to investigate.
- `ordinary_machine.rs:399` clones all structural declarations for call
  emission while the enclosing emission holds mutable structural tables.
  Distinguish the frozen shared namespace from body-local additions. A frozen
  prefix plus an explicit extension owner may express the required visibility
  without copying the entire catalog. First establish whether any consumer
  requires an earlier snapshot.
- `operation_frame.rs:865` clones structural bindings to pass completed results
  beside mutable `OperationBuffer` access. Split the read view from appendable
  emission state, retaining exact pre-call visibility. Rust's borrow conflict
  is revealing an ownership boundary, but the clone may also encode a necessary
  semantic snapshot; removing it blindly changes that contract.
- Several exits copy an operation suffix into a final block. Investigate moving
  completed batches or retaining stable spans, after accounting for companion
  metadata and references to earlier operations. A `drain` alone is not a safe
  substitute for stable operation identity.

The compiled ARM64 candidate retains calls to both the qualification-catalog
clone and `Vec<StructuralTypeDeclaration>::clone`. They are real emitted work,
although their dynamic frequency and share of the 24 ms interval are unmeasured.
`composed_control/callable.rs` already borrows most shared catalogs via `Cow`;
extend a coherent ownership model rather than inventing another emission route.

## Later choices and their evidence threshold

Closure discovery recomputes its closure when provider, scalar, cleanup, or
dynamic-dispatch edges introduce new roots. Computation reachability also uses
linear `visited.contains` checks. A frontier queue and arena-owned visitation
marks could bound that work while preserving generation checks and deterministic
ordering. But discovery is below 1 ms on the stronger workload; it ranks behind
the measured intervals above. Never use arena occupancy as evidence of liveness.

The ordinary-body emitter occupies 43,056 bytes of ARM64 code at dev opt-level 1;
the closure emitter occupies 20,872 bytes. These symbol extents include cold
failure/unwind paths. They do not prove instruction-cache or frontend pressure,
and splitting functions can increase call overhead. Use a native profile or
hardware counters before a code-size campaign. The host reports 128-byte cache
lines; generic x86 cache assumptions in performance advice are not measurements
of this Apple CPU.

SIMD needs a measured contiguous kernel. Threads need enough independent body
work plus disjoint output/identity ownership. Persistent caching needs exact
dependency and invalidation semantics. None is justified as the next step by
these observations. Frozen catalogs and explicit dependencies make those future
choices easier without committing to them now.

For the broader compiler, use the same method on checking, which dominates the
actual command: identify repeated judgments and their exact subjects before
adding reuse. A lowering redesign alone has a measured ceiling here. Keep the
Psi/Omega firewall, check every target-scoped source body, and preserve the
independent verifier; none is expendable to improve a timing number.

## Evidence and navigation

`design-workloads.json` preserves successful and refused commands, input hashes,
binary digest/profile and raw output. `design-attribution.json` binds the
instrumented binary to the committed producer plus `instrumentation.patch`.
`design-assembly.json` records symbol extents, relevant emitted clone calls,
binary identity, command form and hardware reports. Library hashes are retained
in the original `timings.json`; those sources remained unchanged.

Source anchors are relative to
`omega-rust/psi/compilation/pipeline/05_checked-trees-to-lowered-psi/src/` at
`0047571a3f1`: `retention/copied_memberships.rs`,
`unit/attached_unit/{ordinary_machine.rs,operation_frame.rs,call_catalog.rs}`,
`unit/attached_unit/composed_control/callable.rs`,
`expression_preparation/{qualifications.rs,computation_graph.rs}`, and
`emission/operation_emission/buffer.rs`. Architectural ownership follows
`omega-rust/pipeline.md`, particularly invocation-owned working state and
compositional lowering. No additional production changes accompany this review.
