//! Collect declaration ABI storage in the ordinary checked shape namespace.
//!
//! A refused dormant ABI remains a refused row, not a failed source check.
//! Every projection retains callback binders universally and substitutes only
//! the other local operands. Selected Handler bodies cannot choose storage.

use super::types::{self, ShapeCollector};
use checked_trees::{CheckedCallingSignaturePlan, CheckedCallingSignatureStorage};
use typed_trees::service_declarations::signatures::PreparedBoundarySignatures;

pub(super) fn collect(
    program: &dyn validation::CheckingInput,
    signatures: &PreparedBoundarySignatures,
    shapes: &mut ShapeCollector<'_>,
) -> Vec<CheckedCallingSignaturePlan> {
    let mut plans = Vec::with_capacity(signatures.abi_signatures().len());
    for declaration in signatures.abi_signatures() {
        shapes.take_unselected_opaque();
        let mut storage = (|| {
            let telescope = declaration.telescope.as_ref().map_err(Clone::clone)?;
            let mut completed = None;
            for projection in &declaration.projections {
                let projection = projection.as_ref().map_err(Clone::clone)?;
                let mut parameters = Vec::with_capacity(projection.parameters.len());
                for (ordinal, reference) in projection.parameters.iter().copied().enumerate() {
                    let parameter = shapes
                        .add_calling_parameter(reference, &projection.substitutions)
                        .ok_or_else(|| {
                            format!("declaration ABI parameter {ordinal} has no retained storage")
                        })?;
                    parameters.push(parameter);
                }
                let effective_result = types::substituted_formal_type(
                    program,
                    projection.result,
                    &projection.substitutions,
                );
                let result =
                    if !effective_result.is_valid() || types::is_unit(program, effective_result) {
                        None
                    } else {
                        Some(
                            shapes
                                .add_stored_type(projection.result, &[], &projection.substitutions)
                                .ok_or("declaration ABI result has no retained storage")?,
                        )
                    };
                if parameters.len() != telescope.parameters.len() {
                    return Err(
                        "declaration ABI storage differs from its exact native telescope".into(),
                    );
                }
                let next = CheckedCallingSignatureStorage {
                    parameters,
                    result,
                    telescope: telescope.clone(),
                };
                if completed.as_ref().is_some_and(|previous| previous != &next) {
                    return Err(
                        "storage projections disagree under one exact declaration ABI application"
                            .into(),
                    );
                }
                completed = Some(next);
            }
            completed.ok_or_else(|| "declaration ABI has no produced storage projection".into())
        })();
        if shapes.take_unselected_opaque().is_some() {
            storage = storage.map_err(|reason: String| {
                format!("{reason}: opaque representation is not selected")
            });
        }
        plans.push(CheckedCallingSignaturePlan {
            id: declaration.id,
            source_template: declaration.template,
            requirement_identity: declaration.requirement_identity.clone(),
            declaring_application: declaration.declaring_application.clone(),
            operands: declaration.operands.clone(),
            formals: declaration.formals.clone(),
            default_eligible: declaration.default_eligible,
            storage,
        });
    }
    plans
}
