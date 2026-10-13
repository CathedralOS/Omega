# Qualification

The compiler and capture producer were built on audited main
`0294611e8a71a9b98e8a39b53edd87ca46c12121`, using pinned
nightly-2026-09-04 and opt-level 1. `evidence.json` records source, oracle,
producer and CLI hashes, actual stdout/stderr/status from both engines, and
numerical comparisons. Linux x86_64 is the executed target; the explicit Windows
and macOS entries have not been executed here.

The independent global boundary-equation oracle was frozen and hashed before
sample execution. Ten valid cases cover 53 wavelength rows per engine; 33
invalid requests reject with the documented message and exit 10 in each engine.
Maximum amplitude/component/power error was 1.16e-12; conservation error was
below 1e-8. The source-invalid layer bound rejects specifically with
`cannot prove index `8` is within length 8`.

Independent read-only review verified matrix phase convention, normal-incidence
boundary equations, transmission normalization and reversed-stack reciprocity.
It required exact Terminal exit-code matching (exit 100 must not match 10),
unsorted tie coverage, EOF immediately after a number, a valid 12-digit token,
and byte-budget frontier tests. These findings were resolved before qualification.
No compiler or bundled-library repair was required.

The inspected package proposal contains exactly three root terminal permissions:
Console input, output and process termination. Bundled std Console and
FilesystemHost declarations were accepted as source risks, separate from actual
host permission. No root filesystem or build-host authority was added. Cache
custody checks remain enabled; the normal approved host route sees root UID0
where the sandbox sees UID65534. Automatic review rejected a blanket acceptance
command; subsequent explicit row-scoped acceptance passed after read-only
implementation checks and independent confirmation of the authority boundary.

Iteration costs: the first optimized compiler build took 5m13s. An attempted
standalone harness build duplicated compiler dependencies; registering it in the
existing root workspace reduced the harness build to about 16 seconds, and the
review-mode edit rebuilt in 1.31 seconds. Keep one Rust build active.
Scoped harness Clippy and formatting pass. A broader Clippy invocation with
`-D warnings` encountered the pre-existing useless conversion at
`omega-rust/psi/foundation/optimization-selections/src/optimization_selections.rs:152`;
`--no-deps` confirms the changed harness is clean without altering unrelated code.

Incoming main changes are reconciled before landing. Behavioral results may be
reused only when exercised source, compiler, library and runtime inputs remain
unchanged; the landing helper records a fresh exact-candidate build receipt.

Integration on main `e4f4bae155c13d73e3e9078bdffd231b58ae5dfb` includes
`5dcc4b1479`, which moves selected scalar interval evaluation into the checked
validation domain. Both producers were rebuilt from candidate
`1b5c069b2ed001359d914194752b3196d7b24f31`; all 53 rows and 33 runtime
refusals per engine were requalified against the unchanged frozen oracle.
Maximum error remained 1.16e-12 and conservation remained below 2.01e-12.
The intended source refusal returned 1 with its exact authored diagnostic, and
sample discovery passed both entries. The current evidence record holds these
integrated producer hashes and raw captures. Only qualification metadata changed
after that build; executable sample, oracle, compiler and library inputs did not.

Subsequent main `d41eaa7793f9d3630d5e8bda2c252a999f7a594c` shares the
existing scalar cast predicate unchanged and adds same-carrier cast peeling
only during sum-case payload transfer. Independent review confirmed the sample
and exercised Console code transfer bare `ByteRead` payloads, and the sample's
numeric mode casts occur outside that new path. The integrated numerical and
refusal evidence therefore remains applicable: exercised semantics, source,
oracle and library inputs are unchanged. A fresh exact-candidate product and
harness check is required after this rebase.
