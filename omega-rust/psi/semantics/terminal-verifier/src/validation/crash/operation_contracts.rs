//! Operation-level crash contracts substitute the operation's own operands.
//!
//! A row names one ordinary operation and carries the selected operator's
//! published routes in a formal namespace of its own: the scalar operand at
//! zero-based ordinal `k` is formal `k + 1`, typed by that operand. Nothing in
//! the row is trusted. The operation, its positional scalar operand roster and
//! operand types are reconstructed from the machine; the published routes are
//! type-checked against the formal telescope; the carried continuations must
//! equal the published routes under exact operand substitution; and the
//! continuations must then be covered by the owning machine's published
//! crash routes, exactly as `validate_call_crash_coverage` covers a call's
//! continuations. Call operations keep their own carriers and take no row.

use std::collections::BTreeMap;

use semantic_vocabulary::{
    MachineId, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{
    CrashRouteGuard, OperationKind, TerminalMachine, TerminalModule, TerminalOperationCrashContract,
};

use super::super::ModuleError;
use super::{crash_routes_match, substitute_crash_routes, validate_call_crash_coverage};

pub(in crate::validation) fn validate_operation_crash_contracts(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let mut previous = None;
    for contract in &module.operation_crash_contracts {
        let identity = (contract.machine, contract.operation);
        if previous.is_some_and(|previous| previous >= identity) {
            return Err(ModuleError::NonCanonicalOperationCrashContracts);
        }
        previous = Some(identity);
        validate_operation_crash_contract(machines, contract)?;
    }
    Ok(())
}

fn validate_operation_crash_contract(
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    contract: &TerminalOperationCrashContract,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidOperationCrashContract {
        machine: contract.machine,
        operation: contract.operation,
    };
    let machine = machines
        .get(&contract.machine)
        .copied()
        .ok_or_else(invalid)?;
    let operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == contract.operation)
        .ok_or_else(invalid)?;
    let operands = positional_scalar_operands(&operation.kind)
        .filter(|operands| !operands.is_empty())
        .ok_or(ModuleError::UnsupportedOperationCrashContractOperation {
            machine: contract.machine,
            operation: contract.operation,
        })?;
    // Formal identities belong to the row's own positional telescope, not to
    // the owning machine's value table, even when their numbers coincide.
    let formals = operands
        .iter()
        .enumerate()
        .map(|(ordinal, operand)| {
            let formal = u64::try_from(ordinal)
                .ok()
                .and_then(|ordinal| ordinal.checked_add(1))
                .and_then(ValueId::new)
                .ok_or_else(invalid)?;
            let scalar_type = scalar_value_type(machine, *operand).ok_or_else(invalid)?;
            Ok((formal, *operand, scalar_type))
        })
        .collect::<Result<Vec<_>, ModuleError>>()?;
    if contract.published_routes.is_empty()
        || !crash_routes_match(&contract.published_routes, &contract.published_routes)
    {
        return Err(ModuleError::NonCanonicalOperationCrashContractRoutes {
            machine: contract.machine,
            operation: contract.operation,
        });
    }
    let context = PropositionContext::from_value_types(
        formals
            .iter()
            .map(|(formal, _, scalar_type)| (*formal, *scalar_type)),
    )
    .map_err(ModuleError::MalformedProposition)?;
    for predicate in contract
        .published_routes
        .iter()
        .flat_map(|bucket| &bucket.alternatives)
        .filter_map(|guard| match guard {
            CrashRouteGuard::Truth => None,
            CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
        })
    {
        if matches!(predicate, Proposition::Truth | Proposition::Falsehood) {
            return Err(ModuleError::NonCanonicalOperationCrashContractRoutes {
                machine: contract.machine,
                operation: contract.operation,
            });
        }
        // The formal telescope is scalar only: structural, opaque and float
        // terms have no positional operand to bind.
        if !predicate.visit_value_ids(|_| {}) {
            return Err(ModuleError::UnsupportedOperationCrashPredicate {
                machine: contract.machine,
                operation: contract.operation,
            });
        }
        context
            .validate(predicate)
            .map_err(ModuleError::MalformedProposition)?;
    }
    let substitutions = formals
        .iter()
        .map(|(formal, operand, scalar_type)| (*formal, ScalarTerm::value(*operand, *scalar_type)))
        .collect::<BTreeMap<_, _>>();
    let expected = substitute_crash_routes(&contract.published_routes, &substitutions);
    if !crash_routes_match(&contract.crash_continuations, &expected) {
        return Err(ModuleError::OperationCrashContinuationsMismatch {
            machine: contract.machine,
            operation: contract.operation,
        });
    }
    validate_call_crash_coverage(machine, &contract.crash_continuations, contract.operation)
}

/// The operation's direct scalar operands in authored position order, or
/// `None` for a call, which carries its crash continuations itself.
fn positional_scalar_operands(kind: &OperationKind) -> Option<Vec<ValueId>> {
    if matches!(
        kind,
        OperationKind::Call { .. }
            | OperationKind::CallUnit { .. }
            | OperationKind::CallStructuralScalar { .. }
            | OperationKind::CallStructuralWithScalarArguments { .. }
            | OperationKind::CallStructural { .. }
            | OperationKind::CallDynamicScalar { .. }
            | OperationKind::CallDynamicParameterScalar { .. }
            | OperationKind::CallDynamicUnit { .. }
            | OperationKind::CallDynamicParameterUnit { .. }
            | OperationKind::BoundaryCall { .. }
    ) {
        return None;
    }
    let mut operands = Vec::new();
    let mut inventory = kind.clone();
    inventory.map_scalar_uses(&mut |operand| {
        operands.push(operand);
        operand
    });
    Some(operands)
}

fn scalar_value_type(machine: &TerminalMachine, value: ValueId) -> Option<ScalarType> {
    if let Some(parameter) = machine
        .parameters
        .iter()
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .find(|declaration| declaration.id == value)
    {
        return Some(parameter.scalar_type);
    }
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| operation.result.scalar())
        .find(|declaration| declaration.id == value)
        .map(|declaration| declaration.scalar_type)
}
