# PRODUCER-CHECKER-BOUNDARY-AUDIT — bounded audit at `96b4afed92`

Boundary question: does any checker consume a producer's verdict — a flag,
verdict enum, or pre-verified object — instead of re-deriving? Audited the
artifact boundary, the verified-authority carriers, the evidence provenance
gate, and the four decision-adjacent reuse mechanisms. **No boundary trust
found.** linux x86-64.

## Artifact boundary — clean

`terminal-interpreter/src/terminal_interpreter/execution.rs::start_artifact`
is the only execution entry: decodes the canonical module, decodes the proof
section *sealed to that module's reconstructed identity*
(`decode_proof_section_for`), then runs
`terminal_verifier::verify_module_for_interpretation` before binding any
operand. Producer bytes never enter execution as trusted objects.

## Verified-authority carriers — unforgeable

`terminal-verifier/src/verification.rs` mints four distinct wrappers
(`VerifiedTerminalModule`, `VerifiedInterpretableTerminalModule`,
`VerifiedOptimizableTerminalModule`,
`VerifiedFixedFuelTerminalModule`), each holding a private
`VerifiedTerminalModuleState` — no public constructor, so no producer can
forge check authority. Consumers demand the matching carrier:
`terminal-fixed-fuel/src/fuel_certification.rs` requires
`VerifiedTerminalModule`, and `terminal-psi-to-abstract-operations`
lowering/artifact_admission require `Verified{,Optimizable}TerminalModule`.
`verify_module_for_optimization` documents it grants no execution,
interpretation, fixed-fuel, native-lowering, or publication authority.

## Checker re-derivation — clean

`verify_validated_module` reconstructs structural ownership frontiers,
terminal obligations, and proof recursive components *from the module*
(`reconstruct_validated_*`), then validates evidence producer provenance
(`verification/evidence_provenance.rs`) — Requires/Ensures contract lanes are
re-derived from `module.evidence_contract_lanes` and `proof_output_calls`,
duplicate evidence obligations reject (`DuplicateEvidence`). The producer's
`ProofBundle` is evidence to be checked, never a verdict.

## Verdict-flag sweep — clean

No `.verified`/`.admitted`/`.was_checked` verdict field is read anywhere in
`terminal-verifier`, `terminal-semantics`, or `proof/src/checker`.

## Previously-audited mechanisms (DECISION-SHARING-AUDIT, `8734480a01`)

`proof/src/checker/derivation_cache.rs` (re-runs `candidate.verify()` through
the admission kernel), `component-description::verify` (re-derives subject/
schema/entries/custody/assumptions from bytes),
`build-evaluation::verify_independent_component_descriptions` (re-verifies
under the build's own admission profile), PCC admission row replay — all
independently checked, confirmed still current by inspection.

## Trust ledger — self-accounting

`terminal-verifier/src/trusted_surface.rs` inventories every `Proved` /
`ExplicitlyTrusted` / `Unfinished` row: a `Proved` row outside `PROVED_ENTRIES`
fails, an `ExplicitlyTrusted` row must name a registered accepting root whose
policy covers the family, an `Unfinished` row claims nothing, and a duplicate
or claim-bearing dependency edge fails. What remains trusted is enumerated,
not implicit.

## Residual (owned elsewhere)

The named residual is not a boundary violation but the
PCC-CANONICAL-SEMANTIC-LEDGER item's legs: the verifier still runs bounded
proof *search* itself (e.g. the 4096-step search in
`validation/crash/entry_requirements.rs`), and `ExplicitlyTrusted` →
`Proved` conversions plus the total canonical-byte generator are that item's
multi-session work. Sibling stubs on this surface:
PRODUCER-CHECKER-DECISION-SEPARATION, PRODUCER-CHECKER-SHARING-AUDIT,
PRODUCER-HISTORY-CUSTODY.
