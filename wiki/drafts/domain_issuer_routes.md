# DOMAIN-ISSUER-ROUTES — record

Re-verified at `94e764a6da` on linux x86-64 (cargo; `mbx` absent on this
host). The row asks for independent Terminal qualification evidence for
requirement/exact-machine/private-issuer routes. State at tip:

## Minting leg — landed and green

`QualificationEvidenceOrigin::AuthorizedRouteEstablishment` exists in
`language-semantics/semantic_domains/mod.rs:199` and is minted in
`typed-trees-to-checked-trees/src/facts/qualification_evidence.rs:218,223`
for both requirement-spelled and boundary-routed results. Lane
re-verified: `nextest -p typed-trees-to-checked-trees
-E 'test(~qualification_evidence)'` — 17/17 PASS, including
`exact_machine_route_authorizes_its_own_invocation`,
`exact_machine_route_does_not_authorize_other_machines`,
`checked_conformance_authority_is_consumed_from_the_normalized_route_record`,
`boundary_result_authorization_retains_requirement_identity`, and
`exclusive_boundary_receiver_establishes_its_exact_routed_result`.

## Artifact roundtrip — unimplemented at tip

Zero `QualificationEvidence`/`AuthorizedRouteEstablishment` consumers
outside the minting crate: no references in `checked-trees-to-lowered-psi`,
`lowered-psi-to-terminal-psi`, `terminal-codec`, `terminal-semantics`, or
`terminal-verifier` (grep over all crate srcs). The acceptance leg — a
source-free artifact that replays the established qualification against
the exact authorized invocation result and rejects forged/substituted
routes — has no encoding site, no codec section, and no verifier replay.

## Fences over the implementing surfaces

- `checked-trees-to-lowered-psi/src/unit`: STRUCTURAL-UNIT-LOWERING (09:16Z)
- `checked-trees-to-lowered-psi/src/scalar_graph/scalar_contracts.rs`:
  RC-REPOSITORY (14:39Z)
- `checked-trees-to-lowered-psi/src/proofs/scalar_block_invariants`:
  PROOF-CERTIFICATION-BRIDGE (10:52Z)
- `lowered-psi-to-terminal-psi/src/boundary_operator_custody`:
  FILESYSTEM-RELEASE-CONTRACT (14:20Z)
- `terminal-verifier/src/validation/frontier`:
  OWNED-SUCCESSOR-DISCARD-ORDER (15:07Z)
- `terminal-verifier/{validation/foundation/provider_result.rs,
  tests/calls/provider_results.rs}`: REGISTERED-CALLBACK-LIFETIME (14:37Z)
- `terminal-interpreter/src`: REGISTERED-CALLBACK-LIFETIME (14:09Z)
- `terminal-codec/tests/artifact/optimization_execution_custody*`:
  CUSTODY-MATRIX-HARNESS-MIGRATION (09:19Z)

The remaining chain is indivisible for a bounded slice: a codec section
without verifier replay is a dead field; verifier replay without the
encoding has nothing to check; and the encoding has to ride whichever
artifact section owns qualification state (a terminal-semantics/codec
boundary decision, not a local edit). The package-evidence source-side
contract (`nextest -p package-evidence --test suite -E 'test(public_domains)
| test(module_namespaces)'`) is unaffected by the gap — it pins
package-row recovery, which the row notes does not close artifact
acceptance.
