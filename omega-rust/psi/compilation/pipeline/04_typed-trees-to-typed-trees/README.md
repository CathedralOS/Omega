# Typed trees to typed trees

Start at [preparation.rs](src/preparation.rs). This stage owns typed-program
mutation: specialization, conformance elaboration, selected token calls,
service-call binding and result rebinding. It returns the ordinary typed
representation sealed as `FrozenTypedTrees`, plus retained source judgments for
checking. There is no additional program IR and no dependency on the following
checking transform.

`PreparationRequest` carries this invocation's pending-input checkpoint,
privileged-instruction authority, representation receipts and authored wire
demands. Admission runs before any exact-input checking memo can reuse a result.
The opaque preparation result retains its producer's checkpoint and judgments;
callers cannot replace its sidecar fields or choose a second checkpoint for checking.
Package declaration authority belongs to the following checking stage; runtime
provider selection belongs to Omega and is not a source-elaboration input.

Shared readonly declaration, call, operator, place and proof-contract queries
remain independently owned by `validation` and `proof`. Preparation uses those
queries and owns rewrites; checking reads the frozen output and publishes its
conclusions into `CheckFacts`, never into the typed program.

## Specialization

[Specialization](src/monomorphization.rs) runs explicit fixed-point rounds:
discover call tuples, retain their exact bindings,
materialize missing instances, then rewrite calls. Source operand formation
does not run in those rounds. Complete array, record and borrow result types
are derived together in child-before-parent order before tuple discovery:
borrowing a constructed value needs that value's type, not a declared-place
approximation. Unchanged type vocabulary is reused across rounds and final
preparation. This derivation establishes no loan, ownership or field predicate;
immutable checking still owns those judgments.
[Authored selections](src/preparation.rs)
retain each application's declaration, operands and lexical owner before
folding; cloning copies and remaps that custody. Immutable checking owns
[argument admission](../../checking/src/machine_calls/static_arguments.rs),
including unused operands and typed const landing, and
[tuple-bound admission](../../checking/src/machine_calls/static_bindings.rs).
Calls and instances retain binder-keyed type, const, runtime-carrier, machine
and conformance operands in the existing typed application arenas. Their
original declarations own the requirements; display identities do not grant
admission. Conformance closure is fallible derivation before clone allocation,
not an assumption that bound fitting succeeded. Private compile-time programs
admit once before execution. Universal template and application commitments
belong to immutable checking: it reads the retained generic declarations and
their completed fixed-service/binder dependencies, then publishes the digests
in checked specialization facts. Derivation neither infers effects nor hashes
an early graph snapshot.
Authored templates remain generic and unchanged. Application candidates borrow
the round's immutable template metadata while owning their bindings.
[Selection](src/selection_and_reflection.rs),
[body cloning](src/body_cloning_and_rewriting.rs),
[body rewriting](src/body_cloning_and_rewriting.rs), and
[identity encoding](../../checking/src/machine_calls/contract_identities.rs) own those mechanisms.
Cloned runtime calls resolve subjects in a state-owned traversal rather than
rediscovering their enclosing statement trees for each call. The collected
subjects preserve the first lexical owner and exact generational handles.
Separate saved operator-provider templates retain their distinct-source path.

Boundary requirement calls bind to the exact requirement before
specialization. A newly substituted instance binds only its own service calls
at publication. Static call targets are copied and remapped with the body;
there is no projected data-method search, clone-time receiver rescan, or final
whole-program receiver repair. Requirement selection preserves the exact
lexical root and field path without inserting a data argument.

Selected trait token operations use the ordinary requirement-call route too.
Before cloning, an operation selected by a lexical evidence binder becomes
the same exact child call as `Operations::combine(left, right)`. Closed
applications retain their requirement dispatch and complete implementation
arguments. The common call emitter preserves operand order and single
evaluation; there is no separate selected-operator interpreter or builtin
arithmetic fallback. Authored comparison meaning is retained before freeze
so local index equalities cannot treat an arbitrary Boolean relation as
mathematical equality.

Operator-candidate elaboration prepares its immutable authored snapshot once
per invocation, only when generic checked candidates exist. Build
selection is not an input: all authored conformance candidates participate.
Fixed-point rounds
avoid a working graph copy when all demanded tuples already have exact receipts.
An operator candidate with new tuples still gets isolated copy/type-import storage;
that copy is not shared mutable template authority.

This removes the per-template whole-program snapshot, not all copying or
whole-program scans. Selected graphs are copied through staging and then into
their destination; generated names and evidence still allocate. Shared source
payloads retain their existing owners. Compiler behavior is qualified through
the ordinary corpus route, not a synthetic specialization-only program builder.

## Policy operators

[Policy operator typing](src/policy_operators.rs) runs first in every
preparation entrance. A suffixed operator such as `a +% b` arrives from typing
as its base operation with the chosen policy on the node, because the operand
carrier is not known until the whole program is typed. This pass reads the
carrier and rewrites the operation as the same operator on operands cast into
the matching policy domain, with the result cast back to the plain carrier.
Checking, lowering and both engines therefore see only the policy-qualified
arithmetic they already implement; there is no second definition of wrapping,
saturating or trapping arithmetic.

Typing admits a suffixed operator only in executable statements, which are
what this pass walks. The policy stays on the node until it is rewritten, so
a cloned or projected program keeps it, and checking rejects any suffixed
operation that reaches it instead of treating it as the bare Exact operator.

This rewrite is scaffolding for the order the design record sets. Its step 5
inverts the definition: `omega::core` declares the policy domains' operators
through the suffixed forms, and the suffixed operation becomes the primitive
this pass currently spells with casts.

## Record pattern rosters

`let { a, b as _ } = subject;` reaches this stage as a marker local
(`LocalBindingRole::RecordPattern`) whose roster holds one row per field the
author named, bound or waived, still only as spelled. The last preparation
step, [`record_patterns::prepare`](src/lib.rs), resolves each roster against
the subject's declared record, which only now is settled for specialized
bodies too: every row receives the symbol of the field it names
(`TableMatchField::field_symbol`) and the marker the record's own symbol
(`TableLocalData::pattern_subject`). A row naming no field, or a spelling two
fields share, stays without a symbol, and a subject that is not a plain record
leaves the roster unresolved.

Checking (`validation`'s `validate_destructure_exhaustiveness`) and
[structure-only elaboration](../structure-elaboration/README.md) then decide
coverage from those symbols alone; neither reads a row's spelling, and the
marker's name encodes nothing.
