// Runtime adapters are native compilation roots even when the application runs
// in Psi. Runtime metadata captures requests; this producer owns their physical
// planning and encoding. A failed optional adapter remains a boundary-local error.

pub(crate) use entry_planning::native_foreign_bridge::PlannedForeignAdapter;
pub(crate) type EncodedForeignAdapter =
    native_encoding::native_foreign_bridge::EncodedForeignAdapter;
pub(crate) use placement::entry_layout::ResolvedForeignAdapter;
pub(crate) type ObjectForeignAdapter = object_file::ForeignAdapterObject;

pub(crate) fn plan(
    request: crate::driver::runtime::NativeForeignAdapterRequest,
) -> Result<PlannedForeignAdapter, String> {
    let (request, locator, application) = request.into_parts();
    entry_planning::native_foreign_bridge::plan_foreign_adapter(request, locator, application)
}

pub(crate) fn compile(
    request: crate::driver::runtime::NativeForeignAdapterRequest,
) -> Result<ObjectForeignAdapter, String> {
    plan(request)
        .map(entry_planning::installation_binding::entries::bind_foreign_adapter)
        .and_then(encode)
        .and_then(place)
        .and_then(construct)
}

pub(crate) fn encode(
    entry: bound_framed_machine::entries::BoundForeignAdapter<entry_planning::PhysicalRecipeOrigin>,
) -> Result<EncodedForeignAdapter, String> {
    native_encoding::native_foreign_bridge::encode_foreign_adapter(entry)
        .map_err(|reason| format!("foreign runtime adapter physical transport: {reason:?}"))
}

pub(crate) fn place(adapter: EncodedForeignAdapter) -> Result<ResolvedForeignAdapter, String> {
    placement::entry_layout::resolve_foreign_adapter(adapter)
        .map_err(|error| error.to_string())
}

pub(crate) fn construct(adapter: ResolvedForeignAdapter) -> Result<ObjectForeignAdapter, String> {
    object_file::ForeignAdapterObject::from_layout(adapter)
}

/// Exercise the runtime adapter compiler with the explicit ABI inputs supplied
/// by the external C and provider-process tests.
#[cfg(feature = "test-support")]
pub fn compile_runtime_foreign_adapter(
    bridge: calling_conventions::NativeForeignBridgeRequest,
    locator: target::NormalizedForeignLocator,
    application: [u8; 32],
) -> Result<object_file::ForeignAdapterObject, String> {
    compile(
        crate::driver::runtime::NativeForeignAdapterRequest::for_test(
            bridge,
            locator,
            application,
        ),
    )
}
