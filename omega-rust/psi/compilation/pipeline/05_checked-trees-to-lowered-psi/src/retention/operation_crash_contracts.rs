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
//! the exact source-occurrence join emission recorded (the selected IEEE
//! float and selected integer comparison occurrence rosters, both keyed by
//! the checked `operator_use`); and every crash-qualified use inside the
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
use lowered_psi::{LoweredPsi, LoweredSelectedIntegerComparisonOperandOrder};
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
        .map(|site| lower_site(checked, projection_sources, lowered, site))
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
    checked: &CheckedTrees,
    projection_sources: &[(symbols::SymbolHandle, MachineId)],
    lowered: &LoweredPsi,
    site: &CheckedCrashOperatorSite,
) -> Result<Option<TerminalOperationCrashContract>, LoweringError> {
    if site.named_use.is_valid() {
        return validate_named_site_carrier(checked, projection_sources, lowered, site)
            .map(|()| None);
    }
    let (machine_id, operation_id, operand_order) = emitted_operation(lowered, site)?;
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
        lower_formal_crash_routes(published_buckets(checked, site)?, &authored_operand_types)?,
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
    let crash_continuations =
        terminal_verifier::substitute_crash_routes(&published_routes, &substitutions);
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
        .machines
        .iter()
        .find(|machine| machine.id == machine_id)
        .ok_or(LoweringError::Unsupported(
            "named operator crash invocation joins a Terminal machine outside the lowered module",
        ))?;
    let operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == join.terminal_operation)
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
    let declaration = lowered
        .semantic_module
        .boundary_machines
        .iter()
        .find(|declaration| declaration.id == *boundary)
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
        LoweredSelectedIntegerComparisonOperandOrder,
    ),
    LoweringError,
> {
    let operator_use: CheckedOperatorUseHandle = site.operator_use;
    // `IeeeFloatCompare` carries its own comparison identity, so an IEEE
    // comparison is always emitted over the authored operand order.
    let float_joins = lowered
        .selected_ieee_float_comparison_occurrences
        .iter()
        .filter(|occurrence| occurrence.operator_use == operator_use)
        .map(|occurrence| {
            (
                occurrence.terminal_machine,
                occurrence.terminal_operation,
                LoweredSelectedIntegerComparisonOperandOrder::Authored,
            )
        });
    let integer_joins = lowered
        .selected_integer_comparison_occurrences
        .iter()
        .filter(|occurrence| occurrence.operator_use == operator_use)
        .map(|occurrence| {
            (
                occurrence.terminal_machine,
                occurrence.terminal_operation,
                occurrence.operand_order,
            )
        });
    let mut joins = float_joins.chain(integer_joins);
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
    Ok(terminal_verifier::substitute_crash_routes(
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
    direct_call_parts(kind).is_some()
        || matches!(
            kind,
            OperationKind::CallDynamicScalar { .. }
                | OperationKind::CallDynamicParameterScalar { .. }
                | OperationKind::CallDynamicUnit { .. }
                | OperationKind::CallDynamicParameterUnit { .. }
                | OperationKind::BoundaryCall { .. }
        )
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

#[cfg(test)]
mod tests {
    use super::{CheckedTrees, LoweredPsi, LoweringError, retain_operation_crash_contracts};
    use crate::TerminalMachineSelection;

    /// A crash-qualified positional `==` use beside an ordinary machine call:
    /// the checked site's honest join lands on the emitted `IntegerEqual`,
    /// while the call carries its own `crash_continuations`. Corrupting the
    /// recorded occurrence join must fail closed in both directions below.
    const CALL_BESIDE_COMPARISON_SOURCE: &str = r#"
        boundary operator == Meaning::equal(left: u16, right: u16) -> bool crashes Trap;
        machine other(value: u16) -> u16 { value }
        machine choose(left: u16, right: u16) -> bool { other(left) == right }
    "#;

    fn checked(source: &str) -> CheckedTrees {
        crate::front_end::checked_program(source)
    }

    /// Re-run the retention pass on the already-lowered module after the test
    /// rewrites the emission's own occurrence joins. The fixtures select one
    /// machine, so its projection is the module entry.
    fn retain_again(checked: &CheckedTrees, lowered: &mut LoweredPsi) -> LoweringError {
        lowered.semantic_module.operation_crash_contracts.clear();
        let source_machines = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .map(|selection| selection.machine)
            .collect::<Vec<_>>();
        let projection_sources = entry_projection(checked, lowered);
        retain_operation_crash_contracts(checked, &source_machines, &projection_sources, lowered)
            .expect_err("the rewritten occurrence join must fail closed")
    }

    /// The sole recorded integer-comparison join for the fixture's one `==`
    /// use, mutably, so the test can point it at a different carrier.
    fn sole_occurrence(
        lowered: &mut LoweredPsi,
    ) -> &mut lowered_psi::LoweredSelectedIntegerComparisonOccurrence {
        let [occurrence] = lowered
            .selected_integer_comparison_occurrences
            .as_mut_slice()
        else {
            panic!("the selected comparison emits one occurrence row")
        };
        occurrence
    }

    #[test]
    fn crash_contract_rejects_a_call_operation_carrier() {
        let checked = checked(CALL_BESIDE_COMPARISON_SOURCE);
        let mut lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
            .expect("a crash-qualified comparison beside a call lowers");
        assert_eq!(
            lowered.semantic_module.operation_crash_contracts.len(),
            1,
            "the honest join installs one operation crash contract"
        );
        let (call_machine, call_operation) = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| block.operations.iter().map(|op| (machine.id, op)))
            })
            .find_map(|(machine, operation)| {
                super::is_call_operation(&operation.kind).then_some((machine, operation.id))
            })
            .expect("choose emits one call operation beside the comparison");
        let occurrence = sole_occurrence(&mut lowered);
        occurrence.terminal_machine = call_machine;
        occurrence.terminal_operation = call_operation;
        let error = retain_again(&checked, &mut lowered);
        assert!(
            matches!(
                error,
                LoweringError::Unsupported(message) if message.contains("call operation")
            ),
            "a call operation must not carry a positional crash row: {error:?}"
        );
    }

    #[test]
    fn crash_contract_rejects_a_site_with_no_emitted_join() {
        let checked = checked(CALL_BESIDE_COMPARISON_SOURCE);
        let mut lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
            .expect("a crash-qualified comparison beside a call lowers");
        lowered.selected_integer_comparison_occurrences.clear();
        let error = retain_again(&checked, &mut lowered);
        assert!(
            matches!(
                error,
                LoweringError::Unsupported(message)
                    if message.contains("no emitted Terminal operation")
            ),
            "a crash-qualified use without an emitted join must fail closed: {error:?}"
        );
    }

    /// A crash-qualified `Namespace::requirement(...)` call lowers as a
    /// `BoundaryCall` whose own boundary declaration carries the published
    /// routes. No `operation_crash_contracts` row is installed — the
    /// declaration is the contract's carrier — but the checked site must
    /// still join one exact emitted boundary call.
    const NAMED_CRASH_CALL_SOURCE: &str = r#"
        boundary operator == Meaning::equal(left: u16, right: u16) -> bool crashes Trap;
        machine choose(left: u16, right: u16) -> bool { Meaning::equal(left, right) }
    "#;

    /// The named crash invocation beside an unrelated primitive comparison:
    /// `choose` emits an `IntegerLessThan` the corrupted join can point at, a
    /// scalar operation that is not the boundary call.
    const NAMED_CRASH_CALL_BESIDE_COMPARISON_SOURCE: &str = r#"
        boundary operator == Meaning::equal(left: u16, right: u16) -> bool crashes Trap;
        machine choose(left: u16, right: u16) -> bool {
            let order: bool = left < right;
            Meaning::equal(left, right) && order
        }
    "#;

    /// The entry machine's projection for the single-machine fixtures, so the
    /// named-site carrier check can rejoin its own lowered machine.
    fn entry_projection(
        checked: &CheckedTrees,
        lowered: &LoweredPsi,
    ) -> Vec<(symbols::SymbolHandle, semantic_vocabulary::MachineId)> {
        checked
            .typed
            .machines()
            .iter()
            .filter(|machine| machine.name.as_str() == "choose")
            .map(|machine| (machine.symbol, lowered.semantic_module.entry))
            .collect()
    }

    #[test]
    fn named_operator_crash_invocation_lowers_through_its_boundary_call() {
        let checked = checked(NAMED_CRASH_CALL_SOURCE);
        let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
            .expect("a named crash invocation lowers through its boundary call");
        assert!(
            lowered.semantic_module.operation_crash_contracts.is_empty(),
            "the boundary declaration carries the named call's contract, not an operation row"
        );
        let declaration = lowered
            .semantic_module
            .boundary_machines
            .iter()
            .find(|declaration| declaration.identity.contains("equal"))
            .expect("the named call retains its boundary declaration");
        assert_eq!(
            declaration.kind,
            semantic_vocabulary::BoundaryRequirementKind::Operator,
        );
        assert!(
            !declaration.crash_routes.is_empty(),
            "the declaration publishes the named call's checked crash routes"
        );
        assert!(
            lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| machine.blocks.iter())
                .flat_map(|block| block.operations.iter())
                .any(|operation| matches!(
                    &operation.kind,
                    terminal_psi::OperationKind::BoundaryCall { boundary, .. }
                        if *boundary == declaration.id
                )),
            "the named call emits a BoundaryCall on its retained declaration"
        );
        terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("the named crash invocation's module verifies");
    }

    #[test]
    fn named_operator_crash_invocation_rejects_a_missing_occurrence_join() {
        let checked = checked(NAMED_CRASH_CALL_SOURCE);
        let mut lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
            .expect("a named crash invocation lowers through its boundary call");
        lowered.source_call_occurrences.clear();
        let source_machines = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .map(|selection| selection.machine)
            .collect::<Vec<_>>();
        let error = retain_operation_crash_contracts(
            &checked,
            &source_machines,
            &entry_projection(&checked, &lowered),
            &mut lowered,
        )
        .expect_err("a missing occurrence join must fail closed");
        assert!(
            matches!(
                error,
                LoweringError::Unsupported(message)
                    if message.contains("no emitted boundary call")
            ),
            "a named crash invocation without an occurrence join must not lower: {error:?}"
        );
    }

    #[test]
    fn named_operator_crash_invocation_rejects_a_non_boundary_carrier() {
        let checked = checked(NAMED_CRASH_CALL_BESIDE_COMPARISON_SOURCE);
        let mut lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
            .expect("a named crash invocation lowers through its boundary call");
        // Point the exact occurrence at an operation that is not the boundary
        // call: the contract has no positional carrier on a scalar operation.
        let stray_operation = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| machine.blocks.iter())
            .flat_map(|block| block.operations.iter())
            .find_map(|operation| {
                (!matches!(
                    &operation.kind,
                    terminal_psi::OperationKind::BoundaryCall { .. }
                ))
                .then_some(operation.id)
            })
            .expect("choose emits a non-boundary operation beside the call");
        lowered
            .source_call_occurrences
            .iter_mut()
            .for_each(|occurrence| occurrence.terminal_operation = stray_operation);
        let source_machines = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .map(|selection| selection.machine)
            .collect::<Vec<_>>();
        let error = retain_operation_crash_contracts(
            &checked,
            &source_machines,
            &entry_projection(&checked, &lowered),
            &mut lowered,
        )
        .expect_err("a non-boundary carrier must fail closed");
        assert!(
            matches!(
                error,
                LoweringError::Unsupported(message)
                    if message.contains("is not its boundary call")
            ),
            "a named crash invocation must not borrow an arbitrary operation: {error:?}"
        );
    }

    #[test]
    fn crash_contract_rejects_a_duplicated_join() {
        let checked = checked(CALL_BESIDE_COMPARISON_SOURCE);
        let mut lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
            .expect("a crash-qualified comparison beside a call lowers");
        let duplicated = *sole_occurrence(&mut lowered);
        lowered
            .selected_integer_comparison_occurrences
            .push(duplicated);
        let error = retain_again(&checked, &mut lowered);
        assert!(
            matches!(
                error,
                LoweringError::Unsupported(message)
                    if message.contains("more than one Terminal operation")
            ),
            "one use must join exactly one operation: {error:?}"
        );
    }

    #[test]
    fn crash_contract_rejects_a_join_outside_the_lowered_module() {
        let checked = checked(CALL_BESIDE_COMPARISON_SOURCE);
        let mut lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
            .expect("a crash-qualified comparison beside a call lowers");
        sole_occurrence(&mut lowered).terminal_machine =
            semantic_vocabulary::MachineId::new(u64::MAX).expect("nonzero machine id");
        let error = retain_again(&checked, &mut lowered);
        assert!(
            matches!(
                error,
                LoweringError::Unsupported(message)
                    if message.contains("outside the lowered module")
            ),
            "a join outside the module must not lower: {error:?}"
        );
    }

    #[test]
    fn crash_contract_rejects_an_absent_operation_join() {
        let checked = checked(CALL_BESIDE_COMPARISON_SOURCE);
        let mut lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
            .expect("a crash-qualified comparison beside a call lowers");
        sole_occurrence(&mut lowered).terminal_operation =
            semantic_vocabulary::OperationId::new(u64::MAX).expect("nonzero operation id");
        let error = retain_again(&checked, &mut lowered);
        assert!(
            matches!(
                error,
                LoweringError::Unsupported(message)
                    if message.contains("outside its Terminal machine")
            ),
            "a join naming no operation must not lower: {error:?}"
        );
    }

    /// A one-operand operation is still a valid carrier shape, so the
    /// declaration's own parameter roster must disagree — the count check is
    /// what stops a borrowed contract from landing on the wrong telescope.
    #[test]
    fn crash_contract_rejects_an_operand_roster_mismatch() {
        let checked = checked(
            r#"
            boundary operator == Meaning::equal(left: u16, right: u16) -> bool crashes Trap;
            machine choose(flag: bool, left: u16, right: u16) -> bool { !flag || (left == right) }
            "#,
        );
        let mut lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
            .expect("a crash-qualified comparison beside a negation lowers");
        let unary = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| {
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| block.operations.iter().map(|op| (machine.id, op)))
            })
            .find_map(|(machine, operation)| {
                let mut kind = operation.kind.clone();
                let mut count = 0usize;
                kind.map_scalar_uses(&mut |operand| {
                    count += 1;
                    operand
                });
                (count == 1).then_some((machine, operation.id))
            })
            .expect("choose emits one unary operation beside the comparison");
        let occurrence = sole_occurrence(&mut lowered);
        occurrence.terminal_machine = unary.0;
        occurrence.terminal_operation = unary.1;
        let error = retain_again(&checked, &mut lowered);
        assert!(
            matches!(
                error,
                LoweringError::Unsupported(message)
                    if message.contains("operand roster disagrees")
            ),
            "a mismatched operand telescope must not carry the contract: {error:?}"
        );
    }

    #[test]
    fn crash_contract_rejects_two_sites_on_one_operation() {
        let checked = checked(
            r#"
            boundary operator == Meaning::equal(left: u16, right: u16) -> bool crashes Trap;
            machine choose(left: u16, right: u16, extra: u16) -> bool { (left == right) || (extra == left) }
            "#,
        );
        let mut lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
            .expect("two crash-qualified comparisons lower");
        assert_eq!(
            lowered.semantic_module.operation_crash_contracts.len(),
            2,
            "each selected comparison installs its own operation crash contract"
        );
        let [first, second] = lowered
            .selected_integer_comparison_occurrences
            .as_mut_slice()
        else {
            panic!("two selected comparisons emit two occurrence rows")
        };
        second.terminal_operation = first.terminal_operation;
        let error = retain_again(&checked, &mut lowered);
        assert!(
            matches!(
                error,
                LoweringError::Unsupported(message) if message.contains("collide")
            ),
            "two sites must not share one carrier: {error:?}"
        );
    }

    #[test]
    fn crash_contract_rejects_reinstallation() {
        let checked = checked(CALL_BESIDE_COMPARISON_SOURCE);
        let mut lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
            .expect("a crash-qualified comparison beside a call lowers");
        // The honest install is still present, so a second pass must refuse
        // rather than rewrite the carrier.
        let source_machines = checked
            .facts
            .flow
            .terminal_machines
            .machines
            .iter()
            .map(|selection| selection.machine)
            .collect::<Vec<_>>();
        let error = retain_operation_crash_contracts(&checked, &source_machines, &[], &mut lowered)
            .expect_err("reinstalling over existing rows must fail closed");
        assert!(
            matches!(
                error,
                LoweringError::Unsupported(message) if message.contains("already installed")
            ),
            "a second install must not silently rewrite contracts: {error:?}"
        );
    }
    /// A guarded float operator route carries a structured scalar proposition
    /// when the guard's relation has builtin IEEE meaning, which here means a
    /// spelling the boundary does not itself declare. `!(left < right)` lowers
    /// to the unordered complement, `left > right` to `<` over swapped
    /// operands, and `left <= right` and `left != right` directly. Every form
    /// reads the operator's formal telescope (operand `k` is formal `k + 1`)
    /// and the verifier accepts the installed row.
    #[test]
    fn crash_contract_installs_a_scalar_ieee_float_guard() {
        use semantic_vocabulary::{
            IeeeFloatComparisonKind, IeeeFloatFormat, Proposition, ScalarTerm, ScalarType, ValueId,
        };
        use terminal_psi::CrashRouteGuard;

        let float = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
        let formal = |raw: u64| ScalarTerm::value(ValueId::new(raw).expect("formal"), float);
        for (guard, kind, [left, right]) in [
            (
                "!(left < right)",
                IeeeFloatComparisonKind::NotLessThan,
                [1, 2],
            ),
            ("left > right", IeeeFloatComparisonKind::LessThan, [2, 1]),
            (
                "left <= right",
                IeeeFloatComparisonKind::LessOrEqual,
                [1, 2],
            ),
            ("left != right", IeeeFloatComparisonKind::NotEqual, [1, 2]),
        ] {
            let checked = checked(&format!(
                "boundary operator == Float::equal(left: f64, right: f64) -> bool\n\
                 crashes Trap {guard};\n\
                 machine compare(left: f64, right: f64) -> bool crashes Trap {{ left == right }}"
            ));
            let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("compare"))
                .unwrap_or_else(|error| panic!("{guard} lowers: {error:?}"));
            let [row] = lowered.semantic_module.operation_crash_contracts.as_slice() else {
                panic!("{guard}: one float use installs one operation crash contract")
            };
            let [
                terminal_psi::CrashRouteBucket {
                    cause,
                    alternatives,
                },
            ] = row.published_routes.as_slice()
            else {
                panic!("{guard}: one published route")
            };
            assert_eq!(*cause, terminal_psi::CrashCause::Trap);
            let [CrashRouteGuard::Predicate(term)] = alternatives.as_slice() else {
                panic!("{guard}: the guard is one predicate")
            };
            assert_eq!(
                term.proposition(),
                &Proposition::ScalarIeeeFloatComparison {
                    kind,
                    format: IeeeFloatFormat::Binary64,
                    left: formal(left),
                    right: formal(right),
                },
                "{guard}: scalar IEEE comparison over the formal telescope"
            );
            terminal_verifier::validate_module(&lowered.semantic_module)
                .unwrap_or_else(|error| panic!("{guard}: verifier accepts the row: {error:?}"));
        }
    }

    /// A guard that spells the boundary's own declared relation fails closed.
    /// A bodyless declaration does not grant IEEE meaning; only a builtin
    /// spelling or the sealed Float contract does, so `==` here, which
    /// resolves to this look-alike `Float::equal`, has no structured form.
    #[test]
    fn crash_contract_rejects_a_guard_spelled_through_a_declared_float_relation() {
        let checked = checked(
            "boundary operator == Float::equal(left: f64, right: f64) -> bool\n\
             crashes Trap !(left == right);\n\
             machine compare(left: f64, right: f64) -> bool crashes Trap { left == right }",
        );
        let error = crate::lower_machine(&checked, TerminalMachineSelection::Name("compare"))
            .expect_err("a declared look-alike relation has no builtin IEEE meaning");
        assert!(
            format!("{error:?}")
                .contains("guarded crash route is outside structured scalar predicate lowering"),
            "{error:?}"
        );
    }
}
