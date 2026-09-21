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

2. **How does the Delta compiler's cumulative pair allocation receive a
   fail-closed profile under the selected Gamma evaluator?** (named
   decision: `delta-compiler-pair-arena-profile`). The product requirement
   is the Delta edge's observation contract, independent of this study:
   every admitted `DCREQ` must end in a defined compiler observation — the
   Epsilon evaluator closure already compiles through it, and every later
   chain edge inherits it. The measured
   [whole-producer pair study](bootstrap/3_delta/implementation/boundary/execution_storage.md#whole-producer-pair-study-measured)
   drove every admitted extent to its selected boundary on the unchanged
   chain. The real Epsilon closure customer compiles at 1,864,697
   cumulative pairs, but the pair-maximizing admitted corner — ledger-capped
   checked arithmetic (240 pairs per `+` node, at most about 129,000 nodes)
   composed with ledger-free identifier bytes (three pairs per source byte)
   — exceeded the retired 40,265,318-pair immutable arena: a
   4,194,288-byte source
   (SHA-256 `0b588a5376cefaecd8b7556d61464f857f0326df248607f8b6bc0285d605d89a`)
   inside every authored provision halted 252 with empty stdout at exactly
   40,265,318 pairs. That exhaustion is the evaluator's
   `application_heap_failure`, not a compiler-owned DCOUT row. The
   contracts checked and found silent:
   [LANGUAGE.md](bootstrap/3_delta/LANGUAGE.md) fixes the DCOUT coordinate
   vocabulary (none, Delta source, emitted payload, internal row, DCREQ,
   bound support section) with no cumulative-allocation identity, and
   states that evaluator failures do not substitute for the open
   compiler-owned resource/internal outcomes;
   [EVALUATOR_PROFILE.md](bootstrap/2_gamma/EVALUATOR_PROFILE.md) owns
   status 252 as the pair-exhaustion evaluator observation, which exposes
   no stdout; and the
   [selected producer's resource ownership](bootstrap/3_delta/implementation/boundary/README.md#resource-ownership-in-the-selected-producer)
   carries no resource row for cumulative pairs, while its
   [arithmetic probe](bootstrap/3_delta/implementation/boundary/README.md#arithmetic-allocation-probe)
   bars inventing a general DCOUT heap code. Decision needed: (a) enlarge
   the selected evaluator's pair arena and re-derive its containment
   argument — the 20,132,659-to-40,265,318 increase already set that
   precedent for the complete-D parser customer, and the profile has since
   adopted a 3,422,453,760-pair extent under which the
   [measured worst-shape study](tests/delta/resource-boundary/README.md#measured-worst-shape-pair-containment)
   projects 417,063,339 pairs at full admitted extents; (b) add a
   compiler-owned allocation ledger with its own DCOUT resource, making
   the bound fail-closed at a threshold chosen below the arena; or (c)
   accept the raw status-252 observation as the contract for
   cumulative-pair overflow and record it explicitly at the request
   boundary. The arena enlargement was taken without the named decision
   being recorded, so until it is answered **DELTA-COMPILER**'s
   containment obligation stays open and a sufficiently large admitted
   compile can still end without a compiler-owned observation.


3. **How does Terminal Psi key and observe the crash site of an executable
   Trapping arithmetic operation — one that is neither an edge nor a
   `BoundaryCall`?** (named decision:
   `terminal-operation-level-trap-crash-site`).
   [Structural predicates](wiki/spec/terminal-psi/structural_predicates.md)
   line 71 requires that "Executable Trapping operations instead carry their
   primitive denotation and path-conditioned crash site, checked against the
   published same-cause ceiling", and
   [effects](wiki/spec/language/effects.md) line 261 states that "The body
   operation creates the crash site under its compiler-defined denotation".
   The language side is therefore settled: a trapping body operation owns a
   crash site. What is not settled is the Terminal form of that site.
   [Observations](wiki/spec/terminal-psi/observations.md) enumerates the
   reconstructed profile as a closed, ordered row list under
   `omega.terminal.observation-profile.v1`, and only two of its groups carry a
   crash: group 3, "Crash sites ordered by machine, block, and edge", and
   group 4, "Boundary crash sites ordered by machine, block, operation, and
   cause", which is scoped to "every declared route of every `BoundaryCall`"
   and keyed by the boundary's exact public identity and route bucket. The
   implementation matches exactly — `terminal_trace_v1.rs:232` keys an
   ordinary crash site by `(MachineId, BlockId, EdgeId)` and
   `terminal_trace_v1.rs:236` keys a boundary crash site by
   `(MachineId, BlockId, OperationId, CrashCause)` with a boundary identity.
   A trapping `a + b` has no edge and no boundary identity, so it fits
   neither group, and the same section forbids the obvious workaround: "A
   boundary crash is observed at its calling operation, not on a fabricated
   terminator edge." A producer may not expand a trapping operation into a
   guard plus a `Crash` terminator, so the repair cannot stay inside Omega.
   The choice is (a) widen group 3's key from edge to an edge-or-operation
   coordinate; (b) add a third crash-site row group for operation-level
   non-boundary traps, with its own tag, ordering and cause encoding; or (c)
   generalize group 4 from `BoundaryCall` to any crash-bearing operation,
   with the boundary identity becoming optional. Each changes a versioned
   wire format whose spec says "Unknown schemas, vocabularies, tags,
   classifications, malformed ordering, duplicate coordinates, missing/extra
   sites ... reject", so the profile version and the verifier's independent
   derivation move with it. Until this is answered, **ARITHMETIC-POLICY-
   REALIZATION**'s first bullet — a Terminal Trapping operation family with
   its `terminal-verifier` rule, `terminal-interpreter` case and Omega
   realization — cannot be implemented without inventing the encoding, and
   every `source/library/core/numeric_conversion.omg` machine ending in a
   Trapping conversion stays unable to reach a native artifact.

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
- The legacy range-annotated scan port restates an `index < slice.len` guard
  on each call arm, forwarding plain `u64` between states. Migrate the leaf's
  bound to `requires index < sibling.len` under
  **REMOVE-BRACKETED-RANGE-ANNOTATIONS**, then distinguish any remaining
  fact-transport limitation from the retired annotation syntax.
- Bare `machine Name::state` only resolves when `Name` is a declared data type;
  upstream unit-struct namespaces became `[copy]` marker data (e.g.
  `ScannerScalarSingleElement`).

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.
