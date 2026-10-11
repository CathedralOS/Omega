# Intended-invalid bounds control

This complete application selects `values[2]` from a `[i32; 2]`. The documented
collection selection rule requires 0 <= index < length. Length 2 excludes index
2. After its separate package preparation, run `omega --check main.omg` here.
Exit 1 plus the exact diagnostic in `expected.txt` is the intended rejection.
Any package, custody, entry-selection, type or unrelated error is not a pass.

The control remains a separate source application and is never imported by the
valid pendulum entrypoint. This prevents its compile-time error from masking
runtime input validation.
