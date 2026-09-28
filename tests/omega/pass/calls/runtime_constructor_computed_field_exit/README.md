# Computed record constructor result

`Pair8::make(3500)` returns `{ a: 35, b: 7 }`; the caller stores the returned
record and checks both fields. Success exits 1; a wrong field exits 2.
`expected_native_exit.txt` makes the corpus gate enforce that result before
comparing or recording its golden.

After local package-review preparation, run from the repository root:

```sh
target/release/omega run --both tests/omega/pass/calls/runtime_constructor_computed_field_exit/main.omg
```

Both native execution and the checked interpreter must report exit 1.
