# Synthetic instrument hysteresis

IND15 is a complete console application with private calibration, trace analysis,
parsing and reporting modules. It uses only the ordinary bundled std console
provider. No numerics package or active worker is a dependency.

Feed ASCII `stimulus response_milli` records, each terminated by LF, then EOF.
Stimulus is an integer in 0..100 and response is in 0..200000 milliunits.
Only unsigned decimal digits and exactly one space are accepted. CRLF, a sign,
blank lines, trailing characters and an unterminated final record are errors.
Input storage is 64 bytes per record; at most 256 records are accepted.
Empty input emits an empty report. Malformed input exits 64; range or row-limit
failure exits 65. Reports already emitted before an error remain available;
the rejected record never enters the trace.

Each row reports stimulus, inferred branch, predicted response, observed
response, signed residual and reverse-minus-forward hysteresis width.
The first row is ambiguous. A plateau retains the last nonzero direction;
plateaus before any movement stay ambiguous. A reversal uses the newly selected
branch immediately. Ambiguous rows emit `NA` prediction/residual and are excluded
from residual summaries. Branch summaries contain count, sum of absolute
residuals and maximum absolute residual. The final trace summary counts ambiguous
rows, plateaus and reversals and reports maximum sampled width.

`domain/calibration.omg` owns four nonlinear knots for each branch, also listed
in `data/calibration.csv`. Adjacent-knot linear interpolation is bounded;
there is no extrapolation. Responses are fixed-point integer milliunits, rounded
down after rational interpolation. Maximum interpolation product is 2500000;
maximum residual sum is 51200000, so the private Wrapping arithmetic never
actually wraps for accepted input. Table widths are positive by construction.
Direction comparison uses stimulus history, independently of noisy response.

The oracle was frozen **before Omega execution** in `ORACLE_FREEZE.json`.
`reference.py` uses Python `Fraction` independently of the Omega scanner and
reporting implementation. Report comparisons have tolerance **zero milliunits**;
each prediction differs from exact real interpolation by less than 1 milliunit,
and reported width differs from exact branch separation by less than 1 milliunit.
The frozen ideal loop has six forward and six reverse calibrated rows, zero
residuals, one reversal, and peak width 7000. The plateau/reversal trace has four
plateaus, three reversals, forward absolute sum 100 and reverse absolute sum 87.

Existing `arithmetic/dual_float`, `text/parse_int`, `text/format_number` and
`simulation/particle_sim` were compared before authoring. They demonstrate
arithmetic, parsing, formatting or evolving state; none identifies direction
from a calibration trace, applies separate nonlinear calibration branches,
handles ambiguous histories or computes branch residuals and hysteresis widths.

From the repository root, with a matching current-main compiler and library:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
OMEGA=/absolute/path/to/omega
SAMPLE=samples/cli/independent/instrument_hysteresis
"$OMEGA" update --project "$SAMPLE" --target linux_x86_64
# Inspect and accept the generated review decisions, then:
"$OMEGA" update --resume --project "$SAMPLE"
"$OMEGA" --check --target linux_x86_64 "$SAMPLE/main.omg"
"$OMEGA" run --both "$SAMPLE/main.omg" < "$SAMPLE/data/loop.stdin"
python3 "$SAMPLE/validate.py" --omega "$OMEGA"
# Build the stable corpus runner once, then use its reported executable path:
mbx test --release -p compiler --test corpus_runner --no-run
python3 "$SAMPLE/terminal_validate.py" --runner /absolute/path/to/corpus_runner
```

`validate.py` first verifies frozen hashes and recomputes reference results,
checks the source, compiles one native executable, then runs all fixtures
sequentially with exact stdout/exit assertions. It runs Terminal/native
comparisons and verifies separate intended-invalid source rejection. Results,
commands, elapsed times, stderr and native stdout are retained under `build/`.
The current public `--both` contract compares termination only; its native
stdout is checked against the frozen oracle. It does not expose Terminal stdout,
so termination agreement does not claim a byte-for-byte Terminal output check.
`terminal_validate.py` separately uses the stable corpus runner with one job and
the Terminal engine to assert exact stdout, empty stderr and exit for eight
frozen traces/invalid inputs. It stages byte-identical domain, input, reporting
and entry modules in temporary corpus fixtures; only the local std build path
changes. It verifies copy hashes and removes those fixtures afterward. Its
records remain under `build/terminal_validation/`; it never modifies goldens.
The invalid control must fail for the authored value-101 invariant violation,
not merely for missing package review or another infrastructure failure.

All source remains under this namespace. Generated products and accepted local
package locks belong in ignored `build/`/local state, never source inputs.

This main-based candidate also contains two bounded compiler repairs required
by the unchanged application: retained storage roots through dynamic view cuts
(stage 03) and validated interval recovery when call-survival and a fixed-use
view have no common home (stage 05). No source pattern, function identity or
weakened custody/legality check is introduced. The focused regressions are
`runtime_cut_byte_view_cycle_exit` and `call_surviving_return_exit`; their
observations and the original refusal logs belong in the review bundle.

Independent review required retained producer-chain traversal as well. The
chained-cut fixture exposed an earlier state-graph handoff dropping established
locals; it now uses the existing contextual-bindings builder at that join.
Both missing joins were reproduced on the integrated main compiler without
weakening custody checks. The complete application source and frozen numeric
oracles remain unchanged. Final bundle evidence identifies the integration
base, exact compiler and runner hashes, commands and acceptance results.
