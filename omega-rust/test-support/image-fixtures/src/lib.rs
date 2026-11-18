//! Explicit common encoding for low-level container fixtures. Production image
//! writers have no dependency on this crate or on a machine encoder.

pub fn encode_imports(image: &mut image::FinalImage) {
    // A malformed row deliberately has no body. Keep it malformed so the
    // container's existing import preflight reports its precise refusal.
    let identities = image
        .symbol_table
        .imports
        .iter()
        .filter_map(|(_, row)| {
            let identity = image.import_code_identity(row.symbol_handle).ok()?;
            if let machine_code::import_code::ImportCodeIdentity::Normalized(locator) = &identity
                && locator.target().native_target() != image.target
            {
                return None;
            }
            Some(identity)
        })
        .collect::<Vec<_>>();
    let plan = physical_planning::imports::plan_import_program(image.target, identities)
        .expect("fixture import target supports common encoding");
    image.import_program = native_encoding::imports::encode_import_program(plan);
}

pub fn encode_locator_import(
    locator: &target::NormalizedForeignLocator,
) -> machine_code::import_code::EncodedImportProgram {
    let plan = physical_planning::imports::plan_import_program(
        locator.target().native_target(),
        [machine_code::import_code::ImportCodeIdentity::Normalized(
            locator.clone().into(),
        )],
    )
    .expect("foreign adapter has an admitted import target");
    native_encoding::imports::encode_import_program(plan)
}

mod bridges;
pub use bridges::{
    emit_native_entry_bridge, emit_native_foreign_bridge, emit_native_foreign_bridge_object,
};
