slice_0028

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0208 | expression_bindings.rs:4434 | CLOSED | Sole production caller checks readable custody on `home.access` before calling `structural_observation_path` (scalar_computations.rs:4404-4407, 4360-4362). |
| A0209 | expression_preparation.rs:661-662, 743-747 | CLOSED | Empty match arms are leaf kinds; scalars are collected earlier via `call_schedule` and `value_dependencies` closure from checked-trees (values_and_views.rs:1342-1395). |
| A0210 | expression_preparation.rs:388-391 (relocated) | CLOSED | Citation relocated to unwrap_or sites; absent schedule means no effect calls on that value in well-formed checked IR (values_and_views.rs:1398-1416). |
| A0211 | expression_preparation.rs:2144-2148 | CLOSED | Sanitization before terminal seal; ill-formed catalogs fail `foundation_validation` (foundation_validation.rs:4700-4713). |
| A0212 | expression_preparation.rs:1668-1685; operation_emission.rs:269-271 | CLOSED | `UnresolvedStructuralParameterField` is rejected in validation before emission (emission.rs:9489-9491, 9690-9692; scalar_lowering.rs:311-312). |
| A0213 | expression_preparation.rs:3787 | CLOSED | `ArithmeticDomain` is overflow semantics, not a semantic qualification atom; other constraints hit `unsupported` at 3833-3836. |
| A0214 | expression_preparation.rs:3796-3813 | CLOSED | Skip arms require existing checked refinements or validation predicates; locals enforce `scalar_type_refinement_predicates` (unit_operation_frame.rs:3479-3499). |
| A0215 | expression_preparation.rs:4449 | CLOSED | `_ => {}` runs after `qualification_decisions` and `call_schedule` operand walks (4324-4335, 4312-4322). |

### A0208

1. Read `structural_observation_path` in full at expression_bindings.rs:4422-4452 (read).
2. Search `rg 'structural_observation_path' /agent/repos/omega/omega-rust` — 6 matches in 3 files; search `rg 'bindings\.structural_observation_path' /agent/repos/omega/omega-rust` — 1 hit (read).
3. Production caller: `resolved_observation_path` in scalar_computations.rs:4331-4350 → `observe` 4394-4410 and `atomic_observation` 4353-4371 (read). `proofs.rs:2864` is a different function with its own access check at 2874-2876 (read).
4. Input to `root == observed_root` branch: a `StructuralArgument` home whose custody already passed `SharedBorrow.admits(home.access)` (or atomic path checks), with `observation_root_type` equal to the argument’s established root type (read).
5. **Verdict: CLOSED.** Check at scalar_computations.rs:4404-4407 (and 4360-4362) gates every call; the early return at 4434-4436 does not re-check access because callers already did.
6. Unsure: whether path segment validity when roots coincide is fully enforced later in `observe`; not required for the cited “access check” claim.

### A0209

1. Read `structural_dependencies` in full at expression_preparation.rs:606-751 (read).
2. Search `rg 'structural_dependencies\(' /agent/repos/omega/omega-rust` — 9 hits, all in expression_preparation.rs `computation_graph` (read).
3. Callers (`borrowed_arguments_from_roots`, `value_calls`, `collect_reachable_nodes`) use results for closure graphs only; they error on stale `value_dependencies` at 616-618 (read).
4. Input to empty arms: structural value nodes whose kind is `NativeCodeLayout`, `QuotientCall` (661-662) or `QuotientAdaptation`, `FixedArray`, `Call`, `ByteLiteralView`, `ZeroedStructural` (743-747), already enumerated in `values` from `value_dependencies` (read checked-trees values_and_views.rs:1342-1395).
5. **Verdict: CLOSED.** Scalars on those kinds are wired through the per-value `call_schedule` loop at 624-647 and/or child nodes in the dependency vector, not through the kind match.
6. Entry cites “five” kinds; read shows two at 661-662 plus five at 743-747 (seven empty arms total).

### A0210

1. Citation 388-391 does not contain `unwrap_or`; relocated to expression_preparation.rs:169-173, 410-415, 624-627, 4312-4315 (read).
2. Read `call_schedule` in values_and_views.rs:1398-1416 (read); read callers `value_calls` 371-418, `structural_dependencies` 624-627, `collect_domains` 4312-4322 (read).
3. `unwrap_or(&[])` turns `Option` absence into an empty call list.
4. Input: a structural value handle with no schedule row, or `None` from duplicate schedule / invalid node per 1410-1411 (read).
5. **Verdict: CLOSED** for well-formed checked trees where no schedule means genuinely no scheduled effect calls. Ambiguous `None` from corrupt IR was not traced through stage 05.
6. Unsure: whether duplicate/invalid schedule `None` can arrive from a checked program that stage 05 accepted.

### A0211

1. Read `complete_value_components` and nested `complete` at expression_preparation.rs:2139-2218 (read).
2. Caller `parameters::prepare` path at 3191 (read).
3. When `required_memberships_are_well_formed` is false, 2145-2147 clears memberships and predicate (read).
4. Input: `ScalarDomainDeclaration` rows built during qualification catalog preparation from checked types (read).
5. **Verdict: CLOSED.** Terminal module validation rejects ill-formed domains and routed domains without predicates (foundation_validation.rs:4700-4713, read).
6. Unsure: none for primary verdict.

### A0212

1. Read `lower_boolean` arm at expression_preparation.rs:1616-1685 (read) and operation_emission.rs:269-271 (read).
2. `UnresolvedStructuralParameterField` emitted only when `structural_fields.is_empty()` after other resolves fail (1657-1685, read).
3. Trace: `validate_boolean_parameter_types` → unsupported at emission.rs:9489-9491; `validate_short_circuit_expression` → unsupported at 9690-9692; scalar_lowering.rs:311-312 calls both before guards (read). proofs.rs:2515-2517 rejects in crash predicates (read).
4. Input: shared Boolean member on a structural parameter field path with no field table in the current lowering frame.
5. **Verdict: CLOSED.** No production path to `emit_boolean_expression` keeps `Unresolved` without error; operation_emission `unreachable!` is a backstop (read comment at 270).
6. Unsure: every emission entry point calls validation; only scalar_lowering guard path confirmed among callers of those validators.

### A0213

1. Read `type_atoms` constraint loop at expression_preparation.rs:3775-3838 (read).
2. `TypeConstraintNode::ArithmeticDomain(_) => {}` at 3787 accepts the constraint without adding a qualification atom (read).
3. Caller loop at 3110-3112 feeds qualification catalog construction (read).
4. Input: constrained type references carrying arithmetic policy adapters on scalar carriers (read).
5. **Verdict: CLOSED.** Arithmetic domains are not semantic `Domain` atoms; non-vacuous unsupported constraints still error at 3833-3836 (read).
6. Unsure: none.

### A0214

1. Read guard arms at expression_preparation.rs:3787-3813 (read).
2. Three accepting `Domain` arms skip atom collection when: (a) establishment routes plus `scalar_type_refinement_predicates` (3788-3794), (b) value-domain predicate via `validation::scalar_value_domain_predicate` (3796-3800), (c) declared interval via `validation::exact_declared_domain_carrier_interval` (3802-3808), (d) predicate presence plus refinements (3809-3813) — read shows four arms; entry names three predicate-form cases.
3. Enforcement trace: `unit_operation_frame.rs:3479-3499` requires complete checked refinements for qualified scalar locals (read); validation helpers live in validation crate type_references_and_wire.rs:4309+ (read header only).
4. Input: type constraints on carriers already marked as carrying scalar qualifications via `carries_scalar_qualifications` (3667-3674, read).
5. **Verdict: CLOSED.** Skips apply only when checked facts already hold refinements or explicit predicates; catalog atoms are not the sole enforcement channel.
6. Unsure: refinement enforcement on every signature-only qualification path without a qualifying local was not read in full.

### A0215

1. Read `collect_domains` in full at expression_preparation.rs:4292-4453 (read).
2. Before `_ => {}` at 4449: universal `qualification_decisions` ingestion (4324-4335) and `call_schedule` structural operand enqueue (4312-4322) (read).
3. Callers `collect_value_requirements` / statement-scoped collectors at 4275-4289 (read, callers not all read).
4. Input: structural value nodes whose kind is not explicitly matched (e.g. `CopiedStructuralPlace`, `Call`, `Scalar`, `Place` without byte literal) after decisions and calls are processed (read).
5. **Verdict: CLOSED.** Default arm omits extra shape walks only; domains are not required to come solely from the kind match.
6. Unsure: none.

### What I did not read

- Stage 05 checking that prevents ambiguous `call_schedule` `None` (values_and_views.rs:1410-1411).
- Full caller sweep for every `validate_boolean_parameter_types` / `validate_short_circuit_expression` invocation beyond scalar_lowering.rs:311-312.
- All `collect_domains` call sites outside expression_preparation.rs.
- `validation::scalar_type_refinement_predicates` implementation in the validation crate.
- External callers of `expression_bindings.rs` outside omega-rust (search scoped to omega-rust only).

SLICE COMPLETE
