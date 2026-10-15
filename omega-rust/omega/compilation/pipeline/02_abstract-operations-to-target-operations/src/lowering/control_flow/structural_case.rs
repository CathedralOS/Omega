//! Sum inspection retains declared order, layout, and edge-produced parameters.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetControlCasePayload, TargetControlCaseSuccessor, TargetControlTerminator,
    TargetScalarBlockValue, TargetStructuralCaseSource, TargetStructuralParameter,
};
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::StructuralAccess;
use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

pub(super) fn observe(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::StructuralCaseMembership {
        psi_operation,
        result,
        source,
        path,
        case,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let identity = live
        .structural_observation_type(function, *source)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let (tag_byte_offset, case_tag, runs) = case_projection(identity, path, *case, types)?;
    if result.scalar_type != ScalarType::Boolean {
        return Err(LoweringError::unsupported_control_flow(function.machine));
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
        .parameter(source)
        .filter(|parameter| parameter.access != StructuralAccess::WriteOnlyBorrow)
}

/// Resolve the dispatched sum. A live home dispatches as before. Otherwise the
/// root is the function's own parameter, resolved exactly as a tag observation
/// resolves it. Payload bindings retain their checked transfer paths; reading
/// the tag grants no whole-value copy regardless of the owner's claims or
/// refinements. A readable borrow keeps the caller-owned home behind its
/// pointer, so dispatch reads tag and payload leaves through it directly.
fn case_source(
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    structural_types: &StructuralTypeLookup<'_>,
    source: PlaceId,
) -> Result<TargetStructuralCaseSource, LoweringError> {
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
            terminal_psi::reading_borrow_pair_pattern!()
        );
        if !borrowed {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let layout = super::aggregate_results::value_home_layout(
            parameter.structural_type,
            structural_types,
        )?;
        return Ok(TargetStructuralCaseSource::BorrowedBlockParameter {
            parameter: parameter.clone(),
            layout,
        });
    }
    let parameter = parameter_root(prepared, source)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let declaration = function
        .structural_parameter(source)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let borrowed = matches!(
        declaration.access,
        terminal_psi::reading_borrow_pair_pattern!()
    );
    if (!borrowed && declaration.access != StructuralAccess::Owned)
        || parameter.access != declaration.access
        || parameter.multiplicity != declaration.multiplicity
        || parameter.structural_type != declaration.structural_type
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    // Dispatch observes this value's tag; it neither copies a claimed payload
    // nor grants a disposition for one. Payload transfer and cleanup retain
    // their separately checked paths, so unrelated or guarded entry claims
    // cannot forbid this observation.
    let layout =
        super::aggregate_results::value_home_layout(parameter.structural_type, structural_types)?;
    if borrowed {
        // The parameter's shape is the pointer carrier, not the referent's;
        // the sum layout describes the referent the dispatch reads through it.
        if parameter.shape.class != calling_conventions::ValueClass::BorrowedReference {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        return Ok(TargetStructuralCaseSource::BorrowedParameter {
            parameter: parameter.clone(),
            layout,
        });
    }
    if layout.shape() != parameter.shape {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(TargetStructuralCaseSource::Parameter {
        parameter: parameter.clone(),
        layout,
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
    let regions =
        crate::lowering::structural_layout::reference_projection_regions(identity, path, types)
            .ok_or_else(invalid)?;
    let (identity, _, tag_byte_offset, leaf_runs) =
        crate::lowering::structural_layout::runtime_projection(
            regions.root,
            regions.path,
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?;
    // The operation owns selectors on both sides of every pointer load. Keep
    // them in authored region order; offsets belong to the final region only.
    let mut runs = regions
        .pointer_slots
        .into_iter()
        .flat_map(|slot| slot.indices)
        .collect::<Vec<_>>();
    runs.extend(leaf_runs);
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
    let AbstractOperation::StructuralCase {
        source,
        path,
        cases,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let home = case_source(function, prepared, live, structural_types, *source)?;
    let regions = crate::lowering::structural_layout::reference_projection_regions(
        home.structural_type(),
        path,
        structural_types,
    )
    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if regions
        .pointer_slots
        .iter()
        .any(|slot| !slot.indices.is_empty())
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    // The result home remains the reference descriptor. Payload offsets start
    // anew after a pointer crossing and belong to the final backing region.
    let (selected_type, _, byte_offset) =
        crate::lowering::structural_layout::resolve_structural_projection_path(
            regions.root,
            regions.path,
            structural_types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?;
    let declaration = structural_types
        .get(&selected_type)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let (StructuralTypeShape::Sum {
        cases: declared_cases,
    }
    | StructuralTypeShape::Mixed {
        cases: declared_cases,
        ..
    }) = &declaration.shape
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let layout = crate::lowering::structural_layout::structural_sum_layout(
        selected_type,
        structural_types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    if cases.len() != declared_cases.len() || cases.len() != layout.cases.len() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut lowered = Vec::with_capacity(cases.len());
    let mut reference_exits = Vec::with_capacity(cases.len());
    for (case_ordinal, (case, declared)) in cases.iter().zip(declared_cases).enumerate() {
        let target = function
            .block_entry(case.target)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if case.case != declared.id
            || !target.structural_parameters.is_empty()
            || case.payloads.len() != target.parameters.len()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        // A case edge disposes of what it leaves behind by the same rule as
        // every other edge: a plain affine home, or an owned affine arrival
        // such as the dispatched parameter itself, with no claims or
        // qualifications. This checks the supported cleanup carrier, not
        // ownership liveness; the validated abstract graph remains the
        // custody authority.
        let mut edge_live = live.clone();
        super::terminator::plain_home_cleanup(
            function,
            &mut edge_live,
            structural_types,
            &case.cleanup_actions,
        )?;
        reference_exits.push(edge_live.reference_edge_exit(case.target));
        let fields = declared
            .fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .collect::<Vec<_>>();
        if fields.len() != layout.cases[case_ordinal].fields.len() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut payloads = Vec::with_capacity(case.payloads.len());
        for (payload, parameter) in case.payloads.iter().zip(&target.parameters) {
            let field_ordinal = fields
                .iter()
                .position(|field| field.id == payload.field)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            let field = fields[field_ordinal];
            let shape = crate::lowering::scalar_abi::fixed_native_scalar_shape(payload.scalar_type)
                .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
            let field_layout = layout.cases[case_ordinal].fields[field_ordinal];
            if field.field_type.scalar_type() != Some(payload.scalar_type)
                || field_layout.shape != shape
                || parameter.value != payload.parameter
                || parameter.scalar_type != payload.scalar_type
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            payloads.push(TargetControlCasePayload {
                field: payload.field,
                field_byte_offset: byte_offset
                    .checked_add(field_layout.byte_offset)
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?,
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
            case_tag: i32::try_from(case_ordinal)
                .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?,
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
        path: path.clone(),
        byte_offset,
        layout,
        cases: lowered,
    })
}
