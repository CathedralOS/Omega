# Qualification

Initial sample qualification: the compiler and capture producer were built on audited main
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
No compiler or bundled-library repair was required for that initial scalar-matrix implementation.

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

The subsequent compiler advance composes the canonical `Complex<T>` carrier
with nested ordinary static trait calls in quotient and norm. A valid checked
returned-record witness was previously refused at the scalar computation
nominal-route gate. The general repair retains the full owner-scoped closed
conformance selection on the checked call and consumes it against its exact
source occurrence at lowering; public source contracts remain authoritative.
Named proof-output, dynamic and provider execution routes keep their own gates.

On integrated main `9f28e49398a1dd6334d4aa881d39b888524ef6ee`, the repaired
native and exact retained Terminal products pass all 53 wavelength rows and 33
refusals per engine against the unchanged frozen global boundary-equation
oracle. Maximum amplitude/power error is 1.159405904616051e-12 and maximum
conservation error is 1.999955756559757e-12. The layer-index 8 source control
still returns 1 with its authored diagnostic. `composition_qualification.json`
records the dirty producer checkout, exact binaries and executable source hashes;
`evidence.json` contains the latest raw inputs, streams and statuses for all 43
cases and preserves prior producer qualification history.

Focused corpus execution verifies f32/f64 composition, sibling implementations
with identical signatures, nested call results and inherited/default dispatch
inside returned records. Three discriminating invalid fixtures reject wrong
trait arguments and public requires/blocking despite weaker pure private bodies.
Five affected native/Terminal regressions satisfy their authored exits and
outputs; the regression gate reports three positive rows missing from its older
golden, not execution failures. The source proof/effect regressions retain their
expected verdicts. Published corpus records add only the new scoped rows.

One Rust build was active at a time. The continuation's optimized initial build
took 274 seconds, the incoming-main rebuild 294 seconds and the final warm rebuild
24.18 seconds. The capture harness remains in the root workspace, avoiding the
previous dependency duplication. Compiler and package source custody checks,
normal identity refresh and exact Console-only execution permissions remain
active; source-risk declarations grant no host access.

Final integrated applicability: refreshed producer `1c764ac08586` includes the
incoming homogeneous-float admission and selective-guard/mutable-scalar repairs.
The numerical 53/33, composition and seven source checks pass again. Main
`12d4cf2df1721eb103b2b5d67d1901e4e3f53b30` then changes only unrelated recast
fixtures, corpus rows and documentation. Independent review accepts reuse on
that integrated tree. The current evidence and qualification record contain the
refreshed binaries, raw captures and exact executable source hashes. Metadata
amendments do not alter executable inputs. The landing receipt remains required
on the exact final commit.

Final publication integration includes provider-result transport, general
closed-sum transfer admission, expression-fact fallback and entry-scoped
intrinsic admission. Full 53-row/33-refusal captures were refreshed after the
provider/runtime change. Subsequent rebuilt native/Terminal smoke cases cover
Byte, EOF and immediate final-token EOF with exact frozen numerical stdout;
eight affected source fixtures pass, including public precondition, effect,
conformance and returned-field-domain controls. Independent review confirms
numerical runtime/arithmetic reuse and preservation of exact carrier, mechanism,
permission, custody and application gates. Later main changes through
`1877bf177f2e7d1099689a82008acff01a523a75` touch no exercised compiler,
library or application source. Latest smoke and numerical producer identities
are recorded separately so the evidence does not claim an unexecuted producer.

Main integration through `77cde1c51efdee32386859b1d723c18909ba2a67` also includes placed-view reborrowing, explicit range access and exact service module identity. Independent applicability review found no new dispatch or authority defect. Rebuilt default, immediate final-token EOF and malformed-input smoke captures match exact frozen numerical/error stdout, empty stderr and exit0/10 on both engines. The composition fixture, eight source controls and layer8 refusal pass; normal package review retains only Console input/output/exit. Full53/33 numerical captures are retained with their separate producer identity.
