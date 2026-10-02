//! Explicit virtual clock custody supplied by the interpreter's embedding owner.
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

/// How one scalar operand participates in the selected virtual clock contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualClockOperand {
    Milliseconds,
    /// The selected provider does not observe this operand in the admitted form.
    Ignored,
    /// The exact scalar value supported by the selected implementation.
    ExactInteger(i64),
}

/// The exact result shape supplied by the selected virtual clock implementation.
/// Its exact type must match the checked requirement; this carries no OS policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualClockResult {
    Unit,
    Integer(PrimitiveType),
}

impl VirtualClockResult {
    pub fn matches_type(self, checked: &CheckedTrees, result: TypeReferenceHandle) -> bool {
        if result.is_valid() && !checked.type_reference_table.contains_type_reference(result) {
            return false;
        }
        match self {
            Self::Unit => matches!(
                checked.type_reference_table.type_reference(result),
                TypeReferenceNode::Unit
            ),
            Self::Integer(primitive) => {
                matches!(
                    primitive,
                    PrimitiveType::I8
                        | PrimitiveType::I16
                        | PrimitiveType::I32
                        | PrimitiveType::I64
                        | PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
                ) && matches!(
                    checked.type_reference_table.type_reference(result),
                    TypeReferenceNode::Named { .. }
                ) && checked.primitive_type_reference(result) == Some(primitive)
            }
        }
    }
}

/// Deterministic clock meaning selected by the embedding owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualClockOperation {
    /// Finite completion advances time and returns Unit or integer zero.
    Wait {
        maximum_milliseconds: i64,
        result: VirtualClockResult,
    },
    /// A nonadvancing elapsed-time read in positive scaled units. The evaluator
    /// currently represents nonnegative clock results in its signed 64-bit lane.
    Read {
        primitive: PrimitiveType,
        units_per_millisecond: i64,
    },
}

impl VirtualClockOperation {
    pub fn result(self) -> VirtualClockResult {
        match self {
            Self::Wait { result, .. } => result,
            Self::Read { primitive, .. } => VirtualClockResult::Integer(primitive),
        }
    }
}

/// Same-program routing custody, not evidence of provider selection or trust.
/// The embedding owner must independently join its selected provider before
/// constructing this binding. Reusing it with another checked program rejects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtualClockBinding {
    checked_program_address: usize,
    pub(crate) requirement: SymbolHandle,
    /// Top-level calls target the requirement machine's exact entry state.
    pub(crate) call_target: SymbolHandle,
    pub(crate) operands: Vec<VirtualClockOperand>,
    pub(crate) parameter_types: Vec<PrimitiveType>,
    pub(crate) operation: VirtualClockOperation,
    realization: SymbolHandle,
    realization_call_target: SymbolHandle,
}

impl VirtualClockBinding {
    /// Bind a bodyless requirement with integer operands and an exact result.
    /// Waits require one finite duration; reads have no duration operand and
    /// use a positive scale. OS-specific choices belong to the embedding owner.
    pub fn from_compiler_resolved_requirement(
        checked: &CheckedTrees,
        requirement: SymbolHandle,
        operands: &[VirtualClockOperand],
        operation: VirtualClockOperation,
    ) -> Result<Self, &'static str> {
        let durations = operands
            .iter()
            .filter(|operand| **operand == VirtualClockOperand::Milliseconds)
            .count();
        match operation {
            VirtualClockOperation::Wait {
                maximum_milliseconds,
                ..
            } if maximum_milliseconds >= 0 && durations == 1 => {}
            VirtualClockOperation::Read {
                units_per_millisecond,
                primitive,
                ..
            } if units_per_millisecond > 0
                && durations == 0
                && matches!(primitive, PrimitiveType::I64 | PrimitiveType::U64) => {}
            _ => return Err("virtual clock operation has an invalid duration or scale contract"),
        }
        let (parameters, return_type, call_target) = requirement_signature(checked, requirement)
            .ok_or("virtual wait binding is not one exact bodyless requirement")?;
        if parameters.len() != operands.len()
            || !operation.result().matches_type(checked, return_type)
        {
            return Err("virtual wait binding requires its exact operand count and result type");
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
            operation,
            realization: SymbolHandle::invalid(),
            realization_call_target: SymbolHandle::invalid(),
        })
    }

    /// Retain the exact selected external leaf as another endpoint of this
    /// requirement. Selection/evaluated import custody remains the embedder's
    /// responsibility; source identity, signature and satisfaction are replayed here.
    pub fn with_compiler_resolved_realization(
        mut self,
        checked: &CheckedTrees,
        realization: SymbolHandle,
    ) -> Result<Self, &'static str> {
        if self.checked_program_address != std::ptr::from_ref(checked).addr() {
            return Err("virtual clock binding belongs to a different checked program");
        }
        let machine = checked
            .machine_by_symbol(realization)
            .filter(|machine| {
                !machine.body_is_present && checked.machine_type_parameters(machine).is_empty()
            })
            .ok_or("virtual clock realization must be one exact bodyless machine")?;
        let [entry] = checked.machine_states(machine) else {
            return Err("virtual clock realization has no unique entry");
        };
        let parameters = checked.state_parameters(entry);
        if parameters.len() != self.parameter_types.len()
            || !parameters
                .iter()
                .zip(&self.parameter_types)
                .all(|(parameter, primitive)| {
                    !parameter.is_self
                        && checked.primitive_type_reference(parameter.type_reference)
                            == Some(*primitive)
                })
            || !self
                .operation
                .result()
                .matches_type(checked, entry.return_type)
            || checked
                .machine_trait_conformances(machine)
                .iter()
                .filter(|conformance| conformance.requirement_symbol == self.requirement)
                .count()
                != 1
        {
            return Err("virtual clock realization disagrees with its exact requirement");
        }
        self.realization = realization;
        self.realization_call_target = entry.symbol;
        Ok(self)
    }

    pub(crate) fn matches_target(&self, target: SymbolHandle) -> bool {
        target == self.call_target
            || (self.realization_call_target.is_valid() && target == self.realization_call_target)
    }

    pub(crate) fn overlaps(&self, other: &Self) -> bool {
        self.requirement == other.requirement
            || self.matches_target(other.call_target)
            || (other.realization_call_target.is_valid()
                && self.matches_target(other.realization_call_target))
    }

    pub(crate) fn validate_for(&self, checked: &CheckedTrees) -> Result<(), &'static str> {
        if self.checked_program_address != std::ptr::from_ref(checked).addr() {
            return Err("virtual wait binding belongs to a different checked program");
        }
        let reconstructed = Self::from_compiler_resolved_requirement(
            checked,
            self.requirement,
            &self.operands,
            self.operation,
        )?;
        let reconstructed = if self.realization.is_valid() {
            reconstructed.with_compiler_resolved_realization(checked, self.realization)?
        } else {
            reconstructed
        };
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
