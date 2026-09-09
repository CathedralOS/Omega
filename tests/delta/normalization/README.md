# Delta normalization gate

Run `sh tests/delta/normalization/run.sh` from the repository root. It uses the
complete canonical compiler and a separate private diagnostic prefix over the
same source closure, both pinned in `compiler.tsv`.

Twenty-one authored programs check normalized execution under
`ConformanceBytesV1`. Each canonical compilation must succeed twice with
identical bytes. Its generated Gamma must parse and execute with the selected
evaluator, preserving `41 00 80 FF`, except the repeated-reference control,
which returns that payload twice, and the selected authored-trap control,
which must halt 249 with empty stdout. Every invocation has a 300-second
test watchdog, including the 2,048-field compilation measured at 16.0 seconds.
This host allowance is not a language or evaluator limit.

Coverage includes exact expression-list height 255 and adjacent 256, unused
deep bodies, 300 lets, nested checked arithmetic, deep arithmetic guards,
128-field payload bindings, the 256/257 projection transition, a 300-field
constructor and payload match,
same-spelling bindings in disjoint scopes, generated match-selector captures,
and selected versus unselected trapping branches. Two 1,000-iteration controls
exercise tail recursion through extracted let and match bodies. Arithmetic and
match cases require both source bindings and compiler-generated bindings to
retain their values across helper extraction.
The payload cases check first and middle integer fields and the pair-bearing
last `Bytes` field, rather than only exercising the tail case.
Two 600-level controls require outer capture to forward original bindings
correctly to already-extracted inner helpers. Repeated free references must use exactly one
parameter per helper. A same-spelling binder inside an outer let's initializer
retains its independent scope across extraction.
Two controls reach the admitted Delta expression depth of 1,024: repeated
captures must remain singular, and 1,023 nested checked additions must produce
1,024 before returning the unchanged binary input. These exercise successful
compilation and execution, complementing the frontend's adjacent-depth refusal
controls rather than replacing them.

The 28,797-byte, 2,048-field source pins its complete 95,402-byte receipt,
SHA-256 `6c7956785ddd24ff99c344bb2f12ae011fe45a7d35c6c1dda8ca94af1eef6cfc`.
Its unused wide function must still compile and validate before the identity
entry returns binary input. Canonical compiler SHA-256
`7b39266be43a7459a717f6624cc6e128579869398eae3e3ecef5a08006183df5`
passes the complete default gate on macOS arm64, including repeated compilation
of this exact receipt and its `41 00 80 ff`, status-zero execution.
The [capture invariant](../../../bootstrap/3_delta/implementation/normalization/README.md#captured-bindings)
explains sorted-unique helper batches and identity-based scope subtraction.
Lookup in lowering reuses the existing name trie. These changes do not make
runtime traversal of every wide field linear or establish whole-producer
allocation containment.

The fitting height-255 program pins its entire 3,729-byte canonical receipt by
SHA256. That receipt was measured with the preceding 111,464-byte compiler
(`469a007e0114cdf833b61463161a4e6ff7e246b4d01f8861ea39c948bcb3b9b6`)
and executed successfully before normalization. This checks byte preservation
for a fitting whole program, not only application equivalence.

The private diagnostic calls production `prepare_admitted_source(1)` and
`normalize_program`. It publishes initial definition count, original maximum body height,
normalized count, normalized maximum body height, and the maximum helper
parameter count as five little-endian u32 values, followed by the unmarked
evaluator's scalar `00`. The gate checks
the initial count (including a shared projection definition when installed),
extraction-helper presence for oversized programs and absence for the
fitting one, and height at most 255 for every normalized body, including newly
appended helpers. The host reads these explicit observations; it never parses
Gamma, computes syntax height, extracts functions, or lowers source.

This private framing is not a compiler or application envelope. The
[lowering-plan gate](../lowering-plan/README.md) continues measuring the
unnormalized expanded plan. These bounded normalization controls do not close
all resource limits or the Delta bootstrap edge.

## Full-width allocation control

`sh tests/delta/normalization/run.sh --full-width` selects one source with a
65,535-field constructor, a one-parameter function matching all those fields,
and an identity `main : Bytes -> Bytes`. The match returns its last field;
the entry does not execute that unused function, but the complete compiler and
generated Gamma validator must still admit it. The gate uses the same height,
capture, repeated-compilation, and binary-output checks with a 1,200-second
diagnostic allowance per invocation. This is an opt-in conformance control,
not a profile change or a permanent profiler.

The 983,151-byte source has SHA-256
`67a00ec31c05a30042a8ae73e24c12212065a2777eccfca7b767ee355d9ed839`.
Its parameter plus pattern binders exactly fill 65,536 active locals; it has
one nominal type, one constructor, two functions, and expression depth two.
The source-derived parser and grammar allocation is respectively 655,529 and
524,324 pairs: 47,194,120 syntax bytes, below 114,294,752.
The [capture allocation argument](../../../bootstrap/3_delta/implementation/normalization/README.md#capture-allocation-ownership)
tracks splitting and collection's retained allocation owners. This control
checks completion, not a measured arena peak or whole-producer resource proof.

With the preceding canonical compiler SHA-256
`67b578fd34cb9188e66def82c70bd5f489b70962b4eeacbdbbfd259f1f68a86a`,
a disposable canonical DCREQ/profile-1 run on macOS arm64 compiled this source
in 511.187 seconds to 3,102,098 bytes, SHA-256
`d254b0f8497f617dba196196d4a8c03ceca4063687414f25d4e4b11266c9123c`.
That exact receipt returned `41 00 80 ff` with status zero and empty stderr.
The old compiler hit a 120-second diagnostic watchdog; the candidate first hit
300 seconds before the longer successful run. Neither timeout was reported as
a resource frame or used as a speedup baseline.

At `17fa177caf`, the complete `--full-width` gate passed on macOS arm64:
normalization height/helper observations, two byte-identical canonical
compilations matching that receipt, and generated execution. The outer
`/usr/bin/time -p` measurement was 1,404.26 seconds wall time, 1,399.84 user,
and 4.73 system. This is one full test invocation, not a bootstrap-chain timing
or a controlled speedup comparison. It does not cover the separate
[wide-reconstruction payload refusal](../../../bootstrap/3_delta/implementation/normalization/README.md#full-width-payload-refusal).

Free-binding collection, canonical compiler SHA-256
`94775f52b7fa012c2e9ad654c362f40854f5a7e529c59014747b8e44492581bd`,
compiled the same source on macOS arm64 in 469.156 seconds to 3,292,851 bytes,
SHA-256 `15e856f8acd6429be8a1e25f516c68d3e6f06259bc368bf25e74695a99c3a668`.
That exact receipt returned `41 00 80 ff`, status zero, and empty stderr.
Original binding spellings and helper-only numbering explain the new receipt.
Concurrent gates make this a completion observation, not a controlled timing
comparison. The full opt-in diagnostic/repeated-compilation sequence above has
not been rerun for this compiler; its canonical compilation and generated
execution were checked in a disposable probe.
