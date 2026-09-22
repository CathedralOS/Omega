//! Call identities, arguments, results, and target-specific encodings.

pub mod arguments;
pub mod callbacks;
pub mod dynamic;
mod fixups;
pub mod foreign;
pub mod internal;
pub mod results;

pub use arguments::{
    ForeignCallScalarArgumentRecord, InternalUnitCallArgumentRecord,
    InternalUnitScalarArgumentSourceRecord, InternalUnitScalarCallArgumentRecord,
    InternalUnitStructuralArgumentSourceRecord,
};
pub use callbacks::{
    CallbackAddressDestination, CallbackAddressEncoding, CallbackAddressMaterialization,
};
pub use dynamic::{
    DynamicCallRecord, DynamicInstanceMaterializationRecord, DynamicParameterCallMechanismRecord,
    DynamicParameterCallRecord, DynamicTableAddressEncoding, DynamicTableAddressMaterialization,
    DynamicTraitDescriptorAbiRecord, ForwardedDynamicDescriptorAdapterIdentity,
    ForwardedDynamicDescriptorAdapterRecord, ForwardedDynamicDescriptorArgumentRecord,
    ForwardedDynamicDescriptorCallRecord, ForwardedDynamicParameterCallRecord,
    ForwardedDynamicParameterCallStackEvidence, StoredDynamicCallRecord,
    StoredDynamicDescriptorMaterializationRecord,
};
pub use fixups::{
    SelectedFormInternalMachineFixup, SelectedFormInternalMachineFixupKind,
    SelectedFormInternalMachineFixupState, SelectedFormNormalizedForeignCallFixup,
    SelectedFormNormalizedForeignCallFixupKind, SelectedFormNormalizedForeignCallFixupState,
};
pub use foreign::{Aarch64ForeignCallFloatingControlRecord, ForeignCallRelocation};
pub use internal::{
    InstalledProviderUnitScalarCallRecord, InternalCallRelocation, InternalStructuralCallResult,
    InternalStructuralResultHomeRecord, InternalUnitCallRecord, InternalUnitCallSource,
    InternalUnitScalarCallRecord, StructuralCallScalarReturnEvidence,
};
pub use results::{
    ForeignCallScalarResultRecord, InternalUnitScalarCallResultRecord, StructuralReturnRecord,
};
