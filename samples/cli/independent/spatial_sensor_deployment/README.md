# Spatial sensor deployment (NEW070)

A complete bounded deployment optimizer on synthetic scientific data. Three
candidate sites and four weighted demand points use integer grid coordinates,
closed Euclidean disk coverage (`dx² + dy² <= radius²`), one capacity unit per
served demand, nonnegative site costs, and a deployment budget. There is no
claim about real sensor propagation, noise, or geographic distance.

The Omega application considers all eight site subsets. For each subset within
budget, it enumerates all 256 base-four assignments of demands to unserved or
site 0/1/2. Over-budget certificate rows report cost with zero score, feasible
count and witness code. It checks geometric
eligibility and site capacity, counts feasible assignments, and retains maximum
served weight. Global ranking maximizes weight, minimizes cost, then chooses the
smallest site bitmask; the smallest assignment code breaks assignment ties.
Selected sites may be idle. Output includes every subset's cost, best score,
feasible assignment count and witness code, the winning placement and tie count,
and each demand's assigned site (0 = unserved, 1..3 = zero-based site + 1).
A status of 0 certifies that the requested minimum coverage is infeasible under
the budget; it still emits the best available deployment and exits 0.

All arithmetic is exact. Tolerance is **zero**, including radius boundaries.
Private wrapping arithmetic carriers are used within fixed, tiny fixture bounds:
coordinates 0..9, radius 1..2, cost <=7, total weight 14, counts <=256. No numeric
library or other worker's code is imported. The root owns console authority;
search scratch is invocation-local and domain modules own the algorithm.

Input is one command byte, `0` through `7`, or EOF for case 0. Remaining stdin is
outside this one-command protocol. Any other first byte exits 64 with the frozen
error message. There is no closing pause or interactive dependency.

| Command | Variant |
| --- | --- |
| 0 / EOF | Baseline; remote demand unreachable |
| 1 | Minimum 14 is infeasible |
| 2 | Identical sites create tied optimal deployments |
| 3 | First site's capacity reduced to one |
| 4 | Redundant colocated site |
| 5 | Zero budget |
| 6 | Minimum 12 is exactly attainable |
| 7 | Radius boundary changed, budget 5 |

`data/cases.json` declares the model inputs. `data/fixtures.omg` is the authored
Omega fixture provider. `data/reference.py` independently enumerates Cartesian
assignments and site combinations; `data/expected.json` and `expected-*.txt` were
frozen before compiler execution (hashes in `evidence/oracle-freeze.sha256`). The
reference is validation support; `main.omg` is the actual runnable application.

Existing samples were compared before authoring: `sensor_min_max` computes
stream extrema, `wire_protocol` encodes packets, `maze_flood` computes shortest
paths, and `retained_deployment_targets` tests compilation targets. None solves
spatial placement with coupled capacities, weighted coverage, budget ranking and
exhaustive optimality certificates. This workflow is substantive new coverage.

From the repository root, with a compiler built from the same checkout:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
omega update --project samples/cli/independent/spatial_sensor_deployment --target linux_x86_64
# Read the produced review. Accept only its documented decisions, then resume.
omega update --resume --project samples/cli/independent/spatial_sensor_deployment
omega --check --target linux_x86_64 samples/cli/independent/spatial_sensor_deployment/main.omg
printf 0 | omega run --both --keep samples/cli/independent/spatial_sensor_deployment/main.omg
python3 samples/cli/independent/spatial_sensor_deployment/validate.py --omega /absolute/path/to/omega
```

`validate.py` checks exact native and Terminal output and exit for every variant
and invalid runtime input, plus the independent intended-invalid source control.
It records stdout, stderr, elapsed time and exit in local `build/validation/`.
Check `evidence/validation.md` for actual commands and route observations.

`controls/invalid/` is a separate complete checking project. It deliberately
holds two exclusive loans of one sensor in a record literal. Its required
rejection fragment is `` `loans` holds `main.sensor` as mutable more than once ``;
package, parsing or unrelated proof rejection does not pass. The rule is in
`wiki/spec/language/ownership.md`: exclusive loans must not alias. The control uses no foreign dependency or entry schema, so package acceptance
cannot substitute for its intended borrow rejection. Its source is `invalid.omg`
to exclude it from the ordinary valid-sample harness. Controls are
not imported by the application.

Only linux_x86_64 is declared and tested. Generated products stay under `build/`;
source fixtures and expected output are reviewable tracked data.
