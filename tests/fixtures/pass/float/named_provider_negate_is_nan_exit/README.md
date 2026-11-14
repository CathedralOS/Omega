# Named negation and NaN classification

From the repository root, settle the local standard-library dependency (edit
the `pending` decisions in the review the first command names to `accept`)
and run:

```sh
target/release/omega packages update --project tests/fixtures/pass/float/named_provider_negate_is_nan_exit --target macos_arm64
target/release/omega packages update --resume --project tests/fixtures/pass/float/named_provider_negate_is_nan_exit
target/release/omega run --both tests/fixtures/pass/float/named_provider_negate_is_nan_exit/main.omg
```

Expected native exit: **70**. On Windows use the built
`target/release/omega.exe`, selecting `windows_x86_64` during preparation.

Binary32 and binary64 named negation preserve signed zero and negate infinity.
Named `is_nan` returns a Boolean consumed by ordinary stores and transitions;
NaN is true, while finite values and infinity are false. The unchanged source
also exercises surrounding division, comparisons, and nested state dispatch.

The checked interpreter currently declines the selected bodyless float provider;
`--both` must report that limitation rather than claim interpreter agreement.
