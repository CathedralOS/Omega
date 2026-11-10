# slice_0055

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0417 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:90957-90963 | REACHABLE | Empty-bodied machines can leave non-arithmetic ensures silently unproven when `goal_in_language` is false. |
| A0418 | omega-rust/psi/compilation/validation/src/contract_entailment.rs:90781-90784 | REACHABLE | Mention-class ensures are pushed to `proven` whenever `hypotheses_contradictory` without structural judgment. |
| A0419 | omega-rust/psi/compilation/validation/src/declarations_core.rs:133-135 | CLOSED | Unresolved public-interface selections skip visibility here but fail in checked selection normalization. |
| A0420 | omega-rust/psi/compilation/validation/src/declarations_core.rs:998-1000 | REACHABLE | Toolchain, builtin, and intrinsic selections record `Ok(None)` and never enter the dependency `allows` check. |
| A0421 | omega-rust/psi/compilation/validation/src/declarations_core.rs:933 | REACHABLE | `excluded` selection requirements are not validated against package authority at checked-trees time. |
| A0422 | omega-rust/psi/compilation/validation/src/declarations_core.rs:1106 | UNDECIDED | `allows` on `Absent` returns `None` only for incremental source reuse; no path to accepting a bad compile was found. |
| A0423 | omega-rust/psi/compilation/validation/src/declarations_core.rs:1646 | REACHABLE | Symbols with no provenance source file count as product declarations via `is_none_or`. |
| A0424 | omega-rust/psi/compilation/validation/src/declarations_core.rs:3151-3161 | REACHABLE | Terminal result validation returns early when either return type is not Unit, primitive, or named data. |

## A0417

**Cited site (read):** `contract_entailment.rs:90950-90964` inside `validate_machine_contract_entailment`. Lines match the atlas cite at this checkout.

On `Judgment::Unknown { goal_in_language }`, the engine may still prove via `premises_prove_expression` (`contract_entailment.rs:90951-90955`). Otherwise a diagnostic is emitted only when `goal_in_language && requires_fully_visible` (`contract_entailment.rs:90957-90963`); if `goal_in_language` is false, the fact is neither proven nor diagnosed.

**Arithmetic engine (read):** `Engine::judge` at `contract_entailment.rs:6779-6823` returns `Judgment::Unknown { goal_in_language: false }` when `comparison_polynomials` returns `None` (`contract_entailment.rs:6818-6821`), i.e. the goal is outside the arithmetic comparison language.

**Empty-body gate (read):** `body_is_empty` is true when every machine state has no statements (`contract_entailment.rs:90873-90878`). Only then is `Engine::new` / the loop over `engine.judge` reached (`contract_entailment.rs:90905-90912`); bodied machines exit earlier through `inductive_transition_entailment` (`contract_entailment.rs:90879-90902`).

**Ensures filter (read):** Facts without zero-value or proof-only mentions that fail structural proof are retained (`contract_entailment.rs:90764-90778`) unless earlier arms remove them.

**Callers (read):** `rg '\bvalidate_machine_contract_entailment\b' omega-rust` → 4 matches: definition at `contract_entailment.rs:90324`, calls at `program_validation.rs` (including `validate_generic_machine_contract_entailment` at `program_validation.rs:191` and the main entailment path at `program_validation.rs:4756`, read via grep listing).

**Input (plain words):** A proof machine with no state body, satisfiable requires, and an ensures fact that is not a structural or proof-only mention and is not expressible as arithmetic comparisons (for example a call-shaped proposition), can finish entailment with that ensures neither in `proven` nor in `diagnostics`.

**Verdict:** REACHABLE.

## A0418

**Cited site (read):** `contract_entailment.rs:90711-90784` in the ensures `filter` closure of `validate_machine_contract_entailment`. Cite matches.

`mention` is built from zero-value or proof-only mentions (`contract_entailment.rs:90713-90720`). When `mention` is `Some`, the filter sets `any_structural = true` (`contract_entailment.rs:90780`). If `structural.hypotheses_contradictory`, the fact is pushed to `proven.entry` and filtered out (`contract_entailment.rs:90781-90783`) before `judge_structural` (`contract_entailment.rs:90804+`).

**Flag (read, partial):** `hypotheses_contradictory` is set during structural intake (for example `contract_entailment.rs:51417`, `:51993`, `:52078`, `:52105`, `:52336`); not every setter was read.

**Callers (read):** Same as A0417 — `validate_machine_contract_entailment` from `program_validation.rs`.

**Input (plain words):** A machine whose structural judge already carries contradictory requires hypotheses and whose ensures fact is classified as a proof-only or zero-value mention is recorded proven without judging whether the mention obligation holds.

**Verdict:** REACHABLE.

## A0419

**Cited site (read):** `declarations_core.rs:126-135` in `collect_declaration_visibility_diagnostics`. Public-interface selections whose `target()` is not `as_resolved()` hit `continue` (`declarations_core.rs:133-135`) and receive no visibility diagnostic in this pass.

**Closing route (read):** `normalize_checked_selection` returns `Err` for `AuthoredDeclarationSelectionTarget::LateBound` (`declarations_core.rs:1009-1013`). `checked_selection_requirements` stores that in each row's `request` field (`declarations_core.rs:979`). `validate_authored_declaration_selections` matches `Err(reason)` and pushes a diagnostic (`declarations_core.rs:945-946`) before `finish_strict_diagnostics` (`declarations_core.rs:950`).

**Callers (read):** `rg '\bcollect_declaration_visibility_diagnostics\b' omega-rust` → 2 production uses: `declarations_core.rs:234` and prologue at `program_validation.rs:365`. Checked-tree authority validation is at `pipeline/05_typed-trees-to-checked-trees/src/checking.rs:996` (`validate_authored_declaration_selections`, read via grep).

**Input (plain words):** A public-interface declaration selection that remains late-bound through typed validation skips the visibility collector, but checking still fails when normalized selection requirements are validated on the checked tree.

**Verdict:** CLOSED — visibility is not checked at `133-135` for unresolved targets, but the same selection is rejected at `declarations_core.rs:1009-1013` / `945-946`.

## A0420

**Cited sites (read):** `normalize_checked_selection` at `declarations_core.rs:988-1074`.

- Toolchain-authored selection sites return `Ok(None)` (`declarations_core.rs:998-1000`).
- Builtin atoms, builtin functions, builtin type members, and fixed-core symbols return `Ok(None)` (`declarations_core.rs:1020-1031`).
- Selected symbols with `SourceOrigin::Toolchain` return `Ok(None)` (`declarations_core.rs:1063-1064`).

**Authority validation (read):** `validate_authored_declaration_selections` only diagnoses missing dependency when `request` is `Ok(Some(request))` and `!packages.allows_declaration_selection(...)` (`declarations_core.rs:937-943`). `Ok(None)` and the `_ => {}` arm impose no obligation (`declarations_core.rs:947`).

**Callers (read):** `normalize_checked_selection` is called from `checked_selection_requirements` (`declarations_core.rs:979`); requirements are validated at `checking.rs:996`.

**Input (plain words):** Any authored declaration selection whose requesting file is toolchain-origin, or whose selected symbol is builtin/toolchain/fixed-core, is accepted without recording a cross-package authority request, regardless of declared dependencies.

**Verdict:** REACHABLE (intentional omission of authority obligation for those origins).

## A0421

**Cited site (read):** `declarations_core.rs:921-948` in `validate_authored_declaration_selections`. When the checked requirement is `Selection { excluded: true, ... }`, the loop `continue`s at `declarations_core.rs:933-935` before inspecting `request`.

**How `excluded` is set (read):** `checked_selection_requirements` sets `excluded` when the occurrence is in `descriptor_selection_occurrences` skip set or the owning machine is pruned (`declarations_core.rs:974-978`).

**Callers (read):** `validate_authored_declaration_selections` at `checking.rs:996`; `checked_selection_requirements` published from `published_facts.rs:4579` (grep).

**Input (plain words):** A declaration selection occurrence inside a descriptor-skipped region or on a pruned machine is marked excluded and never checked against `allows_declaration_selection`, even if normalization would have produced `Ok(Some(request))` for a cross-package selection.

**Verdict:** REACHABLE.

## A0422

**Cited site (read):** `declarations_core.rs:1100-1110` — `RecordedDeclarationAuthority::allows` returns `None` for `Self::Absent` (`declarations_core.rs:1105-1106`).

**Callers (read):** `rg '\.allows\(requester' omega-rust` → single match at `psi-compiler/src/source_compilation.rs:108`, inside incremental source-reuse comparison. There `None` is converted with `.ok_or(SourceReuseMiss::DeclarationAuthority)?` (`source_compilation.rs:114`), failing reuse rather than accepting a compile.

**Compile-time authority (read):** Package selection obligations for acceptance use live `DeclarationSelectionAuthority::allows_declaration_selection` in `validate_authored_declaration_selections` (`declarations_core.rs:938`), not `RecordedDeclarationAuthority::allows`.

**Verdict:** UNDECIDED — `Absent` → `None` is traced for source-reuse invalidation only; I did not find a program acceptance path that treats `None` as permission to skip dependency checking.

## A0423

**Cited site (read):** `declarations_core.rs:1641-1647` — `is_product_declaration` is true when `symbol_provenance_source_span` is missing or the source file's `dependency_scope` is `Product`, via `Option::is_none_or` (`declarations_core.rs:1644-1646`).

**Consumers (read):** `is_product_declaration` is used in source-completion nomination and acceptance at `declarations_core.rs:1568`, `:1593`, and `:1625` (read).

**Input (plain words):** A trait or machine symbol with no provenance source span (generated or otherwise without a custody file) is treated like a product declaration when discovering or accepting boundary completion roles.

**Verdict:** REACHABLE.

## A0424

**Cited sites (read):** `declarations_core.rs:3142-3182` in `terminal_results::validate`, and `shape` at `declarations_core.rs:3184-3212`.

Unreachable transitions return immediately (`declarations_core.rs:3151-3152`). When `shape` for expected or actual return type is `None` (reference, slice, array, generic, trait, and other non-Unit/primitive/data shapes via `TypeReferenceNode::_` at `declarations_core.rs:3210-3211`), `validate` returns at `declarations_core.rs:3159-3160` without pushing the mismatch diagnostic at `declarations_core.rs:3173-3181`.

**Callers (read):** `rg '\bterminal_results::validate\b' omega-rust` → 2 call sites in `declarations_core.rs:3302` and `:3330` on terminal transition targets (read).

**Input (plain words):** A caller state whose return type is a named data type and a terminal callee state returning a reference (or any type `shape` cannot classify) produce no terminal-result mismatch error.

**Verdict:** REACHABLE.

## What I did not read

- Full inventory of every `hypotheses_contradictory = true` assignment beyond samples cited for A0418.
- Whether excluded selections are validated on any path other than `validate_authored_declaration_selections` (A0421).
- Corpus or samples showing compile success for A0417 silent-unknown ensures.
- Every consumer of `is_product_declaration` beyond the completion helpers (A0423).
- Later pipeline stages that might reject programs with mismatched reference terminal results (A0424).

SLICE COMPLETE
