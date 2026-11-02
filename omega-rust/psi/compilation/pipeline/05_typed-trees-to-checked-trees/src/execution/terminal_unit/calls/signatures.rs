//! The structural and scalar signatures a call is checked against, and the
//! claims a unit holds on entry.

use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{
    ShapeCollector, fold_parameter_entry_domains, parameter_qualifications,
    projected_parameter_qualifications, structural_access_for_type_reference,
    substituted_formal_type,
};
use crate::execution::terminal_unit::{
    CarryPolicy, CheckFacts, CheckedStructuralAccess as StructuralAccess,
    CheckedStructuralScalarParameterPlan as ParameterPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralTypePlan, Multiplicity,
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    StateParameter, SymbolHandle, strips_erased_parameter,
};

/// The qualified structural signature of a state without scalar
/// parameters. A qualified machine contributes no implicit receiver
/// parameter, so the roster is exactly the authored telescope and the plan
/// needs no attachment slot of its own.
pub(crate) fn structural_signature(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<(
    Option<typed_trees::type_identity::TypeIdentityId>,
    Vec<CheckedUnitStructuralParameterPlan>,
)> {
    let parameters = program.state_parameters(state);
    let mut structural_parameters = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        // An erased binding occurrence owns no ABI position: it is skipped
        // here and at every caller-side argument producer, while `position`
        // stays the authored index so entry claims, returns, and the Terminal
        // consumer keep rejoining the typed source signature.
        if strips_erased_parameter(parameter)? {
            continue;
        }
        if parameter.is_const
            || program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        {
            return None;
        }
        let type_identity = shapes.add_type(parameter.type_reference, binders, &[])?;
        // Ingress lends the access-neutral view, but evaluating this formal as
        // a descriptor demands its exact stored shell. Declare both here;
        // operation roots decide whether the stored row survives pruning.
        if validation::slice_view_shell(program, parameter.type_reference, &[]).is_some() {
            shapes.add_stored_type(parameter.type_reference, binders, &[])?;
        }
        let qualifications =
            parameter_qualifications(program, shapes, parameter.type_reference, binders)?;
        let access = structural_access_for_type_reference(program, parameter.type_reference)?;
        // Owned formals transfer stored carriers, including each descriptor
        // inside an aggregate. Borrowed formals expose referent access instead.
        let multiplicity = if access == StructuralAccess::Owned {
            validation::reference_result_custody::result_multiplicity(
                program,
                parameter.type_reference,
            )
        } else {
            crate::checks::type_multiplicity(program, parameter.type_reference)
        };
        // The bounded write-only closure, the exact shared-observer leaf, and
        // an ordinary shared borrow of a primitive referent all admit a
        // borrowed primitive place; the borrow's own custody checks decide
        // whether the callee may read through it.
        let primitive_access_is_admitted = matches!(
            access,
            StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
                | StructuralAccess::SharedBorrow
        );
        // The registered referent retains its complete scalar receiving
        // restriction. Nominal qualifications travel on the same exact
        // carrier through the ordinary structural parameter channel below;
        // a primitive loan does not erase them or become an owned scalar.
        let primitive_referent = shapes.primitive_scalar(type_identity);
        if primitive_referent.is_some()
            && (multiplicity != Multiplicity::Unrestricted || !primitive_access_is_admitted)
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
        });
    }
    fold_parameter_entry_domains(
        program,
        shapes,
        state.symbol,
        binders,
        &mut structural_parameters,
    )?;
    Some((None, structural_parameters))
}

fn scalar_parameter_signature(
    program: &dyn ::validation::CheckingInput,
    position: usize,
    parameter: &StateParameter,
) -> Option<ParameterPlan> {
    // Binding mutability changes the state-local value namespace, not the
    // incoming scalar carrier. The body initializes its current-value slot.
    if parameter.is_const {
        return None;
    }
    Some(ParameterPlan {
        source_position: u32::try_from(position).ok()?,
        primitive_type: program.primitive_type_reference(parameter.type_reference)?,
    })
}

/// A static qualified signature, marking each parameter's erasure, scalar
/// signature, const-ness, type shape, qualifications, access, and projected
/// qualifications that declined the state. Only the general state-graph
/// route traces this, so the marks carry that route's phase prefix.
pub(crate) fn structural_scalar_signature_traced(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    trace: &LocalConstructionTrace,
) -> Option<(
    Option<typed_trees::type_identity::TypeIdentityId>,
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<ParameterPlan>,
)> {
    // Static qualification supplies identity, never implicit storage.
    let (structural_parameters, scalar_parameters) =
        scalar_and_structural_parameters(program, shapes, state, binders, trace)?;
    Some((None, structural_parameters, scalar_parameters))
}

/// A scalar body's explicit parameters and the structural shapes they require.
pub(crate) fn ambient_self_scalar_signature(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
) -> Option<(
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<ParameterPlan>,
    Vec<CheckedUnitStructuralTypePlan>,
)> {
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) = structural_scalar_signature_traced(
        program,
        &mut shapes,
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
) -> Option<(Vec<CheckedUnitStructuralParameterPlan>, Vec<ParameterPlan>)> {
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
) -> Option<(Vec<CheckedUnitStructuralParameterPlan>, Vec<ParameterPlan>)> {
    // Static binders do not themselves occupy runtime slots. Classify the
    // actual formals: an unresolved carrier still declines in the shape
    // collector, while a concrete telescope stays concrete even when the
    // declaration also binds an implementation or an unused type argument.
    scalar_and_structural_parameters(program, shapes, state, binders, trace)
}

/// The entry roster shared by composed bodies and their public family heads.
/// Ordinary invocations retain Requires in their separate contract channel;
/// composed graphs additionally expose authored entry memberships as invariants.
pub(crate) fn free_structural_scalar_entry_signature_traced(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    trace: &LocalConstructionTrace,
) -> Option<(Vec<CheckedUnitStructuralParameterPlan>, Vec<ParameterPlan>)> {
    let (mut structural, scalar) =
        free_structural_scalar_signature_traced(program, shapes, state, binders, trace)?;
    // The published invocation and its composed body share the authored
    // entry contract. Arrival-derived facts remain local to the body graph.
    fold_parameter_entry_domains(program, shapes, state.symbol, binders, &mut structural)?;
    Some((structural, scalar))
}

fn scalar_and_structural_parameters(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    state: &typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
    trace: &LocalConstructionTrace,
) -> Option<(Vec<CheckedUnitStructuralParameterPlan>, Vec<ParameterPlan>)> {
    let (mut structural, scalar) = parameter_signatures(
        program,
        shapes,
        program.state_parameters(state),
        binders,
        &[],
        trace,
    )?;
    // An explicit entry membership belongs to the state signature, independent
    // of the body route or result form that later consumes this declaration.
    fold_parameter_entry_domains(program, shapes, state.symbol, binders, &mut structural)?;
    Some((structural, scalar))
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
) -> Option<(Vec<CheckedUnitStructuralParameterPlan>, Vec<ParameterPlan>)> {
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
        let type_identity = shapes.add_type(parameter_type, binders, substitutions)?;
        // Descriptor operands retain the formal's stored shell independently
        // of its access-neutral ingress identity and of the caller's carrier.
        // Only actual operation demands keep this declaration through pruning.
        if validation::slice_view_shell(program, parameter_type, substitutions).is_some() {
            shapes.add_stored_type(parameter_type, binders, substitutions)?;
        }
        trace.phase("state graph: state signature: parameter signature: parameter qualifications");
        let qualifications = parameter_qualifications(program, shapes, parameter_type, binders)?;
        trace.phase("state graph: state signature: parameter signature: parameter access");
        let access = structural_access_for_type_reference(program, parameter_type)?;
        let multiplicity = if access == StructuralAccess::Owned {
            validation::reference_result_custody::result_multiplicity(program, parameter_type)
        } else {
            crate::checks::type_multiplicity(program, parameter_type)
        };
        trace.phase("state graph: state signature: parameter signature: projected qualifications");
        let projected_qualifications =
            projected_parameter_qualifications(program, shapes, parameter_type, binders)?;
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: source_position,
            primitive_referent: shapes.primitive_scalar(type_identity),
            type_identity,
            multiplicity,
            access,
            qualifications,
            projected_qualifications,
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
    let ownership = &facts.flow.ownership;
    let events = ownership.with_state_permissions(state, |handles| {
        handles
            .iter()
            .map(|handle| ownership.permissions.get(*handle))
            .filter(|event| {
                event.machine_symbol == machine
                    && event.state_symbol == state
                    && event.source == PermissionEventSource::StateEntry
                    && event.kind == PermissionEventKind::Establish
                    && event.access == PermissionAccess::Owned
                    && event.multiplicity == Multiplicity::Linear
                    && event.obligation_live
            })
            .collect::<Vec<_>>()
    });
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
        // The carrier beneath the declaration's `Reference`/`Constrained`
        // shells, from the type table's own query rather than a private walk.
        let source_type = validation::unwrapped_type_reference(program, source.type_reference)?;
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
