# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Before a proposed surface becomes an owner question, audit whether it is
implemented, whether any authored source uses it, and whether ordinary Omega
already expresses the customer. An unimplemented, unused spelling that adds no
capability beyond existing checked machines is retired rather than redesigned.
Hypothetical future utility does not by itself preserve syntax; a concrete
customer requiring a distinct capability may propose a new surface later.

Every `OWNER-BLOCKED` escalation must name an independently motivated product
requirement or credible external use case. Existing corpus use is not required.
A test, experiment, benchmark, or implementation task cannot be the sole
motivation, and machinery introduced only to support such work is removed or
kept non-authoritative rather than promoted into an owner decision.

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

Bootstrap design exploration is delegated engineering work, not an owner
question merely because it compares a different implementation or language
facility. Apply [whole-chain minimization](bootstrap/MINIMIZATION.md): identify
the next compiler/checker customer and compare complete audit cost. Experiments
remain non-authoritative; changes to the trust boundary or required assurances
must be surfaced before relying on them.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

1. **Which declared-default field rows of a `&mut self` receiver cross a
   method call — the machine-storage ZII set or the caller's checked
   incoming set?** (named decision:
   `mutable-self-receiver-declared-field-rows`).
   [Default domains and zero
   initialization](wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization)
   states both halves without joining them for receivers: "machine-owned
   storage may begin zeroed while gated fields remain inaccessible until
   established" governs a callee's `self` entry, while the same section
   makes calls a consumption point where "the domain must be proved again"
   — without saying whether the receiver place at a method call is the
   callee's machine storage or a caller-obligated place like a `&mut`
   actual. [Receiver state](
   wiki/language_guide/chapter_8_domains.md#machines-and-states-can-require-or-guarantee-domains)
   supplies the authored route (`requires self in D` / `ensures self in D`)
   and stays silent on unannotated declared-field rows, and
   [data_and_literals](wiki/spec/language/data_and_literals.md) confines
   zero-initialization to construction, not call contracts. The
   implementation currently picks the machine-storage route: a callee's
   `self` entry assumption seeds only rows whose ZII value satisfies the
   domain (`typed-trees-to-checked-trees/src/semantic/field_domains.rs`),
   `checks/contracts/exits/result_domains.rs` re-proves exactly the assumed
   rows at return, and `flow/call_phases/referents.rs` hands exactly those
   back — so a caller's established non-ZII receiver facts survive a method
   call only when the callee's write frame provably spares the field. Every
   `&mut` parameter already rides the other route: "nominal input storage
   instead relies on the checked incoming argument" (same file).
   Motivating requirement: receiver field invariants are the common shape
   of authored data invariants — a caller that proves `player.health`
   inside its declared bound and then invokes `player.update()` permanently
   loses the row under the current reading, while no authored
   `requires`/`ensures` is needed for declared fields on plain arguments,
   making receivers the one surface where declared defaults drop silently.
   Options:

   - (a) A `&mut self` receiver is a readable `&mut` referent like any
     other: the caller owes the receiver's declared-default field rows at
     the call, the callee assumes them on entry and re-proves them at
     return. Method calls preserve receiver invariants the way argument
     calls do; in exchange the receiver's declared surface becomes
     caller-facing contract.
   - (b) Receiver declared-default rows stay machine-internal: calls on
     `self` carry only ZII-satisfiable rows plus authored
     `requires`/`ensures`, established non-ZII facts retire at the
     boundary, and frame precision remains the only preservation route —
     the current implementation is the contract.

   Until answered, NOMINAL-FIELD-FLOW's receiver-handback widening stays
   open: the flow machinery already transports whatever the entry
   assumption carries, so the missing piece is the ruling, not the
   plumbing.

## Squalr scalar-scan port: surface-driven shape choices

The SCALAR-SCAN-AND-DISPATCH port hit four language-surface limits that forced
interface-shape decisions; none are silent semantic changes, but each is worth a
ruling or at least a note so later legs make the same choice:

- `Vec<T>` has no constructor until `Allocation<T>` can borrow an `Arena`, so
  the upstream eager `Vec<SnapshotRegionFilter>` result was ported as a pull
  driver (`next() -> EmittedRegion`). Emission order/content identical; the
  caller drains instead of receiving a buffer.
- `Optional<T>` scrutinees whose payload is a foreign-package type cannot be
  matched in states ("not a declared ... type in this state"), so
  `Option<ScanFunctionScalar>` became `has_scan_function_scalar: bool` beside a
  `[copy]` payload field, and encoder emissions became a package-local
  `EmittedRegion` enum instead of `Optional<SnapshotRegionFilter>`.
- Sibling-length bounds (`index: u64 [0..sibling.len]`) must be re-proven by an
  explicit guard on every call arm that passes the bounded value, so the scan
  loop forwards plain `u64` between states and enters each bounded leaf under a
  restated `index < slice.len` guard.
- Bare `machine Name::state` only resolves when `Name` is a declared data type;
  upstream unit-struct namespaces became `[copy]` marker data (e.g.
  `ScannerScalarSingleElement`).

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.
