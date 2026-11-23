use omega_resolved::DeclarationId;
pub use omega_target_contract::{
    AddressSpaceIdentity, Endianness, EntryProfileIdentity, EntryStackGuarantee, LayoutExtent, LayoutGeometry, TargetIdentity, TargetSemanticsError,
};
use omega_typed::ValueType;
pub type TargetSemantics = omega_target_contract::TargetSemantics<ValueType, DeclarationId>;
pub type TargetObservation = omega_target_contract::TargetObservation<ValueType, DeclarationId>;
pub type SelectedLayout = omega_target_contract::SelectedLayout<ValueType>;
pub type TargetProperty = omega_target_contract::TargetProperty<ValueType>;
pub type SelectedTargetProperty = omega_target_contract::SelectedTargetProperty<ValueType>;
pub type TargetPropertyQuery<'a> = omega_target_contract::TargetPropertyQuery<'a, ValueType>;
