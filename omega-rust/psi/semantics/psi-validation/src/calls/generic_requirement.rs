//! The same generic receiver requirement selected by ordinary call validation.

use super::*;

/// Resolve only the declared generic receiver-bound channel. The checked
/// borrow gate uses this exact owner when a raw call has no concrete target;
/// this does not instantiate the signature or grant an executable call.
pub fn generic_bound_call_requirement<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &State,
    receiver: &str,
    target: &str,
) -> Result<Option<&'program psi_typed_trees::signature::StateSignature>, String> {
    let Some(receiver_type) = declared_receiver_type_reference(program, machine, state, receiver)
    else {
        return Ok(None);
    };
    crate::traits::generic_bound_requirement_call(program, machine, receiver_type, target)
        .map(|requirement| requirement.map(|requirement| requirement.signature))
}

/// Value receivers retain their complete declared place, including nested
/// member/index projections. Use precisely the owner used by value-call
/// validation rather than reducing a receiver to its final member spelling.
pub fn generic_bound_value_call_requirement<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &State,
    receiver: ExpressionHandle,
    target: &str,
) -> Result<Option<&'program psi_typed_trees::signature::StateSignature>, String> {
    let Some(receiver_type) = declared_place_type(program, machine, Some(state), receiver) else {
        return Ok(None);
    };
    crate::traits::generic_bound_requirement_call(program, machine, receiver_type, target)
        .map(|requirement| requirement.map(|requirement| requirement.signature))
}
