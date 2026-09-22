//! Value signatures and structural places.

mod places;
mod signatures;

pub use places::{
    StructuralPathSegment, StructuralPlaceDeclaration, is_bounded_structural_scalar_store_path,
};
pub use signatures::{
    ErasedProofFormal, StructuralArgument, StructuralParameterDeclaration,
    StructuralReferenceResultSource, StructuralResultDeclaration, TerminalMachineResult,
    ValueDeclaration,
};
