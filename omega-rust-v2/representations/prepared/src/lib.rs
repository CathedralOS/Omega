//! Constants, evaluated indices, and dependencies frozen for a typed package.
#![forbid(unsafe_code)]

mod dependencies;
mod materialization;
mod prepared_trees;
mod resources;
mod target;
mod values;

pub use dependencies::{PreparationDependencies, PreparedValueOrigin, ValueInputs};
pub use materialization::{ConstantMaterialization, ConstantMaterializationError};
pub use prepared_trees::PreparedTrees;
pub use resources::{CertifiedResources, PreparationGrant, PreparationResourceReceipt};
pub use target::{
    AddressSpaceIdentity, Endianness, EntryProfileIdentity, EntryStackGuarantee, LayoutExtent, LayoutGeometry, SelectedLayout, SelectedTargetProperty,
    TargetIdentity, TargetObservation, TargetProperty, TargetPropertyQuery, TargetSemantics, TargetSemanticsError,
};
pub use values::{MaterializationComponent, MaterializationError, PreparedFloat, PreparedValue};
