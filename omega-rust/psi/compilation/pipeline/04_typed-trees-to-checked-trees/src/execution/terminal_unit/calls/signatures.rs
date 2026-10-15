//! The structural and scalar signatures a call is checked against, and the
//! claims a unit holds on entry.

use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{
    ShapeCollector, parameter_qualifications, projected_parameter_qualifications,
    structural_access_for_type_reference, substituted_formal_type,
};
use crate::execution::terminal_unit::{
    CarryPolicy, CheckFacts, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitStructuralParameterPlan, CheckedUnitStructuralTypePlan,
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, StateParameter, SymbolHandle, TypeReferenceNode,
    strips_erased_parameter,
};

/// Which attachment storage a qualified machine's signature retains.
///
/// A carrier qualification names the semantic owner and contributes no
/// argument or storage, so ordinary Terminal plans retain an attachment only
/// for an authored receiver during the static-machine migration. Explicit
/// scalar/structural signatures require a data owner only when retaining
/// owner storage; boundary signatures additionally retain the owner for ABI
/// metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttachmentStorage {
    /// Ordinary static qualification contributes no runtime storage.
    StaticQualification,
    /// Retain the data owner itself. Boundary machine plans keep their
    /// owner's identity as ABI metadata, which stage 05 rejoins against the
    /// authored data attachment.
    DataOwner,
}

/// The qualified structural signature of a state without scalar
/// parameters, and the attachment an authored receiver retains.
pub(crate) fn structural_signature(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<(
    Option<typed_trees::type_identity::TypeIdentityId>,
    Vec<CheckedUnitStructuralParameterPlan>,
)> {
    let (attachment, structural, scalar) =
        structural_signature_for_partial_affine(program, shapes, state, binders, false, false)?;
    scalar.is_empty().then_some((attachment, structural))
}

fn scalar_parameter_signature(
    program: &dyn ::validation::CheckingInput,
    position: usize,
    parameter: &StateParameter,
) -> Option<CheckedStructuralScalarParameterPlan> {
    // Binding mutability changes the state-local value namespace, not the
    // incoming scalar carrier. The body initializes its current-value slot.
    if parameter.is_const {
        return None;
    }
    Some(CheckedStructuralScalarParameterPlan {
        source_position: u32::try_from(position).ok()?,
        primitive_type: program.primitive_type_reference(parameter.type_reference)?,
    })
}

/// The authored parameters of one state as a structural plan, with the
/// scalar formals separated out. A qualified machine contributes no implicit
/// receiver parameter, so the roster is exactly the authored telescope and the
/// plan needs no attachment slot of its own.
fn structural_signature_for_partial_affine(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    allow_partial_affine: bool,
    allow_scalar_parameters: bool,
) -> Option<(
    Option<typed_trees::type_identity::TypeIdentityId>,
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    let parameters = program.state_parameters(state);
    let attachment = None::<(typed_trees::type_identity::TypeIdentityId, Multiplicity)>;
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
        if parameter.is_const {
            return None;
        }
        // Primitive values remain in the scalar namespace. A reference to a
        // primitive may become a structural place only for the bounded
        // write-only store/call closure or the exact bounded shared-observer leaf.
        if let Some(primitive_type) = program.primitive_type_reference(parameter.type_reference) {
            if !allow_scalar_parameters || parameter.is_mutable {
                return None;
            }
            scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                source_position: u32::try_from(position).ok()?,
                primitive_type,
            });
            continue;
        }
        let (type_identity, binding_carrier) =
            if typed_trees::service::classify_exact_service_parameter_carrier(
                program,
                typed_trees::service::ServiceCarrierOccurrence {
                    type_reference: parameter.type_reference,
                    lexical_owner: parameter.symbol,
                    enclosing_data_applications: &[],
                },
            )
            .ok()
            .flatten()
            .is_some()
            {
                if allow_partial_affine || !binders.is_empty() {
                    return None;
                }
                fused_service_parameter_count = fused_service_parameter_count.checked_add(1)?;
                if fused_service_parameter_count != 1 {
                    return None;
                }
                let (identity, receipt) = shapes.add_binding_parameter_type(
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
        let access = structural_access_for_type_reference(program, parameter.type_reference)?;
        // Owned formals transfer stored carriers, including each descriptor
        // inside an aggregate. Borrowed formals expose referent access instead.
        let multiplicity = if access == CheckedStructuralAccess::Owned {
            validation::reference_result_custody::result_multiplicity(
                program,
                parameter.type_reference,
            )
        } else {
            crate::checks::type_multiplicity(program, parameter.type_reference)
        };
        if binding_carrier.is_some()
            && (!service_parameter_access_matches(multiplicity, access)
                || parameter.is_mutable != (access == CheckedStructuralAccess::MutableBorrow)
                || !qualifications.is_empty())
        {
            return None;
        }
        // The bounded write-only closure, the exact shared-observer leaf, and
        // an ordinary shared borrow of a primitive referent all admit a
        // borrowed primitive place; the borrow's own custody checks decide
        // whether the callee may read through it.
        let primitive_access_is_admitted = matches!(
            access,
            CheckedStructuralAccess::MutableBorrow
                | CheckedStructuralAccess::WriteOnlyBorrow
                | CheckedStructuralAccess::SharedBorrow
        );
        let primitive_referent = shapes.primitive_scalar(type_identity);
        if primitive_referent.is_some()
            && (multiplicity != Multiplicity::Unrestricted
                || !qualifications.is_empty()
                || !primitive_access_is_admitted)
        {
            return None;
        }
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: u32::try_from(position).ok()?,
            type_identity,
            multiplicity,
            access,
            primitive_referent,
            qualifications,
            projected_qualifications: projected_parameter_qualifications(
                program,
                shapes,
                parameter.type_reference,
                binders,
            )?,
            binding_carrier,
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

/// Borrowed parameters lend an existing affine carrier; the reference itself
/// does not own that carrier. Keep access separate from value multiplicity so
/// erasure cannot turn a loan into an owned service argument.
fn service_parameter_access_matches(
    multiplicity: Multiplicity,
    access: CheckedStructuralAccess,
) -> bool {
    match access {
        CheckedStructuralAccess::Owned => multiplicity == Multiplicity::Affine,
        checked_trees::checked_borrowed_access_pair_pattern!() => {
            multiplicity == Multiplicity::Unrestricted
        }
        CheckedStructuralAccess::WriteOnlyBorrow => false,
    }
}

/// A boundary machine retains its data owner identity as ABI metadata.
pub(crate) fn boundary_owner_scalar_signature(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<(
    typed_trees::type_identity::TypeIdentityId,
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    let (attachment, structural, scalar) = structural_scalar_signature(
        program,
        shapes,
        machine,
        state,
        binders,
        AttachmentStorage::DataOwner,
        &LocalConstructionTrace::default(),
    )?;
    Some((attachment?, structural, scalar))
}

/// `structural_scalar_signature`, marking the signature guard (attached data
/// shape, then each parameter's erasure, scalar
/// signature, const-ness, type shape, qualifications, access, and projected
/// qualifications) that declined the state. Only the general state-graph
/// route traces this, so the marks carry that route's phase prefix.
pub(crate) fn structural_scalar_signature_traced(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    trace: &LocalConstructionTrace,
) -> Option<(
    Option<typed_trees::type_identity::TypeIdentityId>,
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    structural_scalar_signature(
        program,
        shapes,
        machine,
        state,
        binders,
        AttachmentStorage::StaticQualification,
        trace,
    )
}

#[allow(clippy::too_many_arguments)]
fn structural_scalar_signature(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    storage: AttachmentStorage,
    trace: &LocalConstructionTrace,
) -> Option<(
    Option<typed_trees::type_identity::TypeIdentityId>,
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    trace.phase("state graph: state signature: parameter signature: attached data shape");
    // Static qualification supplies no storage. Resolve a data carrier only
    // when this signature actually retains one; otherwise all storage and
    // multiplicity come from the ordinary explicit parameters. A boundary
    // owner still requires its exact nominal declaration for ABI metadata.
    let attachment = if storage == AttachmentStorage::DataOwner {
        let attached = program.attached_data_definition(machine)?;
        Some((
            shapes.add_attached_application(machine, binders)?,
            attached.properties.multiplicity,
        ))
    } else {
        None
    };
    let (structural_parameters, scalar_parameters) =
        scalar_and_structural_parameters(program, shapes, state, binders, trace)?;
    Some((
        attachment.map(|(identity, _)| identity),
        structural_parameters,
        scalar_parameters,
    ))
}

/// A scalar body's explicit parameters and the structural shapes they require.
pub(crate) fn ambient_self_scalar_signature(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<(
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
    Vec<CheckedUnitStructuralTypePlan>,
)> {
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) = structural_scalar_signature_traced(
        program,
        &mut shapes,
        machine,
        state,
        &[],
        &LocalConstructionTrace::default(),
    )?;
    Some((structural, scalar, shapes.types.into_values().collect()))
}

pub(crate) fn free_structural_scalar_signature(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    trace: &LocalConstructionTrace,
) -> Option<(
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    // Static binders do not themselves occupy runtime slots. Classify the
    // actual formals: an unresolved carrier still declines in the shape
    // collector, while a concrete telescope stays concrete even when the
    // declaration also binds an implementation or an unused type argument.
    scalar_and_structural_parameters(program, shapes, state, binders, trace)
}

fn scalar_and_structural_parameters(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    trace: &LocalConstructionTrace,
) -> Option<(
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    parameter_signatures(
        program,
        shapes,
        program.state_parameters(state),
        binders,
        &[],
        trace,
    )
}

/// Classify a callee's formals into its scalar and structural lanes. A
/// typed state, a selector call's instantiated slot, a bodyless attached
/// boundary declaration and a boundary-trait requirement all present their
/// formals to callers identically, so one classifier serves every plan: a
/// requirement's plan and its adapters' plans must describe one signature.
/// `substitutions` close a prepared requirement's generic formals to the
/// specialization every checked call retained; an ordinary state passes none.
pub(crate) fn parameter_signatures(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    parameters: &[StateParameter],
    binders: &[(SymbolHandle, String)],
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    trace: &LocalConstructionTrace,
) -> Option<(
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
)> {
    let mut structural_parameters = Vec::new();
    let mut scalar_parameters = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        trace.phase("state graph: state signature: parameter signature: erased parameter");
        if strips_erased_parameter(parameter)? {
            continue;
        }
        let source_position = u32::try_from(position).ok()?;
        let parameter_type =
            substituted_formal_type(program, parameter.type_reference, substitutions);
        if program.primitive_type_reference(parameter_type).is_some() {
            trace.phase("state graph: state signature: parameter signature: scalar parameter");
            scalar_parameters.push(scalar_parameter_signature(program, position, parameter)?);
            continue;
        }
        trace.phase("state graph: state signature: parameter signature: const parameter");
        if parameter.is_const {
            return None;
        }
        trace.phase("state graph: state signature: parameter signature: structural parameter type");
        let (type_identity, binding_carrier) =
            if typed_trees::service::classify_exact_service_parameter_carrier(
                program,
                typed_trees::service::ServiceCarrierOccurrence {
                    type_reference: parameter_type,
                    lexical_owner: parameter.symbol,
                    enclosing_data_applications: &[],
                },
            )
            .ok()
            .flatten()
            .is_some()
            {
                trace.phase("state graph: state signature: parameter signature: binding binders");
                if !binders.is_empty() {
                    return None;
                }
                trace.phase("state graph: state signature: parameter signature: binding receipt");
                let (identity, receipt) =
                    shapes.add_binding_parameter_type(parameter_type, parameter.symbol, binders)?;
                (identity, Some(receipt))
            } else {
                trace.phase(
                    "state graph: state signature: parameter signature: ordinary structural type",
                );
                (
                    shapes.add_type(parameter_type, binders, substitutions)?,
                    None,
                )
            };
        trace.phase("state graph: state signature: parameter signature: parameter qualifications");
        let qualifications = parameter_qualifications(program, shapes, parameter_type, binders)?;
        trace.phase("state graph: state signature: parameter signature: parameter access");
        let access = structural_access_for_type_reference(program, parameter_type)?;
        let multiplicity = if access == CheckedStructuralAccess::Owned {
            validation::reference_result_custody::result_multiplicity(program, parameter_type)
        } else {
            crate::checks::type_multiplicity(program, parameter_type)
        };
        if binding_carrier.is_some()
            && (!service_parameter_access_matches(multiplicity, access)
                || parameter.is_mutable != (access == CheckedStructuralAccess::MutableBorrow)
                || !qualifications.is_empty())
        {
            return None;
        }
        trace.phase("state graph: state signature: parameter signature: projected qualifications");
        let projected_qualifications = if binding_carrier.is_some() {
            Vec::new()
        } else {
            projected_parameter_qualifications(program, shapes, parameter_type, binders)?
        };
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: source_position,
            primitive_referent: shapes.primitive_scalar(type_identity),
            type_identity,
            multiplicity,
            access,
            qualifications,
            projected_qualifications,
            binding_carrier,
        });
    }
    Some((structural_parameters, scalar_parameters))
}

pub(crate) fn entry_claims(
    program: &dyn ::validation::CheckingInput,
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
        let expected_root = facts::PlaceRoot::Symbol((source).symbol);
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
        // The source ownership producer already enumerates the complete
        // frontier. Preserve each typed descendant path: an array element may
        // itself contain several claims or another array, not one root claim.
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
            let path = validation::structural_claim_path(
                program,
                source_type,
                facts.flow.ownership.segments.span_or_empty(event.segments),
            )
            .ok()?;
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
    // Synthesized owned-self claims have no recorded event to join; the
    // remaining claims must still account for every recorded entry event.
    (output.len() == events.len()).then_some(output)
}
