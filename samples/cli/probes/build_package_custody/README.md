# A generated package under two aliases

The preparing Build supplies a package to the next Build as `kit`. That
Build calls `|kit|::value()`, then registers the same source as
`product_kit` for the application. Both consumers check the result is 7.

The dependency has its own preparing stage. It supplies its nested
`generator` package to the Build that publishes its source. That Build
checks the generator's value before opening its own input and output files.
The generated wrapper, its handwritten seed, and the receiving Build entry
all travel in one frozen package directory. The generator's own source stays outside that directory.

Run `omega run build.omg`.
To check targeting, run `omega --check --target linux_x86_64 --target macos_arm64 build.omg`.

The [negative cases](../build_package_custody_controls/) cover an executable
binding on an artifact-only application, a forged output obligation, an
unsealed writer, and an unresolved dependency whose private output must
never be published.
