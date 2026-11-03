# Check a call before using a constant result

A constant helper could use its unfinished result to justify a call made while
calculating that result. The compiler now rejects that circular proof while
preserving calls whose preconditions can be established independently.

For example, a helper calculating a bound of seven may first call a function
that requires its input to fit a separately calculated bound. That works. If
the requirement instead depends on the helper's own unfinished bound, checking
rejects it. An earlier call's completed result can still justify the next
arithmetic operation.

Each deferred check now records whether it is a prerequisite or a check of a
completed result. The evaluator and interpreter enforce that distinction, and
serialized programs carry it explicitly. Older artifacts without it reject.

Validation also exposed a valid recursive library calculation that constant
evaluation rejected after losing its saved storage information. The evaluator
now uses the ordinary checker's storage and termination facts for the selected
calls. Every member of a recursive cycle still needs a valid termination proof.
The library and its caller retain their original programs and exit oracle.

## Results

All runtime results below were observed on macOS ARM64. Bindings for the other
three hosted targets are authored; their execution is not claimed.

| Check | Observed result |
| --- | --- |
| Frozen baseline, nine circular twins | Wrongly accepted by optimized R6 |
| Original, generic-function and new pending-call examples | All 30 positives check and run with native and Terminal exit 0 |
| Their invalid controls | All 28 normal updates reach the intended proof diagnostic |
| Subsequent invalid source checks | 19 reach the proof diagnostic; nine stop at the compiler-identity review fence and receive no proof credit |
| Existing layout customer | Normal update succeeds; native and Terminal exit 70 |
| Focused Rust tests | 48 passed |
| Source corpus selection | 13 checked, 10 intended rejections, no wrong-reason refusal |
| Retained calling-policy execution | Separate portable-runtime run matches after releasing the source session |
| CLI and corpus-runner clippy | Both exit 0 |

The termination controls include four valid source checks and two actual
native/Terminal agreements at zero. Three invalid controls match their expected
diagnostic. Two others reject for the intended termination reasons but have
stale expected text, byte-identical on R11 and R12; they are not counted as
matched controls, and their fixtures are unchanged.

The golden update adds 19 fixture rows and changes two existing rows. No keys
or diagnostic lines are removed, and unrelated row blocks remain byte-for-byte.
The calling-policy row retains its earlier timing because its outcome is
unchanged; the fresh timing remains in the raw observation. The old circular
control changes from wrongful acceptance to the intended rejection.

## Source and replay records

[The pinned evidence snapshot](https://github.com/CathedralOS/Omega/blob/f7790912afea928f7a4fbbbd412ca51991159e9c/README.md)
contains the unchanged source bundles, raw receipts and restoration instructions.
It is a separate parentless evidence commit, outside the feature branch and
main. Raw data is not added to this code tree.

R12 was built from source `d76683cd67e98087a367a0d5de4e4cdad0fe3c80` plus
patch `287cbbf11f1449f2b74a1b8ecb91f46072b7f5f654a1841443fde8c597ddb048`.
Its optimized CLI SHA-256 is
`a8c921d8ce448cf36da92e35442b00e04d342ab576e318e7ba2f24d7420174ce`;
the runner SHA-256 is
`b293505e7824ecc3c47b721ae55b12110428fbfd8043c2c028874d4f40a0bfd2`.
The later source checkpoint `810a7821372082dabdeabf63810ed714abda43cc`
changes only documentation comments in compiler inputs. That comparison was
independently reconstructed and reviewed. These remain R12 observations,
not a claim that the later commit was rebuilt.

The portable-runtime verdict reports a dirty working tree because the golden
records had changed. Its saved input patch changes only those records; compiler
and fixture source match the checkpoint. The original row stamp and raw verdict
are both retained. The raw records distinguish setup refusals from proof
rejections and preserve the interrupted postprocessing attempt.

This closes circular call admission for the provider-bound fact. Crash-capable
bound helper admission and the broader scalar-range migration remain open.
The unused-constant control establishes checking of its calculation, not direct
runtime observation of eight. GUI qualification remains deferred.
