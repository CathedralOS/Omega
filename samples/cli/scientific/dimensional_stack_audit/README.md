# Dimensional stack audit

This complete Omega CLI audits six bounded three-contributor assemblies with
signed dimensions, independent local interval tolerances, and correlated named
measurement offsets. Enter a profile (1..6), temperature-low, temperature-high,
one signed decimal integer per line. For example `3`, `-10`, `20` emits the
attainable fit interval [9,51] micrometres, CONDITIONAL, and adverse contributors
at each violated endpoint witness. No numerics-library work or worker candidate
is required; the only dependency is the repository's existing std Console.

This is an explicitly declared sample physical/measurement model, not a
calibrated engineering tool. A contributor is
`nominal + local_error + thermal * temperature + gauge_a * A + gauge_b * B`.
Signed nominals and coefficients already incorporate assembly direction. Local
errors independently range over ±tolerance; temperature, A and B are shared
across all contributors. Linear thermal slopes are authored micrometres per
Celsius degree relative to a declared reference temperature; no nonlinear
expansion law or statistical probability is claimed. A and B are named micrometre calibration offsets; their coefficients are signed
dimensionless sensitivities. Input endpoints and reported results use exact
integer micrometres, while interval variables range continuously. `data/fixtures.json` lists contributor tuples in that order
and bounds `[A_low,A_high,B_low,B_high,fit_low,fit_high]`.

The private `model/audit.omg` sums named coefficients before selecting their
endpoints. The contributor row is a fixed three-element array; calculation
embeds that length and continues only when the returned span is 3. Summing each contributor's standalone uncertainty interval would
lose cancellation and is intentionally not this algorithm. Classification is
GUARANTEED for inclusive containment, IMPOSSIBLE for strict disjointness, and
CONDITIONAL for the remainder. Endpoint touching counts as possible. The attainable
interval is continuous under the declared real-valued interval model; integer
fixtures merely give exact endpoint calculations.

An offending contributor means its deviation from its own nominal pushes
outward at the selected violating witness. This is an attribution report,
not a claim that removing that part would repair the assembly. Shared-variable
cancellation can make another contributor counteract it. Zero aggregate
coefficients use the lower bound for the lower witness and upper for the upper
witness. Contributor identifiers are zero-based fixture row indices.

Profiles cover total cancellation (1), guaranteed correlated fit (2), mixed
thermal conditional fit (3), impossible fit (4), asymmetric named offsets and
inclusive endpoint fit (5), and shared cancellation with independent local
errors (6). Existing unit_converter only converts scalars, stats_compute computes
statistics, interval_member_lit checks a language refinement, and stack_calculator
executes arithmetic tokens. None implements correlated dimensional stack auditing.

Input rejects an absent token, empty line, nondigit, embedded sign, whitespace,
CR, more than five digits, unknown profile, unordered temperature interval or
endpoints outside [-100,100], with exit 64. Signed zero and leading zeros are
accepted. EOF may terminate the final nonempty token. Only three tokens are
consumed; subsequent input is irrelevant. Assembly validation also checks
nominal, coefficient, tolerance, named-offset and fit bounds before calculation.
Saturating arithmetic is declared explicitly; validated limits bound every
intermediate far below i32 limits, so accepted calculations remain exact.

From the repository root, using a compiler built from this exact base:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
OMEGA=/absolute/path/to/verified/omega
SAMPLE=samples/cli/independent/dimensional_stack_audit
"$OMEGA" update --project "$SAMPLE" --target linux_x86_64
# Read the pending review and accept only the required documented package plan.
# Resume only if that acceptance was authorized; a denial is a blocker.
"$OMEGA" --check --target linux_x86_64 "$SAMPLE/main.omg"
"$OMEGA" run --both "$SAMPLE/main.omg" < "$SAMPLE/data/profile_3.stdin"
python3 "$SAMPLE/validate.py" --omega "$OMEGA"
"$OMEGA" update --project "$SAMPLE/controls/invalid_contract" --target linux_x86_64
# Review its local plan independently if requested.
python3 "$SAMPLE/check_invalid.py" --omega "$OMEGA"
```

`build.omg` binds Main::main explicitly on Linux x86-64, macOS ARM64 and Windows
x86-64. Other hosts have no declared entry binding. Generated products and
validation logs stay in ignored `build/`; local package policy is not copied
into the source delivery. The intended-invalid project is separate and complete:
its literal -1 violates nonnegative_tolerance's `requires value >= 0`.
Its expected diagnostic is pinned separately; unrelated rejection is not a pass.

`reference.py --freeze` was run before compiler validation. It enumerates all
64 joint endpoint combinations independently and freezes exact stdout, exit
codes and witness attributions. `ORACLE_FREEZE.md` records its immutable digest;
validation refuses changed oracle inputs. Numerical absolute and relative
tolerances are zero. `validate.py` runs one Omega invocation at a time and
compares every native byte and exit. `--both` uses the produced Terminal artifact;
the CLI compares termination statuses only, so AGREED is not evidence of
Terminal stdout equality. Its limitation is reported explicitly.
