# Gamma seed native acceptance

Accepts the selected Gamma evaluator's seeded native containers as native
artifacts. `tools/bootstrap/gamma/evaluator_env.sh` binds the evaluator's
source/tape identities and stamps the tape into the audited Alpha seed;
`tests/alpha/container.sh` proves the seeds' stamping contract with a probe
tape. This gate proves that contract for the evaluator tape the Gamma chain
actually ships — the artifact every seed-executing Gamma gate runs — in every
audited seed:

- the host-selected seed is stamped through the real materializer (which
  re-signs on macOS), and each non-host seed is stamped at its own recorded
  hole offset;
- each stamped container must satisfy the native format, entry, loader, and
  `[length][tape][zeros]` hole contract enforced by
  `tests/alpha/container.py`, with byte-exact content outside the hole where
  no re-sign ran;
- on seed-execution hosts (macOS arm64, Linux x86-64, Windows x64), the
  materialized container then executes a minimal Gamma source natively and
  publishes its exact receipt byte for byte.

The host-free checks run wherever Python 3 does; the execution leg reports an
explicit skip elsewhere. Deleting this gate requires another gate to cover
the real evaluator tape inside every audited container — the probe tape does
not bind the shipped artifact's extent.
