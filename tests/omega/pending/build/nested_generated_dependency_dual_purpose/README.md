# Nested generated dependency under dual-purpose custody

One dependency package serves the root in both scopes: `build_depend_as`
imports it for Build evaluation while `depend_as` imports the same package
for the product. The dependency's own Build imports a further `generator`
package, so generated source flows through two nested build edges: the
dependency build calls `generator`'s product machine, writes `generated.omg`,
seals it, and publishes it with `include_source`. The root build calls the
generated `value()` through the build edge, and the product `entry` calls the
same generated machine through the product edge — the package's two purpose
occurrence rows must each carry the correct scoped context.

This is corpus coverage for custody the CLI `package_commands/build_purposes`
test asserted before the Rust-test cut: a nested generated-dependency chain
under a dual-purpose edge must check without leaking either scope's imports.
`fail/build/scoped_build_stops_before_unaccepted_effect` pins the pre-effect
rejection half of that suite, and
`fail/build/artifact_only_unsealed_output_refused` with
`pass/build/captured_source_generated_constant` pin required-output
settlement.

Run `omega --check tests/omega/pass/build/nested_generated_dependency_dual_purpose/main.omg`.

## Status: pending (runner limitation)

Verified directly through the real CLI: `omega --check main.omg` compiles 19
source files and evaluates all four occurrence rows (generator@Build,
nested_kernel@Product, nested_kernel@Build, root@Product) successfully on
2026-10-11. The corpus runner rejects it because it stages the fixture root as
a sealed snapshot and does not execute dependency `build_depend_as` chains the
way the CLI does — the same limitation
`pass/build/dependency_implementation_closure/README.md` documents for the
native route. Promote to `pass/` once the runner stages nested dependency
builds, or cover this path at CLI level instead.
