# Named negation and NaN classification

From the repository root, settle the local standard-library dependency and run:

```sh
python3 tools/benchmark/benchmark.py prepare --root tests/omega/pass/float/named_provider_negate_is_nan_exit/main.omg --target macos_arm64 --omega target/release/omega
target/release/omega run --both tests/omega/pass/float/named_provider_negate_is_nan_exit/main.omg
```

Expected native exit: **70**. In PowerShell use `python` and the built
`target/release/omega.exe`, selecting `windows_x86_64` during preparation.

Binary32 and binary64 named negation preserve signed zero and negate infinity.
Named `is_nan` returns a Boolean consumed by ordinary stores and transitions;
NaN is true, while finite values and infinity are false. The unchanged source
also exercises surrounding division, comparisons, and nested state dispatch.

The checked interpreter currently declines the selected bodyless float provider;
`--both` must report that limitation rather than claim interpreter agreement.
