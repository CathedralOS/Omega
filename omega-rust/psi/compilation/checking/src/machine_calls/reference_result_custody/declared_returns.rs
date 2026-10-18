//! A bodyless call promises input-relative regions, not private body paths.
//! Use the same complete lifetime frontier as loan attribution, then normalize
//! parameter ordinals into the executable structural lane. The returned value
//! still supplies its concrete descriptor; these rows cannot form an address.

use super::{is_mutable_structural_parameter, loan_owner_path, record_carrier_sources};
use crate::borrow_contracts::{
    ViewReturnInputSource, ViewReturnSource, resolve_signature_view_return_source,
};
use checked_trees::{
    CheckedReferenceOriginProjection, CheckedReferenceOriginSourcePlan,
    CheckedReturnedReferencePlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment,
};
use semantic_vocabulary::ReferenceOriginExtent;
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceHandle;

pub fn returned_sources(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
) -> Result<Vec<CheckedReturnedReferencePlan>, &'static str> {
    if !crate::borrow_contracts::returns_borrow(program, return_type) {
        return Ok(Vec::new());
    }
    let relation = resolve_signature_view_return_source(program, parameters, return_type);
    let result_leaves = record_carrier_sources(
        program,
        return_type,
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 },
    )
    .ok_or("public result has no complete reference carrier roster")?;
    let fields = match relation {
        ViewReturnSource::Fields { fields } => fields,
        ViewReturnSource::Parameter { parameter_index } => {
            let [leaf] = result_leaves.as_slice() else {
                return Err("public single-input relation has multiple result leaves");
            };
            vec![crate::borrow_contracts::ViewReturnFieldSource {
                owner_path: loan_owner_path(program, return_type, &leaf.path)
                    .ok_or("public result leaf has no loan owner path")?,
                source_path: Vec::new(),
                source: crate::borrow_contracts::ViewReturnInputSource::Referent,
                source_type: parameters
                    .get(parameter_index)
                    .ok_or("public result lifetime names no parameter")?
                    .type_reference,
                parameter_index,
                kind: leaf
                    .source
                    .access
                    .borrow_access_kind()
                    .ok_or("public result leaf has no borrowed access")?,
            }]
        }
        ViewReturnSource::NotApplicable if result_leaves.is_empty() => return Ok(Vec::new()),
        ViewReturnSource::NotApplicable => {
            return result_leaves
                .into_iter()
                .map(|leaf| {
                    let segments = super::loan_place_segments(program, return_type, &leaf.path)
                        .ok_or("invocation result leaf has no storage path")?;
                    let carrier = crate::project_type_reference_from_segments(
                        program,
                        return_type,
                        &segments,
                    )
                    .ok_or("invocation result leaf has no declared carrier")?;
                    Ok(CheckedReturnedReferencePlan {
                        path: leaf.path,
                        source: CheckedReferenceOriginSourcePlan {
                            root: checked_trees::CheckedReferenceOriginRoot::Invocation {
                                carrier_type: program.normalized_type_identity(carrier),
                            },
                            path: vec![CheckedReferenceOriginProjection::Exact(
                                CheckedUnitStructuralPathSegment::Referent,
                            )],
                            extent: ReferenceOriginExtent::Projected,
                            type_identity: leaf.source.type_identity,
                            access: leaf.source.access,
                        },
                    })
                })
                .collect();
        }
        ViewReturnSource::Ambiguous(_) => {
            return Err("public reference result lifetime relation is ambiguous");
        }
    };
    let mut output = Vec::new();
    for field in fields {
        let parameter = parameters
            .get(field.parameter_index)
            .ok_or("public result field names no parameter")?;
        if !is_mutable_structural_parameter(program, parameter) {
            return Err("public result region names a nonstructural parameter");
        }
        let parameter_index = u32::try_from(
            parameters[..field.parameter_index]
                .iter()
                .filter(|parameter| is_mutable_structural_parameter(program, parameter))
                .count(),
        )
        .map_err(|_| "public result structural parameter ordinal overflows")?;
        let input_carrier = match field.source {
            ViewReturnInputSource::Referent => parameter.type_reference,
            ViewReturnInputSource::Stored => {
                super::input_carrier_type(program, parameter.type_reference)
                    .ok_or("public input cannot expose stored descriptors")?
            }
        };
        let input_leaves = super::declared_carrier_references(program, input_carrier)
            .ok_or("public input has no complete reference carrier roster")?;
        let input = input_leaves
            .iter()
            .find(|leaf| {
                super::origin_owner_path(program, input_carrier, leaf.path.iter().cloned()).as_ref()
                    == Some(&field.source_path)
            })
            .ok_or("public input lifetime path has no matching carrier leaf")?;
        let result = result_leaves
            .iter()
            .find(|leaf| {
                loan_owner_path(program, return_type, &leaf.path).as_ref()
                    == Some(&field.owner_path)
                    && leaf.source.access.borrow_access_kind().as_ref() == Some(&field.kind)
            })
            .ok_or("public output lifetime path has no matching carrier leaf")?;
        if !super::reference_access_can_supply(input.access, result.source.access) {
            return Err("public input access does not supply returned access");
        }
        let mut path = Vec::new();
        if field.source == ViewReturnInputSource::Stored {
            // Borrowed container access and its stored descriptor are distinct
            // crossings. Slice inputs expose elements without a pointer step.
            if super::stored_loan_parts(program, parameter.type_reference).is_some() {
                path.push(CheckedReferenceOriginProjection::Exact(
                    CheckedUnitStructuralPathSegment::Referent,
                ));
            }
            path.extend(input.path.iter().cloned());
            if input.crosses_reference {
                path.push(CheckedReferenceOriginProjection::Exact(
                    CheckedUnitStructuralPathSegment::Referent,
                ));
            }
        }
        let origin = CheckedReturnedReferencePlan {
            path: result.path.clone(),
            source: CheckedReferenceOriginSourcePlan {
                root: checked_trees::CheckedReferenceOriginRoot::Parameter {
                    index: parameter_index,
                },
                path,
                extent: ReferenceOriginExtent::WithinRegion,
                type_identity: input.type_identity,
                access: result.source.access,
            },
        };
        if !output.contains(&origin) {
            output.push(origin);
        }
    }
    // A partially resolved relation must not erase an unaccounted result leaf.
    result_leaves
        .iter()
        .all(|leaf| output.iter().any(|origin| origin.path == leaf.path))
        .then_some(output)
        .ok_or("public lifetime relation omits a returned reference leaf")
}
