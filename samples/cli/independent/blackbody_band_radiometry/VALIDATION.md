# Numerical qualification

Qualification used compiler revision
`d3431e5a76cb08a2bf0b64f92b5e11fd9c7d9ea5` (integrated main
`91135b64bbd67ed1f43c56d99961086604b91335`), pinned
`nightly-2026-09-04`, Linux x86_64, four build jobs and a 16 GiB memory limit.
The compiler SHA256 was
`f3bb6866407c8501ba7aff8b1ce944e1767fd8d80aad061c8debf87a0e015f02`.
The optimized build after integrating main took 493 seconds.

All 65 frozen cases passed both native and captured Terminal validation:
36 valid numerical records and 29 runtime refusals. Every stdout stream was
byte-identical between the two engines; stderr was empty. Validators compared
all four numerical columns to the independently frozen 80-digit log-wavelength
tanh-sinh oracle, not just exits. Agreement requires
`max(1e-12, 1e-6 * abs(reference))`, with watts as the absolute-floor unit for
powers and a dimensionless floor for fraction. The largest native numerical
error consumed 0.007485 of the allowed tolerance (left-band emitted power).

| Case | Emitted W | Ambient W | Signed net W | Emission fraction |
| --- | ---: | ---: | ---: | ---: |
| 300 K, 280 K ambient, 8–14 µm | 205.368484 | 148.377037 | 56.9914474 | 0.375742293 |
| 3000 K, 1000 K ambient, 0.9–0.901 µm | 12.9730084 | 0.000305460867 | 12.9727029 | 0.000672503662 |
| 3000 K, zero ambient, 0.1–10000 µm | 4593003.27 | 0 | 4593003.27 | 0.999999999 |

Inputs, including coupling factors, are in the corresponding frozen `.stdin`
files. Qualification covers equal temperatures, signed temperature swap, band
additivity, wide-band Stefan–Boltzmann limits, close-temperature cancellation,
ULP-wide bands, decimal halfway rounding, zero coupling, every quantity bound,
malformed/nonfinite tokens, CRLF/EOF and exact input-capacity boundaries.

The actual quadrature module's 321-panel source control was checked with the
same compiler. It exited 1 with the instantiated
`panels >= 1 && panels <= 320` fact structurally false. The validator requires
that diagnostic and excludes substitute import, entry-acceptance or custody
failures. Positive source checking exited 0.

The native executable SHA256 was
`615cb57baee347453a53161b4a465a47d9aefd2558d3e5ba333dd08f6af0fa10`.
The private consumer compiled through the ordinary accepted package route,
retained the exact native production's 619958-byte Terminal artifact and
executed each case with isolated domain exit and bounded fuel. That artifact's
SHA256 was
`0375f1577ba0053fbdf631b4c3ccee7bfc2c79fe95da3868d1c0600123e6c6a3`.
`tools/link_terminal_sponsor.py` selected the verified CLI's exact 159-unit
compiler dependency closure using byte identity and dependency fingerprints.
The sample wrapper avoids a nested Cargo rebuild and generated source-package
symlinks. No compiler-identity override, trusted-fixture wrapper or source
interpreter was used.

The frozen reference and its 65 inputs predate application execution; hashes
are in `validation/FROZEN_SHA256.txt`. A 100-digit convergence check on all 33
unique valid records differed by at most 1.50e-60 of the allowed tolerance.
Validator self-checks rejected all 144 deliberately perturbed exit-0 numerical
columns. Independent review also challenged decimal conversion with 2008
cases, including halfway ties, exponent carries and tiny fractions.

An initial automatic approval rejection was honored. The user then explicitly
approved the exact 287 pending review rows: 141 callable declarations, 141
external-supply declarations, Console/process and FilesystemHost/filesystem
source-risk declarations, and only the three application Console permissions
for input, output and process exit. The regenerated scope was checked before
acceptance and resumed through normal package review. Source-risk declarations
did not grant broader host filesystem authority. Cache custody, source and
compiler identity, proof and runtime authority checks remained enabled; a stale
review was refused during main integration and regenerated.

Recorded commands and streams are under the ignored `build/validation` and
repository `build/blackbody-evidence` directories and preserved with the source
in the recovery artifact. Qualification is for the Linux host; other authored
entry bindings do not claim native execution on macOS or Windows. This change repairs general record-contained slice metadata at proof call
boundaries and preserves evaluated constructor children for retained lifetime,
type and const applications. The sample scanner exercises the rule through its
seven-element shared view. Ordinary indexing and named calls use the same exact
field extent. Captured metadata never borrows later facts from a replaced
record; saved-record recovery admits only closed constant metadata.

Seven checked regression fixtures and three both-engine fixtures were recorded
through the normal corpus gate. Owned and borrowed floating reads observe 11
and 14; a type/const constructor observes 11. Controls refuse missing bounds,
sibling bounds, stale saved descriptors and invalid geometry for their authored
proof obligations. Existing current-field arithmetic, floating guard and
returned-field unknown controls retain their checked/rejected outcomes and
expected diagnostic fragments. Clippy passed for both affected Rust packages.
The normal sample gate passed in 48 seconds (16 seconds review, 32 seconds run).

The private observer linked the exact 159-unit closure in about one second,
avoiding a second Cargo build. Broad structural tracing was measured at over
1 GiB and replaced during diagnosis by retained small metadata excerpts; debug
instrumentation is absent from the candidate. An executor disconnect during
qualification recovered with existing processes intact, without a duplicate
Rust build. Incoming general compiler changes were integrated before the
recorded numerical qualification.

An integration requalification used candidate
`fd2ff423186dd2326c80300d0c051baf761f1b41` on main
`12d4cf2df1721eb103b2b5d67d1901e4e3f53b30`. Its release rebuild took
82 seconds. All 65 native and retained Terminal records again satisfy the
frozen oracle and are byte-identical, including all 29 refusals. The
321-panel source refusal and the observable both-engine record reads also pass.
The integrated CLI SHA256 is `e5c273c7f3e8dfca8cb060c6b58b14cef943b62de1758e5c88f8aa550a05b7e2`; native SHA256 is
`615cb57baee347453a53161b4a465a47d9aefd2558d3e5ba333dd08f6af0fa10`; Terminal SHA256 is
`0375f1577ba0053fbdf631b4c3ccee7bfc2c79fe95da3868d1c0600123e6c6a3` (619958 bytes).

The normal identity gate caught generated Python bytecode after rebase. The
private validators now disable bytecode writes before importing the sample
helper. Generated cache removal and fresh ordinary review restored acceptance;
no source identity, custody or authority comparison was bypassed. Package
operations are serialized to preserve the ordinary project lock.

The final proof/capture integration is based on main
`6ce29219f01624391814cc260d8ed128633066b8`, with compiler candidate
`f87e5abb71458fa259d783d86c1d2a88f00e515f`. Its CLI SHA256 is
`ea49d549ce26b53508dc577023cc285250ef5bba78d9b2406343eb03378f4089`.
All 65 native and Terminal cases pass again; both retained artifacts have the
same hashes as the preceding qualification. Both observable record fixtures
pass native and Terminal execution. The 16 checked controls retain their
outcomes and expected fragments. Incoming constant unfolding adds the exact
closed expression `0 <= 3 && 3 <= 2` to the geometry refusal; its owned corpus
row is re-recorded through the normal gate. No runtime or proof refusal is
weakened. The observer's exact 159 dependency hashes were checked at link time.

Placed-view integration requalification used compiler candidate
`cefb3a55f69393ecc20de90b56414b5cf3515bea` on main
`0855a204a2df4a305d7c14c3d4e467f465ae2335`. Its CLI SHA256 is
`23897100ee82d2814d34e6229c375d06b7703d68ae7c23332a5cd3114600348c`.
All 65 actual native and newly captured Terminal cases passed and have
byte-identical stdout. Native bytes retain SHA256
`615cb57baee347453a53161b4a465a47d9aefd2558d3e5ba333dd08f6af0fa10`; the
619958-byte Terminal artifact now has SHA256
`c0f1dc6c8d2e814433033a25c0b33b22f55db9b3d2863f4306ae89195958760e`.
Sixteen checked controls are clean (five accepted and eleven refused), and
both observable record-read fixtures execute with exit zero in both engines.
The ordinary review reused the approved policy; the source panel control still
fails for its exact proof obligation. The new observer closure's 159 dependency
hashes were verified before execution.
