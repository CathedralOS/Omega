# Atlas packet unit_072: typed callables and definitions

Packet complete. Every ordinary compiler-correctness observation in this slice has a receipt.

One claim led to a code change. When `declared_binding_type_by_symbol` found no unique binder in a state's own roster, it fell back to block-value bodies and returned the first local with that symbol. It now refuses a binder the body declares twice (R13). Two claims are still open: user traits named `Calling` and `CallingPolicy` stand in for the standard calling vocabulary (R12), and a requirement header whose trait operands fail the ownership check is left unprepared on the call route (R9). The other claims are intended.

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

### R8. A forwarded generic parameter becomes a const scalar without a carrier check

- **Claim.** [05:156](../../digests/05-source-representations.md?plain=1#L156); record §10 "Fail-open risks", third bullet. `Operand::read` accepts a `TypeParameter` symbol for a const binder without comparing its carrier with the binder's.
- **Source.** The check is now in `closed_scalar`, `TT/callables/static_operand_binding/mod.rs:349-375`; its consumer is `:186-227`.
- **Contract.** "Named constants and forwarded binders must match that carrier before canonical identity erases their source names" ([generics](../../../../../wiki/spec/language/generics.md), the paragraph before "Transparent type aliases").
- **Test.** A throwaway probe through the stage 06 handoff harness (`P/06_lowered-program-to-checked-program/tests/support/source_handoff.rs`, `Program::try_of`) on `take<const N: u32>` called from `pass<…>`, with `main` supplying the actual argument. The probe was not committed.
  - `pass<const M: u32>` forwarding `M`: lowered and checked.
  - `pass<const B: bool>` forwarding `B`: stage 05 refused with "a static scalar does not have its binder's carrier".
  - `pass<const M: u8>` forwarding `M`: stage 05 refused with "a static integer is outside its declared carrier".
  - `pass<T>` forwarding the type `T`: stage 05 refused with "a static scalar requires its canonical closed value".
- **Outcome.** Intended. Specialization substitutes the caller's closed value, and the binder's carrier check refuses the mismatch. All three refusals are reported as unsupported specialization ("no pass specializes yet") rather than as a carrier error at the forwarding site. That diagnostic gap is outside this claim.

### R9. A failed ownership check on a requirement's trait operands returns `Ok(None)`

- **Claim.** [05:156](../../digests/05-source-representations.md?plain=1#L156) and [05:193](../../digests/05-source-representations.md?plain=1#L193); record §12.
- **Source.** `TT/callables/static_operand_binding/requirement.rs:86-91`; callers `TT/definitions/product_operand/formation/applications.rs:796-806`, `:876-893` and `P/04_typed-trees-to-prepared-typed-trees/passes/specialization.rs:1136-1149`; stage 05 consumer `P/05_prepared-typed-trees-to-lowered-program/src/read/calls.rs:102-124`.
- **Trace.** The Product requirement route turns `None` into the refusal "a requirement application with no canonical closed header". The call route in stage 04 leaves the call aimed at the generic requirement with its machine arguments in place. Stage 05's `applies_nothing` refuses that call while written static arguments remain. The second product-operand call (`:876`) discards the result. Nobody has traced a requirement whose arguments are all inferred, so no machine arguments remain to trigger the stage 05 refusal.
- **Outcome.** Still open. Next action: a two-file positive/negative twin in which a requirement's trait argument names a binder owned by another declaration, called without explicit static arguments.

### R10. `compiler_primitive_schema` returns `Ok(None)`; `termination_schema` reports `"inferred"`

- **Claim.** [05:160](../../digests/05-source-representations.md?plain=1#L160); record §14 "Fail-open risks".
- **Source.** `TT/definitions/compiler_intrinsics.rs:26-60`, `:478-488`.
- **Trace.** `None` is returned only for a symbol that is not a machine, or for a machine that is not compiler-provided and has no compiler-primitive custody. That is an ordinary machine with its own body. A compiler-provided machine without sealed custody is an error (`:37-43`), as is a primitive with a body or the wrong supply mode. The termination text is an interface description, not termination evidence (comment at `:478-480`).
- **Outcome.** Intended.

### R11. A generic-carrier domain accepts any carrier

- **Claim.** [05:160](../../digests/05-source-representations.md?plain=1#L160); record §16 "Fail-open risks", first bullet.
- **Source.** `TT/definitions/declarations.rs:1315-1329`, `:1358-1392`; callers `P/03_symbol-resolved-trees-to-typed-trees/src/type_reference.rs:369-390` and `P/02_syntax-trees-to-symbol-resolved-trees/selections/domain_operator_homes.rs:230`.
- **Contract.** A carrier-generic domain's integer-family requirement is "a generic admission obligation" that rejects an ineligible application ([integer carrier constraints](../../../../../wiki/spec/language/generics.md#integer-carrier-constraints)).
- **Trace.** The function filters name candidates during domain selection. For `domain D<T> on T`, every carrier is a possible `T`, and the application then owes the domain's requirements. The unit fixture at `TT/callables/callable_applications.rs:1565-1577` covers both accepted and refused carriers for fixed-carrier domains.
- **Outcome.** Intended for selection. The admission obligation belongs to the domain application, not to this filter.

### R12. User traits named `Calling` and `CallingPolicy` become the standard calling vocabulary

- **Claim.** [05:160](../../digests/05-source-representations.md?plain=1#L160); record §16, fifth bullet; record "Five things", item 3.
- **Source.** `TT/definitions/declarations.rs:504-615`; consumers `TT/definitions/service_declarations.rs:2918` and `P/05_prepared-typed-trees-to-lowered-program/src/read/calling.rs:384`.
- **Contract.** "`Calling<Policy>` takes an explicit named implementation parameter declared `const Policy: CallingPolicy`", and bundled policies live in the system library ([calling plans](../../../../../wiki/spec/build/calling_plans.md#authorship-and-evaluation)). The vocabulary traits are declared once, in `source/library/std/calling.omg:291-297`.
- **Trace.** Any non-boundary, non-refining pair with those two names in one file qualifies if its shape matches. The source file, package and sealed custody are never checked. A user package can therefore declare a lookalike `Calling` and have boundary traits that require it treated as selecting a calling policy. The selected plan is still evaluated and validated (`calling_plans.md`, "An accepted value is still subject to compiler validation"). No wrong accepted program was produced in this review. The CLI is not built in this session.
- **Outcome.** Still open. Next action: restrict recognition to the bundled `std/calling.omg` owner, as [#5402](https://github.com/CathedralOS/Omega/pull/5402) did for the core content algebra, with a user-copy negative.

### R13. The block-value fallback of `declared_binding_type_by_symbol` returns the first match

- **Claim.** [05:161](../../digests/05-source-representations.md?plain=1#L161); record §17 "Fail-open risks".
- **Source.** `TT/definitions/declared_bindings.rs:383-430`; consumers `P/03_symbol-resolved-trees-to-typed-trees/src/typing_queries.rs:237` (assignment storage, refused as "unique assignment storage declaration") and `:466` (contract value binders).
- **Contract.** The state's own roster answers only for a binder declared exactly once (`binding_position`, `:466-467`, `:482-484`), and the existing fixture `binding_lookup_preserves_owner_duplicates_and_generations` pins that refusal.
- **Reproduction.** `definitions::declared_bindings::tests::block_value_bindings_require_one_declaration` builds a state with a parameter, one roster local and a block-value statement. Before the change it failed at `tests.rs:185`: a binder declared twice inside block values returned the first declaration's type (`left: Some(..)`, `right: None`). In the old code, a binder repeated in the state's own roster also fell through to the block scan and returned a third copy's type. The test's last assertion pins that case.
- **Repair.** When the roster has no unique answer, the fallback now refuses a symbol the roster declares at all, and otherwise requires exactly one block-value declaration.
- **Proof.** `mbx test -p typed-trees --lib`: 250 passed, including the new test and the 14 existing `declared_bindings` tests.
- **Outcome.** Fixed (representation contract). No source program that produces a duplicated block binder was found, so no end-to-end reproduction exists.
- `first_named_value_declared_type` stays first-match by design (`:519` in the record; comment "Preserve the legacy first-match query").

### R14. `bindings_satisfy_known_multiplicities` skips `[copy]`/`[linear]` for open carriers and missing bindings

- **Claim.** [05:162](../../digests/05-source-representations.md?plain=1#L162); record §21 "Fail-open risks", first bullet.
- **Source.** `TT/definitions/operator_declarations.rs:2431-2505`, `:2649-2672`.
- **Test.** A throwaway probe through the same stage 06 harness used the core by-copy read `read<Element [copy]>`, which [expressions](../../../../../wiki/spec/language/expressions.md#contract-gated-core-operations) declares. It was not committed.
  - `get<T [copy]>(items: @[T; 2]) -> T { items[0] }` called with `u8`: lowered and checked.
  - The same body with an unbounded `T`: the generic form was refused with "expression owned extraction from borrowed storage".
  - That body called with an affine `Coin`, and a direct `b[0]` read of `[Coin; 2]`: stage 05 refused both with the same message.
- **Outcome.** Intended at candidate selection. An open carrier remains a candidate, and the by-value use is refused downstream. The generic refusal is reported as unserved rather than as a multiplicity rejection.

### R15. `closed_boundary_application_nominal` accepts integer-looking names

- **Claim.** [05:162](../../digests/05-source-representations.md?plain=1#L162); record §21, second bullet.
- **Source.** `TT/definitions/operator_declarations.rs:379-439`; producer `P/03_symbol-resolved-trees-to-typed-trees/src/type_reference.rs:1008-1016`.
- **Trace.** Stage 03 replaces each closed const argument with `Named { symbol: invalid, name: <canonical value> }`. Those canonical atoms are the integer-looking names this branch accepts. An authored identifier cannot be an integer.
- **Outcome.** Intended.

### R16. The name fallback compares unresolved types by spelling

- **Claim.** [05:162](../../digests/05-source-representations.md?plain=1#L162); record §21, third bullet.
- **Source.** `TT/definitions/operator_declarations.rs:4436-4450`.
- **Trace.** If either side has a valid symbol, the match requires equal symbols. Spelling is compared only when both symbols are invalid, which is the canonical const-argument form from R15. Equal canonical spellings are equal canonical values.
- **Outcome.** Intended.

### R17. `resolve_named_call` returns the operator without checking arity or path

- **Claim.** [05:162](../../digests/05-source-representations.md?plain=1#L162); record §21, fifth bullet.
- **Source.** `TT/definitions/operator_declarations.rs:752-820`, `:896-927`.
- **Trace.** `resolve_named_call` still has no caller in the repository. Its expression twin `resolve_named_expression_call` has one caller, `selected_named_expression_call_has_no_runtime_receiver`, and that caller checks the argument count and the written path before answering (`:903-926`).
- **Outcome.** Intended. The unused function is dead code, not a correctness defect.

### R18. `fixed_array_lengths_match` falls back to equality

- **Claim.** [05:162](../../digests/05-source-representations.md?plain=1#L162); record §21, last bullet.
- **Source.** `TT/definitions/operator_declarations.rs:4355-4395`; `FixedArrayLength` at `TT/type_records/type_system.rs:3208-3221`.
- **Trace.** The fallback is the derived equality. It compares a const parameter by symbol and name and an expression by handle, so it accepts only the same symbolic length. The name-only comparison applies only when `allow_name_fallback` is set and the actual length is not closed, which is R16's canonical case.
- **Outcome.** Intended.

### R19. A parameterless Product operand has no application identity

- **Claim.** [05:163](../../digests/05-source-representations.md?plain=1#L163); record §22.
- **Source.** `TT/definitions/product_operand/applications.rs:54-63`; consumers include `driver/compile_product/declaration_lookup/root_grants.rs:58-66`, `driver/compile_product/retained_application.rs:98-107` and `P/05_prepared-typed-trees-to-lowered-program/src/read/provider_selections.rs:104-113` (paths under `source-to-psi/`).
- **Trace.** Root grants pair `None` with the selected declaration's own hermetic identity. Retained provider choices refuse an identity that differs from the recorded one. Stage 05 provider selection refuses a missing or empty identity.
- **Outcome.** Intended. A nominal declaration without arguments is identified by its declaration.

### R20. Behavior the record or digest already marks as stated

These were checked at the cited current lines and still match their stated reason. Outcome: intended.
- [05:160](../../digests/05-source-representations.md?plain=1#L160): `boundary_requirement_completion` returns normal return unless it recognizes the process-exit signature (`TT/definitions/declarations.rs`, comment that it recognizes "its signature, not an emptiness proof"). `DomainIssuance::Unrestricted` is documented for a domain with no policy and no routes.
- [05:156](../../digests/05-source-representations.md?plain=1#L156): an affine binder admits any type (`TT/callables/static_operand_binding/mod.rs`, `type_argument_meets_bounds`).
- Record §16: `numeric_intrinsic_execution` returns the token execution first, and `numeric_token_execution` checks the parameters (traced in the record).
- [05:196](../../digests/05-source-representations.md?plain=1#L196): `callable_description_sources` skips candidates it cannot identify. A skipped candidate cannot add a match, and an unmatched request is an error (`TT/definitions/declaration_reflection.rs`).
- Record §7: `fact` marks an invalid index argument through `substitution::rejected`, which becomes `UnresolvedType` (traced in the record).

## Observations in this unit that are not correctness claims

These are left with their original status, for the lane that owns them:
- Identity text built with Rust `Debug` output ([05:48](../../digests/05-source-representations.md?plain=1#L48); record "Five things", item 2). This is a design concern about identity stability, with no wrong result claimed.
- The `dense_id_maps` ratchet count for `declarations.rs` ([05:248](../../digests/05-source-representations.md?plain=1#L248)). This belongs to repository tooling.
- Unused code: `source_call_occurrences`, the statement control-graph helpers, `resolve_named_call` and others (record "Five things", item 1; [05:256](../../digests/05-source-representations.md?plain=1#L256)).
- The record found no fail-open path in `machine_specializations.rs`, `private_callback_slots.rs`, `build_selections.rs`, `native_locators.rs` or `numeric_declarations.rs`, and the schema fingerprint returns no acceptance. This review did not reopen those files.

## Validation

- `mbx test -p typed-trees --lib`: 250 passed. Without the repair, the new test failed at `declared_bindings/tests.rs:185`.
- Stage 06 handoff probes (R8, R14) ran under `mbx test -p source-compiler --features test-support --test checking_runtime_callables atlas_probe -- --nocapture`. They were temporary additions to `runtime_callables.rs` and were removed before commit.
- `mbx clippy -p typed-trees --all-targets -- -D warnings`: clean.
- `rustfmt --check` is clean for the two changed Rust files. `mbx xtask fmt --check` reports existing differences in unrelated files (for example `source-rust/application/tests/`) on this base.
- The `omega` CLI was not built in this session, so there is no end-to-end CLI run.
