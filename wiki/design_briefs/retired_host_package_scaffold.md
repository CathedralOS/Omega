# Retired: Host Package Scaffold

This note was `omega/host/README.md`. That directory held nothing but this
file and has been removed; the fence it recorded lives here instead.

Note that `omega::host::*` is not and never was a directory namespace. The
paths named by `build.omg` `boundary` and `host:` clauses are compiler-owned
calling-policy identifiers, defined as string literals in
`omega-calling-conventions`. Removing the directory resolves nothing away.

The old `capability` / `entry` host scaffold and
`library "..." calling_convention ... { entry ... }` import block are retired.
They were not imported by the compiler or standard library and encoded the
superseded model in which host contracts, target bindings, calling conventions,
and trust were fused into one declaration. Trailing `boundary host` /
`boundary Name` levels are retired with them.

Current surfaces live in these places:

- portable boundary traits and checked adapters: `omega/language/std/`;
- target-owned provider defaults and implementations:
  `omega/language/std/targets/`;
- generic binding, calling-plan, admission, and selection machinery: the
  provider-plan implementation and its architecture briefs.

Do not restore the retired files as compatibility syntax. If a future target
needs a dedicated host package here, build it as an ordinary current-model
provider package: boundary requirements, `satisfies` adapters, `via` leaves,
derived plans, and target-owned defaults.
