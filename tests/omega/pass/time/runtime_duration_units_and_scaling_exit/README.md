# Duration conversion and scaling

This program checks millisecond, microsecond and nanosecond construction,
including `u64::MAX` milliseconds. It then checks multiplication with carry,
zero and overflow, division with a fractional second, and the zero-divisor
result. Every result must match the authored values; success returns exit 0 in
both native and Terminal execution. It reads no host clock.

The existing `runtime_checked_time_arith_exit` remains the exit-70 witness for
Instant/SystemTime carry, borrow, signed epoch arithmetic and overflow.
`instant_constant_unnormalized` and `system_time_constant_unnormalized` reject
unused invalid constants. `instant_mutation_leaves_invalid_return` attempts to
return a timestamp after writing 1,000,000,000 nanoseconds; the current checker
refuses that invalid store before it can escape.

```sh
omega update --project tests/omega/pass/time/runtime_duration_units_and_scaling_exit --target macos_arm64 --offline
omega run --both tests/omega/pass/time/runtime_duration_units_and_scaling_exit/main.omg
python3 tools/corpus_gate.py --filter /time/
python3 tools/corpus_gate.py --both --filter time/runtime_duration_units_and_scaling_exit,time/runtime_checked_time_arith_exit,time/runtime_duration_core_exit
```

For another supported host, select that host's target during package review.
Clock programs require matching native host providers; pure arithmetic agrees
between the two engines without a foreign-clock implementation.

The unchanged real consumer `samples/cli/systems/elapsed_timer` requests 50 ms
through `Time::sleep_for`, reads `Time::elapsed_since`, prints the measured
milliseconds, and exits 70 only when the request and elapsed bounds hold:

```sh
omega run samples/cli/systems/elapsed_timer/main.omg
```

`runtime_fs_mtime_system_time_interop_exit` separately executes
`Time::system_time_now` against a real filesystem timestamp on macOS ARM64.

## Historical declaration inventory

All fifteen positions below carry these exact suffix bounds at `c1303f89d4` and
retain them at the migration base `8dd8358e4c6`. Receivers became explicit
parameters between those revisions; the scalar obligations did not change.
The old clock-subsecond initializer was later explicitly narrowed to `u32`,
without changing its remainder or bound.

| Position in `source/library/std/time.omg` | Original suffix | Current establishment |
| --- | --- | --- |
| `Duration::from_milliseconds.scaled_wide` | `u64 [0..=999000000]` | Plain `u64`; `(milliseconds % 1000) * 1000000` |
| `Duration::from_milliseconds.sub` | `u32 [0..=999999999]` | Plain `u32`; `scaled % 1000000000` |
| `Duration::from_microseconds.scaled_wide` | `u64 [0..=999999000]` | Plain `u64`; `(microseconds % 1000000) * 1000` |
| `Duration::from_microseconds.sub` | `u32 [0..=999999999]` | Plain `u32`; `scaled % 1000000000` |
| `Duration::from_nanoseconds.remainder_wide` | `u64 [0..=999999999]` | Plain `u64`; `nanoseconds % 1000000000` |
| `Duration::from_nanoseconds.sub` | `u32 [0..=999999999]` | Plain `u32`; `remainder % 1000000000` |
| `Duration::checked_multiply.nano_remainder_wide` | `u64 [0..=999999999]` | Plain `u64`; `nano_product % 1000000000` |
| `Duration::divide.sub_quotient_bounded` | `u64 [0..=999999999]` | Plain `u64`; `sub_quotient_wide % 1000000000` |
| `Time::now.subsecond` | `u32 [0..=999999999]` | Plain `u32`; remainder below 1,000,000,000, then value narrowing |
| `Time::system_time_now.subsecond` | `u32 [0..=999999999]` | Plain `u32`; the same remainder and narrowing |
| `Time::system_time_now.unix_bounded` | `u64 [0..=9223372036854775806]` | Plain `u64`; remainder modulo 9,223,372,036,854,775,807 |
| `Time::elapsed_since.now_subsecond_wide` | `u64 [0..=999999999]` | Plain `u64`; `subnanos_wide % 1000000000` |
| `Time::sleep_for.capped` | `u64 [0..=4294967295]` | Plain `u64`; `clamp(total, 0, 4294967295)` |
| `Instant.subsecond_nanoseconds` | `u32 [0..=999_999_999]` | Plain `u32`; containing data `where subsecond_nanoseconds <= 999_999_999` |
| `SystemTime.subsecond_nanoseconds` | `u32 [0..=999_999_999]` | Plain `u32`; the same containing-data predicate |

Unsigned carriers preserve every zero lower bound. Local declarations grant no
new fact: the unchanged initializer must establish the old upper bound needed
by narrowing or construction. No interval-only nominal domain replaces a suffix,
and this file's inventory does not close the repository-wide historical audit.
