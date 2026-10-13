# Principal component analysis qualification

W2-01 is a complete stdin sample at
`samples/cli/independent/principal_component_analysis`, with covariance and
correlation PCA, 3–16 observations and 2–4 features. It reports means, unbiased
covariance, analyzed matrix, decreasing eigenvalues, explained variance,
eigenvectors, principal scores and reconstruction error. Covariance accepts
rank deficiency and constant columns; correlation refuses constants. The reader
bounds bytes, tokens and decimal token length and documents refusals.

## Independent numerical evidence

Before the first application execution, `validation/oracle.py` froze 15 datasets
using centered NumPy SVD, together with 19 refusal streams. It uses no application
Jacobi operation. `FROZEN.sha256` pins the numerical manifest; the manifest also
pins every stdin stream. The datasets exercise both modes, translation, column
permutation, rank-one covariance/correlation, entirely zero covariance, decimal
constants, repeated eigenvalues in two and four dimensions, dimension limits,
small/large measurement scale, and a finite spectrum whose trace exceeds binary64.

Actual native and captured Terminal stdout independently satisfy the frozen
oracle. Maximum spectral-norm eigen-residual relative to the analyzed matrix norm
is `7.450484363295375e-13`; maximum spectral-norm orthogonality error is
`3.212607874966871e-13`, both below `1e-8`. The validator compares subspace
projectors for repeated eigenvalues, rather than requiring arbitrary eigenvectors.
It checks scores against centered measurements times the emitted basis and
reconstruction in original measurement units. All 19 refusals match their exact
exit and output, including numerical-range exit 66 and constant-correlation exit 65.

The captured Terminal product and the checked native executable come from one
normal accepted-project compilation and publication. The sponsor uses the
public runtime API with the exact retained Terminal artifact and original runtime
provisions, finite fresh input, an explicit fuel budget and isolated ProcessExit
delegation. The ordinary `run --both` exit comparison is additional evidence;
it cannot replace these numerical stream checks.

## Compiler failure and repair

The original compiler from fetched `31988b31bf` rejected actual native production
of an explicitly cast case payload: `computed has no executable body`, with
`guarded pair without exact false fallback`. Check-only acceptance did not
establish executable support. The same operation blocked `Input::read` in PCA.

Closed-sum payload forwarding now uses the existing scalar same-carrier cast
judgment. That judgment was moved into the shared semantic-cast owner and the
scalar computation builder calls it there. An arithmetic-policy or interval cast
on the unchanged primitive carrier forwards the checked field's value through
the same payload transfer. Real carrier conversions, borrow recasts and semantic
domains remain opaque and keep their separate computation or custody obligations.
No source-name or sample-specific lowering path was introduced.

`runtime_case_payload_policy_cast_exit` checks actual values -7, 8 and 9 through
computed, parameter and local subjects, nested policy casts, and a field subject.
Five affected fixtures execute with authored native/Terminal exits, and eight
source fixtures check or reject with authored diagnostics. Predicate-domain,
carrier mismatch and false semantic-domain cast refusals remain effective. The
sample check gate checks 2 projects, including its source-invalid fifth-feature
control with `cannot prove index 4 is within length 4` diagnostic identity.

## Environment, authority and workflow

Linux x86-64 on the selected cloud executor; four CPU quota and 16 GiB memory;
Rust nightly 2026-09-04, mbx 1.21.0, nextest 0.9.146. Runtime model/effort metadata
was unreported. The package-free smoke fixture produced native and Terminal exit 0.
Only one Rust build runs at a time. Initial optimized CLI compilation took 7m32s
with 107 cache misses; the incremental repair build took 2m41s, and the warm final
build after restoring unrelated formatting took 34.93s. One compilation publishes
all numerical cases; initial native/Terminal product production took 13.89s and
individual valid Terminal captures took 54–264ms. No repeated per-case Rust or
source rebuild was used for numerical validation.

The sandbox exposed root ancestry owned by UID65534 and failed normal Omega
cache custody. The approved per-command host route passed the unchanged checks.
Automatic approval review initially refused package acceptance, interpreting std
risk disclosures as filesystem authority. Read-only policy inspection established
that DangerousCapability is an inert legacy source-risk summary and only exact
TerminalPermission rows pass into runtime permission consent. Re-review accepted
the command with that evidence: the three application permissions are Console
input, output and process termination. No FilesystemHost permission or Build
host-access grant was added, and no denial was bypassed through another route.

Affected `mbx check -p omega -p typed-trees-to-checked-trees` succeeded. Strict
crate Clippy is known red: the exact unmodified fetched base has 226 findings and
the candidate has 225, with the same remaining diagnostic classes. The comparison
restored the three changed compiler files to the base for one warm baseline check
and then restored the candidate. No unrelated lint cleanup is claimed.

Independent read-only review challenged spectrum normalization and found the
trace overflow. Normalizing eigenvalues before summing repaired it, and the frozen
large-trace case validates explained fractions `[0.5, 0.5]`. Source and numerical
review also checked cast exclusions, finite output, explicit runtime delegation
and strengthened intermediate payload-value observations. Recovery archives retain
source hashes, observed streams, commands, exits and the exact frontier. Final
publication uses the normal landing receipt and queue, followed by fetched-main
ancestry verification; local evidence or a candidate branch is not completion.
