Two named implementations may be checked without choosing one for an executable.
This program calls `Pick::choose`, but Build selects neither implementation on
any target, so publication must refuse the missing provider.

The original `main` call and both bodies are unchanged. A Unit entry observes
its result: 1 completes and the other provider's 2 traps. The four explicit entry
bindings expose the publication obligation without discarding the choice. This replaces the old source-rejection fixture
`fail/providers/provider_type_slot_ambiguous`: source checking alone correctly
accepts that program. The missing-selection obligation survives at publication,
as required by [multi-target compilation](../../../../../wiki/spec/build/configuration.md#multi-target-compilation).

The native-rejection oracle is compile-only; it claims no execution.
