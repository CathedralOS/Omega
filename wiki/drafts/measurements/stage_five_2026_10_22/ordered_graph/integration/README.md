# Integration for authorized main landing

2026-10-22. Rebuilt and rechecked after integrating main
`c00fe7c23ba7be4dff1eb0bc724d2a2a78214418`. Observed candidate:
`23e22919620625b940901d3301a374c71d1dc720`. The subsequent commit adds this
evidence only. [Manifest](manifest.json) records both compiler trees, the library
tree and artifact hashes. All six implementation files still match the original
experiment's source hashes; the only rebase conflict was the shared experiment
ledger, resolved by preserving both entries.

Incoming main changed source representation, proof/checking and native consumers,
then pure Build-result reuse and allocator behavior. Original observations were
not relabelled. Fresh optimized CLI builds and corpus runners exercised the
integrated consumer closure. Both CLI producers used the same explicit
`OMEGA_LIBRARY_ROOT` and unchanged sample sources. No own build overlapped timing.

## Fresh observations

Both repetition runs met the ten-second stable-minimum stopping rule. The host
was heavily shared: one-minute load averages were 21.76–23.80 for length and
19.95–21.76 for subtraction. Every successful observation is retained. These
wide distributions are **inconclusive for a precise integrated speedup**; they
are not pooled with the earlier, lower-variance 5.35% / 2.59% Stage 05 results.
The candidate has lower medians but does not consistently improve minima.

| Workload / metric | Baseline min / median / mean / max, ms | Candidate min / median / mean / max, ms |
| --- | --- | --- |
| length Stage 05, 16 pairs | 645.072 / 1114.690 / 1090.458 / 1956.408 | 641.279 / 841.048 / 961.990 / 1543.591 |
| length total | 782.015 / 1306.059 / 1301.935 / 2199.486 | 792.969 / 1051.533 / 1161.315 / 1889.961 |
| dependent subtraction Stage 05, 12 pairs | 724.183 / 824.335 / 858.436 / 1044.285 | 741.291 / 785.933 / 812.123 / 1004.167 |
| dependent subtraction total | 922.276 / 1028.676 / 1065.944 / 1273.702 | 911.318 / 981.127 / 1023.163 / 1308.736 |

[Length raw records](length-timings.json) and [subtraction raw records](dependent_subtract-timings.json)
retain output, load averages, profiles and binary identities. Baseline executable
SHA-256 is `0b74983d8822d09299bcd08f2512fca1151323f76ee349f005d4f20e9531cccd`;
candidate is `261598c276c48c3109b420691deb6a28ae636d7c8ea45d339f019271e54337a5`.
Both report opt-level 1, on the same M4/macOS arm64 host and pinned toolchain.
The timing runner's `base` field is the checkout HEAD (candidate); the manifest
above supplies the separate baseline source revision. The empty uncommitted
patch hash reflects measurements of committed source, not identical producers.

## Integrated correctness

- [Eight validation tests passed](tests-refresh.log), including the independent
  symbolic-walk oracle, occurrence/generation distinctions and environment scope.
- [49 checked corpus comparisons](checked-comparison.json): zero losses or gains.
  [Candidate](checked-records-0.txt) and [baseline](checked-records-1.txt) retain
  raw diagnostics: 21 positives checked and all 28 controls rejected with their
  expected diagnostic fragments.
- [21 native/Terminal comparisons](both-comparison.json): zero losses or gains.
  [Candidate](both-records-0.txt) and [baseline](both-records-1.txt) retain 20 actual
  exit-0 agreements and one check-only proof fixture.
- [Public CLI](public-length.json): nine length programs produced native exit 0
  and interpreter exit 0; nine invalid twins rejected with their authored
  diagnostic. Source/build hashes and actual command outputs are retained.
- Scoped [Clippy](clippy-refresh.log) completed with existing warnings.
  [Baseline](baseline-refresh-build.log) and [candidate](candidate-refresh-build.log)
  optimized incremental builds succeeded. Log trailing whitespace was trimmed.

Replay the [original commands](../README.md#identity-and-replay), replacing the
comparison base with `c00fe7c23ba7be4dff1eb0bc724d2a2a78214418` and building the
candidate revision above. [Run log](validation.log) records the complete sequence.
The normal landing receipt separately checks the exact publication commit,
intermediate commits and preservation of golden corpus rows. No foreign-host,
release-profile, whole-corpus or new precise performance claim is made here.

Retain the unchanged implementation on its original bounded performance evidence
and these fresh integrated correctness results. The shared-host batch does not
justify a larger speedup claim or an unrelated optimization campaign before
publication.

## Subsequent sample-only main movement

Main `4d917f5ebf6d5d2640395e0409a77b74a47b5dfe` added the unrelated
double-indexed read-modify-write sample/fixture row and its golden records.
The first reservation was released to inspect that delta. Git tree equality
establishes unchanged `omega-rust`, `source/library`, and all 49 selected fixture
directories against the measured main base. The length/subtraction sample inputs
and every selected `corpus_outcomes.txt` record are unchanged too. Consequently
the recorded producers, timings, diagnostics and runtime observations retain
their original applicability; no observation is relabelled as a fresh execution.
The implementation file hashes remain identical after rebase. Publication still
requires the exact-candidate build receipt and full golden preservation guard.
