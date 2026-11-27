#![forbid(unsafe_code)]

mod operation_metadata;

/*
We answer the structural and reference questions that native target lowering
(stage 04) and image emission (stage 15) ask about a Psi optimization unit:
callable values and invocations (`callable_values`), cohort descriptor and
reference flow graphs (`descriptor_results`), native access-view backing
(`access_references`), and structural storage projections. The validators of
Psi rewrites and of whole optimization units served only the stage 03 Psi rule
engine, which no build reaches because the Psi phase runs before Terminal
publication; they are parked in `_recycling/` at this crate's mirrored path.
*/

pub use structural_storage_queries::access_reference_validation as access_references;
pub mod callable_values;
mod current_ownership;
pub mod descriptor_results;
mod error;
mod unit_validation;

pub use error::{OperationSite, OptimizationUnitValidationError};

pub use unit_validation::structural_catalog::{
    StructuralStorageProjection, structural_borrowed_storage_projection,
    structural_storage_projection,
};

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::StructuralTypeDeclaration;

pub(crate) trait StructuralTypes {
    fn declaration(&self, id: StructuralTypeId) -> Option<&StructuralTypeDeclaration>;
    fn declarations(&self) -> impl Iterator<Item = &StructuralTypeDeclaration> + Clone;
}

impl StructuralTypes for [StructuralTypeDeclaration] {
    fn declaration(&self, id: StructuralTypeId) -> Option<&StructuralTypeDeclaration> {
        self.binary_search_by_key(&id, |row| row.id)
            .ok()
            .map(|position| &self[position])
    }

    fn declarations(&self) -> impl Iterator<Item = &StructuralTypeDeclaration> + Clone {
        self.iter()
    }
}
