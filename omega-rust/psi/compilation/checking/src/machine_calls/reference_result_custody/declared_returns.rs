//! A bodyless call promises input-relative regions, not private body paths.
//! Use the same complete lifetime frontier as loan attribution, then normalize
//! parameter ordinals into the executable structural lane. The returned value
//! still supplies its concrete descriptor; these rows cannot form an address.

use super::{is_mutable_structural_parameter, loan_owner_path, record_carrier_sources};
use crate::borrow_contracts::{ViewReturnSource, resolve_signature_view_return_source};
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
            return Err("public reference result has no lifetime relation");
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
        let input_leaves = record_carrier_sources(
            program,
            parameter.type_reference,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
        )
        .ok_or("public input has no complete reference carrier roster")?;
        let input = input_leaves
            .iter()
            .find(|leaf| {
                loan_owner_path(program, parameter.type_reference, &leaf.path).as_ref()
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
        if !super::reference_access_can_supply(input.source.access, result.source.access) {
            return Err("public input access does not supply returned access");
        }
        let mut path = input.source.path.clone();
        // A borrowed formal already denotes external backing. A reference
        // stored inside an owned formal instead retains its descriptor crossing.
        if crate::reference_shell(program, parameter.type_reference).is_some()
            && path.first() == Some(&CheckedUnitStructuralPathSegment::Referent)
        {
            path.remove(0);
        }
        let origin = CheckedReturnedReferencePlan {
            path: result.path.clone(),
            source: CheckedReferenceOriginSourcePlan {
                parameter_index,
                path: path
                    .into_iter()
                    .map(CheckedReferenceOriginProjection::Exact)
                    .collect(),
                extent: ReferenceOriginExtent::WithinRegion,
                type_identity: input.source.type_identity,
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
