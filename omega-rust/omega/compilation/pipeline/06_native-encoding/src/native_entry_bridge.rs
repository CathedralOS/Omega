//! Translate the selected entry adapter and record its exact call relocation.
use physical_recipes::native_entry_bridge::{
    NativeEntryBridgeCode, NativeEntryBridgeError as Error, NativeEntryBridgePlan,
};
/// An encoded bridge issued by stage 06 from the selected physical plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEntryBridge<Origin> {
    code: NativeEntryBridgeCode,
    origin: std::marker::PhantomData<fn(Origin) -> Origin>,
}

impl<Origin> std::ops::Deref for NativeEntryBridge<Origin> {
    type Target = NativeEntryBridgeCode;
    fn deref(&self) -> &Self::Target {
        &self.code
    }
}

impl<Origin> NativeEntryBridge<Origin> {
    pub fn into_code(self) -> NativeEntryBridgeCode {
        self.code
    }
}

pub fn encode_native_entry_bridge<Origin>(
    plan: NativeEntryBridgePlan<Origin>,
) -> Result<NativeEntryBridge<Origin>, Error> {
    let (bytes, call_field_offset) =
        crate::adapters::encode_instructions(plan.instructions()).map_err(|_| Error::Encoding)?;
    Ok(NativeEntryBridge {
        origin: std::marker::PhantomData,
        code: NativeEntryBridgeCode::from_recipe(plan, bytes, call_field_offset),
    })
}
