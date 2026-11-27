# Three generated sibling packages

The application imports three packages as `a`, `b`, and `c`. Their generated
functions call their own authored seeds, yielding 10, 20, and 30. The
application checks that the sum is 60.

Each sibling's first Build supplies the [shared package](../build_sibling_checks_support/)
to its generating Build. All three check the shared seed before copying their own
templates. Each sibling seals its output and selects its own directory for the
receiving Build. The seed is a dependency of the three Builds and stays out
of the application.

Run `omega run build.omg`.
To check both target profiles, run `omega --check --target linux_x86_64 --target macos_arm64 build.omg`.

The [negative cases](../build_sibling_checks_controls/) cover a malformed
middle sibling, alias collisions, Build and Product scope isolation, and
incorrect package roles. The malformed-sibling case requires the diagnostic
to identify `bad_sibling`.
