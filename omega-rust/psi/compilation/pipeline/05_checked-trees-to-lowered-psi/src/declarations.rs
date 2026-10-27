//! Declaration lookups every plan family shares.
//!
//! A checked plan names machines and states by symbol. Lowering needs their
//! declarations — signatures, parameters and return types — to emit
//! operations, and this module is the one place that maps a state symbol to
//! its declaring machine and state. It reads declarations only; authored
//! statement and expression bodies belong to stage 04, which records what
//! lowering needs in the plan.

use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::signature::StateParameter;
use symbols::SymbolHandle;

/// The one declared machine and state that own `state`.
pub(crate) fn state_declaration(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
) -> Result<
    (
        &checked_trees::machine::Machine,
        &checked_trees::state::State,
    ),
    LoweringError,
> {
    let program = &checked.typed;
    let mut states = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter_map(move |candidate| {
                (state.is_valid() && candidate.symbol == state).then_some((machine, candidate))
            })
    });
    let (machine, state) = states.next().ok_or(LoweringError::Unsupported(
        "state symbol names no declared machine state",
    ))?;
    if states.next().is_some() {
        return unsupported("state symbol names more than one declared machine state");
    }
    Ok((machine, state))
}

/// The parameters and owner of one callee, by the symbol a call
/// spells: a machine head or entry state, a boundary operator, a callable
/// machine parameter, a boundary requirement, or a selector call's slot.
pub(crate) struct TargetSignature<'source> {
    pub parameters: &'source [StateParameter],
    pub return_type: typed_trees::types::TypeReferenceHandle,
    pub boundary: bool,
    parameter_coordinates: ParameterCoordinates,
}

enum ParameterCoordinates {
    Authored,
    ServiceOperands,
}

impl TargetSignature<'_> {
    /// Service selection consumes its receiver before the boundary operand
    /// parameter list is numbered. Other retained rosters keep authored positions,
    /// including gaps left by erased or ambient parameters.
    pub(crate) fn authored_parameter_position(&self, retained_position: u32) -> Option<u32> {
        let retained_position = usize::try_from(retained_position).ok()?;
        let position = match self.parameter_coordinates {
            ParameterCoordinates::Authored => {
                self.parameters.get(retained_position)?;
                retained_position
            }
            ParameterCoordinates::ServiceOperands => {
                self.parameters.iter().enumerate().nth(retained_position)?.0
            }
        };
        u32::try_from(position).ok()
    }
}

pub(crate) fn target_signature(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    source_target: SymbolHandle,
) -> Result<TargetSignature<'_>, LoweringError> {
    let program = &checked.typed;
    if !source_target.is_valid() {
        return unsupported("call names no live callee");
    }
    // A selector call's slot: the selector operand, then the requirement's
    // instantiated formals.
    if let Some(slot) = program.dynamic_slot_by_symbol(source_target) {
        return Ok(TargetSignature {
            parameters: &slot.parameters,
            return_type: slot.return_type,
            boundary: false,
            parameter_coordinates: ParameterCoordinates::Authored,
        });
    }
    // Captured calls retain the authored machine head, while execution names
    // its entry state. Resolve only the signature here: replacing the source
    // identity would break exact flow and borrow occurrence custody.
    let target_state = program
        .machine_by_symbol(source_target)
        .and_then(|machine| program.machine_states(machine).first())
        .map_or(source_target, |state| state.symbol);
    let mut states = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter_map(move |state| (state.symbol == target_state).then_some((machine, state)))
    });
    if let Some((machine, state)) = states.next() {
        if states.next().is_some() || !machine.symbol.is_valid() {
            return unsupported("call source custody disagrees with its authored callee owner");
        }
        return Ok(TargetSignature {
            parameters: program.state_parameters(state),
            return_type: state.return_type,
            // Opaque Requirements and boundary declarations share signature
            // transport, not provider authority. Authored bodies stay ordinary.
            boundary: machine.has_bodyless_signature(),
            parameter_coordinates: ParameterCoordinates::Authored,
        });
    }
    if let Some(operator) = program
        .operators()
        .iter()
        .find(|operator| operator.is_boundary && operator.symbol == source_target)
    {
        return Ok(TargetSignature {
            parameters: program.operator_parameters(operator),
            return_type: operator.return_type,
            boundary: true,
            parameter_coordinates: ParameterCoordinates::Authored,
        });
    }
    let requirement = if let Some((owner, signature)) =
        program.machine_parameter_signature(source_target)
    {
        if owner.symbol != caller_machine {
            return unsupported("call source custody uses another machine's callable parameter");
        }
        signature.symbol
    } else {
        source_target
    };
    let mut signatures = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| program.trait_machine_instances(definition))
        .filter(|signature| signature.symbol == requirement);
    let signature = signatures.next().ok_or(LoweringError::Unsupported(
        "call source custody has no authored boundary requirement",
    ))?;
    if signatures.next().is_some() || !signature.symbol.is_valid() {
        return unsupported("call source custody disagrees with its authored boundary requirement");
    }
    Ok(TargetSignature {
        parameters: program.state_signature_parameters(signature),
        return_type: signature.return_type,
        boundary: true,
        parameter_coordinates: ParameterCoordinates::ServiceOperands,
    })
}
