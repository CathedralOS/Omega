# Nested generated dependency under dual-purpose custody

One dependency package serves the root in both scopes: `build_depend_as`
requests it for Build work while `depend_as` imports the same package for the
product. The dependency's own Build requests a further `generator` package.
Its package-local helper supplies the generation input; Build writes
`generated.omg` and publishes it with `include_source`. The product entry
calls the generated `value()` through its Product edge. The package's two
purpose occurrence rows must each carry the correct scoped context, and every
outstanding dependency joins even when no provider is awaited.

The old direct Build import of the newly requested generator is retired by
[Build declarations](../../../../../wiki/spec/build/declarations.md). This
fixture now pins nested discovery and generated-source custody, not implicit
execution authority from a dependency request. Explicit cross-package Build
execution is covered by `explicit_provider_readiness_exit`; the root here does
not call a dependency during Build.

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

The ordinary public Build API can also measure a two-target native request:

```sh
OMEGA_PHASE_PROFILE=1 package_batch --native-evidence --compare-single-targets --run \
  --targets linux_x86_64,windows_x86_64 \
  tests/omega/pass/build/nested_generated_dependency_dual_purpose/main.omg 1 1 3 4
```

Those explicit expectations count one Product source completion, one Product
Terminal publication, three immutable Build sources (root, dependency and
generator), and their three Build checks plus one shared Product check. The
ordinary discovery scheduler retains each immutable source once; purpose-specific
Product inputs remain separate. Four purpose occurrences are not four Build
activations. These coarse counters do not establish numbered Psi-once coverage
or general delegated-context reuse applicability.
