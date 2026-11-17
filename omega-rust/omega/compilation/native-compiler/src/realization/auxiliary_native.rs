// Runtime adapters are native compilation roots even when the application runs
// in Psi. Runtime metadata captures requests; this producer owns their physical
// planning and encoding. A failed optional adapter remains a boundary-local error.

use physical_planning::native_foreign_bridge::{NativeForeignBridgePlan, plan_native_foreign_bridge};
use native_encoding::native_foreign_bridge::{encode_native_foreign_bridge};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(super) struct PlannedForeignAdapter {
    bridge: NativeForeignBridgePlan,
    imports: physical_planning::imports::PhysicalImportProgram,
    locator: target::NormalizedForeignLocator,
    application: [u8; 32],
}

pub(super) fn plan(
    request: native_runtime_provisions::NativeForeignAdapterRequest,
) -> Result<PlannedForeignAdapter, String> {
    let (request, locator, application) = request.into_parts();
    let bridge = plan_native_foreign_bridge(request)
        .map_err(|reason| format!("foreign runtime adapter physical transport: {reason:?}"))?;
    let imports = physical_planning::imports::plan_import_program(
        bridge.request().target,
        [machine_code::import_code::ImportCodeIdentity::Normalized(
            locator.clone().into(),
        )],
    )?;
    Ok(PlannedForeignAdapter {
        imports,
        bridge,
        locator,
        application,
    })
}

pub(crate) struct EncodedForeignAdapter {
    bridge: native_encoding::native_foreign_bridge::NativeForeignBridge,
    imports: machine_code::import_code::EncodedImportProgram,
    locator: target::NormalizedForeignLocator,
    application: [u8; 32],
}

pub(super) fn encode(plan: PlannedForeignAdapter) -> Result<EncodedForeignAdapter, String> {
    let bridge = encode_native_foreign_bridge(plan.bridge)
        .map_err(|reason| format!("foreign runtime adapter physical transport: {reason:?}"))?;
    Ok(EncodedForeignAdapter {
        bridge,
        imports: native_encoding::imports::encode_import_program(plan.imports),
        locator: plan.locator,
        application: plan.application,
    })
}

pub(super) fn emit(
    adapter: EncodedForeignAdapter,
) -> Result<Arc<image_emission::NativeForeignAdapterImage>, String> {
    image_emission::emit_native_foreign_adapter(
        adapter.bridge,
        adapter.locator,
        adapter.application,
        adapter.imports,
    )
    .map(Arc::new)
    .map_err(|error| error.diagnostic.to_string())
}
