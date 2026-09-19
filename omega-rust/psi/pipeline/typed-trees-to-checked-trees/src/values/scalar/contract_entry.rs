//! Declared Requires and crash ceilings describe invocation-entry operands.
//! Body reads continue to use the independent current-storage namespace.
use crate::values::lower_machine_parameter_boolean_expression;
use checked_trees::CheckedBooleanExpression;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedScalarExpression;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

mod crash_entry;
pub(crate) use crash_entry::lower_machine_entry_crash_contract_expression;
pub(crate) use crash_entry::lower_operator_crash_contract_expression;
pub(crate) use crash_entry::lower_signature_crash_contract_expression;

/// The structural crash reader additionally needs a closed signature. This is
/// execution eligibility, not the identity of a concrete scalar contract leaf
/// on a generic declaration awaiting application.
pub(super) fn entry_parameters<'program>(
    program: &'program TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Option<&'program [StateParameter]> {
    let parameters = authored_entry_parameters(program, machine)?;
    if !program.machine_type_parameters(machine).is_empty()
        || !machine.lifetime_parameters.is_empty()
        || !machine.conformance_bounds.is_empty()
        || program.data_definitions().iter().any(|owner| {
            owner.symbol == machine.attached_data_symbol
                && (!program.data_type_parameters(owner).is_empty()
                    || !owner.lifetime_parameters.is_empty())
        })
    {
        return None;
    }
    Some(parameters)
}

/// Scalar contracts and structural crash predicates share exact authored
/// machine/state/formal identity, without borrowing each other's leaf vocabulary
/// or eligibility policy. Unread type parameters do not erase concrete clauses.
pub(super) fn authored_entry_parameters<'program>(
    program: &'program TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Option<&'program [StateParameter]> {
    if !machine.symbol.is_valid()
        || program.symbols.get(machine.symbol).kind != symbols::SymbolKind::Machine
        || program
            .machines()
            .iter()
            .filter(|candidate| candidate.symbol == machine.symbol)
            .count()
            != 1
    {
        return None;
    }
    match &machine.attached_data {
        None if machine.attached_data_symbol.is_valid() => return None,
        None => {}
        Some(name) => {
            let mut owners = program
                .data_definitions()
                .iter()
                .filter(|data| data.symbol == machine.attached_data_symbol);
            let owner = owners.next()?;
            if owners.next().is_some()
                || program.symbols.get(owner.symbol).kind != symbols::SymbolKind::Data
                || owner.name.as_str() != name.as_str()
            {
                return None;
            }
        }
    }
    let entry = program.machine_states(machine).first()?;
    let entry_symbol = program.symbols.get(entry.symbol);
    if entry_symbol.kind != symbols::SymbolKind::State || entry_symbol.parent != machine.symbol {
        return None;
    }
    let parameters = program.state_parameters(entry);
    // Unread structural operands and receivers retain their authored slots;
    // they do not make an exact owned Boolean formal a structural predicate.
    for (position, parameter) in parameters.iter().enumerate() {
        let symbol = program.symbols.get(parameter.symbol);
        if symbol.kind != symbols::SymbolKind::Parameter
            || symbol.parent != entry.symbol
            || parameters[..position]
                .iter()
                .any(|prior| prior.symbol == parameter.symbol || prior.name == parameter.name)
        {
            return None;
        }
    }
    Some(parameters)
}

pub(crate) fn lower_machine_entry_boolean_expression(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    expression: ExpressionHandle,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<CheckedBooleanExpression> {
    let mut predicate = lower_machine_parameter_boolean_expression(
        program,
        operators,
        machine,
        expression,
        exact_integer_casts,
    )?;
    let entry = program.machine_states(machine).first()?;
    EntryOperands {
        program,
        parameters: program.state_parameters(entry),
    }
    .boolean(&mut predicate)?;
    Some(predicate)
}

struct EntryOperands<'program> {
    program: &'program TypedTrees,
    parameters: &'program [StateParameter],
}

impl EntryOperands<'_> {
    fn position(&self, symbol: symbols::SymbolHandle, primitive: PrimitiveType) -> Option<usize> {
        let mut matches = self
            .parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| symbol.is_valid() && parameter.symbol == symbol);
        let (position, parameter) = matches.next()?;
        if matches.next().is_some()
            || parameter.relevance.is_erased()
            || crate::values::mutable_scalar_parameter_type(self.program, parameter)
                != Some(primitive)
        {
            return None;
        }
        // Structural entry operands keep their separate authored positions.
        // Scalar Parameter uses the dense primitive namespace at this boundary.
        Some(
            self.parameters[..position]
                .iter()
                .filter(|parameter| {
                    crate::values::scalar::occupies_scalar_position(self.program, parameter)
                })
                .count(),
        )
    }

    /// Dense erased-scalar index of the parameter carrying `symbol`, or `None`
    /// when no uniquely named erased scalar formal exists. Erased formals share
    /// the authored parameter roster but index their own proof-only namespace.
    fn erased_position(
        &self,
        symbol: symbols::SymbolHandle,
        primitive: PrimitiveType,
    ) -> Option<usize> {
        let mut matches = self
            .parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| symbol.is_valid() && parameter.symbol == symbol);
        let (position, parameter) = matches.next()?;
        if matches.next().is_some()
            || !parameter.relevance.is_erased()
            || crate::values::mutable_scalar_parameter_type(self.program, parameter)
                != Some(primitive)
        {
            return None;
        }
        Some(
            self.parameters[..position]
                .iter()
                .filter(|parameter| {
                    parameter.relevance.is_erased()
                        && crate::values::scalar::occupies_scalar_position(self.program, parameter)
                })
                .count(),
        )
    }

    fn scalar(&self, expression: &mut CheckedScalarExpression) -> Option<()> {
        match expression {
            CheckedScalarExpression::StorageRead {
                symbol,
                primitive_type,
            } => {
                *expression = if let Some(position) = self.position(*symbol, *primitive_type) {
                    CheckedScalarExpression::Parameter {
                        position,
                        primitive_type: *primitive_type,
                    }
                } else {
                    CheckedScalarExpression::ErasedParameter {
                        position: self.erased_position(*symbol, *primitive_type)?,
                        primitive_type: *primitive_type,
                    }
                };
            }
            CheckedScalarExpression::Local { .. }
            | CheckedScalarExpression::StructuralParameterByteLength { .. }
            | CheckedScalarExpression::IntegerTrappingCast { .. }
            | CheckedScalarExpression::IntegerWrappingCast { .. }
            | CheckedScalarExpression::StructuralParameterIndexedRead { .. } => return None,
            CheckedScalarExpression::IntegerBinary { left, right, .. } => {
                self.scalar(left)?;
                self.scalar(right)?;
            }
            CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
            | CheckedScalarExpression::IntegerWiden { operand, .. }
            | CheckedScalarExpression::IntegerExactCast { operand, .. } => self.scalar(operand)?,
            CheckedScalarExpression::Boolean(expression) => self.boolean(expression)?,
            CheckedScalarExpression::Parameter { .. }
            | CheckedScalarExpression::ErasedParameter { .. }
            | CheckedScalarExpression::StructuralParameterField { .. }
            | CheckedScalarExpression::IntegerLiteral { .. }
            | CheckedScalarExpression::IeeeFloatLiteral { .. } => {}
        }
        Some(())
    }

    fn boolean(&self, expression: &mut CheckedBooleanExpression) -> Option<()> {
        match expression {
            CheckedBooleanExpression::StorageRead { symbol } => {
                *expression = if let Some(position) = self.position(*symbol, PrimitiveType::Bool) {
                    CheckedBooleanExpression::Parameter { position }
                } else {
                    CheckedBooleanExpression::ErasedParameter {
                        position: self.erased_position(*symbol, PrimitiveType::Bool)?,
                    }
                };
            }
            CheckedBooleanExpression::Local { .. } => return None,
            CheckedBooleanExpression::Not(operand) => self.boolean(operand)?,
            CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right }
            | CheckedBooleanExpression::Equal { left, right } => {
                self.boolean(left)?;
                self.boolean(right)?;
            }
            CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                self.scalar(left)?;
                self.scalar(right)?;
            }
            CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::ErasedParameter { .. }
            | CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::PayloadlessSumEqual { .. }
            | CheckedBooleanExpression::StructuralCaseMembership { .. } => {}
        }
        Some(())
    }
}

#[cfg(test)]
mod tests;
