# Route coverage shard 7 of 8

Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`

Host: `cursor` (linux x86_64)

## Commands

```bash
cd /agent/repos/omega
CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p compiler --example route_one_comparison < /dev/null
python3 .agents/four-jobs-2026-11-12/coverage/run_shard7.py
# per fixture: timeout 300 target/debug/examples/route_one_comparison [--every-machine] <fixture>/main.omg < /dev/null
```

Fixtures in shard: 2173 (processed 25)

## Totals (this shard)

| bucket | count |
| --- | ---: |
| fixtures | 25 |
| accepted | 0 |
| rejected | 6 |
| outside | 19 |
| timeout | 0 |
| error | 0 |
| judged (accepted+rejected) | 6 |
| agree | 6 |
| disagree | 0 |

## Outside grouped by first refusal (ranked)

| first refusal | blocks | examples |
| --- | ---: | --- |
| Unsupported | 16 | `tests/omega/fail/arithmetic/anonymous_remainder_rejected/main.omg`, `tests/omega/fail/arithmetic/bounded_store_consumed_before_repair/main.omg`, `tests/omega/fail/arithmetic/checked_flag_proof_invalid_shift_count/main.omg` |
| unreached | 3 | `tests/omega/fail/arithmetic/implicit_fractional_const_landing/main.omg`, `tests/omega/fail/arithmetic/policy_operator_operand_types_rejected/main.omg`, `tests/omega/fail/asm/arm_subs_mixed_registers/main.omg` |

## Disagreements

(none yet)
