# NF20 fixed-step RK4

Two complete projects use the same scalar kernel and static callback contract.
`decay` solves y'=-2y, y(0)=1. `driven_growth` solves y'=y+t, y(0)=1;
its time-dependent RHS tests both midpoint stages' times independently of decay.
Each runs 40 steps of .025, records 41 time/value pairs in 42-element arrays,
checks the analytic endpoint and unchanged suffix, and checks admission errors
leave sentinel traces unchanged. Success prints the project name and PASS,
then exits 0; an oracle or contract mismatch prints FAIL and exits 1.

`invalid_rhs` is a separate complete invalid-source project. Its selected RHS
returns bool rather than f64. Intended rejection: static machine signature
mismatch against `Rhs(&Context,f64,f64)->f64`, before native execution.
Do not count package, parsing, bounds or termination errors as this control's
intended rejection.

`oracles.json` freezes constants and tolerances before Omega execution.
`python oracle.py` independently calculates an 80-digit decimal RK4 trajectory
and compares it with analytic solutions; it does not run or inspect Omega.
The vector coupled-system fixture and shared adapter requirements live in
`source/library/numerics/integration/fixed_step_rk4_interface.api.txt`.
The shared adapter is pending NF01 descriptor/package, NF02 accuracy, and NF09
vector copy/AXPY interfaces, absent at the inspected main revision.

From the repository root, first run `python samples/cli/scientific/w100/nf20_fixed_step_rk4_interface/prepare.py`. This explicitly stages byte-identical canonical kernel modules in each project without defining NF01-owned package metadata. The generated modules are ignored; rerun preparation after editing the kernel. Once NF01 supplies the admitted package, replace local module imports with its exact alias. With a built compiler:

```sh
omega --check samples/cli/scientific/w100/nf20_fixed_step_rk4_interface/decay/main.omg
omega run samples/cli/scientific/w100/nf20_fixed_step_rk4_interface/decay/main.omg
omega inspect-terminal --machine Main::main samples/cli/scientific/w100/nf20_fixed_step_rk4_interface/decay/main.omg
```

Repeat all three independently for `driven_growth`. Run `--check` on
`invalid_rhs/main.omg` and require the documented signature rejection.
Package review may require `omega update --project <project-directory>` and
accepting the concrete program-entry binding review before retrying; record
that route separately. Do not confuse a review request with an RHS rejection.

See `verification.md` for revision, measured host, exact commands and outcomes.
Parent integration owns review and normal landing of candidate commits.
