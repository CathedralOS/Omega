# Steady-state metabolic flux optimization

Maximize one designated biomass reaction, subject to S*v=0 and independent
reaction lower/upper bounds. The program reports an optimal feasible vector,
every metabolite balance residual, and each reaction's minimum/maximum on the
entire optimal face. Alternate pathways may yield many equally valid vectors;
validation compares objective/ranges and feasibility, not an arbitrary optimum.
This is flux optimization, rather than integer batch balancing.

Supply finite whitespace-separated decimal input and close stdin:

```
metabolites reactions biomass_reaction_index
S row 0 ...
S row 1 ...
...
reaction_0_lower reaction_0_upper
reaction_1_lower reaction_1_upper
...
```

There are 2–4 metabolites and 3–6 reactions. The biomass reaction index is zero
based. S contains integers in [-3,3]. Bounds satisfy 0 <= lower <= upper <= 10.
ASCII space, tab, CR and LF separate tokens. Decimal tokens may have an optional
minus sign and at most nine fractional digits and thirteen total digits; a point
requires digits on both sides. Input must be shorter than 4096 bytes. NaN,
infinity, exponent notation, unsupported dimensions, invalid indices,
nonintegral coefficients, invalid bounds, malformed/extra/missing tokens and
input over the byte/digit budgets print `ERROR input` and exit 10. A valid but
infeasible system prints `INFEASIBLE` and exits 11. A feasible system starts with
`OPTIMAL` and exits 0. Internal bounds/division failures retain typed Trap.

Output is `objective_pico`, then `flux_pico`, `minimum_pico`, `maximum_pico`
for each reaction in input order, then `residual_pico` for each metabolite.
Divide each signed integer by 1e12; output truncates toward zero. Residuals are
computed from the selected vector's exact rational balance equations, not a
literal success constant. The printed rounded vector is independently checked
for scaled residual <=1e-8; objective and face ranges require agreement <=1e-7.

The solver parses bounds exactly in integer nanoflux units. It enumerates all
3^n free/lower-fixed/upper-fixed assignments. Full-column-rank free systems are
solved by fraction-free Bareiss elimination with runtime-checked exact divisions.
All original rows and all bounds are then checked exactly, including rows
omitted by rank deficiency. Rational cross products select the maximum biomass
and ranges across all EXACTLY optimal vertices. No floating objective tolerance
can mix a nearby suboptimal vertex into the optimal face.

Every vertex has independent bound-active coordinates completing the rank of S;
thus its remaining free columns are independent, and it appears in this
enumeration. A nonempty bounded polytope has vertices. A linear objective and
any reaction projection attain extrema at vertices; vertices of the exposed
optimal face are vertices of the original polytope. These facts justify both
infeasibility detection and the optimal-face ranges, including degeneracy.

For arithmetic safety, a coefficient minor of order k<=4 is bounded by
(3*sqrt(k))^k: at most 1296 overall, and at most 141 for order 3. An assembled
right-hand side is bounded by 6*3*1e10=1.8e11 nanounits. Expanding an augmented
minor in its right-hand-side column gives <=4*1.8e11*141=1.0152e14.
Bareiss update products/differences therefore stay below 2.7e17; back-substitution
products and three-term sums stay below 5.4e17. Rational comparisons stay below
1.4e17 and balance sums below 1.9e15. All fit signed i64. Counter updates are
bounded by 729 candidates, four rows and seven columns. Explicit Wrapping
carriers therefore never actually wrap on admitted input. Exact divisions check
nonzero denominators and zero remainder before dividing. Only final decimal
output uses binary64 conversion; feasible printed magnitudes remain <=1e13.

From repository root, using the documented environment setup and its authorized
execution route:

```
omega update --project samples/cli/independent/metabolic_flux_analysis --target linux_x86_64
omega run samples/cli/independent/metabolic_flux_analysis/main.omg < samples/cli/independent/metabolic_flux_analysis/input.txt
python3 samples/cli/independent/metabolic_flux_analysis/validation/validate.py --omega /absolute/path/to/omega --route all
```

The validator requires scipy 1.17.0 for its independent HiGHS comparison and
Fraction certificate recomputation. Frozen expectations predate sample execution.
It reads both actual native output and `terminal.stdout` retained by
`omega run --both --keep` from that run's exact Terminal product. It keeps source,
compiler and oracle hashes, commands, exits and numerical output under
`build/validation/all`. The separate `controls/reaction_access` project must
reject a constant out-of-range access at source checking. Entry bindings are
explicit for Linux x86-64, Windows x86-64 and macOS ARM64; execution evidence
names the actual host, and does not claim cross-host execution.
