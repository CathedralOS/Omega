# Scope: PROOF-VALUE-SOURCE-CORRESPONDENCE

Scope-verification record for the dispatched name
`PROOF-VALUE-SOURCE-CORRESPONDENCE` (board stub at ~TASKS.md:12788, mined
unattributed in the wave-9 deep-mine batch `749794ddeb64`). Audited at
`8f58b6676b00` on linux x86-64 (cargo; `mbx` absent on this host).
Re-audited at `d781031d40e` (~13:35Z Sep 21, linux x86-64): the name has
been re-mined into a small stub cluster — duplicate undotted
`PROOF-VALUE-SOURCE-CORRESPONDENCE` stubs at ~TASKS.md:15392, ~15433 and
later rows, all still `verify scope then implement`. The resolution and
verdict below are unchanged; the stubs should fold into the owning rows
(or be retired) rather than dispatch a new item.

## Resolution

The name maps to the value↔source-correspondence custody leg — proof and
lowering surfaces that bind a produced/lowered value to its authored source
expression. Two landed surfaces own it:

- `checked-trees-to-lowered-psi/src/expression_preparation/source_custody/
  value_correspondence/` — validates that each call scalar operand
  corresponds to its authored source expression: pure arguments must carry
  the declared primitive type and match the source tree structurally;
  computation roots replay the checked `scalar_computations` plan node by
  node (casts checked by domain policy, `IntegerExactCast` range payloads
  discarded at the boundary — the terminal side emits a fresh exact-cast
  obligation rather than trusting checked range annotations). `extents/`
  owns the eliminated-subslice custody: an eliminated extent needs
  statically established formation bounds and an effect-free source.
- `validation/src/value_custody/owned_value_source.rs` — whole owned
  value→source resolution (`plain_owned_value_source` /
  `affine_owned_value_source` / `linear_owned_value_source`): resolves the
  named storage a plain/affine/linear whole-value transfer names; claim
  accounting and flow availability stay with the caller.

Adjacent proof-side correspondence legs are landed and owned elsewhere:
`checked-trees-to-lowered-psi/src/proofs/quotient_correspondence.rs`
(published quotient correspondence retained across the checked→Terminal
boundary per `wiki/spec/proofs/quotients.md#published-quotient-correspondence`)
and `typed-trees-to-checked-trees/src/checks/termination/progress/
qualification_correspondences.rs`.

## Landed state

The correspondence check is wired into `expression_preparation` call custody
(`source_custody/mod.rs` routes computation-call scalar arguments into
`value_correspondence::validate`) and rejects mismatches as
`LoweringError::Unsupported("scalar operand value differs from its authored
expression")`. Witnessed green at `8f58b6676b00` (linux x86-64):

```
cargo nextest run -p checked-trees-to-lowered-psi value_correspondence
→ 1/1 pass: extents::tests::eliminated_extent_preserves_collection_evaluation_bounds_and_selection
```

Re-witnessed green at `d781031d40e` (code identical to main `4b8d3f36b725`
outside TASKS.md): same command, 1/1 pass.

## Open legs

- **General slice-backed extents**: need a retained view/bounds execution
  plan on the operand; array operands do not yet carry one (module doc).
  This is upstream-gated on retained-view machinery — matching literal
  endpoints alone can never supply that evidence — and sits inside
  `source_custody`, whose family rows (TERMINAL-SOURCE-CUSTODY-ORDER —
  resolved; C2L-SCALAR-RETURN-SOURCE-CUSTODY-FAILURES) own the surface.

## Slice verdict

No independent slice exists under this name. The correspondence substrate
is landed, exercised, and witnessed green at the audit revision; the only
recorded residual is upstream-gated on retained view/bounds execution plans
owned by the source-custody family rows. The stub should fold into the
owning rows (or be retired) rather than dispatch a new item.

## Fences observed at audit time

Live claims adjacent to — but not covering — this surface:

- `checked-trees-to-lowered-psi/src/unit`: STRUCTURAL-UNIT-LOWERING (09:16Z)
- `checked-trees-to-lowered-psi/src/proofs/scalar_block_invariants` +
  `typed-trees-to-checked-trees/src/checks/contracts/exits`:
  PROOF-CERTIFICATION-BRIDGE (10:52Z)
- `checked-trees-to-lowered-psi/src/scalar_graph/scalar_contracts.rs`:
  RC-REPOSITORY (14:39Z)

`value_correspondence/` and `owned_value_source.rs` themselves are unfenced;
re-check `tools/claims.py status` before any code leg.

Fence map at the `d781031d40e` re-audit: the c2l surfaces named above now
also include `tests/nominal_affine_source/integer_comparison.rs` under
RC-GATE-STABILITY-REPAIR (12:15Z), `tests/registered_callback_lifetime.rs`
under NEW-C2L-SUITE-ERASED-PROOF-FORMALS-COMPILE-FIX (13:53Z), the
`proofs/crash_routes/scalar_terms*` guarded-float term vocabulary under
NEW-CC-IEEE-COMPARISON-GUARD-SCALAR-TERMS (18:29Z), and the
entry-requirement crash certificate surfaces under
NEW-PCSL-ENTRY-REQUIREMENT-CERTIFICATE (18:32Z). `value_correspondence/`,
`owned_value_source.rs` and `expression_preparation/source_custody/`
remain unfenced.
