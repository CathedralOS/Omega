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

The final application delegates its moment determinant to a const-generic
helper. Selected returning guards establish the moment extent before indexing;
this application therefore exercises the general compiler repair described below.
The previous compiler's exact helper witness rejected those accesses. The
independent oracle and the selection, cofactor and prediction formulas stayed
unchanged. `QUALIFICATION.json` binds each observation to its source, producer,
compiler, executable and captured Terminal identities. The isolated worktree
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
unsupported observations fail. An earlier reconciled 46-case Terminal replay took 25.03s, with
all native values independently checked from the public CLI's produced image.
The original per-case CLI route remains available; both routes passed.

Compiler advancement uses a natural generic polynomial-moment helper as the
pressure test. The original integrated compiler rejected `moments[0]` under
`transition Count > 0` for `[i64 in Wrapping; Count]`, although an equivalent
machine or state precondition passed. Canonical `language/state_contracts.md`
(returns and proof use) makes selected-arm facts available, and
`language/generics.md` makes const binders available in layouts and contracts.
The range consumer rescanned only preconditions, losing the selected guard's
actual lower bound. The general repair retains branch-local floors keyed by the
exact immutable binder symbol, uses the existing generic receiver declaration
and argument-position join, and recognizes that binder in named-state arrivals.
Requires, returning arms, named states and slices now consume the same facts;
the old precondition-only rescan is removed.

Empty specializations exposed a second consumer gap: a literal index in an
impossible arrival was rejected before its closed guard could discharge the
ordinary bounds contract. Canonical `terminal-psi/integer_certificates.md`
permits checked closed-order contradiction to close impossible arrivals. The
repair retains exact closed integer source-term handles and effective guard
polarity, then uses the existing strict scoped arithmetic contradiction
judgment. Typed operand formation and selected builtin-operator meaning remain
prerequisites; unknown terms decline, address/IEEE operands contribute no
integer premise, and source arms are not pruned. No assumption about a later
bounds certificate substitutes for this frontend judgment.

The focused generic moment fixture independently expects determinants 72 and
1728 and zero for empty direct, named-state, record and slice specializations.
It checks the actual computed results before normal exit; wrong values crash.
Seventeen authored rejection controls preserve weak/wrong-arm/disjunctive
bounds, one-past and negative indices, shadowed/unrelated binders, unproved
predecessors, overwritten aliases, authored comparison meaning, invalid index
types and zero-divisor formation, including under impossible arrivals. Baseline
witnesses, frozen numeric expectations, source/producer identities and complete
refusals are retained in the recovery archive.

Actual execution then exposed a native geometry failure: runtime index extent
zero was represented as failed `extent - 1` rather than an empty admitted
selector domain. The native layout repair distinguishes empty from finite
reach, changing only the universal physical-span condition. Exact root layout,
selected path/leaf type, descriptor readability and all selector/stride/extent/
obligation identities remain checked, including the existing certificate join
at the original operation. Static indexing into an empty array still rejects;
unknown and overflowing nonempty geometry still decline. The complete generic
moment fixture now executes native0 and Terminal0 with all fifteen expected
values, including four empty specializations. Canonical corpus check records
one checked fixture and seventeen exact authored rejections, and the paired
runtime gate records native0/Terminal0. Scoped Clippy passed all three affected
compiler crates; existing warnings were retained. Unmeasured golden rows were
preserved verbatim while adding only the tool's measured new records.

Fresh returned-view integration qualification passed all 46 exact native/raw-Terminal
cases, the 18 repair checks (17 authored refusals), the generic native/Terminal
oracle, eight authored execution regressions and two returned-view custody
refusals. Scoped Clippy and normal sample/source-invalid native gates passed.
The retained check comparison reports one absent historical row and timing-only
moves; it reports no lost success or oracle failure. A private canonical measured
execution gate enforces all eight authored oracles without rewriting shared goldens.

Qualification through main `1877bf177f2e7d1099689a82008acff01a523a75`
covered captured-length facts, expression transfer and entry admission. Final
integration through `335ac2a6ac1bb207fe780a1d17488bc83d1f1c3a` changes
recursive boundary storage, the common unit call roster/cleanup, carrier
retirement, placed-view/endpoint lowering and bundled numeric source. These
exercised paths require a fresh producer and focused requalification. The
merged store retains incoming measured results, branch-local const floors and
closed integer arrivals together. Fresh qualification uses the integrated
producer: the 18 repair controls, five contracted-length controls, their actual
native/Terminal value assertions and the complete numerical application. Five
existing package-free authored runtime regressions also use the integrated producer;
the earlier canonical observations for three std-based fixtures are retained with
explicit producer identities and applicability review. Those three std-based
observations do not establish current335 execution; the common affected paths
are exercised by the authorized sample and seven package-free value assertions. Shared
golden prefixes are preserved verbatim; only the previously measured 18 repair
rows are appended. Exact-SHA landing verification is separate from recorded
runtime qualification.

An optional earlier native probe of `post_mutation_guard_alias` refused with
`SourceCustodyMismatch`; `post_mutation_guard_bound` produced no execution facts.
Neither is reported as a runtime pass. Independent source/dependency review
found these observations outside this repair: neither alias route has a const
binder or bounds access, and nonempty runtime-span algebra is unchanged. This
is a source attribution, not a claimed base execution comparison. Their seven
related source checks passed and the complete receipts remain in the archive.

A release relink previously failed with missing LLVM symbols in one lowering
crate. Supported package-scoped `cargo clean --release -p
terminal-psi-to-abstract-operations`, followed by per-build
`MBX_LEARNED_INCREMENTAL=0 MBX_INCREMENTAL=0 mbx build --release -p omega`,
restored the producer in 2m59s. Global cache configuration and custody checks
remain intact. The SDK observer is linked after release builds settle and checks
all exact dependency hashes before replay; a captured dependency-drift refusal
was retained rather than bypassed.

Final qualification integrates main
`0af7c11c3c7634237d5be7a2e3e1e8daa2a0bf89`. Incoming reference-transfer
and scalar-snapshot rules affect mutable carriers combined with field/index
observations, so a fresh producer qualified the entire46-case numerical suite.
All30 frozen source controls passed:18 repair checks, five captured-length
checks and seven borrowing refusals. Eight native/Terminal programs passed their
authored value assertions, including the incoming scalar-snapshot witness;
independent review reran all eight retained native images. Returning-call
exclusive loans, live child/retained loans, duplicated reference leaves,
shared/mutable access and write-only carrier reads retain their required
refusals. Scoped Clippy passed with existing warnings. No package policy or
runtime/filesystem authority was widened.

Publication integration through main
`d8ee64cf16abec5b10b98ae216730b7439cd29b9` preserves the qualified
operation schedule: the shared frame's dense scalar-array construction, source
operand staging, tail cleanup and local binding match the removed copy. Composed
graph emission and needed derivations remain; removed rejection-only rechecks
had already accepted these programs. Discarded provider selection is absent.
Independent review supports reusing actual90/76bfa observations.

The newly introduced qualification runner captures a fresh custody-validation
check over saved observations and immutable copies of the complete historical
Git/build/library/artifact closure. This records no new native execution and
no d8-built producer. Its final review check binds the exact candidate, base,
full tree and five canonical passage mappings, with explicit continuation timing
after the tool's introduction. The original semantic repair predates this tool;
current context capture supplies no retroactive language authority. Source and
artifact replacement checks, independent review and normal exact-SHA landing
verification remain active. The normal native gate passed the sample and matched
its intended-invalid control with the qualified90 producer.
