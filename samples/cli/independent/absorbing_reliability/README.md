# Absorbing equipment reliability (IND03)

This complete application evolves healthy, degraded and failed equipment. It
prints exact state probabilities, survival P(T>t), absorption P(T<=t), and
E[min(T,H)] for a finite horizon H. Failure is absorbing; time T is the first
failed-state visit, with T=0 for initially failed equipment. This is a truncated
lifetime, not an assertion that eventual absorption is certain.

The first stdin line must contain exactly two ASCII digits and LF: variant then
horizon (0..8). The application processes that line and ignores trailing input.
Variants: 0 healthy degradation, 1 repair, 2 unreachable failure, 3 initially
failed, 4 deterministic failure after two steps, 5 initially degraded. All
transition rows use quarters, including the failed row (0,0,4). The full matrices
are in `reference.py`; the private Omega scalar implementation is in
`domain/process.omg`. No numerics library or worker dependency is used.

Each row is `step healthy degraded failed survival denominator`. Probability
columns are integer numerators with the last column as shared denominator.
The final row is `expected_min_lifetime numerator denominator`. Zero is printed
as `0`, not as missing data. Numerators are deliberately unreduced.

For example input `04\n` gives final probabilities healthy=16/256,
degraded=32/256, absorption=208/256, survival=48/256, and truncated lifetime
656/256=41/16. Repair variant 1 has survival (3/4)^t and lifetime
4*(1-(3/4)^H). Variant 2 has survival 1 forever and truncated lifetime H;
its untruncated lifetime is infinite. Variant 3 stays failed and has lifetime 0.
Variant 4 has lifetime min(H,2). Variant 5 has survival (1/2)^t and lifetime
2*(1-(1/2)^H). Variant 0 has survival (1+t/2)/2^t; its untruncated lifetime is 3.

The private arithmetic is exact. With H<=8, denominator<=65536 and lifetime
numerator<=8*65536. Intermediate distribution products and formatter divisors
fit u64. Wrapping storage does not wrap in the accepted domain. Healthy,
degraded and failed numerators evolve independently; runtime conservation is
checked before each output row. Frozen reference tolerance is exactly zero.
The Fraction reference multiplies the full matrix, independently of the Omega
scalar recurrence. Expected fixture files were frozen before Omega execution.

Closest existing samples were simulation/random_walk (random spatial walk),
simulation/traffic_light (discrete phase cycle), and arithmetic/stats_compute
(statistics over fixed data). None models probability-mass evolution, absorbing
states or survival-sum lifetime. This sample adds that complete workflow.

From the repository root, select the matching library and compiler:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
omega update --project samples/cli/independent/absorbing_reliability --target linux_x86_64
# Read and accept the generated review, then omega update --resume --project ...
omega --check --target linux_x86_64 samples/cli/independent/absorbing_reliability/main.omg
omega --target linux_x86_64 samples/cli/independent/absorbing_reliability/main.omg
printf '04\n' | omega run --both --keep samples/cli/independent/absorbing_reliability/main.omg
python3 samples/cli/independent/absorbing_reliability/reference.py
omega --check samples/cli/independent/absorbing_reliability/intended_invalid/main.omg
```

The last command must reject the complete source control because 5 is outside
its declared probability-weight range 0..4. Runtime malformed inputs exit 64;
valid cases exit 0; conservation failure exits 70. Data includes EOF, truncated,
overlong, CRLF, bad variant, bad horizon and nondigit controls. Compiler output
and retained execution evidence belong in ignored `build/`.
