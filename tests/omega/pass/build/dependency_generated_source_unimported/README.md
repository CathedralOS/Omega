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

Each dependency's generated unit joins only once that dependency's Build
runs, so the fixture binds program entries and its check leg realizes it for
a declared target: a product dependency's generated source is target-scoped,
and a targetless check cannot consume it. The corpus runner executes each
dependency Build occurrence through the driver's package route before
compiling the root, as `omega --check` does.

Run `omega --check tests/omega/pass/build/dependency_generated_source_unimported/main.omg`.
