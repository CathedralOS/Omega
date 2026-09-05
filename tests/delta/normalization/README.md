# Delta normalization gate

Run `sh tests/delta/normalization/run.sh` from the repository root. It uses the
complete canonical compiler and a separate private diagnostic prefix over the
same source closure, both pinned in `compiler.tsv`.

Sixteen authored programs check normalized execution under
`ConformanceBytesV1`. Each canonical compilation must succeed twice with
identical bytes. Its generated Gamma must parse and execute with the selected
evaluator, preserving `41 00 80 FF`, except the repeated-reference control,
which returns that payload twice, and the selected authored-trap control,
which must halt 249 with empty stdout. Every invocation retains a 30-second
watchdog.

Coverage includes exact expression-list height 255 and adjacent 256, unused
deep bodies, 300 lets, nested checked arithmetic, deep arithmetic guards,
128-field payload bindings, a 300-field constructor and payload match,
same-spelling bindings in disjoint scopes, generated match-selector captures,
and selected versus unselected trapping branches. Two 1,000-iteration controls
exercise tail recursion through extracted let and match bodies. Arithmetic and
match cases require both source bindings and compiler-generated bindings to
retain their values across helper extraction.
Two 600-level controls require later extraction to capture an earlier helper's
fresh parameter correctly. Repeated free references must use exactly one
parameter per helper. A same-spelling binder inside an outer let's initializer
retains its independent scope across extraction.

The fitting height-255 program pins its entire 3,729-byte canonical receipt by
SHA256. That receipt was measured with the preceding 111,464-byte compiler
(`469a007e0114cdf833b61463161a4e6ff7e246b4d01f8861ea39c948bcb3b9b6`)
and executed successfully before normalization. This checks byte preservation
for a fitting whole program, not only application equivalence.

The private diagnostic calls production `prepare_admitted_source(1)` and
`normalize_program`. It publishes authored count, original maximum body height,
normalized count, normalized maximum body height, and the maximum helper
parameter count as five little-endian u32 values, followed by the unmarked
evaluator's scalar `00`. The gate checks
the authored count, helper presence for oversized programs and absence for the
fitting one, and height at most 255 for every normalized body, including newly
appended helpers. The host reads these explicit observations; it never parses
Gamma, computes syntax height, extracts functions, or lowers source.

This private framing is not a compiler or application envelope. The
[lowering-plan gate](../lowering-plan/README.md) continues measuring the
unnormalized expanded plan. These bounded normalization controls do not close
all resource limits or the Delta bootstrap edge.
