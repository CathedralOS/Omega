# Pump network operating point (NEW079)

This complete Omega CLI solves a declared synthetic pump and valve model; it is
an arithmetic sample, not a sizing tool for real equipment. It owns its input
parser, root isolation, rational arithmetic and report formatting. It imports
only the existing standard console provider and core numeric conversion.

Input is one line (or an EOF-terminated record) of nine signed decimal integers:

```
a b c d r1 r2 elevation maximum demand
20 0 1 1 2 2 0 8 0
```

The supplied pump curve is `Hp(Q)=(a+b Q-c Q²)/d`. The two parallel linear valve
curves are `r1 q1=r2 q2=H-elevation`, with `Q=q1+q2`. Thus the network curve is
`H=elevation+Q*r1*r2/(r1+r2)`. Linear valve resistance is an explicitly chosen
scientific sample model, rather than an assumed turbulent-flow law. Flow and
head units are synthetic. Search covers `0 <= Q <= maximum`; `demand` is the
minimum acceptable total flow. Every isolated operating point is reported,
including ones below demand. No physical stability claim is attached to roots.

Bounds: `a,elevation` in 0..64, `b` in -16..16, `c` in 0..16, `d,r1,r2` in
1..16, `maximum` in 1..16, and `demand` in 0..16. Negative `c`,
zero resistance, zero denominator, out-of-range values and the identically
coincident curves (a continuum, not an isolated operating point) reject with
exit 64. The parser consumes at most 96 bytes, allows at most nine tokens and
four decimal digits per token, accepts spaces and LF/EOF, and rejects signs
without digits, extra tokens and other bytes. Leading zeros count toward the
magnitude bound; a long line still reaches the independent byte budget.

The private solver exhausts brackets of width at most 1/16 and bisects each
sign-changing bracket to one tick of 1/1024. Endpoint roots are emitted once.
The exact discriminant distinguishes a tangent root from no intersection.
For a quadratic, a vertex split prevents two simple roots sharing an original
bracket from disappearing. Integer coefficients give nonzero discriminant at
least one, and quadratic magnitude at most 512, so distinct roots are at least
1/512 apart. Flooring the vertex to the tick grid therefore leaves it between
the roots. Non-grid tangencies use a floor/ceil enclosure. Model bounds keep
intermediates below 2^40; the explicitly Wrapping i64 carriers never overflow
for accepted inputs. No floating-point arithmetic or external numerics package
is used.

Each `ROOT` row contains these twelve integers:

| Positions | Meaning |
| --- | --- |
| 1, 2 | Lower and upper flow ticks, divided by 1024 |
| 3, 4, 5 | Branch-flow numerators q1, q2 and their common denominator |
| 6, 7 | Network-head numerator and denominator |
| 8, 9 | Pump-head numerator and denominator |
| 10, 11 | Energy residual Hp-H numerator and denominator |
| 12 | 1 if demand is met, 0 otherwise |

All reported quantities are evaluated at the lower tick; the enclosure bounds
the actual total-flow error by 1/1024. All report fractions are exact. The frozen
head residual tolerance is 800/1024, conservatively exceeding the maximum head
derivative 536 times the flow tolerance. `SUMMARY` gives the root count and
feasible-root count. Exit 0 means at least one feasible point; exit 2 and
`INFEASIBLE` mean no feasible point; exit 64 and `INVALID_INPUT` mean bad input.
For the example, Q=4, q1=q2=2, Hp=H=4, energy residual zero.

Existing samples were compared before implementation: `arithmetic/stats_compute`
accumulates fixed floating data, `basics/number_guess` has a scripted search,
`algorithms/bubble_sort` exercises mutable indexed arrays, and
`text/parse_int` parses one pre-authored integer. None performs network
intersections, exhaustive bracket coverage, tangency detection, demand
classification or branch/head/energy reports.

## Frozen oracle and execution

`data/expected.json` contains exact expected stdout and exit codes for 76 cases,
including all 48 combinations a,b in 0..3 and c in 0..2 (other coefficients
fixed), nontrivial rational/irrational/asymmetric/linear variants, multiple
roots, exact and fractional tangencies, shutoff, no intersection, unmet demands,
invalid model fields and malformed runtime records. `data/freeze.py` uses
independent Fraction equations and enumerates every 1/1024 interval rather than
the Omega coarse-bracket algorithm. `data/freeze.sha256` freezes its artifact
before compiler check or runtime execution. `validate.py` verifies the frozen
hash and compares every output byte, exit code and empty stderr; it never
regenerates expected results.

From the repository root, with a compiler built from the recorded base and
`OMEGA_LIBRARY_ROOT` set to this checkout's `source/library`:

```
omega --check --target linux_x86_64 samples/cli/independent/pump_network_operating_point/main.omg
omega update --project samples/cli/independent/pump_network_operating_point --target linux_x86_64
```

Inspect every pending package review. Accept only if authorized, then use the
ordinary `omega update --resume --project ...` route. Package denial is a
remaining blocker, not permission to retry acceptance or bypass review.

```
omega run --both --keep samples/cli/independent/pump_network_operating_point/main.omg < samples/cli/independent/pump_network_operating_point/data/rational.in
omega inspect-terminal --machine main --target linux_x86_64 samples/cli/independent/pump_network_operating_point/main.omg
python3 samples/cli/independent/pump_network_operating_point/validate.py <published-native-executable>
python3 samples/cli/independent/pump_network_operating_point/terminal_matrix.py <verified-corpus-runner>
omega --check samples/cli/independent/pump_network_operating_point/controls/intended_invalid/main.omg
omega --check samples/cli/independent/pump_network_operating_point/controls/valid_extent/main.omg
```

The intended-invalid command must reject the fixed-array extent rule pinned in the control's
`expected.txt`; rejection for another reason is not acceptance. It has its own
complete root, takes no runtime input and is never imported by the application.
The positive extent control must check successfully. The Terminal matrix stages
unchanged application modules temporarily under the repository test tree and
uses its documented fixture acceptance policy. It requires all 76 authored
native and Terminal outputs and exits; it does not rewrite any corpus golden.
Supporting immutable evidence and command results belong in `evidence/`;
generated artifacts belong in ignored `build/`. `evidence/assignment.txt` retains
the isolated scope and base. See `evidence/validation.md` for actual route results.
