# Acquired dependency and completed output set

The first Build gives the second Build access to the generator package. The
second Build checks the generator’s value and reads six captured bytes from
`template.txt`. A local helper uses the bytes to produce a companion file and
emits an Omega declaration. The Build completes both files as one output set.
The application calls its registered dependency and checks that it returns 7.

Run this command from the fixture directory. Use `--engine native` to run the
same program natively.

```sh
omega run --engine psi \
    --build-input build.omg --build-input outputs.omg --build-input main.omg \
    --build-input build_support --build-input generator --build-input template.txt build.omg
```

The program returns with empty output. Running `omega build --build-dir out`
with the same input arguments exports exactly `companion.txt` and
`generated.omg`. Product source comes from its selected package.
