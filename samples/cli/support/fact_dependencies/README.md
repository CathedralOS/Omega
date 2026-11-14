# Fact sample dependency packages

Some fact samples exist to show a capability across a package or component
boundary. The package on the other side of that boundary lives here, outside
the frozen `samples/cli/facts/` directory, and each place of the sample
reaches it through a relative `Source::Path` in its `build.omg`. The rule is
[fact sample dependencies](../../facts/README.md#dependency-packages).

There is one package per distinct dependency contract. Places that need the
same contract share one package; a variant that a test depends on, such as a
negative twin's weaker published contract, is its own package.

| Package | Used by | What distinguishes it |
| --- | --- | --- |
| `reading_provider` | the 9 places of `build_target_provider` | `Reading::total` publishes `ensures result >= 1000u64` |
| `reading_provider_weak_contract` | its 9 `invalid_*` twins | the same package, except that the requirement publishes `>= 999u64`; the twin shows a caller cannot rely on the provider's stronger body |
| `qualified_call_lib` | the 18 places of `dependency_type_qualified_call`, through the alias `lib` | a data type with a machine attached under its name |
| `scalar_lending_producer` | the 18 places of `component_scalar_lending`, through the alias `producer` | providers the sample selects as `CompositionMode::Independent` |

Both `reading_provider` packages declare the name `reading_provider`, which
is the name their consumers import.

The sources were recovered from the commit before #1841, which removed the
per-place copies as duplicates.
