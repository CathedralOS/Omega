# rational_arg_landing

First sample landing anonymous-vs-typed comptime
arithmetic at call arguments: `7 / 2 * 2` stays an exact
rational and lands at 7; `7i32 / 2 * 2` truncates through
i32 and lands at 6; anonymous arithmetic past u64::MAX
wraps exactly.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
