# Invocation-specific operator crash routes

The boundary `==` on `i32` declares `crashes Trap !(right >= 0)`. `safe`
discharges that route with its own `requires value >= 0`; `may_crash` leaves it
uncovered and `wrapper` covers it by declaration. `Main::main` invokes both
with operands that keep every route dormant, observes both `true` results, and
exits 0; a wrong observation traps.

From the repository root, using the locally built compiler on macOS ARM64:

```sh
omega update --project tests/omega/pass/operators/crash_routes --target macos_arm64
omega run --both tests/omega/pass/operators/crash_routes/main.omg
```

On Linux x86-64, use `--target linux_x86_64` for package update, then run the
same comparison command on that host. The target on `update` prepares review
for that host; it does not claim that another host executed the program.

The package has no external dependencies or authority choices. Package update
prepares its local acceptance baseline. Native and interpreted exits must both
be 0. The negative fixture `fail/operators/selected_crash_invocation` leaves a
float `==` Trap route uncovered and must reject.

The registered matching-host gate is:

```sh
python3 tools/corpus_gate.py --both --filter operators/crash_routes
python3 tools/corpus_gate.py --filter fail/operators/selected_crash_invocation
```

The native exit oracle is authored in `expected_native_exit.txt`; a checked-only
result does not satisfy it. The positive path covers discharged and propagated
contracts on non-crashing invocations, not execution of a crash or complete
invocation-outcome trace refinement.
