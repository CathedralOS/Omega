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
) -> Option<RecordCallReception> {
    let call = program.expression_table.call_node(expression)?;
    let destination = program.state_by_symbol(call.target_symbol)?;
    // Compose the existing origin relation through this invocation before
    // joining captured receiver loans. The type lists possible slots; an
    // empty actual or a payload-free callee return need not supply them.
    // This selects storage, not another judgment of input loan authority.
    let origins = super::expression_sources(
        program,
        state,
        expression,
        destination.return_type,
        &mut vec![state.symbol],
        None,
    )?;
    let returned: Vec<_> = record_carrier_sources(
        program,
        destination.return_type,
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: state.symbol,
        },
    )?
    .into_iter()
    .filter(|leaf| origins.iter().any(|origin| origin.path == leaf.path))
    .collect();
    let anonymous = || RecordCallReception {
        residual_paths: returned.iter().map(|leaf| leaf.path.clone()).collect(),
    };
    // The checked call row carries this occurrence's actual capture: which
    // authored construct receives the result. Replaying the typed
    // `local.initial_value` re-derives a node preparation may already have
    // rewritten (substituted specialization arguments, folded constructors),
    // so a mutable holder's genuine receiver can never join its own call.
    let Some(occurrence) = facts
        .flow
        .control
        .calls
        .iter()
        .map(|(_, occurrence)| occurrence)
        .find(|occurrence| {
            occurrence.authored_expression == expression
                && occurrence.statement_index == statement_index as usize
                && occurrence.target_symbol == call.target_symbol
        })
    else {
        // The semantic call roster owns every executed invocation; a
        // missing row is an incomplete relation, not an anonymous receiver.
        return None;
    };
    let Some(local) = program
        .statement_table
        .statement_at(state, statement_index as usize)
        .and_then(typed_trees::statement::StatementNode::as_local_data)
    else {
        return Some(anonymous());
    };
    // Only a binding the checked relation names as receiving the whole
    // result installs leaves here: the immutable local that names the call,
    // or the mutable local an `Operand` capture feeds as its whole
    // initializer. Everything else — enclosing expressions, assignments,
    // discarded calls — owns no slot.
    let receives = match occurrence.authored_result {
        FlowCallResult::ImmutableLocal => !local.is_mutable,
        FlowCallResult::Operand => {
            local.is_mutable && local.initial_value == expression
        }
        FlowCallResult::Discarded => false,
    };
    if !receives {
        return Some(anonymous());
    }
    // Structural roots key on the authored value occurrence, which the
    // captured call row already binds to `expression`; the prepared
    // initializer handle is not needed and may not coincide.
    let Some(root) = facts.values.structural_values.root_for_expression(
        state.symbol,
        statement_index,
        expression,
    ) else {
        return Some(anonymous());
    };
    if root.machine != machine {
        return None;
    }
    let Some(leaves) = record_carrier_sources(
        program,
        local.type_reference,
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: local.symbol,
        },
    ) else {
        return Some(anonymous());
    };
    let mut received = Vec::new();
    for leaf in leaves {
        for source in super::stored_carrier_sources(program, facts, root.root, &leaf.path)? {
            let StoredCarrierSource::Call { value, path } = source else {
                continue;
            };
            let node = facts.values.structural_values.nodes.get(value);
            if node.expression == expression {
                let checked_trees::CheckedStructuralValueKind::Call { source_call } = node.kind
                else {
                    return None;
                };
                if node.machine != machine
                    || node.state != state.symbol
                    || node.statement_index != statement_index
                    || !facts.flow.control.calls.is_valid(source_call)
                {
                    return None;
                }
                let occurrence = facts.flow.control.calls.get(source_call);
                if occurrence.authored_expression != expression
                    || occurrence.statement_index != statement_index as usize
                    || occurrence.target_symbol != call.target_symbol
                {
                    return None;
                }
                if returned.iter().any(|leaf| leaf.path == path) {
                    received.push((leaf.path.clone(), path));
                }
            }
        }
    }
    if received.is_empty() {
        return Some(anonymous());
    }
    let groups = local_record_loans(program, facts, machine, state, statement_index)?;
    let mut covered = vec![false; returned.len()];
    for (receiving_path, result_path) in received {
        let (returned_index, returned_leaf) = returned
            .iter()
            .enumerate()
            .find(|(_, leaf)| leaf.path == result_path)?;
        let (receiving_leaf, group): &RecordLeafLoans = groups
            .iter()
            .find(|(leaf, _)| leaf.path == receiving_path)?;
        if receiving_leaf.source.type_identity != returned_leaf.source.type_identity
            || receiving_leaf.source.access != returned_leaf.source.access
        {
            return None;
        }
        let mut formed = false;
        for (_, loan) in facts.borrow.loan_group(*group)? {
            // A dispatch can form the same slot from distinct call sites.
            // Other alternatives retain their own admission and retirement.
            if loan.formation_expression != expression {
                continue;
            }
            if loan.kind != returned_leaf.source.access.borrow_access_kind()? {
                return None;
            }
            formed = true;
        }
        if !formed {
            return None;
        }
        covered[returned_index] = true;
    }
    Some(RecordCallReception {
        residual_paths: returned
            .into_iter()
            .zip(covered)
            .filter_map(|(leaf, covered)| (!covered).then_some(leaf.path))
            .collect(),
    })
}
