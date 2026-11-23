Built from records unit_041 (section G only), unit_042, unit_042_gap_019, unit_042_gap_020, unit_042_gap_021, unit_043, unit_043_gap_022, unit_043_gap_023, unit_043_gap_024, unit_044, unit_044_gap_025, unit_045, unit_046, unit_047, unit_048, unit_049, unit_050 (Part A only) and `crate-map/07-psi-semantics.md`, at commit `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. unit_049 was read through its fail-open entries, `structural_qualifications.rs` section, "Five things" and "Unsure" sections; its other per-file entries were read only as grep lines.

Path prefixes: `S/` = `source-rust/subsystems/compiler/psi/semantics/`; `T/` = `S/src/` (crate terminal-semantics); `B/` = `S/proofs/bounded/` (crate proof-admission); `M/` = `S/proofs/mathematical/` (crate mathematical-kernel); `D/` = `T/descriptors/`.

## 1. What this area does

This area holds the rules every consumer of a Psi program uses to judge it: the source checker (source-to-psi stage 06), the saved-artifact reader (`psi/artifact/wire`), the interpreter, build evaluation and several native-compiler crates all call these crates instead of re-deriving meaning. A caller hands in a `TerminalModule` or one `TerminalMachine` (plus certificates, predicates or concrete operands) and gets back a yes/no, derived rows (origins, regions, plans, accepted facts), or a work cost. Nothing here lowers, emits or runs a program. Most code is in terminal-semantics (114,411 lines) and proof-admission (47,437 lines); about half of proof-admission is tests (unit_043_gap_022 counts 3,170 of 5,792 lines of `B/integer_affine.rs` 1401-7192 as tests).

**closed-predicates** (`S/closed-predicates`, 936 lines). In: a `Proposition`/`ScalarTerm`/`IntegerMathTerm` with concrete operands. Out: `Option<bool>` (true, false, unknown) or `ClosedIntegerEvaluationError`. The guard walk is `walk` (`S/closed-predicates/guard.rs:40`); integer evaluation is `ClosedIntegerEvaluator::evaluate` (`integer.rs:97`) under limits of 8,192 nodes, depth 128, 65,536 result bits, 262,144 limbs and 1,048,576 limb-work (`integer.rs:8-12`). The interpreter calls the same guard (`closed_guard*` 12 matches in 5 files, unit_041 G3).

**layout-planning** (`S/default-integer-layouts`, 60 lines). In: a bit width. Out: `StorageGeometry` or `None` outside 1..=128 bits. `standalone_integer_geometry`/`field_integer_geometry` (`integer_defaults.rs:8-15`); alignment is `next_power_of_two(size)` capped at 8 or 16 (`:21-25`). 17 bits give size 3, alignment 4 (test `:40`).

**resource-bounds** (`S/storage-bounds`, 83 lines; imported as `resource_bounds`). In: claim rows with branch paths. Out: a composed bound or `ExtentDiagnostic`. `compose_claim_bounds` (`S/storage-bounds/src/lib.rs:48`) takes the maximum over mutually exclusive branches of the sum along each path (`:62-71`). Callers: `T/storage/activation_storage/flow.rs:1129` and two interpreter files (39 grep matches in 4 files, unit_050 A7).

**mathematical-kernel** (`M/`, 14,447 lines). In: a `MathematicalCertificate` (signature, context, term, expected type). Out: accept or `CoreError` (34 variants, `M/typing.rs:106`). `verify_mathematical_certificate` (`M/certificate.rs:658`) copies the certificate into the caller's arena, checks the signature, then runs `check_type` (`M/typing.rs:3837`). All judgments run in one explicit-stack machine, `drive` (`M/typing.rs:2354`). Conversion under a `Strict` shared type is proof-irrelevant (`M/typing.rs:1412-1415`). The quotient scheme has seven assumed declarations (`M/quotient.rs:47-223`); the indexed W scheme has none (`M/indexed.rs:429`).

**proof-admission** (`B/`, 47,437 lines). In: an `Obligation`, assumptions, semantic axioms, machine-parameter values and an `EvidenceRoute`. Out: `AcceptedFact` or `EvidenceError`. `verify_obligation_with_machine_parameters` (`B/bounded.rs:82`) routes kernel-derived evidence to `decide_primitive` (`B/kernel.rs:18`) and certificates to `accept_certificate_with_machine_parameters` (`B/proof.rs:190`). A certificate is checked twice: `traversal::check_node` (`B/proof.rs:2884`) runs each rule's local checker on an explicit work list, then `re_decide_in_mathematical_core` (`B/proof.rs:239`) translates it with `Elaboration` (`B/bounded_denotation.rs:7010`) and has the kernel judge it. The `Admitted` route always ends in `AdmissionNotAcceptedByProfile` (`B/bounded.rs:128`) and `SourceChecked` always fails (`:131`).

**terminal-work** (`S/work`, 796 lines). In: executed `Operation`s and taken edges. Out: `WorkUsage` or `WorkMeterError`. `operation_units` (`S/work/src/lib.rs:32`) charges 1 for every operation kind; `charge` checks budget, then payment, then commits, so a refusal changes nothing (`:309-352`). terminal-semantics does not depend on it (crate map).

**terminal-semantics** (`T/`, 114,411 lines). In: a `TerminalModule`/`TerminalMachine` and type tables. Out: plans, origin rows and `Result` validations. The main engines:
- Descriptor graph and replay: `terminal_flow::adapt_flow` (`D/descriptor_results/terminal_flow.rs:207`, about 2,200 lines in one function) turns a machine into a `DescriptorMachine`; the replay engine (`D/replay/engine.rs`, 4,548 lines) runs a worklist to a fixed point per machine, and `DescriptorAnalysis` (`D/replay/analysis.rs:111`) answers storage, loan and summary questions.
- Ownership: `plain_affine::analyze` (`T/plain_affine.rs:1371`) walks owned regions and derives cleanup; `verify` (`:1030`) re-derives and compares; claims go through `plain_affine/claims.rs` `Flow` (`:513`).
- Value facts: `membership_facts::analyze_bodies` (`T/membership_facts.rs:1637`), `structural_qualifications::validate` (`T/structural_qualifications.rs:2989`), `scalar_qualifications::validate` (`T/scalar_qualifications.rs:975`), `Inhabitation::status` (`T/inhabitation.rs:360`).
- Payload contracts: `payload_contract_at_cut` (`D/descriptor_results/payload_history.rs:635`).
- Continuation: `CheckedPrefix::check` (`T/continuation.rs:40`) compares two programs over an executed prefix.
- Operation catalog: `operation_semantic_row` (`T/operations.rs:309`), about 118 rows, exhaustive over `OperationKind`.

## 2. How it decides

Counts are of places the records cite, per crate.

**Identity (ids, positions in sorted tables, declaration identity).**
- terminal-semantics (about 30 cited places): `exactly_one` unique lookups (81 call lines in `T/`, unit_047 1); call projection by `declaration_identity` (`T/call_projection.rs:39-46`); body reach by sorted ids (`T/body_services.rs:94-109`); inhabitation by `StructuralTypeId` binary search (`T/inhabitation.rs:283-288`); case facts on canonical storage (`D/replay/case_flow.rs:99`); membership facts on `ValueId`/`DescriptorLocation`/domain id (`T/membership_facts.rs:379-399`); `DescriptorAnalysis` ownership by slice pointer (`std::ptr::eq`, `D/replay/analysis.rs:131`); `CheckedPrefix::accepts` by pointer equality (`T/continuation.rs:162`).
- proof-admission (about 12): citations by roster index then proposition match (`B/proof.rs:2002,2019`; `B/bounded_denotation.rs:8597`); witness axiom indices strictly increasing (`B/integer_rules.rs:129-135`, `:2066-2072`); each definition must be the latest `Equal(current, _)` (`B/integer_rules.rs:1324-1334`); forbidden-root root must be in `machine_parameter_values` (`:1284-1297`); kernel relation recognised by declaration slot (`B/bounded_denotation.rs:4862,4876,4881`).
- mathematical-kernel: retention predicate on declaration index (`M/tests.rs:130`); inferred-type cache keyed by (binding scope, handle) (`M/typing.rs:455`).

**Place (root place + field path).**
- terminal-semantics (about 15): affine availability on exact place and canonical path (`T/plain_affine.rs:1-6`, `Region::set` `:198-231`); claims on place + inline path (`T/plain_affine/claims.rs:110-118`); descriptor lifetimes on position and `DescriptorLocation` (`D/descriptor_lifetimes.rs:800-806`); payload history on place, position, domain (`D/descriptor_results/payload_history.rs` "Keys"); version overlap by `contains` either way (`D/descriptor_results/payload_versions.rs:441`); `LocationMap` pages by root id (`D/replay/location_map.rs:32`); suspension liveness by `ValueId`/`PlaceId` (`T/suspension_frontier.rs:95-117`); byte predicates by place kind (`T/bytes/byte_sequence_predicates.rs:959-1213`).

**Value (closed evaluation, literals, intervals).**
- closed-predicates: three-valued guard (`guard.rs:44-67`); integer comparisons need equal types (`guard.rs:93`).
- proof-admission (about 15): negation accepted only on `Some(false)` (`B/kernel.rs:80-84`); carrier bound by literal equality with min/max (`B/kernel.rs:154-165`); `closed_order_bound_evidence` by evaluated values (`B/bounded_denotation.rs:518`); `numeral_binary_operation` checks the result in `BigInt` before interning `op(a,b)=c` (`:11116-11130`); `solve_divisor` with checked i128 arithmetic (`:9288-9357`); polynomial divisibility in `BigInt` (`B/integer_affine.rs:2356-2360`); cast interval narrowing (`B/integer_rules.rs:204-205`); float exactness by `(24,127)`/`(53,1023)` (`B/float_integer.rs:232-235`); float meaning by union-find roots (`B/float_meaning.rs:50-53`).
- terminal-semantics: crash-route guards must evaluate to `Some(false)` (`T/evaluation_crashes.rs:83-107`); scalar index known only with one producer (`T/bytes/byte_sequence_predicates.rs:1769-1782`).

**Position (order of axioms, conjuncts, edges).**
- proof-admission (about 8): affine route from the shape of `endpoint_steps` (`B/integer_affine.rs:434-464`); fixed conjunct order for dividend and quotient bounds (`:1911`, `:1866-1887`); multiple-remainder premise must equal `Truth`, the one bound, or the ordered conjunction (`:2378-2384`); `check_multiple_witness` takes the first matching axiom below `upper_axiom` (`:2225-2229`); `affine_equality_for` takes the first cited equality in list order (`B/bounded_denotation.rs:14327`); interning order fixes kernel declaration order (`B/bounded_denotation.rs:3821-3862`).
- terminal-semantics: plain-affine joins require the same ordered `active` set (`T/plain_affine.rs:404-408`); access lifecycle plans ordered by (boundary id, target name) (`T/access/access_lifecycles.rs:35-42`).

**Spelling (identity text, names, `format!`-built strings).**
- terminal-semantics (about 20): `ProcessExit` identities built from literal names (`T/calls/closed_conformances.rs:19-110,145-242`); `"toolchain::LayoutTarget"`/`"toolchain::LayoutTargetFacts"` (`T/calls/layout_target.rs:93,113`); target name strings (`T/access/access_lifecycles.rs:37,277`); `format!("#{identity}")` member names and `"read"/"take"/"write"` (`T/access/access_view_plans.rs:220-226,297-303`); continuation ids through identity text (`T/continuation/declarations.rs:34-42`); provider requirement text (`T/logical_providers.rs:85-99,231-232`); fixed-image strings (`T/logical_providers/fixed_image.rs:148-196`); numeric primitive paths and about 30 identity strings (`T/numeric_primitives.rs:79-117,439-581`); `"toolchain::Extent"`, `"base"`, `"length"` (`T/program_storage_entry.rs:88,137-143`); provider candidate text (`T/provider_candidates.rs:41-52`); issuer route text (`T/scalar_qualifications.rs:98-112`) and `"builtin:policy:*"` names (`:63-65`); declared lifetimes from type-identity strings (`T/references/declared_results/definition.rs:224-247`); boundary lifetime comparison by identity text (`T/references/boundary_results.rs:488-533`); caller descriptions by path text (`T/structural_qualifications.rs:2351-2362`).
- proof-admission: one test pins the static string `"conjunction arity"` (`B/bounded_denotation.rs:16141,16165`); no production decision on spelling cited.

## 3. The same job in more than one place

- **Unique-match lookup.** `exactly_one` (`T/lib.rs:82-92`) is hand-written again in `Replay::machine` (`D/replay/engine.rs:763-770`), `TerminalContracts::declaration` (`T/inhabitation.rs:561-576`), `exactly_one_row_for_tag` (`T/operations.rs:318-331`), `callable_refinement.rs:92-98`, `evidence_calls.rs:378-400`, `inputs.rs:90-109`; `is_descriptor`/`is_shared_descriptor` repeat the scan (`D/descriptor_results.rs:235,279`). Private `declaration` helpers of the same shape at `D/descriptor_lifetimes/locations.rs:402`, `T/inhabitation.rs:561`, `T/membership_facts.rs:638`, `T/structural/owned_structure/complete_claims.rs:68`, `T/structural_qualifications.rs:362` (grep only). `structural_declarations.rs` uses first-match `find` instead (unit_049).
- **Forward must-flow with AND join.** `payload_history.rs:719-1239`, `payload_versions.rs:175-207` and `:580-616` (`D/descriptor_results/`); reachability pruning in `payload_reachability.rs`. `PayloadMembershipContract` intersection (`contents.rs:907-923`), `State::meet` (`payload_history.rs:113`) and `PayloadScalarObservation::intersect` (`payload_predicates.rs:26`) are three intersections of the same kind.
- **Type-tree recursion with own cycle handling.** `may_contain_descriptor` (`D/replay.rs:362`), `slot_inventory::walk` (`:804`), `indirect_contents_contain_slot` (`:632`), `owned_shape::visit` (`D/descriptor_results/owned_shape.rs:26`); type containment in `T/storage/type_graph.rs`, `owned_structure.rs`/`complete_claims.rs`, `zeroed_structural.rs`, `structural_qualifications.rs:1114`, `storage_dependencies.rs` (unit_049).
- **Storage path walkers.** `primitive_place.rs:826`, `static_path.rs:263`, `record_field.rs:263`, `scalar_observations.rs:120` (all `T/structural/` or `T/scalars/`), with different segment alphabets (unit_049).
- **Scalar write recording.** `D/descriptor_results/scalar_captures.rs:399-507` and `terminal_flow.rs:360-387,1314-1343,1440-1478` use the same operation list.
- **Loan/descriptor end.** `plain_affine.rs::end_references` (`T/plain_affine.rs:2292`), `reference_ends::candidates` (`T/reference_ends.rs:55`) and `reference_completion::disposed_reference_on_return` (`T/references/reference_completion.rs:22`) (suspected overlap, unit_048).
- **Call-kind list.** Written twice in `T/plain_affine/claims.rs` (`:847-857`, `:1329-1338`) and in `install` (`:230-247`).
- **Identity-string rebuild and compare.** `T/logical_providers/fixed_image.rs:108-134` and `T/numeric_primitives.rs:641-698`.
- **Access check over element and byte views.** `reference_source_projection_accessible` and `descriptor_source_projection_accessible` (`T/references/reference_views.rs:309-343` vs `:376-413`).
- **Proposition rewriting.** `T/continuation/predicates.rs:46-58` and `T/calls/callable_refinement.rs:60-86` (suspected).
- **Result-match strictness.** `projected_qualifications` compared with `all(contains)` at `D/descriptor_results/invocation.rs:79-86`, with `!=` at `:195-199`, and again at `provider_invocation.rs:138-141`.
- **Derivation of domain memberships.** `admit_declared_scalar_refinement` (`D/descriptor_results/contents.rs:1157`) by term shape, and `close_with_shared_requirements` (`payload_predicates.rs:137`) by proof.
- **Rule checks in two traversals.** The pure integer-order checkers are called from `B/proof.rs` (`check_node_locally` `:3136`) and again from `B/bounded_denotation.rs:8144,8171,8195,8262,8284,8301`; each has 2 callers (unit_043_gap_023).
- **Ordered-axiom walk.** Cast chain (`B/integer_rules.rs:116`), shift chain (`:2047`) and forbidden-root branch (`:1276`) repeat order, operand-equals-current, cycle and target checks (by reading, not diff).
- **Remainder checkers.** `remainder::check_witness` (`B/integer_affine.rs:1639`), `check_dividend_witness` (`:1757`), `check_multiple_witness` (`:2043`), the direct literal branch (`:3641-3672`) and the `Remainder(i128)` arm of `apply_definition` (`:3090-3117`).
- **Signed `-1` division checks.** Spelled separately at `B/integer_affine.rs:2190-2195`, `:2745`, `:3109`, `:3653`, `:1917`.
- **Native-width list `8 | 16 | 32 | 64`.** `B/integer_rules.rs:39` (with `!is_address()`) and `B/integer_affine.rs:3237`.
- **`meaning()`.** Identical at `B/float_integer.rs:28-33` and `B/float_meaning.rs:443-448`.
- **Miniature float oracles.** Three separate test models: `B/float_integer/tests.rs:213`, `B/float_meaning/model.rs`, `B/ieee_float_rules/model.rs`.
- **J-eliminator construction.** `symmetry`/`transitivity` (`B/bounded_denotation.rs:4904,4940`) and `forbidden_roots` `integer_id_elim` (`:9204`); orientation pattern repeated at `:5197-5207`, `:6282-6288`, `:9037-9048`.
- **Interning styles.** `interned_slot*`, `interned_keyed_assumption`, `roster_law_*`; `PositionRoster` covers both `BTreeMap` and `HashMap` rosters (`B/bounded_denotation.rs:3829-3858,4082`).
- **Numeral helpers.** `numeral_binary_operation` (`B/bounded_denotation.rs:11100`) is shared for sum, difference and product; `numeral_quotient` (`:11334`) has its own copy.
- **Correlated bound evidence.** `correlated_subtract_bound_evidence` (`:12855`), `correlated_multiply_bound_evidence` (`:11374`) and (suspected) `correlated_add_bound_evidence` (`:184`) share the gate sequence.
- **Test helpers.** Three arithmetic-rule test files repeat `carrier/value/literal/context` helpers (`B/tests/multiple_remainder.rs:28-66`, `quotient_at_most_dividend.rs:28-120`, `remainder_within_dividend.rs:28-166`); the 25-field `DescriptorMachine` literal is repeated in three test files (`D/replay/engine/snapshot_tests.rs:97-158,633-674`, `case_flow/result/tests.rs:14-74`).
- **Work accounting.** A second `work` module under `build/evaluation/accounting/work/work.rs` (suspected overlap, unit_050 A9).

## 4. Fixed lists of shapes

- `T/operations.rs:162-294`: one row per `OperationKind` (about 118), 21 with a goal-free scalar-leaf schema; exhaustive by macro.
- `T/continuation.rs:467-541`: listed operation kinds or a goal-free scalar leaf; else "no checked correspondence for an operation before the preparation" (`:540`). Terminators `Jump`, `Conditional`, three returns, `StructuralCase` only (`:328-421`).
- `D/descriptor_results/terminal_flow.rs:634-799`: payload-write match; unlisted non-leaf kinds push `None` (`:796`).
- `D/descriptor_results/scalar_captures.rs:243-253,415-497`: resident transfers and scalar writes.
- `D/descriptor_results/value_identity/flow.rs:140-149`: five kinds carry identity.
- `T/plain_affine/claims.rs:951-1128`: claim-moving operations; calls a fixed list of eight (`:847-857`).
- `T/storage/activation_storage/flow.rs:627-1101`: serial-only model; other kinds `_ => {}`; nine producing kinds checked at `:1104-1127`.
- `T/structural_qualifications.rs:1853-2105`: per-operation membership rules; fallthrough "ordinary structural operation cannot establish a declared domain" (`:2099-2103`).
- `T/callable_values/terminal.rs:385-454`: eight invocation kinds.
- `D/descriptor_results/scalar_contracts.rs:59-87`: five call kinds; stored postconditions only for integer and Boolean fields (`:238-244`).
- `T/access/access_lifecycles.rs`: Relocate 4 params, View 2, Initialize 2 or 3, MoveOut 1 or 2 (`:161-226`); `result_contract.rs` fixes case counts (`:85-90,352,394-421`).
- `T/bytes/operation_bounds.rs:32-87`: six indexed kinds; others `Ok(None)`.
- `T/numeric_primitives.rs:23-38,81-112,271-290`: six float comparisons and listed conversions.
- `T/logical_providers/fixed_image.rs:82-93,57-61`: two operations, target `uefi_x86_64` only.
- `T/calls/closed_conformances.rs:133`: only `HostedOperation::ProcessExit`.
- `T/program_storage_entry.rs:76-85`: exactly `[base, length]`.
- `T/references/reference_storage_geometry.rs:994`: type shapes, else `UnsupportedType`.
- `B/proof.rs:3136-3261` (`check_node_locally`) and `:2956-3108` (`schedule_children`): exhaustive over `ProofRule`, no wildcard.
- `AcceptedProofRule` (`B/proof.rs:31-80`): 36 variants (corrected; unit_043 said 34). `foundation()` exhaustive (`B/classicality.rs:21-58`); `is_provisional` lists 8 (`:71-78`).
- `B/integer_affine.rs:3619-3848`: `check_integer_affine_witness` ladder of ten steps; `CheckedIntegerEndpointStep` has 22 variants (hand count, `:3452-3493`), matched without wildcard in `integer_affine_truth_bounds` (`:2704-2823`).
- `B/integer_affine.rs:3026-3207`: `apply_definition` expression forms; `literal - current` not accepted (`:3051`).
- `B/integer_affine.rs:2032-2036,2336`: multiple remainder over six term kinds, depth 32, 64 nodes, 64 monomials.
- `B/integer_rules.rs:1346-1370`: forbidden-root branches over Add, Subtract, Multiply; only 0 and -1 candidates; signed native carriers only.
- `B/integer_rules.rs:3101-3115`: wrapping equality over wrapping add, subtract, multiply.
- `B/bounded_denotation.rs:10966-10989`: 24 integer operations; unmatched terms become opaque constants (`:10740`).
- `B/bounded_denotation.rs:6541`, `:5698`, `:6236`: replacement context over nine term kinds; coercion and inhabitant engines over listed forms.
- `B/bounded_denotation.rs:7124`: at most three fixed children per rule; `IntegerAffineBound` tries seven derivations in order (`:8343-8412`).
- `B/kernel.rs:36-117`: judgment-proposition pairs; unmatched `_ => false` (`:116`). `primitive_derivation` tries five of six `PrimitiveJudgment`s, not `ClosedFloatIntegerRelation` (`B/bounded.rs:145-151`).
- `B/predicate_denotation.rs:29-41`: eight definition subjects.
- `S/closed-predicates/integer.rs:113-119`: Add, Subtract, Multiply, ShiftLeft evaluated.
- `T/substitution/clauses.rs:614-616`: `_ => Err("clause form outside the scalar subset")`.
- Caps that refuse beyond them: 4096 array elements (`D/replay/contents_queries.rs:774,951`; `D/descriptor_results/contents/incoming_memberships.rs:54`; `D/replay.rs:813,890`; `D/descriptor_lifetimes/locations.rs:167`); 128 depth (`D/replay/engine.rs:355,2972`; `T/references/reference_storage_geometry.rs:877`); 64 export depth (`engine.rs:3621,3658,3759`); 65,536 worklist steps (`payload_history.rs:718`, `payload_reachability.rs:168`, `payload_versions.rs:129`); 131,072 leaves (`T/structural/zeroed_structural.rs:10`); 1,048,576 selection work (`D/selected_memberships.rs:112`). None of these carries a comment on why (units 046, 047, 048, 049).

## 5. Accepts when it does not know

Classes kept as the records give them.

- **Certificates the kernel cannot translate are admitted on local checks.** `re_decide_in_mathematical_core` turns `BoundedDenotationError::Unsupported(family)` into `Ok(MathematicalCoreDecision::Refused(family))` (`B/proof.rs:257-259`). Traced within file (unit_043); whether callers read `mathematical_core` not traced there. See section 10 for my grep.
- **Rule-instance fallback.** `Elaboration::rule_instance` (`B/bounded_denotation.rs:8660-8672`) declares `premises -> conclusion` as a kernel assumption via `rule_axiom` (`:5240-5261`). Always for `IntegerFloatExact`, `FloatIntegerConversion`, `FloatMeaningNormalization`, `IeeeFloatOrderChain` (`:7748-7810`) and `IntegerPowerOfTwoOrder` (`:8579`): stated. As fallback when derivation returns `None` for about 16 rules including `IntegerAffineBound`, `IntegerCastBound`, `IntegerCorrelatedForbiddenRoots` (`:8555-8571`): traced (the rule's checker runs first in every arm read). The rule is recorded in `rules` (`:8670`); whether a consumer restricts it: not read.
- **Provisional arithmetic rules.** `IntegerRemainderWithinDividend` (`B/integer_affine.rs:1750`), `IntegerQuotientAtMostDividend` (`:1847`), `IntegerMultipleRemainder` (`:2038`), `power_of_two_order` (`B/proof.rs:2145-2146`, tested only for exponents below 128 at `:2310`): stated.
- **Two remainder policies accepted.** `check_integer_affine_bound_conversion` accepts a conclusion equal to the `RemainderUpperBound::TotalImage` ("legacy") mapping as well as the current one (`B/integer_affine.rs:387-396`): open.
- **Primitive decision.** After `decide_primitive` accepts, the denoted conclusion gets a `decision` assumption if no closed inhabitant exists (`B/bounded_denotation.rs:7820-7829`): traced.
- **Duplicate definitions.** `check_multiple_witness` takes the earliest matching axiom (`B/integer_affine.rs:2225-2229`) while `check_branch` takes the latest (`B/integer_rules.rs:1324-1334`): open.
- **Direct affine shapes with no steps.** `identity_ring_target` and the shift-left branch return forms with no `context.validate` on the witness terms (`B/integer_affine.rs:3696-3715`): open.
- **Opaque denotations.** Unrecognised propositions become fresh atoms (`B/bounded_denotation.rs:5383,5411`): traced; unrecognised `ScalarTerm`s become opaque integer constants (`:10740-10745`): inferred.
- **Inverted interval.** `5 <= left <= 2` proves the corner product (`B/tests/exact_multiply_result_bound.rs:505-521`): stated.
- **`compare_integer_math_terms` returns `Ok(None)` when unordered** (`B/integer_rules.rs:31`): not traced across 5 caller files.
- **`is_provisional` defaults new rules to `false`** (`B/classicality.rs:69-79`): stated (`:66-67`).
- **Descriptor graphs default to empty.** `terminal_graph`, `terminal_graph_from`, `terminal_reference_graph`, `terminal_borrowed_storage_graph` return `Vec::new()` on adapt failure (`D/descriptor_results.rs:1215,1222,1663,1667`): open. (unit_046 named three; `terminal_graph_from` at `:1222` also does this, see section 10.)
- **`terminal_definition` `_ => Some(Local)`** (`D/descriptor_results.rs:1781`): open.
- **`is_descriptor` false on unknown or duplicate type** (`D/descriptor_results.rs:240-243`): open.
- **Empty and literal loans** pass compatibility silently (`D/descriptor_results/compatibility.rs:98-101`): open.
- **`terminal_payload_contract` takes the first producing operation** (`D/descriptor_results/contents.rs:1399-1444`): open.
- **`AtomicCell(_) => true`** in `owned_shape` (`D/descriptor_results/owned_shape.rs:87`): open; `Opaque => true` (`:85`): stated.
- **Stale scalar observation** when a write has several destinations (`D/descriptor_results/payload_history.rs:1001-1003`): open.
- **First definition only** in `TerminalScalarExpressions::expression` (`D/descriptor_results/payload_predicates.rs:476-478`): open.
- **Callee `ensures` adopted** without re-derivation (`D/descriptor_results/scalar_contracts.rs`): open.
- **Private parameters kept across scalar-only boundary calls** (`D/descriptor_results/private_parameters.rs:47-66`): stated.
- **Writes treated as none** for leaf copies, locals, fences, address queries (`terminal_flow.rs:769-776`): open; second `_ => {}` (`:1887`): open.
- **`case_feasibility` vs `edge_feasibility`** return `Unknown` vs `ProvedImpossible` for the same missing state (`D/replay/analysis.rs:300` vs `:336`): open. `_ => {}` for non-input origins (`:1966`): open.
- **`may_write_indirect_contents` false for SharedBorrow** without type check (`D/replay.rs:567`): traced. `indirect_contents_contain_slot` for `Opaque` (`:670`): open.
- **`select` keeps every branch feasible** when unknown (`D/replay/case_flow.rs:168-172`): stated and traced as conservative.
- **Dropped retired incoming write** (`D/replay/engine.rs:2632-2643,2710-2712`): open.
- **Empty-interface summary** claims no writes from the root's parameters only (`D/replay/summary_interface.rs:21-35,38-70`): open.
- **Plain-affine without a module** skips restoration (`T/plain_affine.rs:2264,2346`): open. Untracked copyable places skip checks (`:4281,4310`): traced.
- **Claims untouched for unlisted operations** (`T/plain_affine/claims.rs:1127`): open. Unreachable blocks publish rows without a walk (`:1319-1370`): open.
- **Empty provider selection passes** when no instance requirement exists (`T/provider_candidates.rs:87`): open.
- **`FixedRange => break`** shortens an origin path (`T/references/reference_origins/descriptor_regions.rs:524`): open.
- **Representation geometry with no applications** falls back (`T/references/representation_geometry.rs:135-141`): open.
- **Target ranges**: untraceable operations get no range (`T/scalar_target_ranges.rs:123-130`): open/traced; `layout_call_conditions` skips a call with `invocation_requires == None` (`calls.rs:43-45`): open.
- **Bodies outside `bodies`** get structural validation only (`T/scalar_qualifications.rs:1093`): open (stated `:984-985`).
- **Crash paths retire nothing** in nominal retirement (`T/content_transformations/ancestry/retirement.rs:606-609`): open (traced that only one grounded normal return is required, `:612`).
- **Operation safety goal** `Ok(None)` for unlisted kinds (`T/bytes/operation_bounds.rs:87`): open.
- **Crash goals only under `Closed` policy** (`T/calls/callable_refinement.rs:608`): open for other callers.
- **`dominators` panics** on an unreachable block instead of refusing (`T/control_graph.rs:385`): open.
- **`closed_owned_sequence_bounds` missing rows** (`T/owned_sequence_bounds.rs:43-52`): open.
- **Activation flow `_ => {}`** (`T/storage/activation_storage/flow.rs:1101`): traced for producers, open for readers and borrowers.
- **`validate_machine` skips operations without a structural result** other than `StoreStructuralField` (`T/structural_qualifications.rs:1852-1873`): open. `EstablishStructuralLocal` and empty produced sets accepted (`:2054,2091`): stated.
- **Substitution keeps the original term** when the replacement type differs (`T/substitution.rs:331,339`): open; content-term substitution drops the replacement prefix (`:454-455`): open.
- **`structural_store` roster** checks only listed stores (`T/structural/structural_store.rs:70-76`): open.
- **`known` identity**: `continuation/declarations.rs` `identity` true when both absent (`:23-25`), `scalar_set` true for id 0 (`:101`): traced.
- **Clause connectives** stop at a deciding constant (`T/substitution/clauses.rs:824-826`): stated; safe only if the earlier `Truth` of a disjunction is genuine.
- **`call_may_suspend` `None`** (`T/suspension_frontier.rs:57-70`): open; unknown type rows treated as no borrows (`psi/types/structural/queries.rs:127-129`): open.
- **Unlisted computed-call site skipped** (`T/synchronous_calls.rs:229-234`): open. Boundary `Evaluate` entry without a selection skips proof entries (`T/synchronous_calls/scalar_roots.rs:149-158`): open.

## 6. Silent drops

- `terminal_graph*` (`D/descriptor_results.rs:1215,1222,1663,1667`): a machine whose graph fails disappears with the rest of the graph; `terminal_graph_rows` keeps per-machine `Result`s (`:1539`).
- `T/structural_qualifications.rs:2999-3014`: `filter_map(Result::ok)` drops unreadable bodies (stated `:2996-2998`).
- `D/descriptor_results/provider_invocation.rs:104-109`: an output whose slot is not owned is skipped.
- `B/bounded_denotation.rs:6289-6293`: reflexive or duplicate-source equations skipped.
- `B/integer_rules.rs:1525-1527`: non-`LessOrEqual` axioms skipped when selecting bounds (traced: missing bounds then refuse).
- `T/bytes/fixed_arrays.rs:165`: `Disjunction`/`Implication` clauses skipped (lose facts).
- `D/descriptor_results/payload_entry.rs:20-23,79,100`: budget ends the walk early, fewer memberships.
- `T/access/access_ram_correspondence.rs:107-109`: `Inaccessible` fields yield no row.
- `D/replay/engine.rs:2632-2643,2710-2712`: callee write dropped for retired incoming slots.
- `T/plain_affine.rs:1802-1807`: no tracked root, no windows, no claims: empty plan, no walk.
- `T/references/boundary_results.rs:136`: provider without a normal exit skipped.
- `T/logical_providers.rs:219-221`: undemanded schema yields no row.
- `T/scalar_target_ranges.rs:97-101,123-130`: untraceable expressions give no range.
- `T/content_transformations/ancestry/retirement.rs:606-609`: crash, empty-elimination, external completion, no-normal invoke end a path without disposal.
- `T/structural/owned_structure.rs:70-74` (unit_049): a held claim with no owned argument prefix gets no transfer row (traced: claim stays with caller).
- `T/substitution.rs:454-455`: replacement prefix dropped from content terms.
- `T/references/reference_origins/borrowed_leaves.rs:110,145`: unlisted shapes contribute no leaves (correct per record).
- `T/suspension_frontier.rs:372`: no reference graph gives no loan rows (stated).

## 7. Unused or unwired

Counts are grep lines over `source-rust/` unless stated.

- `resolve_callable_value_at_invocation` (`T/callable_values.rs:1297`): 1, the definition (unit_045; confirmed).
- `body_service_reaches` (`T/body_services.rs:26`): 5, definition plus 4 test calls in `T/callable_values/tests.rs` (unit_044; confirmed).
- `reference_carrier_membership_contract` (`D/selected_memberships.rs:46`): 2, definition and `T/lib.rs:98` re-export (unit_047; confirmed).
- `exact_operation_semantic_row_in` (`T/operations.rs:296`): 3, definition, own use `:312`, re-export `T/lib.rs:235` (confirmed).
- `present_structural_membership_contract` (`D/membership_presentations.rs:23`): re-export `T/lib.rs:95` and in-file call only (unit_046).
- `validate_access_view_referent` (`T/access/access_view_referent.rs:18`): own file only (suspected, combined grep, unit_044).
- `byte_carrier_scalar_layout` (`T/bytes/byte_carriers.rs:34`): own file only (combined grep, unit_044).
- `byte_sequence_concatenation_capacity_goal` / `_length_equation`: 3 matches in 2 files, re-export only (unit_045).
- unit_049: `constant_goal_free_scalar_leaf`, `collection_view_backing`, `representation_region_length`, `zero_valid_record_storage`, `primitive_place_type`, `fixed_array_place_shape`, `primitive_read_requirement`, `interior_member_borrow`: 0 calls beyond definition and re-export. Confirmed for `collection_view_backing` (definition `T/storage/collection_backing.rs:135` and `T/lib.rs:323`) and `zero_valid_record_storage` (definition, one recursive self-call `zero_valid_storage.rs:35`, re-export `T/lib.rs:252`).
- `SourceContentTheory` (`B/bounded_denotation.rs:12301`): 5 matches, definition, impl, one test, `pub use` and re-export `B/bounded.rs:428` (confirmed).
- `bounded_verification_entries` (`B/bounded_observations.rs:5`): 0 production calls, tests in `B/proof.rs` only (unit_042: 14 uses; unit_043_gap_024: 16 matches in 2 files). The counters are not behind `cfg(test)`.
- `check_multiple_bound`, `preserves_strict_endpoint`: 2 lines each, one call inside `B/integer_affine.rs` (the first at `:378`, confirmed).
- `bitwise_or_image_maximum`: 2 lines (definition and one use).
- `integer_cast_truth_bounds`: 1 caller (`B/proof.rs:753`).
- `aggregate_descriptor_slots`, `reconstruct_return_slots_external`: 1 caller each, both in native `optimization-unit-semantics` (unit_046).
- `terminal_residence_rules`: 1 caller (`T/plain_affine.rs:2151`).
- `Replay::resolved`: 1 non-test caller (`D/replay/analysis/call_origins.rs:66`).
- `input_contents_origins`, `returned_packet_input`, `empty_interface_summary`: 2 lines each (definition plus one call).
- `known_predicate_with`: 1 external caller (stage 06 `prove/counterexample.rs:64`).
- `comparison_predicate`: 2 lines (definition plus stage 05 `read/contracts.rs:1156`).
- `structural_qualifications`: module doc names three users; grep found artifact ingress (`psi/artifact/wire/src/semantic_module.rs:4289`) and stage 06 (`formation.rs:87`), none in stage 05 (unit_049).
- `T/substitution/clauses.rs` header says it is shared by elaboration and checking; no hits under stage 05 (suspected not by name, unit_050 A1).
- terminal-work: not a dependency of terminal-semantics though `S/README.md` lists it under semantics (crate map).
- Native crates reach into this crate's `descriptor_results` API (3+ call sites, unit_046).

## 8. Stated limits

**Proof kernel coverage.**
- "A provisional row was tested against computed arithmetic and is to be replaced by a checked proof; none may remain when the kernel is claimed verified." (`B/classicality.rs:61-67`).
- "Provisional float rules: the core has no IEEE model to derive them from" (`B/bounded_denotation.rs:7740-7743`); "Provisional: the core has no power of two to derive it from" (`:8573-8574`).
- "the kernel trusts this statement; the mathematical core does not derive it" (`B/integer_affine.rs:1750,1847,2038`).
- `MathematicalCoreDecision::Refused(&'static str)` records an uncovered family (`B/proof.rs:99`).
- "a carrier bound of an integer whose width is a parameter" (`B/bounded_denotation.rs:382`); "discreteness needs a fixed literal" (`:1483`); "integer-math-normalized citation across denotation shapes" (`:8625`).
- "`i1` holds only -1 and 0 ... the rule declines there" (`:11420-11421`).
- "The exact-order theorem below does not apply to clamped subtraction" (`:12739-12742`).
- "the kernel has no `Fin` former" (`B/tests/indexed_nested.rs:39-44`); "The kernel does not look for that" (`B/tests/exact_multiply_result_bound.rs:505-510`).
- "admit exactly seven named assumptions" (`M/tests.rs:2811-2813`); effectivity takes equivalence proofs as hypotheses (`M/quotient.rs:68-73`).
- `Admitted` evidence never accepted (`B/bounded.rs:128`).

**Numeric and float.**
- "an equality with zero cannot discard a negative zero sign" (`B/float_integer.rs:5-7`); "Unknown inputs remain unknown" (`:36-37`).
- "it never cancels arbitrary narrowing" (`B/float_meaning.rs:3-4`).
- "an empty chain establishes nothing" (`B/ieee_float_rules.rs:48-50`).
- "numeric primitive has no canonical result contract yet" (`T/numeric_primitives.rs:24,37`).
- "A parameter width has no interval to narrow by" (`B/integer_rules.rs:189-191`); "A parameter width has no literal endpoint" (`B/kernel.rs:157`).
- "A carrier endpoint exists only at a closed width." (`B/integer_affine.rs:1195`).

**Access and memory checks not implemented.**
- "memory access checks for these saved rules are not implemented yet" (`T/structural_qualifications.rs:2851`); "the compiler cannot check access through this public entry yet" (`:2596`); `operation_permissions.rs:214,309,364,552` (four more "yet" refusals).
- Serial-only activation storage: messages containing "serial realization" (`T/storage/activation_storage/flow.rs`).
- "A suspension plan states none of that yet" (`T/plain_affine.rs:2449-2451`); "the loan lineage of a live value that holds a reference or a view" is unresolved (`T/suspension_frontier.rs:26-27`).

**Descriptors and loans.**
- "An unsupported body must not erase useful analyses of independent bodies" (`D/descriptor_results.rs:1536`); "Missing is not a call; None is a real invocation whose effects cannot be represented" (`:484`).
- "An absent result never proves that a call writes nothing." (`terminal_flow.rs:1182-1185`); "cannot publish an empty frame" (`:794-795`).
- "None means the storage effect could not be placed; it must use the caller's conservative ceiling, never an empty frame." (`D/replay/analysis.rs:720`).
- "A recursive dependency sees an unknown frame." (`D/replay/engine.rs:589-590`).
- "Descriptor liveness proposes disposal candidates, not permission to dispose storage" (`T/reference_ends.rs:1-4`).
- "An empty intersection at a join proves no release" (`T/references/reference_completion.rs:31`).

**Values and types.**
- "Unknown contracts and cycles alone prove nothing." (`T/inhabitation.rs:5`); "An owned empty source may fit any result type" (`T/normal_completion.rs:5-6`).
- "an identity string proves neither fact" (`T/calls/closed_conformances.rs:52-54`).
- "Neither an ordinary result annotation nor an unbound coercion can mint membership" (`T/scalar_qualifications.rs:1-3`).
- "A loop-carried storage value needs an inductive proof" (`T/scalar_target_ranges/storage.rs:78`); "An unknown runtime guard supplies no restriction" (`guards.rs:101`).
- "never searches for functions with an eligible signature or calling plan" (`T/callable_values.rs:3-4`).

## 9. Disagreements

**Between records.**
- `AcceptedProofRule` size: unit_043 says 34 variants; unit_042 (classicality) says `ALL` has 36 entries. The enum has 36 (`B/proof.rs:31-80`, section 10).
- `bounded_verification_entries` uses: unit_042 says 14 uses in `B/proof.rs` tests; unit_043_gap_024 says 16 matches in 2 files (1 + 15). Different greps; not re-checked.
- `B/tests/suite.rs`: unit_044 says 20 `mod` lines; the crate map says 21 topic modules. The file has 21 `mod` lines (section 10).
- unit_042_gap_021 item 3 reads the `"conjunction arity"` expectation at `:16165` for the disjunctive case as needing confirmation; no other record resolves it.
- unit_043 calls `check_integer_affine_bound_conversion`'s second policy "legacy" (open); no other record says whether a producer still emits it.
- unit_046 says the `*_from` graph builders keep per-machine errors; `terminal_graph_from` (`D/descriptor_results.rs:1222`) also returns an empty `Vec` on failure (section 10).

**Between records and the crate map.**
- The crate map says proof-admission "runs no proof search"; unit_046 finds a small proof search in terminal-semantics (`D/descriptor_results/payload_entry.rs:117-199`), checked afterwards by `proof_admission::check_certificate`. Not a contradiction about proof-admission itself, but the area does search.
- resource-bounds line numbers: crate map gives `lib.rs:6,33,40,51,77`; unit_050 gives `:7,30,36,48,76`. The code matches unit_050. Crate map's call site `flow.rs:1128` is at `:1129`.
- terminal-work dependents: crate map counts 14 manifests; unit_050 counts 69 files using `terminal_work`. Different units (manifests vs source files), not a conflict.
- `structural_qualifications` users: the module doc and crate-map summary name three users; unit_049 grep finds two.
- unit_047 reads the doc at `T/logical_providers.rs:109-110` as saying all matching uses IDs while the code compares text. The doc sits on an error type and speaks of that error's IDs (section 10).

## 10. What I checked myself

1. **Claim** (unit_045): `resolve_callable_value_at_invocation` has no caller. **Code:** grep over `source-rust/` finds only `T/callable_values.rs:1297`. **Matched.**
2. **Claim** (unit_047): `reference_carrier_membership_contract` has no call site. **Code:** `D/selected_memberships.rs:46` and the re-export `T/lib.rs:98` only. **Matched.**
3. **Claim** (unit_044): `body_service_reaches` has no non-test caller. **Code:** definition `T/body_services.rs:26`; four calls, all in `T/callable_values/tests.rs:641,664,670,687`. **Matched.**
4. **Claim** (unit_049): `zero_valid_record_storage` and `collection_view_backing` have no call site beyond definition and re-export. **Code:** `collection_view_backing` at `T/storage/collection_backing.rs:135` and `T/lib.rs:323`; `zero_valid_record_storage` at `T/storage/zero_valid_storage.rs:7`, its own recursive call `:35`, re-export `T/lib.rs:252`. **Matched** (the only other use is self-recursion).
5. **Claim** (unit_042_gap_020): `SourceContentTheory` is used outside its file only by the `B/bounded.rs:428` re-export. **Code:** 5 matches: `B/bounded.rs:428`, `B/bounded_denotation.rs:12301,12305,12342` (test), `:12432`. **Matched.**
6. **Claim** (unit_042, same job): `meaning()` is defined identically in two files. **Code:** `B/float_integer.rs:28-33` and `B/float_meaning.rs:443-448` are the same five lines. **Matched.**
7. **Claim** (unit_043_gap_022, same job): the `8 | 16 | 32 | 64` list is spelled twice. **Code:** `B/integer_rules.rs:39` (with `!is_address()`) and `B/integer_affine.rs:3237`. **Matched.**
8. **Claim** (unit_043): `AcceptedProofRule` has 34 variants. **Code:** `B/proof.rs:32-79` lists 36 variants, the same count as `classicality.rs`'s `ALL`. **Wrong.** Corrected in sections 4 and 9.
9. **Claim** (unit_043, open fail-open): an `Unsupported` denotation is admitted with `Refused(family)` recorded. **Code:** `B/proof.rs:257-259` returns `Ok(MathematicalCoreDecision::Refused(family))`. **Matched.** My grep for `mathematical_core` finds non-test reads only where the field is set (`B/proof.rs:93,129-134,228-236`); every read that inspects it is in tests (`B/bounded.rs:218`, `B/integer_rules.rs:884`, `B/proof.rs:1880` onward, stage 06 `src/tests.rs` and `prove/tests.rs`). So no production code found that refuses a `Refused` decision by reading this field. This is a grep, not a trace through every consumer of `CertificateAcceptance`; section 5 keeps the record's class.
10. **Claim** (unit_043_gap_019, fail-open): `rule_instance` declares `premises -> conclusion` as an assumption, with 23 `rule_instance(` matches. **Code:** `B/bounded_denotation.rs:8660-8672` calls `rule_axiom`, which builds a Pi type through `interned_keyed_assumption` (`:5240-5261`); grep count is 23. **Matched.**
11. **Claim** (unit_043, open fail-open): `check_integer_affine_bound_conversion` accepts either remainder policy. **Code:** `B/integer_affine.rs:380-396` accepts `expected` or the `TotalImage` result bound to a local named `legacy`. **Matched.** The same function calls `check_multiple_bound` at `:378`, which confirms that function's single caller (section 7).
12. **Claim** (unit_046, fail-open and "Five things" 2): `terminal_graph`, `terminal_reference_graph`, `terminal_borrowed_storage_graph` return an empty `Vec` on failure, "while the `*_from` and `terminal_graph_rows` variants keep per-machine errors". **Code:** the three default at `D/descriptor_results.rs:1215,1663,1667`; `terminal_graph_from` also defaults (`:1222`); `terminal_reference_graph_from` and `terminal_borrowed_storage_graph_from` return one whole-graph `Result` (`:1655-1660`, `:1225-1236`), not per-machine errors; `terminal_borrowed_storage_graph_with_nominal_calls` collects per-machine rows into one `Result` (`:1238-1253`). **Partly right.** Corrected in sections 5, 6 and 9.
13. **Claim** (unit_044): `B/tests/suite.rs` has 20 `mod` lines. **Code:** grep counts 21. **Wrong.** Corrected in section 9.
14. **Claim** (crate map): resource-bounds items at `lib.rs:6,33,40,51,77`. **Code:** `S/storage-bounds/src/lib.rs:7,30,36,48,76`, as unit_050 says; the terminal-semantics call is at `flow.rs:1129`. **Wrong** (crate map lines). Corrected in section 1 and 9.
15. **Claim** (unit_045, open fail-open): crash, empty-elimination, external-completion and no-normal-invoke terminators retire nothing and continue. **Code:** `T/content_transformations/ancestry/retirement.rs:606-609` is an empty arm; `:612-614` then require one grounded normal return. **Matched.**
16. **Claim** (unit_047): the doc at `T/logical_providers.rs:109-110` says matching uses IDs, but the code compares text at `:231-232`. **Code:** `:230-232` does compare `identity_text(...)` strings with `partition_point`. The doc comment, "Labels are diagnostic only; matching continues to use these IDs", is attached to the error struct that follows (`:111`) and speaks of the IDs kept in that error. **Partly right:** the text compare is real; reading the comment as a statement about all matching is a stretch.

Tally: 11 matched, 2 partly right, 3 wrong.
