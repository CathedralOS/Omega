# slice_0024

Digest: area_10 (Psi stages 06–08 lowering). Preamble read at `.agents/atlas/compiler-2026-11-10/digests/area_10.md` lines 1–10, 343–366 (`read`). Path shorthand: `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/attached_unit.rs`.

| id | file:line | verdict | one-line reason |
| A0176 | attached_unit.rs:541 | REACHABLE | Structural-result boundary ensures that need the result formal are dropped and Terminal Psi validation accepts the weakened `scalar_ensures`. |
| A0177 | attached_unit.rs:6290 | UNDECIDED | `None` authored slots are published from stage 05; not traced whether lowering should reject those callables instead of omitting all authored `requires`. |
| A0178 | attached_unit.rs:7101 | UNDECIDED | Empty candidates for catalog-less boundary symbols are not traced to a later rejection of missing provider obligations. |
| A0179 | attached_unit.rs:6711 | CLOSED | Invalid receiving handles match stage-05’s intentional default handle for “no receiving” and do not bypass `InvalidBoundaryApplication` on published data. |
| A0180 | attached_unit.rs:8375 | CLOSED | Skips are gated on `collect_program_references`; unreferenced constants are omitted and referenced ones still hit error paths when incomplete. |
| A0181 | attached_unit.rs:8071 | UNDECIDED | Default proof ids and `Unavailable` descriptions are not traced to whether mis-bound proof scope fails validation later. |
| A0182 | attached_unit.rs:9345 | UNDECIDED | Open families are excluded from both lowering and the `runtime_families` length check; not traced whether such a program should be rejected entirely. |
| A0183 | attached_unit.rs:13701 | CLOSED | Only byte/element-view carriers are skipped; remaining paths must have a reference leaf or lowering returns `unsupported`. |

## A0176

Cited lines match digest (`read`). Enclosing function: `boundaries::lower_scalar_ensures` (attached_unit.rs:503–552, `read`).

At 541 and 547, implication lowering maps unavailable sides to `LoweringError::UnavailableContractResult`, and the loop swallows that error so the row is not pushed (attached_unit.rs:541, 545–547, `read`). Unavailability is raised when contract views mark the structural result unavailable (`scalar_lowering.rs:1435–1436`, `read`), and `lower_declarations` sets `ContractStructuralResult::Unavailable` when the boundary result is structural (attached_unit.rs:304–320, 321–326, `read`).

**Caller search:** `rg '\blower_scalar_ensures\b' omega-rust` → **2** matches (definition at attached_unit.rs:503, call at attached_unit.rs:321). Upward: `boundaries::lower_declarations` (attached_unit.rs:236, `read`) → `lower_unit_closure` (attached_unit.rs:16938, `read`).

**Input:** A retained boundary plan whose result is structural and whose `scalar_ensures` mention the result position reaches the swallow branch after stage 05 has already admitted the boundary.

**Acceptance:** Dropped rows yield a `BoundaryMachineDeclaration` with fewer `scalar_ensures` (attached_unit.rs:424–425, `read`). Terminal Psi foundation validation returns `Ok(())` when `scalar_ensures` is empty (foundation_validation.rs:1977–1978, `read`), so sealing does not reject the omission.

**Verdict: REACHABLE** — A program whose boundary should still carry scalar ensures over a structural result can complete lowering and validation with those ensures absent from the portable boundary declaration.

## A0177

Cited line matches digest (`read`). Enclosing function: `entry_requirements::lower` (attached_unit.rs:6230–6328, `read`).

At 6289–6318, `authored_requires` is taken from the closed scalar contract; lowering runs the authored map only when `authored_requires.iter().all(Option::is_some)`; otherwise authored clauses are not emitted and `requires` may still gain `runtime_requirements` (attached_unit.rs:6244–6287, 6319–6322, `read`). Stage 05 builds `requires` with `requires.push(lower_clause(contract))` where `lower_clause` can return `None` (published_facts.rs:3253, 3100–3131, `read`).

**Caller search:** `rg 'entry_requirements::lower' omega-rust` → **2** call sites (`attached_unit.rs:9288`, `unit_composed_control.rs:6784`, `read`). Upward: `signatures::lower` (attached_unit.rs:9288–9303, `read`) including `cleanup_context.validate_contract` on the resulting `requires`; composed crash lowering (`unit_composed_control.rs:6783–6801`, `read`).

**Verdict: UNDECIDED** — `read` that `None` is stored at publication, but did not trace whether a callable with any `None` authored slot should be rejected before lowering or whether omitting all authored `requires` while keeping runtime rows lets a program be accepted that should not be.

## A0178

Cited line matches digest (`read`). Enclosing function: `providers::boundary_candidates` (attached_unit.rs:7058–7110+, `read`).

In the `exact_requirements` empty branch, trait/top-level/operator resolution falls through to `None => return Ok(Vec::new())` (attached_unit.rs:7088–7102, `read`).

**Caller search:** `rg '\bboundary_candidates\b' omega-rust` → **2** matches (definition at attached_unit.rs:7058, call at attached_unit.rs:7020). Upward: `checked_unit_provider_candidates` (attached_unit.rs:6959, `read`) aggregates candidates for every deduped boundary symbol from the closure (attached_unit.rs:6970–7025, `read`) → `call_catalog::discover` (attached_unit.rs:956, `read`) → `lower_unit_closure` (attached_unit.rs:16807–16808, `read`).

**Input:** A boundary symbol retained in the call catalog (e.g. from `boundary_targets` or boundary calls) that is not exactly one trait requirement, a top-level requirement, or a boundary operator.

**Verdict: UNDECIDED** — `read` the silent empty list; did not trace provider conformance or build gates that must reject a program when no candidates exist for a required boundary.

## A0179

Cited lines match digest (`read`). Enclosing function: `providers::selection_coordinates::intern_prepared_receiving_application` (attached_unit.rs:6706–6724, `read`).

At 6711–6716, `!receiving.is_valid()` returns `Ok` with default identity fields and empty `regions` (attached_unit.rs:6711–6716, `read`). Otherwise publication lookup errors with `InvalidBoundaryApplication` (attached_unit.rs:6718–6722, `read`).

**Caller search:** `rg 'intern_prepared_receiving_application' omega-rust` → **3** matches (definition attached_unit.rs:6706; calls attached_unit.rs:6699, 6878). `rg 'intern_receiving_application' omega-rust` → additional wrapper at attached_unit.rs:6694, call at attached_unit.rs:7253 (`read`).

**Input:** `arena::Handle::default()` for `receiving`, which stage 05 publishes for provider declarations without a receiving edge (published_facts.rs:8964–8967, `read`).

**Verdict: CLOSED** — Every production path goes through this function for receiving interning; invalid handle is an explicit encoding for “no receiving,” not a substitute for a failed lookup on valid handles.

## A0180

Cited lines match digest (`read`). Enclosing function: `scalar_applications::SourceScalarApplications::prepare` (attached_unit.rs:8355–8507, `read`).

Skipped paths: unreferenced constant recipes (`retained` false → `continue`, attached_unit.rs:8365–8377), const declarations already evaluated or without evaluation entry (8431–8434), layout observations not referenced or already landed (8467–8472, `read`).

**Caller search:** `rg 'SourceScalarApplications::prepare' omega-rust` → **3** matches (attached_unit.rs:16788 production; 9047, 9083 in unit tests, `read`). Upward: `lower_unit_closure` (attached_unit.rs:16787–16789, `read`) before boundary/signature lowering.

**Reference gate:** `typed_trees::references::collect_program_references` at start of `prepare` (attached_unit.rs:8357–8358, `read`).

**Verdict: CLOSED** — Inputs that reach the skip branches are not in the program reference set; referenced formations that still cannot be retained hit `Unsupported` (e.g. attached_unit.rs:8413–8415, `read`).

## A0181

Cited lines match digest (`read`). Enclosing function: `scalar_applications::proof_scope::publish` (attached_unit.rs:8058–8091, `read`), closure `identity` at 8069–8090.

At 8071–8072, invalid `symbol` returns `ProofDeclarationId::default()` without interning a description (`read`). At 8085–8087, `describe(...)` failure inserts `ProofScopeDeclaration::Unavailable` (`read`). `describe` (attached_unit.rs:8288–8316, `read`).

**Caller search:** `rg 'proof_scope::publish' omega-rust` → **1** production call (attached_unit.rs:8894 inside `SourceScalarApplications::publish`, `read`) → `lower_unit_closure` scalar publication path (`read`).

**Downstream:** Terminal Psi validation allows `ProofScopeDeclaration::Unavailable` when the reason is non-empty (foundation_validation.rs:2918–2921, `read`); did not read full proof-call binding.

**Verdict: UNDECIDED** — `read` the default id and `Unavailable` insertion; not traced whether a satisfiable program can pass artifact validation with mis-associated proof declarations.

## A0182

Cited line matches digest (`read`). Enclosing function: `target_families::runtime_families` (attached_unit.rs:9321–9350, `read`).

At 9332–9347, only families whose head lacks specialization parameters (`!open` after member consistency check) are pushed (`read`).

**Caller search:** `rg '\bruntime_families\b' omega-rust` → **5** matches (attached_unit.rs:869, 9321, 9382; machine_lowering.rs:9 import, 1962, `read`). Upward: `call_catalog::discover` root extension (attached_unit.rs:869, `read`); `target_families::lower` (attached_unit.rs:9382, `read`); post-lowering length check `lowered.semantic_module.target_machine_families.len() != runtime_families(checked)?.len()` (machine_lowering.rs:1962–1963, `read`).

**Verdict: UNDECIDED** — `read` symmetric exclusion from lowering and the length check for closed families only; not traced whether a program that relies on open (generic) target families should be rejected rather than lowered with none recorded.

## A0183

Cited line matches digest (`read`). Enclosing function: `parameters::lower_reference_result_sources` (attached_unit.rs:13658–13740+, `read`).

At 13689–13701, `continue` skips references whose field carrier or leaf shape is a byte-sequence view or element view (`read`). At 13703–13709, non-skipped rows must have a reference-shaped leaf or lowering errors (`read`).

**Caller search:** `rg 'lower_reference_result_sources' omega-rust` → **4** matches (definition attached_unit.rs:13658; re-export 13798; calls attached_unit.rs:11624, unit_composed_control.rs:13555, `read`). Upward: ordinary boundary/ machine structural result lowering (attached_unit.rs:11624, `read`).

**Input:** Checked `reference_sources` entry for a returned reference plan whose static path resolves to a byte or element view carrier.

**Verdict: CLOSED** — Skipped rows are intentionally excluded from `reference_sources`; any non-skipped row still passes the borrowed-carrier leaf gate or lowering fails closed with `unsupported`.

## What I did not read

- Full `attached_unit.rs` outside cited functions and caller chains (`inferred` from grep only for some upstream modules).
- Complete `scalar_lowering::contracts::namespace::inspect` and proposition lowering paths beyond `UnavailableContractResult` (`inferred`).
- Stage 05 rejection rules for boundaries without catalog entries, provider obligations, and callable programs with `None` authored requires (`inferred`).
- Stage 07–08 modules beyond cited `foundation_validation` proof-scope and boundary-scalar snippets.
- `git show 457adf4a11f:...` (cited lines matched at current checkout without relocation).
- Other digests, corpus fixtures, and atlas records beyond area_10 preamble and accept list.

SLICE COMPLETE
