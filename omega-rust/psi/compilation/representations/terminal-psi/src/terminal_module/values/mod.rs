//! Value signatures and structural places.

mod places;
mod reference_origins;
mod signatures;

pub use reference_origins::{ReferenceOriginProjection, ReferenceOriginSource};

pub use places::{
    OperationProjection, StructuralPathSegment, StructuralPlaceDeclaration,
    is_static_structural_path, is_static_structural_scalar_store_path,
    is_structural_scalar_store_path,
};
pub use signatures::{
    ErasedProofFormal, StructuralArgument, StructuralParameterDeclaration,
    StructuralReferenceOrigin, StructuralReferenceResultSource, StructuralResultDeclaration,
    TerminalMachineResult, ValueDeclaration,
};
