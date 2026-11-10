# slice_0069

Claim: area_14 (ACCEPT rows A0529–A0536). Digest preamble read at `.agents/atlas/compiler-2026-11-10/digests/area_14.md:1-56`. Code paths under `omega-rust/psi/compilation/validation/src/` unless noted.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0529 | proof_contracts_core.rs:8971-8977 | REACHABLE | Boolean contract facts key only on rendered proof text, so distinct expressions with the same rendering collapse in entailment sets. |
| A0530 | proof_contracts_core.rs:9095 | REACHABLE | Call targets that fall through to `Propagated` still supply a qualification source, so callee membership ensures are not dropped in entailment. |
| A0531 | proof_contracts_core.rs:9684-9686 | REACHABLE | Boundary membership authorization treats carrier and domain as matching when nominal heads agree, without requiring generic arguments to agree. |
| A0532 | proof_contracts_core.rs:10571-10572 | REACHABLE | Runtime call-receiver projection returns without diagnostic when an intermediate owner type cannot be resolved to a data definition. |
| A0533 | proof_contracts_core.rs:10464-10469 | REACHABLE | Struct literals whose type symbol has no data definition skip erased-field obligation checks after visiting field values only. |
| A0534 | proof_contracts_core.rs:10996-10999 | UNDECIDED | Validation emits a scalar ensure whose conclusion is tautological; downstream lowering was only partially traced. |
| A0535 | quotients.rs:477-479 | REACHABLE | Proof-dependency walk skips dependency symbols that do not resolve to a machine instead of failing the quotient checked-body gate. |
| A0536 | quotients.rs:3586-3588 | CLOSED | Quotient proof-fact identity requires both rendered text and trace strings to match; rendering includes operators though trace omits them. |

## A0529

**Digest:** area_14 claims proposition entailment compares `BTreeSet<String>` labels built via rendering (area_14.md:93, 300).

**Cited site (read):** In `proposition_entailment::proposition_labels` (`proof_contracts_core.rs:8956-8987`), `ProofFact::Expression` with `include_boolean_expressions` true formats `boolean:{}` using `program.render_proof_expression` with `ProofSubstitutions::ByParameter(substitutions)` (`proof_contracts_core.rs:8970-8977`).

**Enclosing function (read):** Full `proposition_labels` and module `validate_proposition_entailment` (`proof_contracts_core.rs:8506-8709`).

**Callers (search):** `rg '\bvalidate_proposition_entailment\b' /agent/repos/omega` → 3 matches (1 definition `proof_contracts_core.rs:8506`, 1 production `program_validation.rs:377`, 1 atlas record). `rg 'proposition_labels\(' omega-rust/psi/compilation/validation/src/proof_contracts_core.rs` → 8 call sites in the same file (read uses at 8534, 8564, 8588, 8597, 8732, 8808, 8911).

**Upward path (read):** `program_validation.rs:377` runs during program validation prologue. Known and required goals are `BTreeSet<String>` / `Vec<String>` compared with `known.contains(goal)` and `retain` (`proof_contracts_core.rs:8596-8604, 8650-8705`). Distinct expression handles that render identically become one label.

**Input that reaches the branch:** A checked machine whose state or machine `requires`/`ensures` include two different boolean proof expressions that `render_proof_expression` prints the same way (for example the same parameter display spellings under shadowing) so one is treated as establishing the other.

**Verdict:** REACHABLE (read).

## A0530

**Digest:** area_14.md:301 — uncategorized call targets “keep” qualification facts.

**Cited site (read):** `CallQualificationTarget::resolve` ends with `Self::Propagated` (`proof_contracts_core.rs:9089-9095`). `fact_source` maps that to `Some(CallQualificationSource::Propagated)` (`proof_contracts_core.rs:9109`).

**Enclosing functions (read):** `resolve` and `fact_source` (`proof_contracts_core.rs:9066-9111`); `CallQualificationContract::prepare` wires `target: CallQualificationTarget::resolve(program, target)` (`proof_contracts_core.rs:9129-9157`).

**Callers (search):** `rg 'CallQualificationContract::prepare' omega-rust` → `contract_entailment.rs:59706` (read). `rg '\bcall_qualification_source\b' omega-rust` → 3 matches (definition, re-export, `published_facts.rs` ×2, not read).

**Upward path (read):** In contract entailment, for each callee `Ensures` membership fact, if `qualification_contract.fact_source(...).is_none()` the fact is skipped (`contract_entailment.rs:59720-59729`). `Propagated` yields `Some`, so membership ensures are retained for targets that matched none of the machine/boundary/trait branches in `resolve`.

**Input that reaches the branch:** A call whose `target_symbol` is not resolved as checked/boundary machine, boundary requirement, or boundary trait (falls through to `Propagated`), while the callee signature still carries membership ensures the entailment judge ingests.

**Verdict:** REACHABLE (read).

## A0531

**Digest:** area_14.md:302 — return type match ignores generic arguments.

**Cited site (read):** `return_type_matches_domain_target` (`proof_contracts_core.rs:9670-9687`) returns true on normalized type identity equality, else compares `nominal_head_symbol` pairs only (`proof_contracts_core.rs:9684-9686`).

**Enclosing function (read):** `boundary_qualification_authorization` (`proof_contracts_core.rs:9606-9667`) calls the matcher at `proof_contracts_core.rs:9660` and returns `None` when false.

**Callers (search):** `rg '\breturn_type_matches_domain_target\b' /agent/repos/omega` → 2 (definition + call). `rg '\bboundary_qualification_authorization\b' omega-rust/psi/compilation/validation` → 7 matches in 4 files (3 sites in `proof_contracts_core.rs` read at 9294, 9235 path, 9606).

**Upward path (read):** Successful `boundary_qualification_authorization` enables `CallQualificationContract::fact_source` to return admitted qualification for boundary membership ensures (`proof_contracts_core.rs:9225-9244`). Failure returns `None` and obligations are skipped in `boundary_provider_qualification_obligations` (`proof_contracts_core.rs:9294-9303`).

**Input that reaches the branch:** Boundary-trait ensures membership whose subject carrier and domain `target_type` share the same closed nominal head but differ in generic arguments, where normalized identities differ.

**Verdict:** REACHABLE (read).

## A0532

**Digest:** area_14.md:303 — erased field via unresolvable receiver not reported (includes `pcc:10571-10582`).

**Cited site (read):** `runtime_uses::validate_statement_call_receiver` (`proof_contracts_core.rs:10537-10595`). When `owner` is `None`, the `for member in projected` loop returns immediately (`proof_contracts_core.rs:10571-10572`) without pushing a diagnostic.

**Enclosing function (read):** Full function `proof_contracts_core.rs:10537-10595`.

**Callers (search):** `rg '\bvalidate_statement_call_receiver\b' /agent/repos/omega` → 2 (definition + `proof_contracts_core.rs:9796`).

**Upward path (read):** `relevance::validate_expression` on runtime calls invokes the validator (`proof_contracts_core.rs:9795-9802`). `validate_relevance` is scheduled from `program_validation.rs:485`.

**Related site (read):** `report_runtime_erased_member_by_receiver` returns silently when `data_definition_for_type` is `None` (`proof_contracts_core.rs:10714-10717`).

**Input that reaches the branch:** A runtime `Call` whose receiver name path projects through a member chain where `receiver_root_type` or a field’s type does not map to a data definition (`owner` becomes `None` before an erased member would be diagnosed).

**Verdict:** REACHABLE (read).

## A0533

**Digest:** area_14.md:304 — struct literal without data definition skips erased-field check.

**Cited site (read):** `relevance::validate_struct_literal` (`proof_contracts_core.rs:10441-10535`). On `data_definition_by_symbol` miss, only field values are visited, then `return` (`proof_contracts_core.rs:10464-10468`).

**Enclosing function (read):** Full `validate_struct_literal` (same range).

**Callers (search):** `rg 'validate_struct_literal' omega-rust/psi/compilation/validation` → `proof_contracts_core.rs:10380` (expression walk) and separate `literals_and_struct_literals.rs` prologue helper (not the same fn).

**Upward path (read):** Dispatched from `validate_expression` for `ExpressionNode::StructLiteral` in runtime relevance checking (`proof_contracts_core.rs:10380-10388`), under `validate_relevance` (`program_validation.rs:485`).

**Input that reaches the branch:** A struct literal expression at runtime whose `type_symbol` does not resolve to a data definition (missing or foreign type) while omitted erased fields would otherwise be diagnosed when a definition exists (`proof_contracts_core.rs:10514-10522`).

**Verdict:** REACHABLE (read).

## A0534

**Digest:** area_14.md:305 — emitted conclusion is `result == result`; consumption “not traced”.

**Cited site (read):** `scalar_boundary_guarantees::meaning_consequence` builds `Guarantee::Implication` with `conclusion: compare(Compare::Equal, result.clone(), result)` (`proof_contracts_core.rs:10996-10999`).

**Enclosing functions (read):** `meaning_consequence` (`proof_contracts_core.rs:10870-11000`); `derive_scalar_boundary_guarantees` (`proof_contracts_core.rs:10839-10867`) pushes guarantees from contract ensures.

**Callers (search):** `rg '\bderive_scalar_boundary_guarantees\b' /agent/repos/omega` → 4 matches: definition, `lib.rs` re-export, `execution_terminal_unit_control.rs:436` (read), atlas record.

**Partial consumption (read):** `execution_terminal_unit_control.rs:436-438` stores guarantees on the boundary plan. `attached_unit.rs:516-540` lowers `CheckedBoundaryScalarGuarantee::Implication` to terminal propositions without rejecting tautological conclusions.

**What would decide acceptance:** Whether any later stage treats a tautological scalar conclusion as proving a substantive float postcondition that validation should have required, or discards it as vacuous end-to-end. Not fully traced beyond lowering.

**Verdict:** UNDECIDED (read for emit; inferred for full acceptance impact).

## A0535

**Digest:** area_14.md:308 — non-machine dependency skipped in checked proof dependency walk.

**Cited site (read):** `quotients::checked_proof_dependency` (`quotients.rs:463-485`). For each dependency symbol, `machine_by_own_or_state_symbol` `None` → `continue` (`quotients.rs:477-478`).

**Enclosing function (read):** Full function `quotients.rs:463-485`.

**Callers (search):** `rg '\bchecked_proof_dependency\b' /agent/repos/omega` → 4 references in `quotients.rs` (definition, recursive call 481, `equivalence_selection` 203 read, `require_checked_proof_closure` 7285 read).

**Upward path (read):** `equivalence_selection` treats `Err` from the walk as a diagnostic (`quotients.rs:203-210`); `Ok(())` when all resolvable callees are checked and unresolved symbols are skipped. `require_checked_proof_closure` maps `Ok` to success (`quotients.rs:7285-7290`).

**Input that reaches the branch:** A quotient conformance proof machine whose call graph includes a `machine_call_dependency_symbols` entry that does not resolve to any machine (for example a non-machine callable symbol); the walk does not error on that edge.

**Verdict:** REACHABLE (read).

## A0536

**Digest:** area_14.md:309 — trace key omits Binary/Unary operators; sound if rendered half includes them.

**Cited site (read):** `expression_symbol_trace` formats `ExpressionNode::Binary` as `left|right` only (`quotients.rs:3586-3588`) and `Unary` as `trace(operand)` (`quotients.rs:3616`).

**Enclosing functions (read):** `expression_symbol_trace` (`quotients.rs:3495-3627`); `proof_expression_identity` pairs rendered string from `render_proof_expression` with trace (`quotients.rs:3462-3492`). `proof_facts_match` requires both halves equal (`quotients.rs:3358-3362`).

**Rendering (read):** `typed-trees/.../evidence_and_inspection.rs:1739-1744` renders binary with `binary.operator.display_name()`; unary at `1837-1838` includes operator text.

**Callers (search):** `rg '\bproof_expression_identity\b' /agent/repos/omega` → 3 in `quotients.rs` (definition + `proof_facts_match`).

**Closing check:** Any `ProofFact::Expression` identity collision must match both `(rendered, trace)` tuples (`quotients.rs:3360-3362`). Operator-distinguishable expressions differ in `rendered` even when trace collides.

**Input that reaches the branch:** Quotient proof-fact matching on expression facts uses the pair; ambiguous trace alone cannot equate distinct binaries/unaries.

**Verdict:** CLOSED (read).

## What I did not read

- `published_facts.rs` call-qualification consumers (2 `call_qualification_source` sites).
- Full `contract_entailment.rs` beyond membership intake at 59706-59729.
- Terminal Psi runtime enforcement of `scalar_ensures` propositions after lowering.
- `git show 457adf4a11f:` snapshots (cited lines matched current `proof_contracts_core.rs` / `quotients.rs` at listed numbers).

SLICE COMPLETE
