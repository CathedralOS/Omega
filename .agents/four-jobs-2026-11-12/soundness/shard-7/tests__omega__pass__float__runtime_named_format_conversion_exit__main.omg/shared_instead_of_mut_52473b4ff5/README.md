# Named format conversion and repeated classification

From the repository root:

```sh
python3 tools/benchmark/benchmark.py prepare --root tests/omega/pass/float/runtime_named_format_conversion_exit/main.omg --target macos_arm64 --omega target/release/omega
target/release/omega run --both tests/omega/pass/float/runtime_named_format_conversion_exit/main.omg
```

Expected native exit: **70**. In PowerShell use `python` and
`target/release/omega.exe`, selecting `windows_x86_64` during preparation.

The unchanged program distinguishes halfway rounding from a value above the
halfway point, checks exact widening and overflowing narrowing, and consumes two
named infinity-classification calls in the same Boolean expression. Classification
uses the existing unary IEEE operation route with a Boolean result; it must retain
each authored provider application rather than infer one from a shared target.

The checked interpreter currently declines this program with `unknown value-call
target from_f64`. `--both` must report that limitation, not claim interpreter
agreement.
