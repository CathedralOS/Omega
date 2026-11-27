#![forbid(unsafe_code)]

/*
Target lowering (native stage 04) and image emission (stage 15) read the
abstract program's descriptor and reference structure through this crate:
`descriptor_results` builds the cohort descriptor and reference flow graphs,
`callable_values` resolves which machine each callable value names,
`access_references` checks placed native backing, and the structural catalog
answers which storage a borrowed place projects.

These are read-only queries over one `PsiOptimizationUnit` revision. Each one
derives its answer from the unit's current operations and signatures rather than
from cached summaries, and refuses (`OptimizationUnitValidationError`) when the
unit does not support the answer.

The whole-unit validator, the rewrite-candidate replay and the ownership-frontier
reconstruction served only the native abstract-operations optimizer. They are
archived beside it under
`_recycling/source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics/`.
*/

mod operation_metadata;

pub use structural_storage_queries::access_reference_validation as access_references;
pub mod callable_values;
mod current_ownership;
pub mod descriptor_results;
mod error;
mod unit_validation;

pub use error::{OperationSite, OptimizationUnitValidationError};

pub use unit_validation::structural_catalog::{
    StructuralStorageProjection, structural_borrowed_storage_projection,
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
