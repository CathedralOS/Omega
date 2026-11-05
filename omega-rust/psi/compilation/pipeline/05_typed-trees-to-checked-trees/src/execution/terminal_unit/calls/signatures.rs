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
    CheckedUnitStructuralParameterPlan, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, StateParameter, SymbolHandle,
    strips_erased_parameter,
};

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

/// `free_structural_scalar_signature` with per-parameter trace marks.
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
        // The formal may require more than its underlying storage type does.
        // Keep those entry facts separate from the duties every write retains;
        // callers still establish the complete ordinary qualification roster.
        let mut additional_input_qualifications = Vec::new();
        if let Some(shell) = validation::reference_shell(program, parameter_type) {
            let storage =
                validation::structural_qualification_contract(program, shell.referee).ok()?;
            additional_input_qualifications.extend(
                qualifications
                    .iter()
                    .filter(|domain| !storage.qualifications.contains(domain))
                    .map(|domain| checked_trees::CheckedStructuralPathQualification {
                        path: Vec::new(),
                        domain: *domain,
                    }),
            );
            additional_input_qualifications.extend(
                projected_qualifications
                    .iter()
                    .filter(|row| !storage.projected_qualifications.contains(row))
                    .cloned(),
            );
        }
        structural_parameters.push(CheckedUnitStructuralParameterPlan {
            position: source_position,
            primitive_referent: shapes.primitive_scalar(type_identity),
            type_identity,
            multiplicity,
            access,
            qualifications,
            projected_qualifications,
            additional_input_qualifications,
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
