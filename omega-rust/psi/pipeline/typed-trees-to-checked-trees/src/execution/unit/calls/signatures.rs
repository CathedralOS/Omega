//! The structural and scalar signatures a call is checked against, and the
//! claims a unit holds on entry.

use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::{
    BTreeSet, CarryPolicy, CheckFacts, CheckedStructuralAccess,
    CheckedStructuralScalarParameterPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypeShape, MachineSupplyMode, Multiplicity, PermissionAccess,
    PermissionClaimIdentity, PermissionEventKind, PermissionEventSource, ShapeCollector,
    StateParameter, SymbolHandle, TypeReferenceNode, TypedTrees, is_reference,
    parameter_qualifications, parameter_root_symbol, projected_parameter_qualifications,
    strips_erased_parameter, structural_access_for_type_reference, terminal_field_identity,
};

pub(crate) fn structural_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    retain_reference_self: bool,
) -> Option<(String, Vec<CheckedUnitStructuralParameterPlan>)> {
    let (attachment, structural, scalar) = structural_signature_with_partial_affine(
        program,
        shapes,
        machine,
        state,
        binders,
        false,
        false,
        retain_reference_self,
    )?;
    scalar.is_empty().then_some((attachment?, structural))
}

pub(crate) fn fused_service_scalar_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    retain_reference_self: bool,
) -> Option<(
    String,
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    let (attachment, structural, scalar) = structural_signature_with_partial_affine(
        program,
        shapes,
        machine,
        state,
        binders,
        false,
        true,
        retain_reference_self,
    )?;
    Some((attachment?, structural, scalar))
}

fn scalar_parameter_signature(
    program: &TypedTrees,
    position: usize,
    parameter: &StateParameter,
) -> Option<CheckedStructuralScalarParameterPlan> {
    if parameter.is_self || parameter.is_const || parameter.is_mutable {
        return None;
    }
    Some(CheckedStructuralScalarParameterPlan {
        source_position: u32::try_from(position).ok()?,
        primitive_type: program.primitive_type_reference(parameter.type_reference)?,
    })
}

pub(crate) fn free_fused_service_scalar_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<(
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    if !binders.is_empty() {
        return None;
    }
    let mut structural_parameters = Vec::new();
    let mut scalar_parameters = Vec::new();
    for (position, parameter) in program.state_parameters(state).iter().enumerate() {
        if strips_erased_parameter(parameter)? {
            continue;
        }
        if parameter.is_self || parameter.is_const || parameter.is_mutable {
            return None;
        }
        let source_position = u32::try_from(position).ok()?;
        if program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
        {
            scalar_parameters.push(scalar_parameter_signature(program, position, parameter)?);
            continue;
        }
        if typed_trees::service::exact_bound_service_requirement(program, parameter.type_reference)
            .is_none()
            || !structural_parameters.is_empty()
        {
            return None;
        }
        let (type_identity, fused_service_erasure) = shapes.add_fused_service_parameter_type(
            parameter.type_reference,
            parameter.symbol,
            binders,
        )?;
        let qualifications =
            parameter_qualifications(program, shapes, parameter.type_reference, binders)?;
        let multiplicity = crate::checks::type_multiplicity(program, parameter.type_reference);
        let access = structural_access_for_type_reference(program, parameter.type_reference)?;
        if multiplicity != Multiplicity::Affine
            || access != CheckedStructuralAccess::Owned
            || qualifications.len() != 1
        {
            return None;
        }
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: source_position,
            is_self: false,
            type_identity,
            multiplicity,
            access,
            qualifications,
            projected_qualifications: Vec::new(),
            fused_service_erasure: Some(fused_service_erasure),
        });
    }
    matches!(structural_parameters.as_slice(), [parameter] if parameter.fused_service_erasure.is_some())
        .then_some((structural_parameters, scalar_parameters))
}

pub(crate) fn partial_affine_structural_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<(Option<String>, Vec<CheckedUnitStructuralParameterPlan>)> {
    let (attachment, structural, scalar) = structural_signature_with_partial_affine(
        program, shapes, machine, state, binders, true, false, false,
    )?;
    scalar.is_empty().then_some((attachment, structural))
}

/// `retain_reference_self` keeps a borrowed `self` as structural parameter 0
/// with the reference's access, so a body can root `self.field` arguments at
/// it. Without it a borrowed attachment stays ambient and only its
/// provider-specialized fields have places.
fn structural_signature_with_partial_affine(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    allow_partial_affine: bool,
    allow_scalar_parameters: bool,
    retain_reference_self: bool,
) -> Option<(
    Option<String>,
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    let parameters = program.state_parameters(state);
    let attachment = match machine.attached_data.as_ref() {
        Some(attached_name) => {
            let attached = program
                .data_definitions()
                .iter()
                .find(|data| data.name == *attached_name)?;
            Some((
                shapes.add_attached_application(machine, binders)?,
                attached.properties.multiplicity,
            ))
        }
        None if allow_partial_affine => None,
        None => return None,
    };
    let shared_primitive_observer_type = (machine.supply_mode == MachineSupplyMode::CheckedBody
        && program
            .statement_table
            .statements(state.statement_nodes)
            .is_empty()
        && matches!(parameters.len(), 2 | 3))
    .then(|| {
        let TypeReferenceNode::Reference { referee, .. } = program
            .type_reference_table
            .type_reference(parameters[0].type_reference)
        else {
            return None;
        };
        let primitive = program.primitive_type_reference(*referee)?;
        parameters
            .iter()
            .filter(|parameter| !parameter.relevance.is_erased())
            .all(|parameter| {
                let TypeReferenceNode::Reference { referee, .. } = program
                    .type_reference_table
                    .type_reference(parameter.type_reference)
                else {
                    return false;
                };
                !parameter.is_self
                    && !parameter.is_const
                    && program.primitive_type_reference(*referee) == Some(primitive)
                    && structural_access_for_type_reference(program, parameter.type_reference)
                        == Some(CheckedStructuralAccess::SharedBorrow)
            })
            .then_some(primitive)
    })
    .flatten();
    let mut structural_parameters = Vec::new();
    let mut scalar_parameters = Vec::new();
    let mut fused_service_parameter_count = 0_usize;
    for (position, parameter) in parameters.iter().enumerate() {
        // An erased binding occurrence owns no ABI position: it is skipped
        // here and at every caller-side argument producer, while `position`
        // stays the authored index so entry claims, returns, and the Terminal
        // consumer keep rejoining the typed source signature.
        if strips_erased_parameter(parameter)? {
            continue;
        }
        if parameter.is_const || (parameter.is_self && attachment.is_none()) {
            return None;
        }
        if !parameter.is_self
            && matches!(
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                TypeReferenceNode::Reference { referee, .. }
                    if program.placed_view_plan_for_type_reference(*referee).is_some()
            )
        {
            // A direct concrete Placed<P, T> reference is retained by the
            // separate semantic-custody row and intentionally has no runtime
            // structural parameter or ABI carrier.
            continue;
        }
        // Primitive values remain in the scalar namespace. A reference to a
        // primitive may become a structural place only for the bounded
        // write-only store/call closure or the exact bounded shared-observer leaf.
        if !parameter.is_self
            && let Some(primitive_type) = program.primitive_type_reference(parameter.type_reference)
        {
            if !allow_scalar_parameters || parameter.is_mutable {
                return None;
            }
            scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                source_position: u32::try_from(position).ok()?,
                primitive_type,
            });
            continue;
        }
        if parameter.is_self
            && !retain_reference_self
            && is_reference(program, parameter.type_reference)
        {
            continue;
        }
        // Typed attached `self` intentionally carries the machine/Self symbol,
        // not the data-definition symbol. Its carrier is the independently
        // resolved attachment above.
        let (type_identity, fused_service_erasure) = if parameter.is_self {
            (attachment.as_ref()?.0.clone(), None)
        } else if typed_trees::service::exact_bound_service_requirement(
            program,
            parameter.type_reference,
        )
        .is_some()
        {
            if allow_partial_affine || parameter.is_mutable || !binders.is_empty() {
                return None;
            }
            fused_service_parameter_count = fused_service_parameter_count.checked_add(1)?;
            if fused_service_parameter_count != 1 {
                return None;
            }
            let (identity, receipt) = shapes.add_fused_service_parameter_type(
                parameter.type_reference,
                parameter.symbol,
                binders,
            )?;
            (identity, Some(receipt))
        } else if allow_scalar_parameters {
            return None;
        } else if allow_partial_affine {
            (
                shapes.add_partial_affine_type(parameter.type_reference, binders)?,
                None,
            )
        } else {
            (
                shapes.add_type(parameter.type_reference, binders, &[])?,
                None,
            )
        };
        let qualifications =
            parameter_qualifications(program, shapes, parameter.type_reference, binders)?;
        let multiplicity = if parameter.is_self && !is_reference(program, parameter.type_reference)
        {
            attachment.as_ref()?.1
        } else {
            crate::checks::type_multiplicity(program, parameter.type_reference)
        };
        let access = structural_access_for_type_reference(program, parameter.type_reference)?;
        if fused_service_erasure.is_some()
            && (multiplicity != Multiplicity::Affine
                || access != CheckedStructuralAccess::Owned
                || qualifications.len() != 1)
        {
            return None;
        }
        let primitive_access_is_supported = matches!(
            access,
            CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
        ) || (shared_primitive_observer_type.is_some()
            && access == CheckedStructuralAccess::SharedBorrow);
        if matches!(
            shapes.types.get(&type_identity).map(|shape| &shape.shape),
            Some(CheckedUnitStructuralTypeShape::PrimitiveScalar(_))
        ) && (multiplicity != Multiplicity::Unrestricted
            || !qualifications.is_empty()
            || !primitive_access_is_supported)
        {
            return None;
        }
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: u32::try_from(position).ok()?,
            is_self: parameter.is_self,
            type_identity,
            multiplicity,
            access,
            qualifications,
            projected_qualifications: projected_parameter_qualifications(
                program,
                shapes,
                parameter.type_reference,
                binders,
            )?,
            fused_service_erasure,
        });
    }
    if allow_scalar_parameters
        && (fused_service_parameter_count != 1 || structural_parameters.len() != 1)
    {
        return None;
    }
    Some((
        attachment.map(|(identity, _)| identity),
        structural_parameters,
        scalar_parameters,
    ))
}

pub(crate) fn structural_scalar_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    retain_reference_self: bool,
) -> Option<(
    String,
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    structural_scalar_signature_traced(
        program,
        shapes,
        machine,
        state,
        binders,
        retain_reference_self,
        &LocalConstructionTrace::default(),
    )
}

/// `structural_scalar_signature`, marking the signature guard (attached data
/// shape, then each parameter's erasure, receiver attachment, scalar
/// signature, const-ness, type shape, qualifications, access, and projected
/// qualifications) that declined the state. Only the general state-graph
/// route traces this, so the marks carry that route's phase prefix.
pub(crate) fn structural_scalar_signature_traced(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    retain_reference_self: bool,
    trace: &LocalConstructionTrace,
) -> Option<(
    String,
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    trace.phase("state graph: state signature: parameter signature: attached data shape");
    let attached_name = machine.attached_data.as_ref()?;
    let attached = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *attached_name)?;
    let attachment_type_identity = shapes.add_attached_application(machine, binders)?;
    let (structural_parameters, scalar_parameters) = scalar_and_structural_parameters(
        program,
        shapes,
        state,
        binders,
        Some((&attachment_type_identity, attached.properties.multiplicity)),
        retain_reference_self,
        trace,
    )?;
    Some((
        attachment_type_identity,
        structural_parameters,
        scalar_parameters,
    ))
}

pub(crate) fn free_structural_scalar_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<(
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    free_structural_scalar_signature_traced(
        program,
        shapes,
        state,
        binders,
        &LocalConstructionTrace::default(),
    )
}

/// `free_structural_scalar_signature` with the per-parameter trace marks of
/// `structural_scalar_signature_traced`.
pub(crate) fn free_structural_scalar_signature_traced(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    trace: &LocalConstructionTrace,
) -> Option<(
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    if !binders.is_empty() {
        return None;
    }
    scalar_and_structural_parameters(program, shapes, state, binders, None, false, trace)
}

fn scalar_and_structural_parameters(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    attachment: Option<(&str, Multiplicity)>,
    retain_reference_self: bool,
    trace: &LocalConstructionTrace,
) -> Option<(
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    let mut structural_parameters = Vec::new();
    let mut scalar_parameters = Vec::new();
    for (position, parameter) in program.state_parameters(state).iter().enumerate() {
        trace.phase("state graph: state signature: parameter signature: erased parameter");
        if strips_erased_parameter(parameter)? {
            continue;
        }
        trace.phase("state graph: state signature: parameter signature: receiver attachment");
        if parameter.is_self && attachment.is_none() {
            return None;
        }
        let source_position = u32::try_from(position).ok()?;
        if program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
        {
            trace.phase("state graph: state signature: parameter signature: scalar parameter");
            scalar_parameters.push(scalar_parameter_signature(program, position, parameter)?);
            continue;
        }
        trace.phase("state graph: state signature: parameter signature: const parameter");
        if parameter.is_const {
            return None;
        }
        if parameter.is_self
            && is_reference(program, parameter.type_reference)
            && !retain_reference_self
        {
            continue;
        }
        trace.phase("state graph: state signature: parameter signature: structural parameter type");
        let type_identity = if parameter.is_self {
            attachment?.0.to_owned()
        } else {
            shapes.add_type(parameter.type_reference, binders, &[])?
        };
        trace.phase("state graph: state signature: parameter signature: parameter qualifications");
        let qualifications =
            parameter_qualifications(program, shapes, parameter.type_reference, binders)?;
        // Borrowed self is the same reference-typed parameter as an
        // explicit &Record argument. Only owned self carries the attached
        // data's multiplicity; its referent shape remains attached above.
        let multiplicity = if parameter.is_self && !is_reference(program, parameter.type_reference)
        {
            attachment?.1
        } else {
            crate::checks::type_multiplicity(program, parameter.type_reference)
        };
        trace.phase("state graph: state signature: parameter signature: parameter access");
        let access = structural_access_for_type_reference(program, parameter.type_reference)?;
        trace.phase("state graph: state signature: parameter signature: projected qualifications");
        let projected_qualifications =
            projected_parameter_qualifications(program, shapes, parameter.type_reference, binders)?;
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: source_position,
            is_self: parameter.is_self,
            type_identity,
            multiplicity,
            access,
            qualifications,
            projected_qualifications,
            fused_service_erasure: None,
        });
    }
    Some((structural_parameters, scalar_parameters))
}

pub(crate) fn entry_claims(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    source_parameters: &[StateParameter],
) -> Option<Vec<CheckedUnitEntryClaimPlan>> {
    let events = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol == state
                && event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    for (parameter_index, parameter) in structural_parameters.iter().enumerate() {
        if parameter.multiplicity == Multiplicity::Unrestricted {
            continue;
        }
        let source = source_parameters.get(parameter.position as usize)?;
        let expected_root = facts::PlaceRoot::Symbol(parameter_root_symbol(machine, source));
        let matching = events
            .iter()
            .filter(|event| event.root == expected_root)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            if parameter.multiplicity == Multiplicity::Affine {
                continue;
            }
            return None;
        }
        let mut source_type = source.type_reference;
        while let TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } = program.type_reference_table.type_reference(source_type)
        {
            source_type = *base_type;
        }
        if let TypeReferenceNode::FixedArray {
            length: typed_trees::types::FixedArrayLength::Literal(length),
            ..
        } = program.type_reference_table.type_reference(source_type)
        {
            let indices = matching
                .iter()
                .map(|event| {
                    let [facts::PlaceSegment::FixedIndex { index }] =
                        facts.flow.ownership.segments.span_or_empty(event.segments)
                    else {
                        return None;
                    };
                    Some(*index)
                })
                .collect::<Option<BTreeSet<_>>>()?;
            if matching.len() != *length
                || indices != (0..*length).collect::<BTreeSet<_>>()
                || !parameter.qualifications.is_empty()
            {
                return None;
            }
        }
        for event in matching {
            if event.claim_identity == PermissionClaimIdentity::Unknown {
                return None;
            }
            let policies = facts
                .carry
                .claim_policies
                .iter()
                .filter(|policy| policy.claim_identity == event.claim_identity)
                .collect::<Vec<_>>();
            let carry = match policies.as_slice() {
                [] => CarryPolicy::STRICT,
                [policy] => policy.effective,
                _ => return None,
            };
            let path = facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .iter()
                .map(|segment| match segment {
                    facts::PlaceSegment::Field { symbol } => {
                        terminal_field_identity(program, *symbol)
                            .map(CheckedUnitStructuralPathSegment::Field)
                    }
                    facts::PlaceSegment::FixedIndex { index } => u64::try_from(*index)
                        .ok()
                        .map(CheckedUnitStructuralPathSegment::FixedIndex),
                    facts::PlaceSegment::Case { .. }
                    | facts::PlaceSegment::FixedRange { .. }
                    | facts::PlaceSegment::Index { .. } => None,
                })
                .collect::<Option<Vec<_>>>()?;
            output.push(CheckedUnitEntryClaimPlan {
                claim_identity: event.claim_identity,
                parameter_index: u32::try_from(parameter_index).ok()?,
                path,
                carry,
            });
        }
    }
    output.sort_by(|left, right| {
        (left.parameter_index, &left.path).cmp(&(right.parameter_index, &right.path))
    });
    (output.len() == events.len()).then_some(output)
}
