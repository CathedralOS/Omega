# Call-containing scalar requirement transport — design

Planner dispatch `NEW-OMS-CALL-CONTAINING-REQUIREMENT-TRANSPORT-DESIGN`;
owning leg is the OPERATOR-MACHINE-SUPPLY bullet "Finish Terminal transport
of call-containing scalar requirements" (TASKS.md, re-measured `94e764a6da`).
Audited at `6f918986063` on linux x86-64; re-verified at `661a4d50c0af`
(zergling-168) and again at `2dbfecd98e49` (z12) — every anchor still live:
`lower_closed_clause` + `lower_scalar_contract_predicate` fallback at
`contract_plan_facts.rs:387,452`, the `unsupported clause` rejection at
`scalar_contracts.rs:57`, the five `ClosedScalarContractValue` variants at
`contract_plans/scalar_contracts.rs:40` (enumerated in `covered_requires` at
`scalar_graph/scalar_contracts.rs:44-93`), and the reproducer at
`contract_application_terms.rs:172`. Delete this design once cited-call
transport lands and the reproducer's artifact passes independent terminal
verification.

## The reproducer and where it breaks

`compiler/tests/contract_application_terms.rs::runtime_body_calls_execute_with_checked_premises`:

```omega
pub machine observe(value: bool) -> bool
terminates;
{ transition { _ -> (value == true) } }

pub machine restricted(left: bool, right: bool) -> bool
requires observe(left) == observe(right);
terminates;
{ left }

machine caller(value: bool) -> bool { let saved: bool = value; restricted(saved, value) }
```

Source checking and the checked interpreter already execute this correctly:
both `observe` calls run under checked premises and `run_false`/`run_true`
return 7/11 through the transition contract. The breakage is strictly in
Terminal transport:

- `04_typed-trees-to-checked-trees/src/facts/contract_plan_facts.rs::lower_clause`
  tries `lower_closed_clause`, then `lower_scalar_contract_predicate`. Both
  produce `Option` shapes over `CheckedBooleanExpression`; a predicate whose
  operands are machine calls has no closed form, so the requires row records
  `None` in `ClosedScalarValueContractPlan::requires
  (Vec<Option<ClosedScalarContractValue>>)`.
- `checked-trees-to-lowered-psi/src/scalar_graph/scalar_contracts.rs::
  covered_requires` rejects any `None` row with `scalar contract contains an
  unsupported clause`. `produce_artifact()` on `caller` dies there.

`ClosedScalarContractValue` has five retained variants (`Boolean`, `Integer`,
`Predicate`, `FloatRange`, `FloatMeaningEquality`) — none can cite a call.
`semantic-vocabulary`'s `ScalarTerm` is similarly closed (values, field paths,
Boolean/Integer connectives); there is no call operand and adding one would
make contract verification depend on arbitrary callee execution.

## Design

Transport the clause as a **cited requirement**, not an evaluable
proposition. The call inside the requirement is already a checked object —
the callee's own `requires`/`terminates` obligations were discharged at the
caller's call site. Terminal needs to carry exactly that citation so the
verifier replays the *obligation join*, not the call.

### 1. Checked retention (04_typed-trees-to-checked-trees)

- Keep `lower_scalar_contract_predicate` unchanged for the call-free subset.
- When the clause's predicate contains calls, retain a new
  `ClosedScalarContractValue::CallCited { expression, calls }` variant:
  `expression` is the authored contract node verbatim (same discipline as
  `FloatMeaningEquality`'s `expression` field), and `calls` is the ordered
  roster of the clause's checked call coordinates — each row carrying the
  callee identity, the actual operand coordinates, and the callee's own
  contract row references (the `ContractProofFactRef`/`FlowConstraintRef`
  bindings the checked side already resolves in
  `checks/contracts/call_bounds` and the `admissibility/call.rs` requires
  rosters).
- Admission rule at retention time: every cited callee must carry
  `terminates` (or an equivalent checked termination row). A non-terminating
  callee inside a contract clause would make premise evaluation unbounded —
  that clause is rejected at check time, never deferred to lowering.
- `requires` keeps positional correspondence row-for-row: the clause stays a
  `Some`, so `covered_requires`'s `None` rejection continues to catch a
  genuinely dropped predicate.

### 2. Lowering (checked-trees-to-lowered-psi)

- `covered_requires` admits `CallCited` rows alongside `Predicate` rows;
  `clauses()` converts the *call-free spine* of the predicate to the ordinary
  `Proposition` and emits each cited call into a new terminal roster
  (paragraph below), bound to the proposition by clause position. A
  call-containing clause must never silently weaken to `Truth` — the
  Boolean/Integer reflexivity shortcut stays restricted to its current
  same-literal operands.
- The emitted terminal form is a pair: the canonical proposition (whose
  operands replace each call subterm with the introduced result-witness
  value declaration — the same dense-`value_id` machinery erased proof
  formals already use in `allocate_dense`) plus a `CallCitedRequirement`
  roster recording callee identity + actuals + callee contract coordinates.

### 3. Codec + vocabulary (terminal-psi, terminal-codec)

- `ScalarTerm` gains no call variant. The result-witness declaration is an
  ordinary terminal `Value` term; the call binding lives in the sidecar
  roster so proposition evaluation stays pure.
- The roster lands as a new canonical section row (same pattern as the
  floating-range roster `float_entry_ranges` and the float-meaning equality
  rejoin): canonical order derived by the codec's ordering key, exact-count
  correspondence with the clause rows, and refusal of unauthored rows —
  mirroring `covered_requires`'s existing roster discipline so a dropped
  citation cannot hide behind the proposition.

### 4. Verifier replay (terminal-verifier)

- The verifier independently re-establishes each cited call's premise join:
  callee exists at the cited identity, its contract rows are the ones the
  citation names, and the actual operand witnesses satisfy the callee's
  `requires` under the caller's caller-visible obligations — the same join
  `runtime_body_calls_*` performs in checked execution, replayed from the
  terminal rows rather than from source.
- `terminates` evidence is replayed, not trusted: the cited callee's
  termination row must be present in the artifact.
- A `CallCited` clause whose roster cites a callee the artifact does not
  contain, whose actuals don't satisfy the callee premises, or whose
  proposition mentions a witness with no binding roster entry, all reject.

### 5. Boundaries this design must not cross

- No call execution inside proposition evaluation — calls stay cited, never
  evaluated by the verifier's predicate walk.
- No weakening: the callee requirement discharge uses the exact same
  `requires` set source checking used (no premise relaxation because the
  call sits inside a contract clause).
- `FloatRange`/`FloatMeaningEquality` tail positions unchanged; the roster
  correspondence machinery is the model to copy, not to merge with.
- Per the row: runtime transport beyond builtin Boolean/integer equality
  still needs citation and induction evidence — cited calls are the citation
  mechanism; induction obligations stay in their own rows. Nested operands
  need actual premise checking in the verifier replay, not a lowering
  refusal counted as coverage.

## Acceptance carried from the row

`TerminalProductionRequest::new(&checked, "caller").produce_artifact()`
succeeds; the produced artifact reloads canonically, passes independent
terminal verification, interprets `run_false`/`run_true` to 7/11, and
executes natively — all without dropping `observe(left) == observe(right)`.
Negative controls: a clause citing a non-`terminates` callee rejects at
check; an artifact with a forged or missing roster row rejects at verify;
`opaque_receiver_call_cannot_hide_a_changed_requirement_argument`'s
wrong-argument rejection is retained beside its accepted exact-argument twin.

## Fences to re-check before implementing (refreshed at `2dbfecd98e49`)

Live claims overlapping the producing surfaces, refreshed at `2dbfecd98e49`:
PROOF-CERTIFICATION-BRIDGE (t2c `checks/contracts/exits`, c2l
`proofs/scalar_block_invariants`, exp ~10:52Z), PROOF-SUBJECT-CHECKED-CALL-
ATTRIBUTION (`proof_contracts/contract_entailment/{specification_calls,
refuted_requires,call_requirements}.rs` + signature_call test dirs, exp
~16:19Z), REGISTERED-CALLBACK-LIFETIME (terminal-verifier `provider_result.rs`
+ `tests/calls/provider_results.rs` ~14:37Z, terminal-interpreter src
~14:09Z), RC-GATE-STABILITY-REPAIR (c2l
`tests/nominal_affine_source/integer_comparison.rs`, ~12:15Z), and
NEW-BSR-CONDITIONAL-ARM-AGREEMENT-PRODUCER (t2c
`execution/terminal_unit/borrowed_windows.rs`, ~16:41Z).

Drained since `661a4d50c0af`: STRUCTURAL-UNIT-LOWERING (c2l `src/unit` is
free), CUSTODY-MATRIX-HARNESS-MIGRATION (terminal-codec artifact tests),
ARITHMETIC-POLICY-REALIZATION (c2l `expression_preparation`),
NEW-CC-BOUNDARY-CRASH-SOURCE-TO-EXECUTION-CONTROLS (c2l
`tests/scalar_boundary_arguments.rs`), and the t2c cluster —
PROVIDER-ATTACHMENT-MACHINE-PLAN (`execution/terminal_unit/providers.rs`), BLOCKEXEC
(`checks/multiplicity`, lease expired), CALL-REQUIRES-INDEXED-WRITE-FACTS
(`flow/transfers.rs`), FLOW-LITERAL-THRESHOLD-MEMOIZATION (`flow/context.rs`),
NEW-BPC-SLICE-START-BOUND-CANDIDATE (`checks/ranges/indexes/validation.rs`).
DYNAMIC-UNIT-CALL-GRAPH-AND-REACH-EDGES remains drained.

New adjacent fences not on this design's producing surfaces: LIFETIME-SOURCE-
CORRESPONDENCE (t2c `checks/borrows` + `borrow/{view_link,loans}.rs`, ~18:00Z),
NEW-PCM-MACHINE-VALUED-BODY-DENOTATION (t2c `proof/mathematical_declarations`,
~18:17Z), REPRESENTATION-SPECIALIZATION (optimization representations +
l2l `psi_optimization.rs`, ~17:59Z), NEW-C2L-SUITE-ERASED-PROOF-FORMALS-
COMPILE-FIX (c2l `tests/registered_callback_lifetime.rs`, ~13:53Z).
The doc path itself remains unfenced; implementers should claim the
specific surfaces they touch, not this file.
