//! Whole plain-record locals end in one exact initializer transfer or lexical drop.
//! Moving storage preserves the value's original establishment provenance.

use checked_trees::{CheckFacts, CheckedStructuralAccess, CheckedStructuralValueKind};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::{StatementNode, TableLocalData};

/// Returns whether this local retains an affine obligation at state exit.
/// Unrestricted values return false. Unsupported custody or malformed ownership
/// returns None; callers must not interpret that as an absent obligation.
/// Parameter-origin moves and selected ownership retain their separate contracts.
/// Callers retain record shape admission and independent authored-value replay;
/// this join matches the local ledger to retained initializer Place occurrences.
pub fn record_local_disposition(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    ordinal: u32,
) -> Option<bool> {
    let owner = program
        .machines()
        .iter()
        .find(|owner| owner.symbol == machine)?;
    let source = program
        .machine_states(owner)
        .iter()
        .find(|source| source.symbol == state)?;
    let statements = program.statement_table.statements(source.statement_nodes);
    let StatementNode::LocalData(local) = statements.get(ordinal as usize)? else {
        return None;
    };
    if !local.symbol.is_valid()
        || program.state_parameters(source).iter().any(|parameter| parameter.symbol == local.symbol)
        || statements.iter().filter(|statement| matches!(statement, StatementNode::LocalData(candidate) if candidate.symbol == local.symbol)).count() != 1
        || !crate::has_plain_owned_contents_with_numeric_constraints(program, local.type_reference)
    {
        return None;
    }
    let events = || {
        facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == machine
                    && event.state_symbol == state
                    && event.root == facts::PlaceRoot::Symbol(local.symbol)
            })
    };
    let multiplicity = program.type_multiplicity(local.type_reference);
    if multiplicity == Multiplicity::Unrestricted {
        return events().next().is_none().then_some(false);
    }
    if multiplicity != Multiplicity::Affine || events().count() != 2 {
        return None;
    }
    let establishment = PermissionEventSource::Statement {
        statement_index: ordinal as usize,
    };
    let provenance = local_provenance(program, statements, machine, state, ordinal as usize)?;
    let mut established = false;
    let mut disposition = None;
    for event in events() {
        if event.multiplicity != multiplicity
            || event.access != PermissionAccess::Owned
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != provenance
            || event.obligation_live
            || !facts
                .flow
                .ownership
                .segments
                .span(event.segments)?
                .is_empty()
        {
            return None;
        }
        match (event.kind, event.source) {
            (PermissionEventKind::Establish, source) if source == establishment && !established => {
                established = true;
            }
            (PermissionEventKind::AffineDrop, PermissionEventSource::StateExit)
                if disposition.is_none() =>
            {
                disposition = Some(true);
            }
            (
                PermissionEventKind::Transfer,
                PermissionEventSource::Statement { statement_index },
            ) if statement_index > ordinal as usize && disposition.is_none() => {
                initializer_transfer(
                    program,
                    facts,
                    statements,
                    machine,
                    state,
                    statement_index,
                    local,
                )?;
                disposition = Some(false);
            }
            _ => return None,
        }
    }
    if established { disposition } else { None }
}

fn local_provenance(
    program: &TypedTrees,
    statements: &[StatementNode],
    machine: SymbolHandle,
    state: SymbolHandle,
    ordinal: usize,
) -> Option<PermissionProvenance> {
    let StatementNode::LocalData(local) = statements.get(ordinal)? else {
        return None;
    };
    let origin =
        crate::expression_permission_provenance(program, local.initial_value, &mut |expression| {
            let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
                return Ok(None);
            };
            if !name.symbol.is_valid() {
                return Err("record origin has an invalid source");
            }
            if name.symbol != name.head_symbol || name.members.count() != 1 {
                return if expression == local.initial_value {
                    Err("record origin is not a whole source")
                } else {
                    Ok(None)
                };
            }
            let mut origins = statements
                .iter()
                .enumerate()
                .filter_map(|(position, statement)| match statement {
                    StatementNode::LocalData(origin) if origin.symbol == name.symbol => {
                        Some((position, origin))
                    }
                    _ => None,
                });
            let Some((position, origin)) = origins.next() else {
                let parameter = program
                    .machines()
                    .iter()
                    .find(|owner| owner.symbol == machine)
                    .and_then(|owner| {
                        program
                            .machine_states(owner)
                            .iter()
                            .find(|source| source.symbol == state)
                    })
                    .and_then(|source| {
                        program
                            .state_parameters(source)
                            .iter()
                            .find(|parameter| parameter.symbol == name.symbol)
                    })
                    .ok_or("record origin has no source declaration")?;
                return if program.type_multiplicity(parameter.type_reference)
                    == Multiplicity::Unrestricted
                {
                    Ok(None)
                } else {
                    Err("record parameter origin requires its own custody join")
                };
            };
            if origins.next().is_some()
                || position >= ordinal
                || (expression == local.initial_value
                    && program.normalized_type_identity(origin.type_reference)
                        != program.normalized_type_identity(local.type_reference))
            {
                return Err("record origin changed its prior source declaration");
            }
            if program.type_multiplicity(origin.type_reference) == Multiplicity::Unrestricted {
                return Ok(None);
            }
            local_provenance(program, statements, machine, state, position)
                .map(Some)
                .ok_or("record origin lost its prior establishment")
        })
        .ok()?;
    Some(origin.unwrap_or(PermissionProvenance::Established {
        machine_symbol: machine,
        state_symbol: state,
        source: PermissionEventSource::Statement {
            statement_index: ordinal,
        },
    }))
}

fn initializer_transfer(
    program: &TypedTrees,
    facts: &CheckFacts,
    statements: &[StatementNode],
    machine: SymbolHandle,
    state: SymbolHandle,
    ordinal: usize,
    source: &TableLocalData,
) -> Option<()> {
    let StatementNode::LocalData(destination) = statements.get(ordinal)? else {
        return None;
    };
    let plans = &facts.values.structural_values;
    let root = plans.root_at(state, u32::try_from(ordinal).ok()?)?;
    if root.machine != machine
        || root.expression != destination.initial_value
        || root.type_reference != destination.type_reference
    {
        return None;
    }
    let mut pending = vec![root.root];
    let mut visited = Vec::new();
    let mut transfers = 0;
    while let Some(handle) = pending.pop() {
        if !plans.nodes.is_valid(handle) || visited.contains(&handle) {
            return None;
        }
        visited.push(handle);
        let node = plans.nodes.get(handle);
        match &node.kind {
            CheckedStructuralValueKind::Record { fields, .. } => {
                for field in plans.record_fields.span(*fields)? {
                    if let checked_trees::CheckedStructuralRecordFieldValue::Structural(child) =
                        field.value
                    {
                        pending.push(child);
                    }
                }
            }
            CheckedStructuralValueKind::Place(argument) => {
                if argument.source
                    != (checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol: source.symbol,
                    })
                {
                    continue;
                }
                if !program
                    .expression_table
                    .expression_is_valid(node.expression)
                {
                    return None;
                }
                let ExpressionNode::Name(name) =
                    program.expression_table.expression(node.expression)
                else {
                    return None;
                };
                if name.symbol != source.symbol
                    || name.head_symbol != source.symbol
                    || name.members.count() != 1
                    || argument.access != CheckedStructuralAccess::Owned
                    || !argument.path.is_empty()
                    || argument.type_identity
                        != program
                            .normalized_type_identity(source.type_reference)
                            .as_str()
                {
                    return None;
                }
                transfers += 1;
            }
            _ => return None,
        }
    }
    (transfers == 1).then_some(())
}
