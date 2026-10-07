# Typed trees to typed trees

Start at [preparation.rs](src/preparation.rs). This stage owns typed-program
mutation: specialization, conformance elaboration, selected token calls and
receiver/result rebinding. It returns the ordinary typed representation sealed
as `FrozenTypedTrees`, plus retained source judgments for checking. There is no
additional program IR and no dependency on the following checking transform.

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

The `03b` directory inserts this same-form transform between typing (03) and
checking (04) without renumbering their existing downstream consumers.

## Specialization

[Specialization](src/monomorphization/mod.rs) runs explicit fixed-point rounds:
discover call tuples, validate bounds, prepare the round's contract analyses,
materialize missing instances, then rewrite calls and recheck selections.
Authored templates remain generic and unchanged. Application candidates borrow
the round's immutable template metadata while owning their bindings.
[Selection](src/monomorphization/selection.rs),
[body cloning](src/monomorphization/body_cloning.rs),
[body rewriting](src/monomorphization/body_rewriting.rs), and
[identity encoding](../../checking/src/machine_calls/contract_identities.rs) own those mechanisms.
Cloned runtime calls resolve subjects in a state-owned traversal rather than
rediscovering their enclosing statement trees for each call. The collected
subjects preserve the first lexical owner and exact generational handles.
Separate saved operator-provider templates retain their distinct-source path.

Operator-candidate elaboration prepares its immutable authored snapshot and reach
summary once per invocation, only when generic checked candidates exist. Build
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
