//! Sum inspection retains declared order, layout, and edge-produced parameters.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetControlCasePayload, TargetControlCaseSuccessor, TargetControlTerminator,
    TargetScalarBlockValue, TargetStructuralCaseSource, TargetStructuralHomeLayout,
    TargetStructuralParameter,
};
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};
use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

pub(super) fn observe(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractOperation::StructuralCaseMembership {
        psi_operation,
        result,
        source,
        path,
        case,
    } = operation
    else {
        return Err(invalid());
    };
    let identity = live
        .structural_observation_type(function, *source)
        .ok_or_else(invalid)?;
    let (tag_byte_offset, case_tag, runs) = case_projection(identity, path, *case, types)?;
    if result.scalar_type != ScalarType::Boolean {
        return Err(invalid());
    }
    // Each runtime element scales its selector's exact dominating source by
    // the array stride; its bound is the path segment's verified obligation.
    let indices = super::scalar_sources::runtime_indices(
        runs,
        function,
        &super::scalar_sources::ScalarSources::from(&*live),
    )?;
    super::primitive_storage::retain_result(*psi_operation, *result, live)?;
    operations.push(TargetUnitOperation::StructuralCaseMembership {
        psi_operation: *psi_operation,
        result: *result,
        source: *source,
        path: path.clone(),
        tag_byte_offset,
        case: *case,
        case_tag,
        indices,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// A tag observation may read the function's own incoming parameter under any
/// readable access; a write-only loan carries no readable tag.
pub(super) fn parameter_root(
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    source: PlaceId,
) -> Option<&TargetStructuralParameter> {
    prepared
        .parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .filter(|parameter| parameter.access != StructuralAccess::WriteOnlyBorrow)
}

/// Resolve the dispatched sum. A live home dispatches as before. Otherwise the
/// root is the function's own parameter, resolved exactly as a tag observation
/// resolves it, and then held to the owned-arrival contract a block parameter
/// home already meets: payload bindings read the activation's value copy, so a
/// linear value or a qualified root has no such copy. A readable borrow is the
/// one exception: the referent stays a caller-owned home behind the borrow
/// pointer, so the dispatch reads tag and payload leaves through it directly.
fn case_source(
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
    source: PlaceId,
) -> Result<TargetStructuralCaseSource, LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    if let Some(home) = live.structural_homes.get(&source) {
        return Ok(TargetStructuralCaseSource::Home(home.clone()));
    }
    // An edge-carried join keeps the referent behind the destination block's
    // retained pointer. A readable join dispatches through it exactly like a
    // borrowed function parameter; the block's declared parameter row stands
    // in for the prepared parameter row a function root keeps.
    if let Some(parameter) = super::transfers::address_parameter(function, live, source) {
        let borrowed = matches!(
            parameter.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        );
        if !borrowed
            || parameter.multiplicity == StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !function.entry_claims.is_empty()
        {
            return Err(invalid());
        }
        let layout = crate::lowering::structural_layout::structural_sum_layout(
            parameter.structural_type,
            structural_types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?;
        return Ok(TargetStructuralCaseSource::BorrowedBlockParameter {
            parameter: parameter.clone(),
            layout: TargetStructuralHomeLayout::Sum(layout),
        });
    }
    let parameter = parameter_root(prepared, source).ok_or_else(invalid)?;
    let declaration = function
        .structural_parameters
        .iter()
        .find(|declaration| declaration.place == source)
        .ok_or_else(invalid)?;
    let borrowed = matches!(
        declaration.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    );
    if (!borrowed && declaration.access != StructuralAccess::Owned)
        || declaration.multiplicity == StructuralMultiplicity::Linear
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
        || !function.entry_claims.is_empty()
        || parameter.access != declaration.access
        || parameter.multiplicity != declaration.multiplicity
        || parameter.structural_type != declaration.structural_type
        || !parameter.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    let layout = crate::lowering::structural_layout::structural_sum_layout(
        parameter.structural_type,
        structural_types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    if borrowed {
        // The parameter's shape is the pointer carrier, not the referent's;
        // the sum layout describes the referent the dispatch reads through it.
        if parameter.shape.class != calling_conventions::ValueClass::BorrowedReference {
            return Err(invalid());
        }
        return Ok(TargetStructuralCaseSource::BorrowedParameter {
            parameter: parameter.clone(),
            layout: TargetStructuralHomeLayout::Sum(layout),
        });
    }
    if layout.shape != parameter.shape {
        return Err(invalid());
    }
    Ok(TargetStructuralCaseSource::Parameter {
        parameter: parameter.clone(),
        layout: TargetStructuralHomeLayout::Sum(layout),
    })
}

/// The observed sum's tag offset, the case's tag, and the runtime elements
/// the path selects through. A path through a runtime element (`items[i]`)
/// resolves like any runtime projection: the static offset is the carrier's,
/// and each selector scales by its element stride, rebuilt from declarations.
#[allow(clippy::type_complexity)]
fn case_projection(
    identity: StructuralTypeId,
    path: &[StructuralPathSegment],
    case: semantic_vocabulary::StructuralCaseId,
    types: &StructuralTypeLookup<'_>,
) -> Result<(u32, u32, Vec<(semantic_vocabulary::ValueId, u32)>), LoweringError> {
    let invalid = || LoweringError::UnknownStructuralType(identity);
    let (identity, tag_byte_offset, runs) = if path.is_empty() {
        (identity, 0, Vec::new())
    } else if terminal_psi::is_static_structural_path(path) {
        let (endpoint, _, offset) =
            crate::lowering::structural_layout::resolve_structural_projection_path(
                identity,
                path,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?;
        (endpoint, offset, Vec::new())
    } else {
        let (endpoint, _, offset, runs) = crate::lowering::structural_layout::runtime_projection(
            identity,
            path,
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?;
        (endpoint, offset, runs)
    };
    let declaration = types.get(&identity).ok_or_else(invalid)?;
    // A mixed shape's tag sits at the same carrier offset as a sum's: its
    // common fields follow the tag, so the case ordinal alone selects.
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &declaration.shape
    else {
        return Err(invalid());
    };
    let case_tag = cases
        .iter()
        .position(|candidate| candidate.id == case)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .ok_or_else(invalid)?;
    Ok((tag_byte_offset, case_tag, runs))
}

pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<TargetControlTerminator, LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractOperation::StructuralCase { source, cases } = operation else {
        return Err(invalid());
    };
    let home = case_source(function, prepared, live, structural_types, *source)?;
    let declaration = structural_types
        .get(&home.structural_type())
        .ok_or_else(invalid)?;
    let (StructuralTypeShape::Sum {
        cases: declared_cases,
    }
    | StructuralTypeShape::Mixed {
        cases: declared_cases,
        ..
    }) = &declaration.shape
    else {
        return Err(invalid());
    };
    let layout = home.layout().sum().ok_or_else(invalid)?;
    if cases.len() != declared_cases.len() || cases.len() != layout.cases.len() {
        return Err(invalid());
    }
    let mut lowered = Vec::with_capacity(cases.len());
    let mut reference_exits = Vec::with_capacity(cases.len());
    for (case_ordinal, (case, declared)) in cases.iter().zip(declared_cases).enumerate() {
        let target = function
            .block_entries
            .iter()
            .find(|block| block.block == case.target)
            .ok_or_else(invalid)?;
        if case.case != declared.id
            || !target.structural_parameters.is_empty()
            || case.payloads.len() != target.parameters.len()
        {
            return Err(invalid());
        }
        // A case edge disposes of what it leaves behind by the same rule as
        // every other edge: a plain affine home, or an owned affine arrival
        // such as the dispatched parameter itself, with no claims or
        // qualifications. This checks the supported cleanup carrier, not
        // ownership liveness; the validated abstract graph remains the
        // custody authority.
        let mut edge_live = live.clone();
        if !super::terminator::plain_home_cleanup(
            function,
            &mut edge_live,
            structural_types,
            &case.cleanup_actions,
        )? {
            return Err(invalid());
        }
        reference_exits.push(edge_live.reference_edge_exit(case.target));
        let fields = declared
            .fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        if fields.len() != layout.cases[case_ordinal].fields.len() {
            return Err(invalid());
        }
        let mut payloads = Vec::with_capacity(case.payloads.len());
        for (payload, parameter) in case.payloads.iter().zip(&target.parameters) {
            let field_ordinal = fields
                .iter()
                .position(|field| field.id == payload.field)
                .ok_or_else(invalid)?;
            let field = fields[field_ordinal];
            let ScalarType::Integer(integer_type) = payload.scalar_type else {
                return Err(invalid());
            };
            let shape = crate::lowering::scalar_abi::fixed_native_integer_shape(integer_type)
                .ok_or_else(invalid)?;
            let field_layout = layout.cases[case_ordinal].fields[field_ordinal];
            if field.field_type.scalar_type() != Some(payload.scalar_type)
                || field_layout.shape != shape
                || parameter.value != payload.parameter
                || parameter.scalar_type != payload.scalar_type
            {
                return Err(invalid());
            }
            payloads.push(TargetControlCasePayload {
                field: payload.field,
                field_byte_offset: field_layout.byte_offset,
                parameter: TargetScalarBlockValue {
                    block: target.block,
                    value: parameter.value,
                    scalar_type: parameter.scalar_type,
                },
            });
        }
        lowered.push(TargetControlCaseSuccessor {
            psi_edge: case.psi_edge,
            case: case.case,
            case_tag: i32::try_from(case_ordinal).map_err(|_| invalid())?,
            target: case.target,
            payloads,
            cleanup_actions: case.cleanup_actions.clone(),
        });
    }
    live.finish_reference_edges(reference_exits);
    provenance
        .edges
        .extend(cases.iter().map(|case| case.psi_edge));
    Ok(TargetControlTerminator::StructuralCase {
        source: home,
        cases: lowered,
    })
}
