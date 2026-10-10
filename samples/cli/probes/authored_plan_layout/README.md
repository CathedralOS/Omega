# authored_plan_layout

Hand-declared plan-laid layout vocabulary (no `core::layout` import): Schema/SchemaField where-bounds, `FieldPlan::IntegerAt{offset, stored_width, interpretation}` placement cases, `SignedByte::plan(schema) -> Plan` producing entries, `SignedByte<PortableByte>` record-field instantiation. Plain `[u8]` array storage stays independently checked beside the declared layout — unrelated encodings don't bleed into byte views.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
