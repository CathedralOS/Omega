# unit_038 triage receipts (area 08, Psi program artifact codecs)

Status: Packet complete. All of unit_038's ranges in area 08 were reviewed.
Twelve claims have receipts, and two earlier dispositions are kept. Outcomes:
- One fixed: an equality inside an implication kept its operand order after
  contract substitution (C12).
- Three code-cleanup duplications left open (C11).
- Every other claim is intended, with the owning check named.
- The interpreter's entry-machine resolution under C4 was not traced.

Every receipt checks the current source at `24ad2700685e6cc6062026c87ed5fd24ed415265`
of `main`. The claims come from [record unit_038](../../records/unit_038.md) and
the area 08 digest [`digests/08-psi-program.md`](../../digests/08-psi-program.md),
both written at checkout `4f3c255d`. No unit_038 range belongs to a gap packet.
Paths below are relative to `source-rust/subsystems/compiler/`. Line numbers
for the digest and record are from those files at the baseline.

Outcomes use the packet vocabulary: fixed, intended, duplicate, feature gap,
reproduced, still open. Each "intended" receipt names the code that sets the
contract.

## Previously established dispositions (kept)

| Claim | Original evidence | Disposition kept from |
| --- | --- | --- |
| P1. A `Region` with `Atomic` observation encodes as tag 2 and fails to decode | Digest §10 item 3 (line 328), §9 item 4 (line 316); record §3 `access/views.rs` | [07-13 ledger](../07-13-psi-and-native.md), row "atomic repeated-region observation": excluded at access admission (`psi/access/validated-placements/plan_policy.rs`). |
| P2. `encode_service_ceiling` has no caller outside `reach.rs` | Digest line 272, §10 item 12 (line 337); record §1 `reach.rs` item 7 | 07-13 ledger, row "service ceiling writer": active encoding path. |

## Receipts

### C1. Closed reach decoder accepts any parameter kind and unordered rows
- **Original:** digest line 156; record §1 `reach.rs` item 9.
- **Current source:** `psi/artifact/callable-encoding/src/reach.rs` (`decode_kinds`, `decode`).
- **Contract:** the decoder reads bytes only. `decode_module` runs
  `validate_structural_foundation`, which calls
  `terminal_semantics::closed_reach::validate_closed_reach_applications`
  (`psi/artifact/wire/src/foundation_validation.rs:12070`). In
  `psi/semantics/src/calls/closed_reach.rs`:
  - `validate_telescope` requires `parameter_kinds.len() == telescope.len()`
    and each kind to equal its parameter's tag 0–3. A decoded kind of 4 or 5
    can never match, so the program is refused.
  - `row(&application.fixed)` refuses a service row that is not strictly
    ordered or has lost a parent service.
  - The dependency binders must be strictly ordered ("closed reach dependency
    binders are not strictly ordered").
- **Trace:** source read. No test pins these three refusals by name.
- **Outcome:** intended. The receiving check is the foundation validator.

### C2. A publication section with no evidence bytes decodes with `evidence = None`
- **Original:** digest lines 159 and 267; record §2 `canonical_artifact.rs` item 9.
- **Current source:** `psi/artifact/codec/src/canonical_artifact.rs`
  (`from_bytes`, `with_publication`).
- **Contract:** evidence is optional at every producer: `with_publication`
  takes `Option<Arc<EvidenceSeals>>`. Native lowering then fills accepted
  facts without proof fingerprints. The doc comment on
  `VerifiedPsiOptimizationInput::evidence_seals`
  (`psi-to-native/representations/abstract-operations/verified-optimization-input/src/lib.rs`)
  says "host authorization does not require local seals". When evidence is
  present, `from_bytes` now validates every seal against the decoded module
  with `EvidenceSealValidator::validate_all` before attaching it. The record
  did not mention this check.
- **Outcome:** intended.

### C3. `evidence_seals` stamps this module's identity on caller-supplied seals
- **Original:** digest line 160; record §2 `closed_module.rs` item 9.
- **Current source:** `psi/artifact/codec/src/closed_module.rs`
  (`ClosedTerminalModule::evidence_seals`).
- **Contract:** the function binds seals to a module and does not validate
  them. The doc comment says it "binds evidence to this exact closed module
  without encoding or closing it again". It has one production caller,
  `folded_entry.rs` in native stage 01. That caller first requires the prior
  evidence to name the prior module's identity. The seal contents are then
  checked against the module:
  - a saved artifact is checked in `CanonicalTerminalArtifact::from_bytes`
    (C2);
  - a fresh execution is checked in
    `PreparedNativeExecution::from_scoped_providers`
    (`psi-to-native/pipeline/01_admitted-native-inputs-to-target-bound-program/src/native_execution.rs`),
    which compares `evidence.module` with the view identity and runs
    `validate_all` "before any lowering filters the module".
- **Outcome:** intended. The identity comparison in `with_publication` guards
  seals decoded with their own module identity (`obligation_evidence.rs`,
  `decode_operation_evidence`). For stamped seals the validator is the
  check.

### C4. `rejoin_program_entry_receipt` compares only the semantic identity
- **Original:** digest line 161; record §2 `entry_receipt.rs` item 9.
- **Current source:** `psi/artifact/codec/src/entry_receipt.rs`; caller
  `source-to-psi/pipeline/07_checked-program-to-psi/src/custody.rs:156`.
- **Contract:** rejoining a saved Build companion ties the receipt to the
  artifact. The native consumer checks the entry itself.
  `psi-to-native/representations/target-bound-program/entry_settlement.rs`
  requires exactly one machine with the receipt's `terminal_entry`
  (`TerminalEntryMultiplicity`). It also requires that machine's source
  declaration to equal the receipt's declaration (`SourceMachineSubstitution`).
  The interpreter's `TerminalRuntime::prepare` (`interpreter/host/src/execution.rs`)
  checks the receipt's identity and provisions. I did not trace how it
  resolves the entry machine.
- **Outcome:** intended for the native route. The interpreter route was not
  traced.

### C5. `attach` skips the custody check when the artifact is `Err`
- **Original:** digest line 162; record §2 `entry_receipt/application.rs` item 9.
- **Current source:** `psi/artifact/codec/src/entry_receipt/application.rs` (`attach`,
  `ProgramEntryReceiptRequest::check`).
- **Test:** `canonical_artifact.rs`,
  `application_checks_view_identity_and_retains_portability_errors_on_the_receipt`.
- **Outcome:** intended. When the receipt is checked, it returns the
  portability error that was held back.

### C6. `PartialEq` for the artifact ignores the publication section
- **Original:** digest line 163; record §2 `canonical_artifact.rs` items 5 and 9.
- **Current source:** `canonical_artifact.rs` (`impl PartialEq`, `with_publication`).
- **Contract:** `with_publication` replaces the manifest with
  `manifest.with_publication(&bytes)`, and `eq` compares manifests. Two
  artifacts with different publication bytes therefore have different
  manifests.
- **Outcome:** intended.

### C7. The proposition decoder accepts a mutated float-meaning commitment
- **Original:** digest line 166; record §3 `terms/proposition.rs`; maintainer note 2.
- **Current source:** `psi/artifact/declaration-encoding/src/terms/proposition.rs`
  (`read_proposition_row`); `terms/order.rs` (`validate_canonical_proposition`).
- **Contract:** the table decoder checks syntax only. Meaning is checked in
  two places:
  - The module decoder runs `validate_canonical_proposition` on contract,
    invariant, refinement, requirement and crash-route propositions
    (`wire/src/semantic_module.rs`, for example `:227`, `:335`, `:508`,
    `:610`; `crash_routes.rs:50`). That check calls `FloatMeaningClause::validate`.
  - The proof consumer `psi/semantics/proofs/bounded/float_meaning.rs` calls
    `clause.validate()` before it normalizes an equation.
- **Test:** the unit test at the end of `proposition.rs` requires
  `validate()` to reject the mutated commitment.
- **Outcome:** intended.

### C8. Scalar term decoders for tags 34, 35, 36 and 39 skip `validate_after_operands`
- **Original:** digest line 167; record §3 `terms/scalar.rs`.
- **Current source:** `declaration-encoding/src/terms/scalar.rs`;
  `foundation/semantic-records/src/scalar_terms.rs`
  (`ScalarTerm::validate_after_operands`).
- **Contract:** for `BooleanField`, `IntegerField`, `ViewExtent` and
  `ByteSequencePredicate`, `validate_after_operands` returns `Ok(())` after
  one common check, which refuses a `Callable` scalar type. These decoders
  build only Boolean or integer types, so that check cannot fail.
- **Outcome:** intended. These leaves have no local rule to skip.

### C9. `validate_declarations` treats an `Err` calling custody as matching
- **Original:** digest line 170 and §10 item 5 (line 330); record §5
  `published_entry.rs`; maintainer note 5.
- **Current source:** `psi/artifact/publication/published_entry.rs`.
- **Contract:** the stored error is returned by `require_calling_custody` and
  `calling_schema_custody`. `CanonicalTerminalArtifact::calling_declaration_owner`
  then refuses any caller that needs calling custody.
- **Outcome:** intended (refusal deferred to the lookup).

### C10. Provenance labels skip a machine with no `unoptimized_behavior` entry
- **Original:** digest line 258; record §5 `machine_provenance.rs` item 7.
- **Current source:** `psi/artifact/publication/machine_provenance.rs`.
- **Outcome:** intended. Only diagnostic attribution is lost: the label falls
  back to "Terminal machine N", and no check or result depends on it.

### C11. Duplicated mechanisms reported in the codec crates
- **Original:** digest lines 94, 96, 97, 106; record §1 `signatures.rs` item 6, §2
  `canonical_artifact.rs` item 6, §3 `access/mode.rs`, `domains.rs`; "Unsure about"
  item 3.
- **Path encoders (line 97) and structural access tags:** not duplicates.
  `wire/src/structural_wire.rs` re-exports `psi_declaration_encoding::places::*`
  (`:326`) and `access::mode` (`:1028`). The wire crate has no second path or
  access-tag codec. Outcome: intended (one implementation).
- **`MachineParameterKind` / `ParameterKind` (line 106):** both still exist in
  `psi/artifact/callable-encoding/src/signatures.rs`. They share byte values
  and differ only in the error label. Outcome: still open, as code cleanup
  with no wrong result.
- **Identity formation (line 94):** `InvocationTerminalCode::from_prefix` and
  `ClosedTerminalModule::from_encoding` still build `TerminalPsiIdentity` the
  same way. Both build it from `encoded_fingerprint` over their own
  encodings. Outcome: still open, as code cleanup with no wrong result.
- **Reach-service and permission records (line 96):** still repeated in
  `declaration-encoding/src/domains.rs` and `access/views.rs`. Outcome:
  still open, as code cleanup with no wrong result.

### C12. `contract_forms::canonical_clause` leaves symmetric equalities unordered after substitution
- **Original:** digest line 195 and §3 line 95; record §6 `contract_forms.rs`.
- **Current source:** `psi/artifact/wire/src/contract_forms.rs` (`canonical_clause`).
  Stage 05 has a second copy at
  `source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/canonical.rs`.
- **Contract:** `declaration-encoding/src/terms/order.rs` refuses a symmetric
  equality whose left operand sorts after its right one, at any depth,
  including inside an `Implication`.
- **Trace of the claim as written:** the four forms the digest names cannot
  reach `canonical_clause` out of order. The shared substitution
  (`psi/semantics/src/substitution/clauses.rs`, `substitute_clause_with_computation_names`)
  swaps symmetric `ScalarIeeeFloatComparison` operands itself, and its unit
  test covers every comparison kind. It refuses `IeeeFloatComparison`,
  `ByteSequenceEqual` and `StructuralIdentity` ("clause form outside the
  scalar subset").
- **Defect found instead:** substitution does not reorder `Equal`; it relies
  on `canonical_clause`. The wire copy in `contract_forms.rs` did not recurse
  into `Implication`, while the stage 05 copy did. Source lowering produces
  implications for `!(a == b)` and Boolean `a != b`
  (`read/contracts.rs`). The wire copy restates crash-route guards for a call
  (`06_lowered-program-to-checked-program/src/minted.rs`,
  `substituted_routes`). Those guards currently come from numeric
  primitives, so I found no source program that reaches this today.
- **Reproduction:** `contract_forms::tests::substitution_restores_equality_order_inside_an_implication`
  restates `x == y -> x == y` over actuals 9 and 3. On the parent it fails
  with `NonCanonicalOrder("equality operands")` from
  `validate_canonical_proposition`.
- **Repair:** `contract_forms::canonical_clause` is now public and handles
  `Implication`. Stage 05's `canonical::canonical_clause` delegates to it, so
  one implementation serves both producers.
- **Outcome:** reproduced and fixed. The four forms the digest names are
  intended (see trace).
- The `routes()` widening to `[Truth]` that the same digest line reports is
  intended, as `crash_routes_are_canonical` requires
  (`declaration-encoding/src/crash_routes.rs:36`).

## Validation

The commands ran on branch `claude/omega-unit-038-triage-gcn9h3`, on top of
`24ad2700685`, in a Linux x86_64 cloud container with no warm build.

| Command | Result |
| --- | --- |
| `mbx test -p terminal-wire --lib contract_forms` with the test and no fix (`20f2a05299a`) | 1 failed: `NonCanonicalOrder("equality operands")` |
| `mbx test -p terminal-wire --lib contract_forms` with the fix | 1 passed |
| `mbx test -p prepared-typed-trees-to-lowered-program --lib canonical` | 4 passed |
| `mbx clippy -p terminal-wire -p prepared-typed-trees-to-lowered-program --all-targets -- -D warnings` | clean |
| `mbx xtask fmt --check` | exit 1. It reports 3,214 differences in files this change does not touch. `rustfmt --check` reports none in the two changed Rust files. |

No end-to-end program was compiled. I found no source program whose
crash-route guard contains an implication, and the packet rules excluded a
global compiler rebuild.
