//! Join a retained call's returned descriptors to their receiving loan slots.
//! Borrow checking already admitted the complete input lifetime frontier. The
//! call planner consumes that formation rather than replaying input syntax.
//! An anonymous operand with no receiving slot remains a different lifetime
//! owner; absence of a stored receiver is not evidence that its loans exist.

use super::{RecordLeafLoans, StoredCarrierSource, local_record_loans, record_carrier_sources};
use checked_trees::{CheckFacts, CheckedUnitStructuralArgumentSourcePlan, FlowCallResult};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

pub struct RecordCallReception {
    /// Returned leaves not installed in the receiving local. These remain
    /// owned by the temporary result until projection/operand disposal.
    pub residual_paths: Vec<Vec<checked_trees::CheckedUnitStructuralPathSegment>>,
}

/// Find the exact receiving leaves of this call occurrence. Constructor
/// prefixes are consumed by the common graph selector; the remaining path
/// belongs to the declared call result. Each installed leaf must have its
/// complete formation group; uninstalled siblings retain transient custody.
pub fn record_call_reception(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: ExpressionHandle,
) -> Result<RecordCallReception, &'static str> {
    let call = program
        .expression_table
        .call_node(expression)
        .ok_or("reference reception has no authored call")?;
    let return_type = crate::call_target_return_type(program, call.target_symbol)
        .ok_or("reference reception has no destination result contract")?;
    // The checked call row carries this occurrence's actual capture: which
    // authored construct receives the result. Replaying the typed
    // `local.initial_value` re-derives a node preparation may already have
    // rewritten (substituted specialization arguments, folded constructors),
    // so a mutable holder's genuine receiver can never join its own call.
    let flow = super::state_flow(facts, machine, state.symbol)
        .ok_or("reference reception has no unique caller flow")?;
    let Some(occurrence) = facts
        .flow
        .control
        .calls
        .span(flow.calls)
        .ok_or("reference reception has a stale call span")?
        .iter()
        .find(|occurrence| {
            occurrence.authored_expression == expression
                && occurrence.statement_index == statement_index as usize
                && occurrence.target_symbol == call.target_symbol
        })
    else {
        // The semantic call roster owns every executed invocation; a
        // missing row is an incomplete relation, not an anonymous receiver.
        return Err("reference reception has no matching call occurrence");
    };
    let paths = super::captured_returns::call_result_paths(program, facts, occurrence)
        .ok_or("reference result has no complete captured origin paths")?;
    let returned: Vec<_> = record_carrier_sources(
        program,
        return_type,
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: state.symbol,
        },
    )
    .ok_or("reference result has no declared carrier frontier")?
    .into_iter()
    .filter(|leaf| paths.contains(&leaf.path))
    .collect();
    let anonymous = || RecordCallReception {
        residual_paths: returned.iter().map(|leaf| leaf.path.clone()).collect(),
    };
    let Some(local) = program
        .statement_table
        .statement_at(state, statement_index as usize)
        .and_then(typed_trees::statement::StatementNode::as_local_data)
    else {
        return Ok(anonymous());
    };
    // The completed store, not a local's typed initializer, identifies its
    // whole receiving value. Immutable and mutable bindings share this
    // capture operation; nested calls remain anonymous operands.
    let statement = facts
        .flow
        .control
        .statements
        .span(flow.statements)
        .ok_or("reference reception has a stale statement span")?
        .iter()
        .find(|statement| statement.statement_index == statement_index as usize)
        .ok_or("reference reception has no receiving statement")?;
    let receives = occurrence.authored_result != FlowCallResult::Discarded
        && facts.flow.control.operands.is_valid(statement.stored_value)
        && facts
            .flow
            .control
            .operands
            .get(statement.stored_value)
            .expression
            == expression
        && facts.semantic.places.is_valid(statement.stored_destination)
        && {
            let destination = facts.semantic.places.get(statement.stored_destination);
            destination.root == facts::PlaceRoot::Symbol(local.symbol)
                && facts
                    .semantic
                    .place_segments
                    .span(destination.segments)
                    .ok_or("reference reception has stale destination segments")?
                    .is_empty()
        };
    if !receives {
        return Ok(anonymous());
    }
    // Structural roots key on the authored value occurrence, which the
    // captured call row already binds to `expression`; the prepared
    // initializer handle is not needed and may not coincide.
    let Some(root) = facts.values.structural_values.root_for_expression(
        state.symbol,
        statement_index,
        expression,
    ) else {
        return Ok(anonymous());
    };
    if root.machine != machine {
        return Err("reference reception root belongs to another machine");
    }
    let Some(leaves) = record_carrier_sources(
        program,
        local.type_reference,
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: local.symbol,
        },
    ) else {
        return Ok(anonymous());
    };
    let mut received = Vec::new();
    for leaf in leaves {
        for source in super::stored_carrier_sources(program, facts, root.root, &leaf.path)
            .ok_or("reference reception has no complete stored carrier sources")?
        {
            let StoredCarrierSource::Call { value, path } = source else {
                continue;
            };
            let node = facts.values.structural_values.nodes.get(value);
            if node.expression == expression {
                let checked_trees::CheckedStructuralValueKind::Call { source_call } = node.kind
                else {
                    return Err("reference reception source is not a captured call");
                };
                if node.machine != machine
                    || node.state != state.symbol
                    || node.statement_index != statement_index
                    || !facts.flow.control.calls.is_valid(source_call)
                {
                    return Err("reference reception source has a different call coordinate");
                }
                let occurrence = facts.flow.control.calls.get(source_call);
                if occurrence.authored_expression != expression
                    || occurrence.statement_index != statement_index as usize
                    || occurrence.target_symbol != call.target_symbol
                {
                    return Err("reference reception source differs from its authored call");
                }
                if returned.iter().any(|leaf| leaf.path == path) {
                    received.push((leaf.path.clone(), path));
                }
            }
        }
    }
    if received.is_empty() {
        return Ok(anonymous());
    }
    let groups = local_record_loans(program, facts, machine, state, statement_index)
        .ok_or("reference reception has no complete local loan groups")?;
    let mut covered = vec![false; returned.len()];
    for (receiving_path, result_path) in received {
        let (returned_index, returned_leaf) = returned
            .iter()
            .enumerate()
            .find(|(_, leaf)| leaf.path == result_path)
            .ok_or("reference reception names an undeclared result leaf")?;
        let (receiving_leaf, group): &RecordLeafLoans = groups
            .iter()
            .find(|(leaf, _)| leaf.path == receiving_path)
            .ok_or("reference reception has no loan group for its receiving leaf")?;
        if receiving_leaf.source.type_identity != returned_leaf.source.type_identity
            || receiving_leaf.source.access != returned_leaf.source.access
        {
            return Err("reference reception changes a leaf type or access");
        }
        let mut formed = false;
        for (_, loan) in facts
            .borrow
            .loan_group(*group)
            .ok_or("reference reception has a stale loan group")?
        {
            // A dispatch can form the same slot from distinct call sites.
            // Other alternatives retain their own admission and retirement.
            if loan.formation_expression != expression {
                continue;
            }
            if loan.kind
                != returned_leaf
                    .source
                    .access
                    .borrow_access_kind()
                    .ok_or("reference reception leaf has no borrowed access")?
            {
                return Err("reference reception loan changes the returned access");
            }
            formed = true;
        }
        if !formed {
            return Err("reference reception leaf has no loan from this call formation");
        }
        covered[returned_index] = true;
    }
    Ok(RecordCallReception {
        residual_paths: returned
            .into_iter()
            .zip(covered)
            .filter_map(|(leaf, covered)| (!covered).then_some(leaf.path))
            .collect(),
    })
}
