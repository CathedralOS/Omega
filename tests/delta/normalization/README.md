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
test watchdog, including the 2,048-field compilation measured at 134.6 seconds.
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
Two 600-level controls require later extraction to capture an earlier helper's
fresh parameter correctly. Repeated free references must use exactly one
parameter per helper. A same-spelling binder inside an outer let's initializer
retains its independent scope across extraction.
Two controls reach the admitted Delta expression depth of 1,024: repeated
captures must remain singular, and 1,023 nested checked additions must produce
1,024 before returning the unchanged binary input. These exercise successful
compilation and execution, complementing the frontend's adjacent-depth refusal
controls rather than replacing them.

The 28,797-byte, 2,048-field source pins its complete 91,238-byte receipt.
Its unused wide function must still compile and validate before the identity
entry returns binary input. On macOS arm64 the previous compiler exceeded a
300-second watchdog; preparation/lowering alone completed in 6.7 seconds.
Shared projection lowering completed the unchanged source in 134.6 seconds.
This witnesses compiler completion, not an observed heap exhaustion or a
claim that runtime traversal of every wide field is now linear.

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
