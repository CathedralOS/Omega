//! Operation-level crash contracts for selected operator invocations.
//!
//! A selected operator that lowers to one ordinary scalar operation has no
//! Terminal callee to carry its crash contract. Each checked
//! `CheckedCrashOperatorSite` in the lowered source closure therefore becomes
//! one `TerminalOperationCrashContract` row at the exact emitted operation:
//! the operator's published routes in the same declaration-local formal
//! telescope boundary declarations use (scalar operand ordinal `k` is formal
//! `ValueId` `k + 1`), and the continuations the verifier reconstructs by
//! substituting the operation's own operands into those routes.
//!
//! Nothing here is inferred from the emitted operation: the published routes
//! come from the checked site and must agree with the operator declaration's
//! own contract plan when one exists; the operation is found only through
//! the exact source-occurrence join emission recorded; and every
//! crash-qualified use inside the closure must produce a row, so a use whose
//! join or contract cannot be reconstructed fails closed instead of lowering
//! as a crash-free operation.

use std::collections::BTreeMap;

use checked_trees::signature::{SignatureContract, SignatureContractKind};
use checked_trees::{
    CheckedCrashOperatorSite, CheckedOperatorUseHandle, CheckedTrees, CheckedValueOrigin,
};
use lowered_psi::LoweredPsi;
use semantic_vocabulary::{MachineId, OperationId, ScalarTerm, ScalarType, ValueId};
use terminal_psi::{OperationKind, TerminalMachine, TerminalOperationCrashContract};

use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::{LoweringError, unsupported};
use crate::proofs::crash_routes::lower_formal_crash_routes;
use crate::terminal_identities::dense_identity;

/// Install one operation crash contract row per checked operator crash site
/// owned by a machine in `source_machines`, and reject any crash-qualified
/// operator use in that closure that has no such site.
pub(crate) fn retain_operation_crash_contracts(
    checked: &CheckedTrees,
    source_machines: &[symbols::SymbolHandle],
    lowered: &mut LoweredPsi,
) -> Result<(), LoweringError> {
    let sites = source_machines
        .iter()
        .filter_map(|machine| checked.facts.contract_plans.for_machine(*machine))
        .flat_map(|plan| plan.crash.checked_operators())
        .collect::<Vec<_>>();
    validate_every_crash_qualified_use_has_a_site(checked, source_machines, &sites)?;
    let mut rows = sites
        .iter()
        .map(|site| lower_site(checked, lowered, site))
        .collect::<Result<Vec<_>, _>>()?;
    rows.sort_by_key(|row| (row.machine, row.operation));
    if rows
        .windows(2)
        .any(|pair| (pair[0].machine, pair[0].operation) == (pair[1].machine, pair[1].operation))
    {
        return unsupported("operator crash sites collide on one Terminal operation");
    }
    if !lowered.semantic_module.operation_crash_contracts.is_empty() {
        return unsupported("operation crash contracts were already installed on this module");
    }
    lowered.semantic_module.operation_crash_contracts = rows;
    Ok(())
}

/// Reject, before any body lowering, a crash-qualified named
/// `Namespace::requirement(...)` call in the selected machine. A named call
/// has no source-occurrence join to an emitted operation, so its crash
/// contract has no Terminal carrier yet; refusing it here keeps the refusal
/// a crash obligation rather than whatever incidental plan gap the body
/// would otherwise report first.
pub(crate) fn reject_unjoinable_named_sites(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    let operators = &checked.facts.operators;
    let publishes_crash = |contracts: arena::HandleSpan<SignatureContract>| {
        checked
            .typed
            .signature_contracts
            .span_or_empty(contracts)
            .iter()
            .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
    };
    for (_, named_use) in operators.named_uses.iter() {
        if matches!(named_use.origin, CheckedValueOrigin::StateStatement { machine_symbol, .. }
                if machine_symbol == machine)
            && operator_declaration(checked, named_use.selected_operator_symbol)
                .is_some_and(|(_, contracts)| publishes_crash(contracts))
        {
            return unsupported(
                "named operator crash invocation has no Terminal operation join to carry its contract",
            );
        }
    }
    Ok(())
}

/// Every selected use whose requirement publishes a crash contract must have
/// been captured as a checked site; the fact builder skips only statically
/// unreachable Match arms, and an emitted operation without a row would be
/// indistinguishable from a crash-free one.
fn validate_every_crash_qualified_use_has_a_site(
    checked: &CheckedTrees,
    source_machines: &[symbols::SymbolHandle],
    sites: &[&CheckedCrashOperatorSite],
) -> Result<(), LoweringError> {
    let program = &checked.typed;
    let operators = &checked.facts.operators;
    let publishes_crash = |contracts: arena::HandleSpan<SignatureContract>| {
        program
            .signature_contracts
            .span_or_empty(contracts)
            .iter()
            .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
    };
    let in_closure = |origin: &CheckedValueOrigin| {
        matches!(origin, CheckedValueOrigin::StateStatement { machine_symbol, .. }
            if source_machines.contains(machine_symbol))
    };
    for (handle, operator_use) in operators.uses.iter() {
        if !in_closure(&operator_use.origin)
            || !operators
                .selected_candidate(operator_use)
                .is_some_and(|selected| publishes_crash(selected.contracts))
        {
            continue;
        }
        if !sites.iter().any(|site| site.operator_use == handle) {
            return unsupported(
                "selected operator crash invocation has no checked crash site to lower",
            );
        }
    }
    for (handle, named_use) in operators.named_uses.iter() {
        if !in_closure(&named_use.origin)
            || !operator_declaration(checked, named_use.selected_operator_symbol)
                .is_some_and(|(_, contracts)| publishes_crash(contracts))
        {
            continue;
        }
        if !sites.iter().any(|site| site.named_use == handle) {
            return unsupported(
                "named operator crash invocation has no checked crash site to lower",
            );
        }
    }
    Ok(())
}

fn lower_site(
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
    site: &CheckedCrashOperatorSite,
) -> Result<TerminalOperationCrashContract, LoweringError> {
    let (machine_id, operation_id) = emitted_operation(lowered, site)?;
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == machine_id)
        .ok_or(LoweringError::Unsupported(
            "operator crash site joins a Terminal machine outside the lowered module",
        ))?;
    let operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == operation_id)
        .ok_or(LoweringError::Unsupported(
            "operator crash site joins an operation outside its Terminal machine",
        ))?;
    let operands = positional_scalar_operands(&operation.kind)?;
    let operand_types = operands
        .iter()
        .map(|operand| {
            scalar_value_type(machine, *operand).ok_or(LoweringError::Unsupported(
                "operator crash site operand has no Terminal scalar declaration",
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_operator_signature(checked, site, &operand_types)?;
    let published_routes =
        lower_formal_crash_routes(published_buckets(checked, site)?, &operand_types)?;
    if published_routes.is_empty() {
        return unsupported("operator crash site publishes no lowerable crash route");
    }
    let substitutions = operands
        .iter()
        .zip(&operand_types)
        .enumerate()
        .map(|(ordinal, (operand, scalar_type))| {
            Ok((
                ValueId::new(dense_identity(ordinal)?).ok_or(LoweringError::Unsupported(
                    "operator crash formal identity is zero",
                ))?,
                ScalarTerm::value(*operand, *scalar_type),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
    let crash_continuations =
        terminal_verifier::substitute_crash_routes(&published_routes, &substitutions);
    Ok(TerminalOperationCrashContract {
        machine: machine_id,
        operation: operation_id,
        published_routes,
        crash_continuations,
    })
}

/// The exact emitted operation for one site. Only emission's own
/// source-occurrence join may answer; a positional or shape-based guess
/// would let one comparison borrow another's contract.
fn emitted_operation(
    lowered: &LoweredPsi,
    site: &CheckedCrashOperatorSite,
) -> Result<(MachineId, OperationId), LoweringError> {
    if site.named_use.is_valid() {
        return unsupported(
            "named operator crash invocation has no Terminal operation join to carry its contract",
        );
    }
    let operator_use: CheckedOperatorUseHandle = site.operator_use;
    let mut joins = lowered
        .selected_ieee_float_comparison_occurrences
        .iter()
        .filter(|occurrence| occurrence.operator_use == operator_use)
        .map(|occurrence| (occurrence.terminal_machine, occurrence.terminal_operation));
    match (joins.next(), joins.next()) {
        (Some(join), None) => Ok(join),
        (None, _) => unsupported(
            "selected operator crash invocation has no emitted Terminal operation to carry its contract",
        ),
        (Some(_), Some(_)) => {
            unsupported("selected operator crash invocation joins more than one Terminal operation")
        }
    }
}

/// The published routes for the site's selected operator. When the operator
/// declaration owns a checked machine contract plan, that plan is the
/// requirement carrier (it retains the structured scalar predicates) and the
/// site's own roster must agree with it identity-for-identity; otherwise the
/// site's roster is lowered directly, where guarded routes without a
/// structured scalar form fail closed.
fn published_buckets<'checked>(
    checked: &'checked CheckedTrees,
    site: &'checked CheckedCrashOperatorSite,
) -> Result<&'checked [checked_trees::CrashRouteBucket], LoweringError> {
    let Some(declaration) = checked
        .facts
        .contract_plans
        .for_machine(site.selected_operator)
    else {
        return Ok(site.published());
    };
    if declaration.crash.published() != site.published() {
        return unsupported(
            "operator crash site disagrees with its declaration's published crash routes",
        );
    }
    Ok(declaration.crash.published())
}

/// The formal telescope binds one scalar operand per declared parameter, in
/// authored order and with the declared primitive type. A structural or
/// non-scalar parameter has no positional operand to bind.
fn validate_operator_signature(
    checked: &CheckedTrees,
    site: &CheckedCrashOperatorSite,
    operand_types: &[ScalarType],
) -> Result<(), LoweringError> {
    let program = &checked.typed;
    let (parameters, _) = operator_declaration(checked, site.selected_operator).ok_or(
        LoweringError::Unsupported("operator crash site names no operator declaration"),
    )?;
    if parameters.len() != operand_types.len() {
        return unsupported(
            "operator crash site operand roster disagrees with the declared parameter count",
        );
    }
    for (parameter, operand_type) in parameters.iter().zip(operand_types) {
        let declared = program
            .primitive_type_reference(parameter.type_reference)
            .ok_or(LoweringError::Unsupported(
                "operator crash contract requires an all-scalar operator signature",
            ))?;
        if terminal_scalar_type(declared)? != *operand_type {
            return unsupported(
                "operator crash site operand type disagrees with its declared parameter",
            );
        }
    }
    Ok(())
}

/// The exact operator declaration a site selected: a root or domain operator
/// declaration, or the operator-signature view of a token-bearing machine
/// (which shares that machine's symbol and is never a second declaration).
/// Returns the declaration's parameter roster and contract span; the
/// declaration type itself is typed-tree vocabulary this crate does not name.
fn operator_declaration(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<(
    &[checked_trees::signature::StateParameter],
    arena::HandleSpan<SignatureContract>,
)> {
    let program = &checked.typed;
    program
        .operators()
        .iter()
        .chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain)),
        )
        .chain(program.machine_token_bindings())
        .find(|operator| operator.symbol == symbol)
        .map(|operator| (program.operator_parameters(operator), operator.contracts))
}

/// The operation's direct scalar operands in authored position order. Calls
/// carry their own continuations and never take a row.
fn positional_scalar_operands(kind: &OperationKind) -> Result<Vec<ValueId>, LoweringError> {
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
        return unsupported("operator crash site joins a call operation");
    }
    let mut operands = Vec::new();
    let mut inventory = kind.clone();
    inventory.map_scalar_uses(&mut |operand| {
        operands.push(operand);
        operand
    });
    if operands.is_empty() {
        return unsupported("operator crash site joins an operation without scalar operands");
    }
    Ok(operands)
}

fn scalar_value_type(machine: &TerminalMachine, value: ValueId) -> Option<ScalarType> {
    machine
        .parameters
        .iter()
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .map(|declaration| (declaration.id, declaration.scalar_type))
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| operation.result.scalar())
                .map(|declaration| (declaration.id, declaration.scalar_type)),
        )
        .find_map(|(id, scalar_type)| (id == value).then_some(scalar_type))
}
