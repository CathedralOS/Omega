//! Bound entry recipes carry the selected instructions and the context encoding must preserve.
use function_identity::MachineFunctionIdentity;
use installation_binding_issuer::BindingIssuer;
use physical_recipes::{
    callable_entry::CallableContext,
    hosted_program::HostedProgramParts,
    imports::PhysicalImportProgram,
    native_entry_bridge::NativeEntryBridgePlan,
    native_foreign_bridge::{ForeignAdapterParts, NativeForeignBridgePlan},
    physical_entry::PhysicalEntryParts,
};

pub struct BoundEntry<Parts>(Parts);

impl<Parts> BoundEntry<Parts> {
    pub fn from_binding(_issuer: BindingIssuer, parts: Parts) -> Self {
        Self(parts)
    }

    pub fn into_parts(self) -> Parts {
        self.0
    }
}

pub type BoundForeignAdapter<Origin> =
    BoundEntry<ForeignAdapterParts<NativeForeignBridgePlan<Origin>, PhysicalImportProgram<Origin>>>;
pub type BoundHostedEntry<Origin, Context> = BoundEntry<HostedProgramParts<Origin, Context>>;
pub type BoundPhysicalEntry<Origin, Context> = BoundEntry<PhysicalEntryParts<Origin, Context>>;
pub type BoundCallableEntry<Origin> =
    BoundEntry<(CallableContext, Option<NativeEntryBridgePlan<Origin>>)>;
pub type BoundPrivateEntry<Origin> =
    BoundEntry<(MachineFunctionIdentity, BoundCallableEntry<Origin>)>;
