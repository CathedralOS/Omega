# Atlas packet unit_072: typed callables and definitions

Status: in progress. The receipts below are verified; the rest of the slice is still under review.

This packet reviews the ordinary compiler-correctness observations that
`records/unit_072.md` made about `typed-trees` callables and definitions, and the
lines of [area 05's digest](../../digests/05-source-representations.md) built from
them. Each receipt checks the claim against current source on `main` at
`24ad2700685e`. The survey cited `4f3c255d0559`; line numbers below are current.
No gap packet covers area 05 (digest line 1).

Paths below are relative to
`source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/` (`TT/`)
or to `source-rust/subsystems/compiler/source-to-psi/pipeline/` (`P/`).

## Receipts

### R1. A receiverless call is classified as having no runtime receiver

- **Claim.** [05:154](../../digests/05-source-representations.md?plain=1#L154); record §1 "Fail-open risks", first bullet. `call_has_no_runtime_receiver_for_target` returns true at the selected-operator branch, the static-record-bound branch and for `!call.receiver.is_valid()` after the single-trait scan.
- **Source.** `TT/callables/calls_and_control_flow.rs:420-504`; consumers `P/05_prepared-typed-trees-to-lowered-program/src/read/calls.rs:897`, `read/byte_observations.rs:96`, `TT/proofs/evidence_and_inspection.rs:37`.
- **Contract.** The predicate answers whether the written receiver is a qualifier rather than an extra runtime argument (comment at `read/calls.rs:893-895`). A true answer can only drop a receiver expression from the argument list.
- **Trace.** At `:496` the call has no receiver expression, so nothing can be dropped. The operator branch (`operator_declarations.rs:896-927`) requires the argument count to match the operator's parameters and the written namespace to match the operator's path. The static-record branch requires `static_record_bound_requirement` to return the bound requirement for the receiver symbol, which is a static record and so has no runtime value. The two other consumers use the predicate only to refuse or to skip a byte-observation shortcut.
- **Outcome.** Intended.

### R2. A missing identifier reads as empty text

- **Claim.** [05:154](../../digests/05-source-representations.md?plain=1#L154); record §1, last fail-open bullet. `Identifier::as_str` returns `""` for `Missing`, so a missing name compares equal to an empty written name.
- **Source.** `TT/callables/calls_and_control_flow.rs:2876-2951`.
- **Contract.** `Missing` is the `Default` sentinel; `PartialEq` compares text so that `Static` and `Generated` storage of the same name are equal.
- **Trace.** The lexer produces no zero-length identifier, so no authored name equals the sentinel. The name comparisons the record cites in `call_has_no_runtime_receiver_for_target` (`:458`, `:475`, `:492`) run only after the target symbol has matched a declared requirement.
- **Outcome.** Intended. No authored input reaches an empty-name match.

### R3. `static_machine_binding` returns `Ok(None)`

- **Claim.** [05:154](../../digests/05-source-representations.md?plain=1#L154); record §2 "Fail-open risks". A caller reading `None` as "not statically bound" would treat the call as dynamic.
- **Source.** `TT/callables/machine_instances.rs:607-675`; only production caller `P/05_prepared-typed-trees-to-lowered-program/src/read/calls/callback_bindings.rs:39-43`.
- **Trace.** The caller runs only for a valid binder and turns `None` into the refusal "static call target is not selected by its binder". The other use is the stage 04 test at `P/04_typed-trees-to-prepared-typed-trees/passes/specialization/tests.rs:2234`.
- **Outcome.** Intended. The only production consumer refuses.

### R4. Map-capture index discards insert errors; `get` can return `Ok(None)`

- **Claim.** [05:154](../../digests/05-source-representations.md?plain=1#L154) and [05:193](../../digests/05-source-representations.md?plain=1#L193); record §3.
- **Source.** `TT/callables/machine_instances/map_captures.rs:39-156`; consumers `P/05_prepared-typed-trees-to-lowered-program/src/read/calls.rs:462-468` and `P/04_typed-trees-to-prepared-typed-trees/passes/specialization/named_conformances.rs:103`.
- **Trace.** A rejected record is absent from the index, and two records with one key collapse to `None` (`:59-65`). Stage 05 turns both an error and `Ok(None)` into the refusal "named map body has no prepared capture", then rechecks the owner, the single entry and the template correspondence (`read/calls.rs:469-496`). Stage 04's incremental `insert` propagates its error as `Stop::Defect`.
- **Outcome.** Intended, as the comment at `map_captures.rs:42-43` states.

### R5. An authored service reach adds nothing to the callable identity

- **Claim.** [05:155](../../digests/05-source-representations.md?plain=1#L155); record §7 "Fail-open risks", first bullet. Two signatures whose authored rows differ but whose resolved rows are equal get the same identity text.
- **Source.** `TT/callables/runtime_callable_identity.rs:1061-1110`; producer `P/02_syntax-trees-to-symbol-resolved-trees/selections/service_reaches.rs:150-240`.
- **Trace.** Stage 02 resolves each pending `reaches` clause once, and that call returns both the signature's `service_reach_row` and its authored row (`service_reaches.rs:206-213`, `:233`). The identity encodes every service of the resolved row (`:1079-1095`) and adds each static dependency, which the row does not hold (`:1102-1103`). An authored service is already in the resolved row, so the identity only checks that its definition exists.
- **Outcome.** Intended. The identity follows the resolved reach set. Two signatures with equal resolved rows have the same reach.

### R6. Carry is always `"omitted"` in the callable identity

- **Claim.** [05:155](../../digests/05-source-representations.md?plain=1#L155); record §7, second bullet.
- **Source.** `TT/callables/runtime_callable_identity.rs:468-477`.
- **Outcome.** Intended, as the comment at `:473` states.

### R7. A stored callback roster is reused without recomputing its text

- **Claim.** [05:156](../../digests/05-source-representations.md?plain=1#L156); record §9 "Fail-open risks".
- **Source.** `TT/callables/static_callback_contracts.rs:79-81`, `:147-180`, `:315-406`.
- **Trace.** `prepare_static_callback_requirements` reuses the roster only when `static_callback_requirements` accepts it. That check requires one row per callback binder with the same parameter, the same source application, the original binder signature, the same lexical owner, an equal binding snapshot and equal lifetime substitutions. These are all the inputs of the substituted signature. Frozen decode runs a separate check, which `frozen_recovery_rejects_callback_contract_edits_with_the_same_symbol` (`:542`) covers.
- **Outcome.** Intended. The reuse depends on every input of the computation.
