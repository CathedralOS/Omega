# progress_provider

First sample wiring a progress-qualified provider through
the build declaration: `domain SchedulerHandle::WeakFair
satisfies ProgressProfile established by
Scheduler::grant`, per-target satisfies impls via
`ForeignBinding::Syscall(1)` /
`ForeignBinding::VtableField(field)`, and
`Build::select_provider<Scheduler,
SchedulerProvider>` in build.omg.

The provider is a static slot selected by `build.omg`, not a
carrier value. Source calls the requirement directly —
`Scheduler::notify(&main.provider, 65)` — with the provider
borrowed explicitly as the first parameter, which is the
shape `VtableField` requirement bodies take
(`notify(provider: &SchedulerProvider, value: i32)`). The
handle a `grant` issues has an ordinary wire carrier
(`SchedulerHandle { identity: u64 }`), because a zero-sized
data type cannot cross a boundary by value.

## Learnings

`Build::select_provider` resolves the earlier
`Binding<T>`-field gap from `multi_effect_trait`: a
provider selected in build.omg is the Fused route that lets
the trait's requirements admit calls at `--check` — no
package dependency needed and no source-visible carrier.

## Migration

Until 2026-10-20 this probe carried a `Binding<Scheduler>`
field and relied on it for receiver admission. The
specification retired source-visible `Binding<R>`
(`wiki/spec/build/component_publication.md:198`,
`wiki/language_guide/chapter_19_capabilities_effects_boundaries.md:152`),
so the field and the import are gone; the probe now
demonstrates the same progress-qualified provider selection
through the requirement application itself.
