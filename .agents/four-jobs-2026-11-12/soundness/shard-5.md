# Soundness mutations shard 5 (PIN 0835b3aaf803ed97f905be42d94727ff58459263)

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`
Host: cursor
Shard selection: fixture index `i % 8 == 5` over pass/ and run/ (`5` fixtures).

## Counts
- fixtures in shard: 5
- mutants generated: 17
- mutants checked: 17
- both checkers reject (discarded): 16
- kept candidates: 1
- confirmed-invalid holes/disagreements: 1
- discarded-valid: 0
- discarded-no-rule: 0

## Commands
```bash
cd /agent/repos/omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison
OMEGA_LIBRARY_ROOT=$PWD/source/library timeout 120 ./target/debug/omega --check <mutant>/main.omg
timeout 120 ./target/debug/examples/route_one_comparison <mutant>/main.omg
python3 .agents/four-jobs-2026-11-12/soundness/run_shard5.py
```

## Confirmed holes and disagreements

### `drop_requires` — `tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg`
- Original: `tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg`
- Mutant: `.agents/four-jobs-2026-11-12/soundness/shard-5/tests_omega_pass_arithmetic_bitwise_singleton_signed_and_unsigned_exit_main_omg_0b7021d138__drop_requires`
- Current checker: **accepted**; route: **rejected**
- Spec rule (wiki/spec/language/state_contracts.md):
> Machine `requires` applies at every invocation, including terminal and recursive calls. Every arrival establishes the target's requirements and declared domains under actual argument substitution.
