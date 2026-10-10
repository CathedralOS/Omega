# NF12 triangular solves checkpoint

Two distinct complete fixed-shape sample projects exercise NF12's independent
pivot operation. `lower_multiple_rhs` performs forward substitution on a
nonunit lower triangular matrix with two RHS columns, checks every solution
and residual, and proves a late zero pivot leaves RHS unchanged.
`upper_transpose_unit` solves the transposed upper system with two RHS columns,
then performs ordinary backward substitution and unit upper substitution with
stored diagonal zeros, and checks that negative zero returns `ZeroPivot`.
Neither is a cosmetic change of the other. Both print a PASS line and exit 0;
any failed numerical/status assertion prints FAIL and exits 1. No input pause.

The separate `invalid_unproved_pivot` project deliberately calls the checked
pivot operation with zero. Its intended rejection is an unfulfilled
`diagonal != 0.0` precondition, before any execution. A runtime `ZeroPivot`
status in a valid sample is success, not a source-rejection test.

All matrices and RHS are column-major explicit caller-owned backing arrays.
Sample-private three-row sweeps avoid inventing NF01's descriptor surface.
They are scientific customers, not the general rank-two solver release.
The reusable descriptor entrypoint and NF02 policy integration remain pending
actual NF01/NF02/NF05/NF10 signatures, detailed in
`source/library/numerics/linalg/triangular_solves.api.txt`.

The hand-authored `oracles.json` was frozen before Omega execution at SHA256
`91ca0766aedb08928308493e67643a8510236073786413342cb32569384d9679`.
Every expected solution is independently certified by exact rational A*X=B,
not by reimplementing substitution. Solution and residual absolute tolerances
are each 1e-12; relative tolerance is zero. Status and unchanged-output oracles
are exact. The eight fixtures also include lower-transpose and unit-lower
mathematical cases for the future reusable entrypoint; these two variants are
not claimed as Omega-executed here.

NF01 owns package metadata, so `prepare.py` explicitly copies the canonical
owned pivot source into disposable local projects under `.prepared/`. It copies
authored main/build sources and makes only the std dependency path absolute.
This is a temporary qualification adapter, not a second numerics package.
Once NF01 metadata arrives, replace the adapter with its real dependency edge.

From the repository root, prepare and certify:

```sh
python samples/cli/scientific/w100/nf12_triangular_solves/verify_oracles.py
python samples/cli/scientific/w100/nf12_triangular_solves/prepare.py
```

For each valid prepared project (`lower_multiple_rhs`, `upper_transpose_unit`),
prepare the std package review with `omega update --project <project> --target
linux_x86_64 --offline`, inspect and accept its pending ordinary std entry
bindings, and resume. Then qualify routes separately:

```sh
omega --check <project>/main.omg
omega run <project>/main.omg
omega run --both <project>/main.omg < /dev/null
```

The last route uses Terminal Psi comparison; it must report agreement rather
than a declined interpreter. Native success does not certify Terminal support.
Use `omega --check <invalid-project>/main.omg` for the invalid source and require
an actual precondition diagnostic; a cache, review, timeout or syntax failure
cannot satisfy the rejection oracle.

Portable exact-command recording (no automatic review acceptance):

```sh
python samples/cli/scientific/w100/nf12_triangular_solves/qualify.py --omega /absolute/path/to/omega
```

Evidence includes revision, host, full argv, elapsed time, raw stdout/stderr,
exit/timeout, source hashes, and route classification. Source snapshots are kept
beside failures. The adapter does not alter system ownership, permissions or
cache custody checks.

Additional bounded evidence: the pivot library passed standalone source check;
the unchanged invalid main rejected directly for its nonzero precondition.
Source-only companions of both customers also passed checking, with Console
imports/binding removed and failed assertions expressed as Trap. Their full
sources and exact commands are in evidence/source-only-checks/. These are
partial source checks, not successful hosted, native or Terminal routes.
