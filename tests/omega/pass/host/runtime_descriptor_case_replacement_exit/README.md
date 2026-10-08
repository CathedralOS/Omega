# Original case storage across descriptor calls

On macOS ARM64:

```sh
omega run --both tests/omega/pass/host/runtime_descriptor_case_replacement_exit/main.omg < tests/omega/pass/host/runtime_descriptor_case_replacement_exit/input.txt
```

The authored binary input retags Full to Other, replaces it with Empty, then
constructs Full again from complete incoming bytes. The caller observes the
new active payload, and its containing record's prefix/suffix survive. Forwarded
exclusive loans retain the original case home. Shared output checks the exact
native tag and payload bytes before and after replacement; the stdout oracle
includes four NUL tag bytes before each A/newline. EOF leaves valid contents intact.

The fixture's source program exits zero. Its input and output files contain raw
binary bytes; they are not UTF-8-only text contracts.
