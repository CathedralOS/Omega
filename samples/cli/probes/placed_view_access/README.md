# placed_view_access

`use omega::language::core::layout` + `Placed<RegPlacement, Registers>` custody param: the plan machine grants per-field `AccessPlan::with(..., FieldAccess::External { read: ExternalRead::Read|None, write, exposure: Exposure::Exported })`, which materializes the generated `PlacedField` accessor carriers — `view.status.read(&view.status)`, `view.transmit.write(&mut view.transmit, 9)`. LEARNING: `AccessPlan::inaccessible` yields a Placed type with NO field members — External access rows are what generate the field roster.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
