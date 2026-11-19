# projected_binding_call

First sample calling a boundary requirement (`Settlement::settle(...)`)
from two former receiver spellings — a nested field path and a
machine-parameter reference — both of which admitted the same
boundary requirement roster (`reaches Settlement`).

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.

## Migration

Until 2026-10-20 the two receiver spellings were
`context.services.settlement.settle(...)` through a nested
`Binding<Settlement>` field and `binding.settle(...)` through a
`&mut Binding<Settlement>` parameter. The specification retired
source-visible `Binding<R>`
(`wiki/spec/build/component_publication.md:198`), so both carrier
lanes are gone: the requirement is called directly as
`Settlement::settle(prefix, receipt, suffix)`, and `direct` keeps
its ordinary `Receipt` argument instead of the retired parameter.
