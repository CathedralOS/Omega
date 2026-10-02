//! Explicit virtual wait custody supplied by the interpreter's embedding owner.
//!
//! Omega selects and validates a physical provider; Psi receives only its exact
//! checked requirement and a virtual operand contract. Neither a declaration's
//! name nor its integer signature grants clock behavior. These tokens permit no
//! real host effects and cannot replace an ordinary checked adapter.

use checked_trees::CheckedTrees;
use language_semantics::MachineSupplyMode;
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// How one scalar operand participates in the selected virtual wait contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualClockWaitOperand {
    Milliseconds,
    /// A nonzero value requests a capability the virtual provider cannot serve.
    Zero,
    /// The selected provider does not observe this operand in the admitted form.
    Ignored,
}

/// Same-program routing custody, not evidence of provider selection or trust.
/// The embedding owner must independently join its selected provider before
/// constructing this binding. Reusing it with another checked program rejects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtualClockWaitBinding {
    checked_program_address: usize,
    pub(crate) requirement: SymbolHandle,
    /// Top-level calls target the requirement machine's exact entry state.
    pub(crate) call_target: SymbolHandle,
    pub(crate) operands: Vec<VirtualClockWaitOperand>,
    pub(crate) parameter_types: Vec<PrimitiveType>,
    pub(crate) maximum_milliseconds: i64,
}

impl VirtualClockWaitBinding {
    /// Bind a bodyless Unit requirement with integer operands. Exactly one
    /// operand is a finite, nonnegative millisecond duration; its upper limit
    /// excludes any provider-specific indefinite-wait sentinel.
    pub fn from_compiler_resolved_requirement(
        checked: &CheckedTrees,
        requirement: SymbolHandle,
        operands: &[VirtualClockWaitOperand],
        maximum_milliseconds: i64,
    ) -> Result<Self, &'static str> {
        if maximum_milliseconds < 0
            || operands
                .iter()
                .filter(|operand| **operand == VirtualClockWaitOperand::Milliseconds)
                .count()
                != 1
        {
            return Err("virtual wait requires one finite millisecond operand");
        }
        let (parameters, return_type, call_target) = requirement_signature(checked, requirement)
            .ok_or("virtual wait binding is not one exact bodyless requirement")?;
        if parameters.len() != operands.len()
            || (return_type.is_valid()
                && !checked
                    .type_reference_table
                    .contains_type_reference(return_type))
            || !matches!(
                checked.type_reference_table.type_reference(return_type),
                TypeReferenceNode::Unit
            )
        {
            return Err("virtual wait binding requires its exact operand count and Unit result");
        }
        let parameter_types = parameters
            .iter()
            .map(|parameter| {
                if parameter.is_self {
                    return Err("virtual wait cannot provision a receiver");
                }
                match checked.primitive_type_reference(parameter.type_reference) {
                    Some(
                        primitive @ (PrimitiveType::I8
                        | PrimitiveType::I16
                        | PrimitiveType::I32
                        | PrimitiveType::I64
                        | PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64),
                    ) => Ok(primitive),
                    _ => Err("virtual wait requires scalar integer operands"),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            checked_program_address: std::ptr::from_ref(checked).addr(),
            requirement,
            call_target,
            operands: operands.to_vec(),
            parameter_types,
            maximum_milliseconds,
        })
    }

    pub(crate) fn validate_for(&self, checked: &CheckedTrees) -> Result<(), &'static str> {
        if self.checked_program_address != std::ptr::from_ref(checked).addr() {
            return Err("virtual wait binding belongs to a different checked program");
        }
        let reconstructed = Self::from_compiler_resolved_requirement(
            checked,
            self.requirement,
            &self.operands,
            self.maximum_milliseconds,
        )?;
        if reconstructed != *self {
            return Err("virtual wait binding no longer matches its checked requirement");
        }
        Ok(())
    }
}

fn requirement_signature(
    checked: &CheckedTrees,
    requirement: SymbolHandle,
) -> Option<(&[StateParameter], TypeReferenceHandle, SymbolHandle)> {
    if !requirement.is_valid() {
        return None;
    }
    let mut signatures = Vec::new();
    for machine in checked
        .machines()
        .iter()
        .filter(|machine| machine.symbol == requirement)
    {
        if machine.body_is_present || machine.supply_mode != MachineSupplyMode::TopLevelRequirement
        {
            return None;
        }
        let [entry] = checked.machine_states(machine) else {
            return None;
        };
        signatures.push((
            checked.state_parameters(entry),
            entry.return_type,
            entry.symbol,
        ));
    }
    for definition in checked
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        for signature in checked
            .trait_machine_signatures(definition)
            .iter()
            .filter(|signature| signature.symbol == requirement)
        {
            if !signature.native_callback_parameters.is_empty()
                || !checked
                    .state_signature_type_parameters(signature)
                    .is_empty()
            {
                return None;
            }
            signatures.push((
                checked.state_signature_parameters(signature),
                signature.return_type,
                signature.symbol,
            ));
        }
    }
    match signatures.as_slice() {
        [signature] => Some(*signature),
        _ => None,
    }
}
