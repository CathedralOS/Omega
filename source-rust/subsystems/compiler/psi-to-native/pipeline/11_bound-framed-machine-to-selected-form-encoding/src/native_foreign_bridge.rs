//! Encode the selected adapter with its unresolved foreign-call field.
use machine_code::import_code::EncodedImportProgram;
use physical_recipes::native_foreign_bridge::ForeignAdapterParts;
use physical_recipes::native_foreign_bridge::{
    NativeForeignBridgeCode, NativeForeignBridgeError as Error, NativeForeignBridgePlan,
};

pub struct EncodedForeignAdapter(
    ForeignAdapterParts<NativeForeignBridgeCode, EncodedImportProgram>,
);

impl EncodedForeignAdapter {
    pub fn into_parts(self) -> ForeignAdapterParts<NativeForeignBridgeCode, EncodedImportProgram> {
        self.0
    }
}

/// Encoding accepts the bound entry issued by stage 11.
/// ```compile_fail,E0308
/// fn skip_binding(plan: physical_recipes::native_foreign_bridge::ForeignAdapterPlan<entry_planning::PhysicalRecipeOrigin>) {
///     let _ = native_encoding::native_foreign_bridge::encode_foreign_adapter(plan);
/// }
/// ```
pub fn encode_foreign_adapter(
    plan: installation_binding::entries::BoundForeignAdapter,
) -> Result<EncodedForeignAdapter, Error> {
    let ForeignAdapterParts {
        bridge,
        imports,
        locator,
        application,
    } = plan.into_parts();
    Ok(EncodedForeignAdapter(ForeignAdapterParts {
        bridge: encode_native_foreign_bridge(bridge)?.into_code(),
        imports: crate::imports::encode_import_program(imports),
        locator,
        application,
    }))
}

pub fn encode_native_foreign_bridge<Origin>(
    plan: NativeForeignBridgePlan<Origin>,
) -> Result<NativeForeignBridge<Origin>, Error> {
    let (bytes, call_field_offset) =
        crate::adapters::encode_instructions(plan.instructions()).map_err(|_| Error::Encoding)?;
    Ok(NativeForeignBridge {
        origin: std::marker::PhantomData,
        code: NativeForeignBridgeCode::from_recipe(plan, bytes, call_field_offset),
    })
}

/// An encoded bridge issued by stage 12 from the selected physical plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeForeignBridge<Origin> {
    code: NativeForeignBridgeCode,
    origin: std::marker::PhantomData<fn(Origin) -> Origin>,
}

impl<Origin> std::ops::Deref for NativeForeignBridge<Origin> {
    type Target = NativeForeignBridgeCode;
    fn deref(&self) -> &Self::Target {
        &self.code
    }
}

impl<Origin> NativeForeignBridge<Origin> {
    pub fn into_code(self) -> NativeForeignBridgeCode {
        self.code
    }
}
