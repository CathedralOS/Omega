# Acquired dependency and completed output set

The first Build supplies the generator and the installed system filesystem to
the second Build. The filesystem receives the content-read, content-write,
metadata-mutation and namespace-mutation authorities used by its two operations.

The second Build reads the first six captured template bytes, checks the acquired
generator returns 7, and uses a local helper to generate a declaration and a
companion file. It writes private staging files and completes them under the
public names `generated.omg` and `companion.txt`. It forwards the generator
to the application, which calls the package and checks its result.

Interpret the program from the fixture directory. To test the native image, compile
with the same Build inputs and launch the published executable.

```sh
omega run \
    --build-input build.omg --build-input outputs.omg --build-input main.omg \
    --build-input build_support --build-input generator --build-input template.txt build.omg
```

Run `omega build --build-dir out` with the same input arguments to export the
two completed files. Compare `files/companion.txt` with `template.txt` and
`files/generated.omg` with `public const GENERATED: u32 = 73;` followed by a
newline. A short template or a generator that returns another value fails both
obligations before publication.
