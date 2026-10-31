//! Public dynamic-slot metadata for one trait requirement.
//!
//! Returned-borrow origins and structural qualification preconditions are
//! records on the checked slot. Review copies one shared roster and rejects
//! a slot that dropped its declared origin or a roster that changes across
//! selector applications of the same requirement.

use crate::capture::PackageReviewInput;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::{
    PackageReviewBorrowOrigin, PackageReviewParameterQualification,
    PackageReviewReturnedBorrowOrigin, PackageReviewTypeIdentity,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(super) struct RequirementSlotMetadata {
    pub(super) returned_borrow_origins: Vec<PackageReviewReturnedBorrowOrigin>,
    pub(super) parameter_qualification_preconditions: Vec<PackageReviewParameterQualification>,
}

pub(super) fn project_requirement_slot_metadata(
    compilation: &PackageReviewInput<'_>,
    trait_symbol: SymbolHandle,
    requirement: SymbolHandle,
) -> Result<RequirementSlotMetadata, Vec<Diagnostic>> {
    let mut projected = Vec::new();
    for plan in compilation
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_slots
        .iter()
        .filter(|plan| plan.declaring_trait == trait_symbol && plan.requirement == requirement)
    {
        let slot = compilation
            .dynamic_slots
            .iter()
            .find(|slot| slot.symbol == plan.slot)
            .ok_or_else(|| rejected("public dynamic slot has no selector signature"))?;
        let declared = validation::reference_result_custody::declared_return_sources(
            compilation.checking_input,
            &slot.parameters,
            slot.return_type,
        )
        .map_err(|_| rejected("public dynamic slot dropped its returned-borrow origin"))?;
        if plan.result_reference_sources != declared {
            return Err(rejected(
                "public dynamic slot dropped its returned-borrow origin",
            ));
        }
        projected.push(project_plan(compilation, plan)?);
    }
    let Some(first) = projected.first() else {
        return Ok(RequirementSlotMetadata {
            returned_borrow_origins: Vec::new(),
            parameter_qualification_preconditions: Vec::new(),
        });
    };
    if projected.iter().any(|row| row != first) {
        return Err(rejected(
            "public dynamic slot metadata disagrees across selector applications",
        ));
    }
    Ok(RequirementSlotMetadata {
        returned_borrow_origins: first.0.clone(),
        parameter_qualification_preconditions: first.1.clone(),
    })
}

fn project_plan(
    compilation: &PackageReviewInput<'_>,
    plan: &checked_trees::CheckedDynamicSlotPlan,
) -> Result<
    (
        Vec<PackageReviewReturnedBorrowOrigin>,
        Vec<PackageReviewParameterQualification>,
    ),
    Vec<Diagnostic>,
> {
    let mut origins = Vec::new();
    for reference in &plan.result_reference_sources {
        let origin = match reference.source.root {
            checked_trees::CheckedReferenceOriginRoot::Parameter { index } => {
                if plan.structural_parameters.get(index as usize).is_none() {
                    return Err(rejected(
                        "public dynamic slot returned-borrow origin names no structural formal",
                    ));
                }
                PackageReviewBorrowOrigin::Parameter { index }
            }
            checked_trees::CheckedReferenceOriginRoot::Invocation { carrier_type } => {
                let canonical = compilation
                    .typed
                    .type_identity_text(carrier_type)
                    .filter(|text| !text.is_empty())
                    .ok_or_else(|| {
                        rejected("public dynamic slot invocation origin has no carrier identity")
                    })?;
                PackageReviewBorrowOrigin::Invocation {
                    carrier: PackageReviewTypeIdentity { canonical },
                }
            }
        };
        let row = PackageReviewReturnedBorrowOrigin { origin };
        if !origins.contains(&row) {
            origins.push(row);
        }
    }
    origins.sort();
    let mut qualifications = Vec::new();
    for (index, parameter) in plan.structural_parameters.iter().enumerate() {
        let structural_index = u32::try_from(index)
            .map_err(|_| rejected("public dynamic slot structural parameter count exceeds u32"))?;
        for domain in &parameter.qualifications {
            let symbol = {
                let mut matches = compilation
                    .domain_definitions()
                    .iter()
                    .filter(|definition| definition.semantic_id == *domain);
                let Some(definition) = matches.next() else {
                    return Err(rejected(
                        "public dynamic slot qualification has no domain declaration",
                    ));
                };
                if matches.next().is_some() {
                    return Err(rejected(
                        "public dynamic slot qualification names more than one domain",
                    ));
                }
                definition.symbol
            };
            qualifications.push(PackageReviewParameterQualification {
                structural_index,
                domain: nominal_identity(compilation, symbol)?,
            });
        }
    }
    qualifications.sort();
    if qualifications.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(rejected(
            "public dynamic slot repeats a qualification precondition",
        ));
    }
    Ok((origins, qualifications))
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(reason)]
}
