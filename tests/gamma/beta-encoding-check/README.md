# Beta-encoding certificate check

Run `sh tests/gamma/beta-encoding-check/run.sh` from the repository root, on
macOS arm64, Linux x86-64, or Windows x64 Git Bash; other hosts report
unsupported. An absent Python skips rather than fails.

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
`requested=135485028`). The running checker still carries the 8,388,608-byte
request bound; the selected profile's `request_bytes` provision of
136,314,880 ([REQUEST.md](../../../bootstrap/proofs/checker/REQUEST.md))
reaches the checker source in the fenced `bootstrap/proofs/checker` /
`tests/gamma/derivation-*` leg, so the complete check cannot yet observe a
`Checked` verdict on this host.
