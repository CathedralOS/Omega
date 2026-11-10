# slice_0040

Claim: ACCEPT | area_11 (`digests/area_11.md` preamble, path shorthand `ct/` = `omega-rust/psi/compilation/representations/checked-trees/src/`).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0301 | `contract_and_obligation_facts.rs:2503` | UNDECIDED | `PartialEq` is spelling-only canonical bytes; production guard match at `published_facts.rs:8580` read, but `validation/contract_entailment.rs` consumers not read. |
| A0302 | `admissibility_and_borrow.rs:292` | CLOSED | `StateOperationAcceptance::statement_index` has no production callers (`rg`); default `0` is unreachable for acceptance. |
| A0303 | `lib.rs:328` | CLOSED | Missing key yields `&[]` via `map_or`; sampled callers treat empty as absent fact, not acceptance. |
| A0304 | `providers_and_trust.rs:107-108` | UNDECIDED | Non-installation-bound requirement returns `Ok(None)` reach; downstream admission of that omission not read. |
| A0305 | `providers_and_trust.rs:161-163` | UNDECIDED | Non-trait requirement returns empty external apps without error; whether review rejects that list not read. |
| A0306 | `termination_and_cleanup_facts.rs:115` | UNDECIDED | `path` handle span not remapped while `segments` are shifted; whether merged walk ever accepts bad cleanup facts not verified. |
| A0307 | `facts.rs:266` | REACHABLE | Non-Binary/Unary/Borrow nodes return `true` without child check, so a call-shaped guard can pass while an argument uses a user operator. |
| A0308 | `facts.rs:2999-3004` | CLOSED | `classify_type_references` (`validation/lib.rs:2072-2087`) fills one row per table handle before facts are sealed; OOB handle would be a separate bug. |

## A0301

**Trace (read).** `CrashPredicateIdentity::eq` at `omega-rust/psi/compilation/representations/checked-trees/src/contract_and_obligation_facts.rs:2501-2504` returns true when `canonical_bytes` match, without comparing symbols. `write_canonical` for `Name` / `Member` encodes member strings (`:2353-2365`, `:2361-2365`), and crash lowering uses spelled names via `published_facts_crash_calls.rs:4992-5005` (`member_entry_name`, read). Production matching uses identity equality at `pipeline/05_typed-trees-to-checked-trees/src/published_facts.rs:8574-8581` and `BTreeSet` membership at `pipeline/06_checked-trees-to-lowered-psi/src/proofs.rs:4520-4531` (read).

**Input (plain words).** Two crash-route predicates that spell the same parameter or field name but resolve to different symbols could share one canonical byte sequence.

**Why UNDECIDED.** Digest names `validation/contract_entailment.rs` (42 matches) as unwalked consumers. Without reading whether entailment or earlier checking already separates those symbols, cannot say a program that should be rejected is accepted end-to-end.

## A0302

**Trace (read).** `StateOperationAcceptance::statement_index` at `admissibility_and_borrow.rs:283-294` returns `0` for `Operator` origins that are not `CheckedValueOrigin::StateStatement`.

**Caller search (read).** `rg -w 'StateOperationAcceptance' omega-rust` outside `admissibility_and_borrow.rs`: 1 hit (`lib.rs` re-export). `rg '\.statement_index\(\)' admissibility_and_borrow.rs`: only the definition at `:283` (no call sites). The method is not invoked on the acceptance API path described in `pipeline/05_typed-trees-to-checked-trees/README.md:47` (AcceptanceView gathers evidence; read heading only).

**Why CLOSED.** No production path reads this default; compilation acceptance does not branch on it.

## A0303

**Trace (read).** `KeyedRows::with_rows` at `lib.rs:317-329` calls `groups.get(key).map_or(&[], Vec::as_slice)`.

**Caller search (read).** `rg 'with_rows\(' omega-rust/psi/compilation/representations/checked-trees` → 14 production call sites in `flow.rs`, `values_and_views.rs`, `operators_and_proof.rs`, `admissibility_and_borrow.rs`, `termination_and_cleanup_facts.rs` (count inferred from listing). Sampled: `flow.rs:1339-1345` `state_by_symbol` uses `rows.first()` → `None` when key missing (read); `flow.rs:1348-1356` `unique_state_by_symbol` returns `None` unless exactly one row (read).

**Why CLOSED.** Missing-key empty slice surfaces as absent lookup, not as a positive acceptance branch in sampled callers.

## A0304

**Trace (read).** `provider_reach` at `providers_and_trust.rs:93-150`: after `requirement_reach` (`:333-353`, read), if `!bounded` at `:107-108` returns `Ok(None)` with no error. Called from `provider_declarations` only when `request.installation_reach` is true (`:51-54`, read). Downstream capture: `provider_planning/selected_declarations.rs:57-65` passes `installation_reach` and stores the resulting declarations (read).

**Input (plain words).** A provider row whose exact requirement is not flagged installation-bound still participates in an installation-reach declaration request but gets no reach record.

**Why UNDECIDED.** Did not read `require_selected_provider_declarations` or review gates to see whether `None` reach fails closed for programs that should be rejected.

## A0305

**Trace (read).** `provider_external_applications` at `providers_and_trust.rs:153-163`: `typed.trait_machine_signature(row.requirement)` `None` → `Ok(Vec::new())` without diagnostic. Invoked from `provider_declarations` for `ProviderApplicationView::External` (`:76-78`, read).

**Input (plain words).** A provider declaration row labeled external import whose requirement handle is not a trait machine yields an empty application list silently.

**Why UNDECIDED.** Did not read provider admission / review validation to determine whether an empty list allows a bad program through.

## A0306

**Trace (read).** `ContextualCleanupFacts::append_shifted` at `termination_and_cleanup_facts.rs:47-122`: `segment_rows` shifted (`:67`, `:85-86`); disposition `path` copied verbatim at `:115` while `exclusions` / `definite_exclusions` / `inactive_cases` use `shift_span` (`:116-118`). `path` type is `HandleSpan<PlaceSegment>` (`:334`, read). Production caller: `checks_multiplicity_permission_events.rs:597-601` aborts walk when `append_shifted` returns false (read).

**Why UNDECIDED.** Inferred (not verified) that stale `path` spans could index the wrong `PlaceSegment` rows after merge; did not prove that walk failure always rejects the program or that a succeeding walk accepts incorrect cleanup.

## A0307

**Trace (read).** `has_builtin_operators` at `facts.rs:244-267`: early `false` only when `operators.uses` records a non-`BuiltinFallback` use for this expression (`:249-253`); `Binary`/`Unary`/`Borrow` recurse (`:256-265`); all other kinds including `Call` hit `_ => true` (`:266`). Definition site plus **42** occurrences in `omega-rust` (`rg -w has_builtin_operators`, read count); production call sites include `checks_contracts_prover.rs:2195`, `checks_contracts.rs:4896`, `checks_contracts_exits.rs` (sampled, read).

**Input (plain words).** A contract or exit guard written as a call expression is treated as “builtin operators only” at the call node even when a subexpression (e.g. argument) uses a user-defined operator, because child expressions are not visited for `Call`.

**Why REACHABLE.** Callers gate proof / closed-boolean evaluation on `has_builtin_operators(program, operators, expression)` on the whole handle (`closed_boolean_value` `:233-234`, read); the call node can return `true` while an argument would fail if checked.

## A0308

**Trace (read).** `ContentClassFacts::classes` at `facts.rs:2999-3004` (`get(arena_index).or_else(|| rows.first())`). Parallel pattern `ReferenceCustodyFacts::custody` at `:3171-3177` (read). Table built by `validation::classify_type_references` at `validation/lib.rs:2072-2087`: one classified row per existing `TypeReferenceHandle` up to max arena index (read). Filled during check at `pipeline/05_typed-trees-to-checked-trees/src/checking.rs:1243-1264` (read). Consumers: `pipeline/06_checked-trees-to-lowered-psi/src/shared.rs:356-360`, `:1469-1473` (read).

**Why CLOSED.** For handles present in the typed program at fact construction, index is in range; fallback to row 0 applies only to out-of-range or invalid handles, not to normal checked programs unless a separate handle bug exists (not traced).

## What I did not read

- `omega-rust/psi/compilation/validation/src/contract_entailment.rs` (digest: 42 matches for crash-guard consumers).
- Every `has_builtin_operators` call site (42 total); only representative `checks_contracts*` paths.
- Provider admission: `require_selected_provider_declarations`, `close_provider_declarations`, review capture after `provider_declarations`.
- Full `append_shifted` / `shift_span` implementation and all cleanup disposition readers.
- Whether `TypeReferenceHandle` can be allocated after `content_classes` / `reference_custody` tables are sealed.

SLICE COMPLETE
