# Library packages for corpus fixtures

Corpus fixtures exercise cross-package behavior by depending on these
packages through `Source::Path`. The corpus runner treats a directory with a
`main.omg` as a fixture only under `pass/`, `fail/` and `run/`, so nothing here
is compiled on its own.

- `assert-kit`: an `Assert` requirement with a checking and a no-op provider
  behind the same Trap ceiling, used by the `build/crash_exclusion_*`
  fixtures.
- `logger-kit`: an abstract `Sink` service, a silent provider, and a logger
  that publishes a Sink allowance without invoking it, used by the
  `build/*service_exclusion*` fixtures.
- `paradigm-internals`: target-scoped paradigm internals, used by the
  `targets/single_target_internal_package_*` fixtures.

A package that no fixture depends on does not belong here.

`authored-target-supplies` belongs to
`targets/private_target_supplies_remain_disclosed`: its unused private leaves
exercise complete target-scoped package disclosure, not runtime provider selection.
