//! Operation-level crash contracts for selected operator invocations.
//!
//! A selected operator that lowers to one ordinary scalar operation has no
//! Terminal callee to carry its crash contract. Each checked
//! `CheckedCrashOperatorSite` in the lowered source closure therefore becomes
//! one `TerminalOperationCrashContract` row at the exact emitted operation:
//! the operator's published routes in the emitted operation's own positional
//! formal telescope (operand position `k` is formal `ValueId` `k + 1`), and
//! the continuations the verifier reconstructs by substituting the
//! operation's own operands into those routes.
//!
//! Boundary declarations index the same routes by authored parameter ordinal.
//! The two telescopes coincide whenever the emission keeps the authored
//! operand order, and differ exactly when it does not: `>` and `>=` emit the
//! reversed `IntegerLessThan`/`IntegerLessOrEqual`. The emission records that
//! mapping on its occurrence row, and the routes are reindexed through it
//! before publication, because the verifier binds formal `k + 1` to the
//! operand at position `k` and would otherwise read the other operand. A
//! mapping that cannot address the emitted operand roster exactly fails
//! closed rather than publishing a guessed order.
//!
//! Nothing here is inferred from the emitted operation: the published routes
//! come from the checked site and must agree with the operator declaration's
//! own contract plan when one exists; the operation is found only through
//! the exact source-occurrence join emission recorded (the selected
//! comparison occurrence roster, keyed by the checked `operator_use`); and
//! every crash-qualified use inside the
//! closure must produce a row, so a use whose join or contract cannot be
//! reconstructed fails closed instead of lowering as a crash-free operation.
//!
//! A named `Namespace::requirement(...)` call lowers as a `BoundaryCall`
//! operation instead of a scalar operation: its crash carrier is the call's
//! own boundary declaration, whose `crash_routes` the boundary lowering
//! already installs in the declaration-local formal telescope the verifier
//! substitutes at each call. The named site therefore installs no operation
//! row of its own — `validate_named_site_carrier` joins the checked site to
//! that declaration through the same exact source-occurrence relation and
//! fails closed when the carrier or its published routes cannot be
//! reconstructed.

use std::collections::BTreeMap;

use checked_trees::signature::{SignatureContract, SignatureContractKind};
use checked_trees::{
    CheckedCrashOperatorSite, CheckedOperatorUseHandle, CheckedTrees, CheckedValueOrigin,
};
use lowered_psi::{LoweredPsi, LoweredSelectedComparisonOperandOrder};
use semantic_vocabulary::{MachineId, OperationId, ScalarTerm, ScalarType, ValueId};
use terminal_psi::{OperationKind, TerminalMachine, TerminalOperationCrashContract};

use crate::direct_calls::direct_call_parts;
use crate::emission::scalar_types::terminal_scalar_type;
use crate::lowering_error::{LoweringError, unsupported};
use crate::proofs::crash_routes::lower_formal_crash_routes;
use crate::terminal_identities::dense_identity;

/// Install one operation crash contract row per checked operator crash site
/// owned by a machine in `source_machines`, verify each named site's
/// boundary-call carrier through `projection_sources`, and reject any
/// crash-qualified operator use in that closure that has no such site.
pub(crate) fn retain_operation_crash_contracts(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    source_machines: &[symbols::SymbolHandle],
    projection_sources: &[(symbols::SymbolHandle, MachineId)],
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
        .map(|site| lower_site(identities, checked, projection_sources, lowered, site))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    projection_sources: &[(symbols::SymbolHandle, MachineId)],
    lowered: &LoweredPsi,
    site: &CheckedCrashOperatorSite,
) -> Result<Option<TerminalOperationCrashContract>, LoweringError> {
    if site.named_use.is_valid() {
        return validate_named_site_carrier(identities, checked, projection_sources, lowered, site)
            .map(|()| None);
    }
    let (machine_id, operation_id, operand_order) = emitted_operation(lowered, site)?;
    let machine = lowered
        .semantic_module
        .machine(machine_id)
        .ok_or(LoweringError::Unsupported(
            "operator crash site joins a Terminal machine outside the lowered module",
        ))?;
    let operation = machine
        .operation(operation_id)
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
    // The declaration binds authored parameter ordinals; the emitted operation
    // binds positions. Read the declaration through the emission's own
    // mapping, then reindex the lowered routes into the operation's telescope.
    let authored_positions = (0..operands.len())
        .map(|ordinal| {
            operand_order
                .terminal_operand_position(ordinal, operands.len())
                .ok_or(LoweringError::Unsupported(
                    "operator crash site operand mapping does not address the emitted operand roster",
                ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let authored_operand_types = authored_positions
        .iter()
        .map(|position| operand_types[*position])
        .collect::<Vec<_>>();
    validate_operator_signature(checked, site, &authored_operand_types)?;
    let published_routes = reindex_formal_telescope(
        lower_formal_crash_routes(
            identities,
            published_buckets(checked, site)?,
            &authored_operand_types,
        )?,
        &authored_positions,
        &authored_operand_types,
    )?;
    if published_routes.is_empty() {
        return unsupported("operator crash site publishes no lowerable crash route");
    }
    let substitutions = operands
        .iter()
        .zip(&operand_types)
        .enumerate()
        .map(|(ordinal, (operand, scalar_type))| {
            Ok((
                formal_identity(ordinal)?,
                ScalarTerm::value(*operand, *scalar_type),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
    let crash_continuations = terminal_semantics::substitution::substitute_crash_routes(
        &published_routes,
        &substitutions,
    );
    Ok(Some(TerminalOperationCrashContract {
        machine: machine_id,
        operation: operation_id,
        published_routes,
        crash_continuations,
    }))
}

/// A named `Namespace::requirement(...)` call's crash carrier is the
/// `BoundaryCall` operation's own boundary declaration: the boundary lowering
/// already publishes the checked routes on that declaration, and the
/// verifier substitutes the call's actuals into them. Verify the whole join
/// rather than trusting a shape — the named call's authored expression and
/// checked selected identity name exactly one source-occurrence row; that
/// row lands on a `BoundaryCall` inside the site's own lowered machine; and
/// the call's boundary declaration is an operator requirement whose scalar
/// telescope agrees with the site's declaration and which publishes exactly
/// the site's checked routes. Any drift means the contract has no Terminal
/// carrier and the program fails closed instead of lowering crash-free.
fn validate_named_site_carrier(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    projection_sources: &[(symbols::SymbolHandle, MachineId)],
    lowered: &LoweredPsi,
    site: &CheckedCrashOperatorSite,
) -> Result<(), LoweringError> {
    let named_use = checked.facts.operators.named_uses.get(site.named_use);
    let CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        ..
    } = named_use.origin
    else {
        return unsupported("named operator crash invocation has no machine-local origin");
    };
    let source_site = checked_trees::NominalMachineUseSite::Expression(named_use.expression);
    let mut joins = lowered.source_call_occurrences.iter().filter(|occurrence| {
        occurrence.source_site == Some(source_site)
            && occurrence.source_state == state_symbol
            && occurrence.source_target == site.selected_operator
    });
    let join = match (joins.next(), joins.next()) {
        (Some(join), None) => join,
        (None, _) => {
            return unsupported(
                "named operator crash invocation has no emitted boundary call to carry its contract",
            );
        }
        (Some(_), Some(_)) => {
            return unsupported("named operator crash invocation joins more than one emitted call");
        }
    };
    let machine_id = projection_sources
        .iter()
        .find_map(|(source, id)| (*source == machine_symbol).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "named operator crash invocation's machine has no Terminal lowering",
        ))?;
    let machine = lowered
        .semantic_module
        .machine(machine_id)
        .ok_or(LoweringError::Unsupported(
            "named operator crash invocation joins a Terminal machine outside the lowered module",
        ))?;
    let operation = machine
        .operations()
        .find(|operation| join.terminal_site.operation() == Some(operation.id))
        .ok_or(LoweringError::Unsupported(
            "named operator crash invocation joins an operation outside its Terminal machine",
        ))?;
    let terminal_psi::OperationKind::BoundaryCall {
        boundary,
        arguments,
        ..
    } = &operation.kind
    else {
        return unsupported(
            "named operator crash invocation joins an operation that is not its boundary call",
        );
    };
    let declaration =
        lowered
            .semantic_module
            .boundary_machine(*boundary)
            .ok_or(LoweringError::Unsupported(
                "named operator crash invocation's boundary call names no retained declaration",
            ))?;
    if declaration.kind != semantic_vocabulary::BoundaryRequirementKind::Operator
        || arguments.len() != declaration.scalar_parameters.len()
    {
        return unsupported(
            "named operator crash invocation's boundary call disagrees with the operator requirement shape",
        );
    }
    validate_operator_signature(checked, site, &declaration.scalar_parameters)?;
    if declaration.crash_routes
        != lower_formal_crash_routes(
            identities,
            published_buckets(checked, site)?,
            &declaration.scalar_parameters,
        )?
    {
        return unsupported(
            "named operator crash invocation's boundary declaration publishes different crash routes than its checked site",
        );
    }
    Ok(())
}

/// The exact emitted operation for one site, and how that operation reads the
/// operator's authored operands. Only emission's own source-occurrence joins
/// may answer, and every join is keyed by the same checked `operator_use`
/// identity whichever scalar family emitted it; a positional or shape-based
/// guess would let one comparison borrow another's contract, and an inferred
/// operand order would let it borrow the other operand.
fn emitted_operation(
    lowered: &LoweredPsi,
    site: &CheckedCrashOperatorSite,
) -> Result<
    (
        MachineId,
        OperationId,
        LoweredSelectedComparisonOperandOrder,
    ),
    LoweringError,
> {
    let operator_use: CheckedOperatorUseHandle = site.operator_use;
    // Every selected comparison records where its emitted operation reads the
    // authored operands: IEEE float rows always read the authored order, and
    // `>`/`>=` integer rows record `Swapped`.
    let mut joins = lowered
        .selected_comparison_occurrences
        .iter()
        .filter(|occurrence| occurrence.operator_use == operator_use)
        .map(|occurrence| {
            (
                occurrence.terminal_machine,
                occurrence.terminal_operation,
                occurrence.operand_order,
            )
        });
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

/// Rewrite routes lowered over the operator declaration's authored formal
/// telescope into the emitted operation's positional one.
///
/// The verifier binds formal `k + 1` to the operand at position `k` and
/// recomputes the continuations itself, so an emission that reversed the
/// authored pair must publish the reversed formal identities; publishing the
/// authored ones would make the verifier read the other operand. An
/// authored-order emission already sits in that telescope and keeps its
/// routes exactly as lowered rather than passing through a rewrite.
fn reindex_formal_telescope(
    routes: Vec<terminal_psi::CrashRouteBucket>,
    authored_positions: &[usize],
    authored_operand_types: &[ScalarType],
) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
    if authored_positions
        .iter()
        .enumerate()
        .all(|(ordinal, position)| ordinal == *position)
    {
        return Ok(routes);
    }
    let renaming = authored_positions
        .iter()
        .zip(authored_operand_types)
        .enumerate()
        .map(|(ordinal, (position, scalar_type))| {
            Ok((
                formal_identity(ordinal)?,
                ScalarTerm::value(formal_identity(*position)?, *scalar_type),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
    Ok(terminal_semantics::substitution::substitute_crash_routes(
        &routes, &renaming,
    ))
}

/// The formal `ValueId` of one zero-based scalar operand ordinal.
fn formal_identity(ordinal: usize) -> Result<ValueId, LoweringError> {
    ValueId::new(dense_identity(ordinal)?).ok_or(LoweringError::Unsupported(
        "operator crash formal identity is zero",
    ))
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
    typed_trees::operator::declaration_by_symbol(program, symbol)
        .map(|operator| (program.operator_parameters(operator), operator.contracts))
}

/// The operation's direct scalar operands in authored position order. Calls
/// carry their own continuations and never take a row. No legitimate join
/// lands on a call — the occurrence rosters only record comparison
/// emissions — so a call here means the join itself is corrupt; the site is
/// rejected rather than assigned a guessed positional telescope.
fn positional_scalar_operands(kind: &OperationKind) -> Result<Vec<ValueId>, LoweringError> {
    if is_call_operation(kind) {
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

/// A direct, dynamic or boundary call.
fn is_call_operation(kind: &OperationKind) -> bool {
    direct_call_parts(kind).is_some() || matches!(kind, OperationKind::BoundaryCall { .. })
}

fn scalar_value_type(machine: &TerminalMachine, value: ValueId) -> Option<ScalarType> {
    machine
        .parameters
        .iter()
        .chain(machine.block_parameters())
        .map(|declaration| (declaration.id, declaration.scalar_type))
        .chain(
            machine
                .operations()
                .filter_map(|operation| operation.result.scalar())
                .map(|declaration| (declaration.id, declaration.scalar_type)),
        )
        .find_map(|(id, scalar_type)| (id == value).then_some(scalar_type))
}
