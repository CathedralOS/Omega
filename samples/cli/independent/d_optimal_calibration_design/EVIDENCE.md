# Qualification evidence

The complete stdin application was qualified on Linux x86_64 with all 46 frozen
inputs. Every case validated actual native stdout and independently captured
`terminal.stdout` from normal `omega run --both --keep`. Exact indices,
information matrices and determinants matched the independent Python-integer
oracle. All covariance entries and prediction variances matched the independent
Gauss–Jordan rational inverses exactly, exceeding the requested 1e-8 absolute
and relative accuracy. Native and Terminal stream hashes were identical in
all 46 cases. The complete candidate-bound source control rejected with exit 1,
exactly one diagnostic: ``cannot prove index `12` is within length 12``.

Coverage includes unequal precision, lexicographic ties differing from numeric
mask order, repeated settings, all-singular populations, undersized designs,
minimum and maximum candidate/subset/precision bounds, small determinants,
asymmetric predictions, malformed modes and integers, invalid counts and
precisions, missing and extra tokens, negative/out-of-range predictions, tabs
and CRLF, absent final newline, and exact 512/513-byte input boundaries. Singular
designs report their chosen indices, information and zero determinant with
explicit non-identifiable status; undefined covariance and variance are omitted.

The independent manifest was frozen before application execution. Its SHA256 is
`a22eed47caab37c8cd16fcdec6dfe70e2f607844b048dcf720af0c098cabdfe1`.
`reference.py` independently enumerates combinations, expands determinants over
permutations, and performs `Fraction` Gauss–Jordan inversion. Source selection
instead enumerates masks with integer moment determinants and computes inverse
numerators by cofactors. An independent reviewer challenged determinants and
tie behavior with Cauchy–Binet on 300 seeded designs/25,026 subsets, and inverse
and prediction formulas on 209 identifiable random designs. The final review
also checks recorded execution values and integration inputs.

Final producer revision: `ff2dec64d07f3e36eb5fad054d86462a84702ec8`,
integrating main `5acf32321f42c59b07d53df354052ca4ee214c75`.
Optimized compiler SHA256:
`e98b7c7460a56c81e246074f79a6c7dd63fa177e5c8d2d203e1424851eec428b`.
The initial full qualification used the unchanged sample sources and producer
`0294611e8a71a9b98e8a39b53edd87ca46c12121`. Main's scalar-interval validation
change required a fresh producer build (5m59s) and full requalification: all
46 native/captured-Terminal cases and all 46 direct-native cases passed again.
Native executable SHA256:
`abf661d4cf5070f3df55ba43691ad93350807e48cb32d92f0c48f9166d275c00`.
Field mutation, guarded multiplication and reused signed/unsigned division
regressions also passed native0 and Terminal0 through normal package-free
preparation. Source/oracle/validator hashes and exact run receipts are recorded
in `QUALIFICATION.json`. Independent review covers both actual streams and
integration applicability. No compiler or library repair specific to this
sample, proof weakening, or trust bypass was needed. The isolated worktree
preserved the starting checkout.

The pinned toolchain is nightly-2026-09-04, nextest 0.9.146 and mbx 1.21.0.
Executor doctor passed eight checks. The executor has four effective CPU cores
and 16 GiB memory. Only one Rust build was active; the initial release build
completed in 11m13s. The unchanged cache custody check rejected the sandbox's
UID65534 root view and passed through the supported approved host route, which
sees actual UID0 ownership. Package-free smoke passed native and Terminal exit 0
with the current compiler. The authorized repository-local noreply identity was
used for commits and coordination.

Package review retained the actual initial automatic approval denial. The user
then explicitly approved the exact inventory and directed normal qualification
and landing while preserving filesystem/runtime authority checks. Source changes
correctly invalidated the old proposal. Normal refresh established all 287
canonical policy payloads identical to the approved inventory, retaining a
mapping of original/refreshed decision IDs; acceptance then resumed normally.
A later combined source-refresh/gate action was also rejected before execution.
Read-only audit then established `requires-review false` and both accepted
package policies equal to fresh findings. Normal source-only refresh succeeded
with zero pending decisions; the rejected combined action was not retried.
There are 141 callable, 141 external-supply, three Console input/output/exit,
and two unchanged std source-risk declaration rows. Independent review found
no root filesystem grant or `Build::grant`. No blanket acceptance flag, borrowed
accepted lock, denied-action bypass or authority-check modification was used.

The earlier full CLI native/Terminal case loop took 308.42s (6.05–10.22s per CLI invocation).
Repeated native iteration can reuse a recorded produced executable through
`validate.py --native ... --native-only`, avoiding repeated compilation while
retaining the full Terminal gate separately. All 46 direct-native cases took
0.0381s in total. Native-only runs cannot silently
skip execution and preserve their own result directory. Coordination initially
exceeded its budget and suffered Git OOM kills; verified cache tips, an existing
authorized GitHub credential helper scoped to this worktree, and one pack thread
restored bounded claim/status/note operations. No credential was printed.

Detailed commands, actual exits and streams, source/oracle/producer hashes,
package inventory continuity, authority/custody evidence and review findings are
preserved through the supported Library recovery archive. Local accepted locks
are excluded from transfer. `QUALIFICATION.json` records the compact receipts.
Publication uses the normal `tools/landing.py` exact-SHA verification and queue,
followed by fetched remote-main ancestry verification.

Further main changes to loans, nested collection reads and callable rosters
required another producer rebuild (7m55s). All 46 native and captured Terminal
cases passed again. To remove the measured 308.42s repeated-compilation cost,
the private SDK sponsor links 159 exact dependency units selected by the shared
`tools/link_terminal_sponsor.py`, compiles once through normal project admission,
and reuses the original Terminal artifact and runtime provisions. It makes no
policy decisions or source-interpreter fallback. Compiler/executable/source and
all dependency hashes are checked; each fresh runtime retains isolated-domain
exit delegation, ten-million fuel allowance and a 30s timeout. Incomplete or
unsupported observations fail. Final current-main 46-case Terminal replay took 15.58s, with
all native values independently checked from the public CLI's produced image.
The original per-case CLI route remains available; both routes passed.
