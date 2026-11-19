# stable_access_plan

`FieldAccess::Stable { read: true, write: false, exposure:
Exposure::Exported }` — a plain bool-grant access row beside the
`FieldAccess::External` materialization kind, granted through
`AccessPlan::with` over `AccessPlan::inaccessible(layout)` and
`satisfies Access::plan`.

Compile-checked only: `use omega::language::core::layout` is depend-free.
