# Computed scalar contract bounds

This program checks arithmetic and nested function calls used as scalar bounds.
It combines four results into 275, exits 70 when they agree, and exits 71 on a
mismatch. The generic `Wrap` remains an ordinary data carrier.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. The historical migration
`818f643f12d` introduced `Bound` solely to replace a range annotation inside
`Wrap`'s type argument. The wrapper now carries bare `u64`. Its initializer
establishes seven, and the private `wrapped` helper states the computed bound in
an ordinary result contract. No nominal domain identity is needed.

| Declaration before that migration | Current contract |
| --- | --- |
| `nested_sum.value: u64 [0..=1 + 2 * 3]` | Input requires `value <= 1 + 2 * 3`. |
| `nested_difference.value: u64 [0..10 - 2 - 3]` | Input requires `value < 10 - 2 - 3`. |
| `nested_call.value: u64 [0..=endpoint(endpoint(256) + 1)]` | Input retains that computed upper bound in `requires`. |
| `Holder.value: u64 [0..=4 * 8 - 2]` | The containing data requires `value <= 4 * 8 - 2`. |
| `held: Wrap<u64 [0..=2 * 64 + 128]>` | Bare `Wrap<u64>` receives the initializer's facts; `wrapped` ensures its result is at most `2 * 64 + 128`. |

Unsigned carriers retain every zero floor. The arithmetic, nested calls,
concrete values, and runtime oracle are unchanged. This reconciles these five
declarations, rather than claiming completion of the full historical audit.

The pre-migration source failed with compiler `3009cab5a41` while checking
constant arguments in the standard library's generic macOS function. With the
call-checking repairs, that same source runs with native and Terminal exit 70.
The migrated source also runs with native and Terminal exit 70 using the final
optimized compiler on macOS ARM64. The claim record identifies the source and
compiler used for each run.
