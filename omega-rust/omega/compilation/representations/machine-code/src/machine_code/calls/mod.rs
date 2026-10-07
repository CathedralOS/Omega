//! Call identities, arguments, results, and target-specific encodings.

pub mod arguments;
pub mod callbacks;
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
pub use fixups::{
    SelectedFormCallbackMaterialization, SelectedFormCallbackMaterializationEncoding,
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
