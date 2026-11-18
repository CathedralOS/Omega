//! Encode the selected adapter with its unresolved foreign-call field.
use physical_recipes::native_foreign_bridge::{
    NativeForeignBridgeCode, NativeForeignBridgeError as Error, NativeForeignBridgePlan,
};
pub fn encode_native_foreign_bridge<Origin>(
    plan: NativeForeignBridgePlan<Origin>,
) -> Result<NativeForeignBridge<Origin>, Error> {
    let (bytes, call_field_offset) =
        crate::adapters::encode_instructions(plan.instructions()).map_err(|_| Error::Encoding)?;
    let frame_bytes = plan.frame_bytes();
    let request = plan.into_request();
    Ok(NativeForeignBridge {
        origin: std::marker::PhantomData,
        code: NativeForeignBridgeCode::from_parts(request, bytes, frame_bytes, call_field_offset),
    })
}

/// An encoded bridge issued by stage 06 from the selected physical plan.
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
