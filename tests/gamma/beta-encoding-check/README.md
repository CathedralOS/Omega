# Beta-encoding certificate check

Run `sh tests/gamma/beta-encoding-check/run.sh` from the repository root, on
macOS arm64, Linux x86-64, or Windows x64 Git Bash; other hosts report
unsupported. An absent Python skips rather than fails.

`sh tests/gamma/beta-encoding-check/run.sh --reference-vm` is the diagnostic
leg on any host with `cc` and python3: it compiles the gate-local Alpha VM
[alpha_vm.c](alpha_vm.c), drives the pinned evaluator tape through it instead
of a stamped seed, and otherwise runs the identical packing, emission,
production, and check. The reference leg's observation is a recorded reading,
never artifact admission — the audited seeds remain the admission route, and
a divergent reference result is a gate failure to investigate, not a verdict
to accept.

The gate checks the complete certificate for the owner proposition

```text
encode_Beta(S, 0x4000000, 0xfffffc) = Success(T)
```

where `S` is the selected Gamma evaluator's raw Beta source and `T` its
persisted Alpha tape, following the
[complete encoding acceptance](../../../bootstrap/proofs/beta_encoding/ACCEPTANCE.md)
and the [checking profile](../../../bootstrap/proofs/checker/CHECKING.md).

[run.sh](run.sh) packs the bound emitter and checker closures (the same
audited materializations the theory and checking gates use) and materializes
the selected evaluator. [check.py](check.py) then:

1. emits the theory through the evaluator itself and verifies the emitted
   package against the fixed wire identity (shared
   [prepare](../beta-encoding-theory/gate.py) step);
2. reproduces the complete 135 MiB request with the host-side
   [stepper](../beta-encoding-theory/stepper.py) production, requiring the
   emitted bytes to match the recorded request SHA-256 before the run starts;
3. frames the request for the evaluator and requires the checker's exact
   17-byte `Checked` observation: proof count equal to the produced rows and
   cumulative work inside the selected 2^26-unit provision, with wall time
   printed for the profile record.

A refusal or malformed observation is decoded and reported; a nonzero
evaluator status, a host timeout, or a partial observation is a failed gate,
not a checker verdict. This gate observes the checker's verdict on one
independently produced certificate — artifact admission, the full-subject
mutations, and the retained-rule audit remain separate legs of
GAMMA-DERIVATION-CHECKER.

First native run (Linux x86-64, ab8ac294f3): the host-side production
reproduced the recorded request byte-exactly — 135,485,028 bytes,
`sha256=7c0e3bf230a2675a170ea77dc6962ef6aa7c03ce15248b27475c7c6a3e592908`,
3,182,974 proof rows in 37.3 s — and the checker returned
`Incomplete`/`request_bytes` (`coordinate=limit=8388608`,
`requested=135485028`). The running checker still carried the 8,388,608-byte
request bound; the selected profile's `request_bytes` provision of
136,314,880 ([REQUEST.md](../../../bootstrap/proofs/checker/REQUEST.md))
then reached the checker source at `a31bdf79e5`.

Second native run (Linux x86-64, 5246ff65f4, ~2026-09-21): the request
reproduced byte-exactly again (31.7 s production), the reframed admission
accepted the 135,485,028-byte request under the landed extent, and the
checker consumed roughly two hours of evaluation before refusing
`Incomplete` code 4 — `comparison_exhausted` — at `coordinate=57759240`
with `limit=655360`, `requested=3182975`: the comparison-session bound
(`comparison_limit` in
[session.gamma](../../../bootstrap/proofs/checker/implementation/comparison/session.gamma))
does not cover the real certificate, whose fresh proof-index reservation
alone demands one unit per row over 3,182,974 rows. The selected ledger
places all post-Grounded work — proof-index setup, row checks, congruence
premises, and every comparison and substitution transition — on the shared
67,108,864-unit counter
([CHECKING.md](../../../bootstrap/proofs/checker/CHECKING.md)), so the
session bound is a stale sub-budget to raise toward that provision rather
than a separate allowance. Exhaustion legs sized to 655,360 cannot be
regenerated at a 2^26-unit bound — an exhausting table would exceed the
request extent — so the derivation-checking boundary legs
(`exact_complete_checking_work`, `adjacent_final_root_comparison`,
`proof_index_and_rows_share_work`, `fresh_proof_index_reservation` in
[resources.py](../derivation-checking/resources.py)) instead migrated to
checked completions under the raised counter, and any future exhaustion
leg retires under the same opt-in pattern the heap/pair-boundary veterans
use. The re-pin of checker identities across
`tools/bootstrap/proofs` and the seven derivation gates follows the
`a31bdf79e5` shape and stays fenced to its claim lanes until it lands.
