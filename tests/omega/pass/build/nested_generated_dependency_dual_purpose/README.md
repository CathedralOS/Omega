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

The fixture binds program entries so its check leg realizes it for a
declared target: the product edge's generated `value()` is target-scoped
source a targetless check cannot consume. The corpus runner executes the
four occurrences (generator@Build, nested_kernel@Product, nested_kernel@Build
and the root) through the driver's package route, as `omega --check` does;
authored source never selects its own package's generated declarations
(`wiki/spec/build/execution.md`, "Generated-source boundary"), so the
generated `value()` calls the authored `seed()`. The native leg exits 0
after the entry observes 7.

Run `omega --check tests/omega/pass/build/nested_generated_dependency_dual_purpose/main.omg`.
