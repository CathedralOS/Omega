# Captured supplier through match join transport

Fact: A default trait member keeps an alias's captured const supplier separate from its same-named fresh invocation binder while forwarding the same array value.

This place carries `Padded<3>` through a match join of alias and explicit-array values, executing both boolean branches before
`Selected<3>::forward<17>`. Declaration `Bias=1` remains separate from caller
`Bias=9`, and the same array retains its 17/42 observations. The sole twin
changes `Selected<3>` to `Selected<4>` and rejects required extent 5 against
actual 4. Programs and exact rejection fragments were frozen before execution.

[Run 41366197763](https://github.com/CathedralOS/Omega/actions/runs/41366197763)
at source `a1964f50142cfb4810e3b0a69a60046a64f2c69b` passed this sample pair
and its canonical mirrors: source checked, native exit 0, Terminal exit 0,
agreement, and the intended twin rejected. Execution was hosted Linux x86-64;
checking rows retain their actual default check target. The optimized CLI
SHA-256 is `22e399f99016b9e33132d7a45f05a0c92d6a14e8bbffbbb69b4d5d2b44918850`,
runner `9b02a8b5c10653724cae71154888a9f1aa451254a448b137178274f3b5177ace`,
opt1 under pinned Rust `a69a63265`. Shared preparation took 1128.865s,
authentication 1.780s, checking 12.157s, canonical execution 9.540s and
samples 26.979s. The fifteen-minute checkpoint retained this one run;
no compiler changes or extra checks were needed.

The previously qualified local, field and parameter places were excluded.
Raw reports, logs, bundles and frozen sources remain outside scratch at
`/Users/zcanann/Omega-build/alias-supplier-result-join-20261101/`.
This adds one transport place and does not complete the nine-place row.
