// Runtime adapters are native compilation roots even when the application runs
// in Psi. Runtime metadata captures requests; this producer owns their physical
// planning and encoding. A failed optional adapter remains a boundary-local error.

pub(super) use entry_planning::native_foreign_bridge::PlannedForeignAdapter;
pub(crate) type EncodedForeignAdapter =
    native_encoding::native_foreign_bridge::EncodedForeignAdapter;
pub(crate) type PlacedForeignAdapter = function_layout::entry_placement::PlacedForeignAdapter;
pub(crate) type ObjectForeignAdapter = object_file::ForeignAdapterObject;

pub(super) fn plan(
    request: super::runtime_provisions::NativeForeignAdapterRequest,
) -> Result<PlannedForeignAdapter, String> {
    let (request, locator, application) = request.into_parts();
    entry_planning::native_foreign_bridge::plan_foreign_adapter(request, locator, application)
}

pub(super) fn encode(
    entry: installation_binding::entries::BoundForeignAdapter,
) -> Result<EncodedForeignAdapter, String> {
    native_encoding::native_foreign_bridge::encode_foreign_adapter(entry)
        .map_err(|reason| format!("foreign runtime adapter physical transport: {reason:?}"))
}

pub(super) fn place(adapter: EncodedForeignAdapter) -> Result<PlacedForeignAdapter, String> {
    placement::entry_layout::resolve_foreign_adapter(adapter)
        .map(relaxation::retain_fixed_entry)
        .map(function_layout::place_fixed_entry)
        .map_err(|error| error.to_string())
}

pub(super) fn construct(adapter: PlacedForeignAdapter) -> Result<ObjectForeignAdapter, String> {
    object_file::ForeignAdapterObject::from_placement(adapter)
}
