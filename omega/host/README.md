# Host Package Status

The old `capability` / `entry` host scaffold was retired in July 2026. It was
not imported by the compiler or standard library and encoded the superseded
model in which host contracts, target bindings, calling conventions, and trust
were fused into one declaration.

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
