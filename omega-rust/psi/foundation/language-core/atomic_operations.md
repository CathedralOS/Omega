# Atomic vocabulary and downstream admission

Contract: [concurrency and atomic observation](../../../../wiki/spec/language/concurrency.md).
The [atomic vocabulary](src/atomic.rs) defines the shared atomic orderings. The [access-plan crate](../access-plans/README.md)
validates placed permissions, resident ownership and specialized requests.

Keep decisive/single-attempt and observing/non-observing independent. Recognizing an ordering or publishing a core outcome
type does not accept the source operation, its result ownership, a Terminal
event or a target realization. An unsupported single-attempt route must reject
rather than collapse `Uncommitted` into a decisive scalar result.
Generic outcomes keep the payload's multiplicity; neither result shape nor
zero initialization proves an event happened.

Every atomic operation takes shared access to its storage, recorded in the
access-plan vocabulary as `AccessOperation::receiver_polarity`. The
lowered-form checker grants shared access only to an authenticated atomic event
on dedicated atomic storage; ordinary assignment still needs exclusive access.

The source checker, interpreter and native code must use the same operation
and the prior value the instruction observed. A separately
loaded prior is not a valid swap, fetch or exchange. Target retries still
count as work when the source has one primitive event.

## Serial producer route

The lowered-form reader decodes the parser's atomic syntax once in
`typed-trees-to-terminal-psi/src/read/atomics.rs`. A writing operation's result
placeholder must name the exact local that receives that prior value. The
reader keeps the place and authored operands apart and uses the existing `AtomicAccessEvent` throughout lowering; it never
loads the prior value separately. Lowering evaluates the place and
operands once and emits one `AtomicAccess` with the complete rooted path.

The lowered-form checker verifies dedicated atomic storage, exact operand and
result types, orderings and shared access. A modifying event invalidates its
exact storage path's facts; it establishes no plain-value equation. Ordinary shared writes and competing exclusive loans stay forbidden.
This route currently accepts unbounded builtin atomic loads, stores, integer
fetch operations and swap. Bounded residents and single-attempt source
operations are still refused; their event, codec and target representations
below do not by themselves make the source accepted.

The codec's independent operation verifier
(`terminal-psi/codec/src/foundation_validation.rs`)
and the interpreter consume that event. The verifier refuses illegal orderings,
runtime-chosen locations, bounded writes that lack their checked resident
obligation, mistyped operands or results, and events on ordinary scalar
storage.
`StructuralTypeShape::AtomicCell` keeps a whole cell's resident contract;
`StructuralFieldType::Atomic` and `AtomicBoundedInteger` keep the same contract
for inline fields. The shared endpoint query supplies resident type and
bounds without inventing a containing field. Shared loans permit atomic events only,
not stores or non-atomic snapshots. An exact typed atomic loan keeps
that contract and never becomes a plain scalar reference. Interpreter backing
selection and native layout use the same complete endpoint.

Single-attempt compare-exchange binds one structural local directly. Its
receiver and two operands are evaluated as a list, not as an ordinary call. The source checker requires the sealed canonical outcome with the exact
resident type; Terminal admission rechecks its three-case schema, operand
types and orderings. Both single-attempt families share this event. The
result contract separates observing failure payloads from non-observing
proposal payloads, and from the displaced value a non-observing success
returns. The result has its declared multiplicity: the observing scalar
outcome is copyable, while the non-observing outcome stays affine (not
copyable) even when the resident scalar is copyable.

The non-observing scalar route uses the dedicated cell's builtin identity
representation for its resident and comparison key; the receiving outcome
type never supplies that correspondence. Custom keys,
user-chosen encodings, affine residents and placed-only permissions need their
own acceptance evidence; this scalar route must not infer them.

Serial execution commits a matching attempt and invents no interference. Native instruction selection keeps the instruction's
three-way tag: x86 CMPXCHG always decides the attempt, while an AArch64
exclusive store may leave a matching attempt Uncommitted. The
non-observing family returns the proposal on failure and the observed
displaced value on commit. Equal key and prior value do not replace the
commit tag.

Omega's terminal-to-abstract lowering
(`00_terminal-psi-to-abstract-operations/src/lowering_storage.rs`) keeps each
Terminal event as one `AbstractOperation::AtomicEvent` at the same
`AbstractAtomicLocation` (root place and complete cell path). It adds the
`reads_from` and `modification_after` edges that the optimization unit's
happens-before replay rechecks. A backward latest-write analysis follows
the actual whole-storage edge bindings, so a state parameter keeps the same
modification history under its own place identity. The independent unit
checker rebuilds that relation from its current edges and rejects substituted
backing, stale initial-residency claims and unknown storage.
Different known paths keep a canonical list of all latest residencies,
including the initial one and a prior iteration of the same event. Replay
requires that exact list and grants no observed-value equality or
cross-activation synchronization. A non-atomic write, or any call lending the
atomic backing, between two events ends knowledge of the earlier writer.
The next event may keep an `AfterEffect` boundary naming that exact latest
operation on every path; replay rebuilds it without claiming which internal
write supplied the resident or recovering an old value.
Shared access does not prove an atomic resident unchanged. Source and
Terminal proof composition each drop observations that shared calls may
change; copied SSA values survive.
The event keeps its block-local root and the ordinary shared receiver path,
including crossings through stored references. Arrival and local declarations
supply access and layout through target lowering and independent native
selection. Ambiguous edge bindings still refuse rather than guess a storage
identity.

## Portable fence route

The sealed `core/atomic.omg` declaration supplies `Atomic::fence` and its
copyable `FenceOrdering` cases. The checker selects one proved case from the
argument's value when the operand completes; runtime-unknown or overwritten
cases reject. Calls producing the ordering still run once. Only
the exact sealed symbol and case handles invoke the primitive; an authored
machine with the same spelling keeps its ordinary body.

Terminal `AtomicFence` keeps the ordering without a location, result or
observation edge. Omega keeps it through abstract, target and legalized
operations. Selection uses a register-free external-effect
instruction, emitting `DMB SY` on AArch64 and `MFENCE` on x86-64 for all three
orderings. These full barriers are at least as strong as requested; the
effect catalog and independent byte decoder tell them apart from a no-op or
storage access. The Terminal interpreter steps past the event without
inventing concurrent participants or synchronization. A fence alone proves no
publication.

Instruction mappings and serial differential tests do not prove concurrent
behavior. Complete memory-model and fence axioms, target-refinement proofs,
coverage under real concurrent activations, and proof-scoped weaker
acquire selection remain separate items in [TASKS.md](../../../../TASKS.md).
