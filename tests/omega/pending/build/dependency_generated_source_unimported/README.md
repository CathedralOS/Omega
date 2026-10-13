# A dependency's generated source resolves its authored base unimported

The root reaches two dependencies and imports neither: `kit` on a product
edge and `tool` on a build edge. Each dependency's Build includes a generated
unit that selects the package's authored `seed()`; `tool`'s unit also declares
a data type and a second machine the first one calls. Generated source may
select its package's admitted authored declarations
(`wiki/spec/build/execution.md`, "Generated-source boundary"), so the
consumer's compile joins each generated unit together with its package's
authored root, the same base the dependency's own review compiles, whether or
not any consumer source imports that package.

Before the repair, the consumer loaded a dependency's authored files only
through imports while its generated units joined unconditionally, so each
unimported generated `seed()` call was refused as unresolved. Authored source
selecting its own generated declarations still rejects
(`samples/cli/probes/build_retry_reseal/controls/invalid_authored_selects_generated`).

Run `omega --check tests/omega/pending/build/dependency_generated_source_unimported/main.omg`.

## Status: pending (runner limitation)

The corpus runner does not execute dependency Builds, so neither generated
unit joins there and a runner built without the repair also checks this
fixture; it cannot witness the repair. Verified through the real CLI on
2026-10-13: `omega --check` compiles 20 source files; the compiler before the
repair refused `seed(..)` in both generated units of `tool`. Promote to `pass/`
once the runner stages dependency builds (see
`nested_generated_dependency_dual_purpose`).
