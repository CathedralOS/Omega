# Float digit separators at runtime

The program returns normally (exit 0) only when grouped whole, fractional and
exponent digits, leading-dot literals, signed exponents, and both IEEE formats
match their ungrouped values. A grouped hexadecimal integer is unchanged. The
binary32 midpoint witness must round directly from its exact decimal rational;
rounding through binary64 first changes the answer. A mismatch traps.

The program has no package dependency or service authority. Public acceptance:

```sh
omega update --project tests/omega/pass/float/runtime_float_digit_separators_exit --target linux_x86_64
omega run --both tests/omega/pass/float/runtime_float_digit_separators_exit/main.omg </dev/null
```

Use the matching host target in the update command on other supported hosts.
Corpus coverage:

```sh
python3 tools/corpus_gate.py --both --filter runtime_float_digit_separators_exit
python3 tools/corpus_gate.py --filter digit_separator
```

The negative fixtures preserve format-suffix disagreement, missing exponent
and unknown suffix diagnostics. This fixture exercises machine-body literal
payloads. The anonymous numeric const-argument evaluator has a separate
raw-syntax decimal path; this fixture does not establish that route's support.
