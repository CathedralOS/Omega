//! Value signatures and structural places.

mod places;
mod signatures;

pub use places::{
    OperationProjection, StructuralPathSegment, StructuralPlaceDeclaration,
    is_bounded_structural_scalar_store_path, is_static_structural_path,
    is_structural_scalar_store_path,
};
pub use signatures::{
    ErasedProofFormal, StructuralArgument, StructuralParameterDeclaration,
    StructuralReferenceResultSource, StructuralResultDeclaration, TerminalMachineResult,
    ValueDeclaration,
};
