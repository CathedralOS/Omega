//! Rejoin whole affine parameter custody without duplicating path frontiers.

use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarComputationPlans, CheckedStructuralAccess,
    CheckedUnitStructuralParameterPlan, FlowOwnershipFacts,
};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, state::State};

pub(super) fn validate(
    program: &TypedTrees,
    ownership: &FlowOwnershipFacts,
    control: &checked_trees::FlowControlFacts,
    computations: &CheckedScalarComputationPlans,
    structural_values: &checked_trees::CheckedStructuralValuePlans,
    machine: SymbolHandle,
    state: &State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    transition_transfers: &[(PermissionEventSource, facts::PlaceRoot)],
) -> Option<()> {
    if !parameters
        .iter()
        .any(|parameter| parameter.access == CheckedStructuralAccess::Owned)
    {
        return Some(());
    }
    let mut expected_transfers = Vec::new();
    for root in super::computation_walk::state_computation_roots(computations, state.symbol) {
        if root.machine != machine {
            return None;
        }
        super::computation_walk::walk_computation(computations, root, |pending, kind, root| {
            let CheckedScalarComputationKind::Call {
                source_call,
                callee,
                call_ordinal,
                ..
            } = kind
            else {
                return Some(());
            };
            if !control.calls.is_valid(*source_call) {
                return None;
            }
            let call = control.calls.get(*source_call);
            let retained = control.call_at(
                state.symbol,
                usize::try_from(root.statement_ordinal).ok()?,
                usize::try_from(*call_ordinal).ok()?,
            )?;
            if !std::ptr::eq(call, retained)
                || control.is_retired(state.symbol, call)
                || validation::exact_external_leaf_boundary_requirement(program, call.target_symbol)
                    .map_or(call.target_symbol, |(requirement, _)| requirement)
                    != callee.called_signature()
            {
                return None;
            }
            super::computation_walk::expand_call_operands(computations, kind, pending, |argument| {
                if argument.access != CheckedStructuralAccess::Owned {
                    return Some(());
                }
                if let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                            expression,
                        } = argument.source
                        {
                            let value = structural_values.root_for_expression(
                                state.symbol,
                                root.statement_ordinal,
                                expression,
                            )?;
                            let contract = validation::structural_result_contract(program, value.type_reference).ok()?;
                            if value.machine != machine
                                || !structural_values.nodes.is_valid(value.root)
                                || structural_values.nodes.get(value.root).expression != expression
                                || (argument.path.is_empty()
                                    && program.normalized_type_identity(contract.carrier).as_str()
                                        != argument.type_identity)
                            {
                                return None;
                            }
                            // A produced owner (whole or projected) is not an
                            // ingress parameter. Its child transfer and residual
                            // disposition belong to the consuming call.
                            // Its scalar children own separate computation
                            // roots. Any embedded affine parameter transfer
                            // must still match the event ledger below; this
                            // classification grants no missing move receipt.
                            return Some(());
                        }
                let parameter = parameters.get(argument.source_parameter_index()? as usize)?;
                if parameter.access != CheckedStructuralAccess::Owned
                    || parameter.type_identity != argument.type_identity
                    || !argument.path.is_empty()
                {
                    return None;
                }
                if parameter.multiplicity != Multiplicity::Affine {
                    return Some(());
                }
                let source = program
                    .state_parameters(state)
                    .get(parameter.position as usize)?;
                let transfer = (
                    PermissionEventSource::Call {
                        statement_index: usize::try_from(root.statement_ordinal).ok()?,
                        call_ordinal: usize::try_from(*call_ordinal).ok()?,
                        target_symbol: call.target_symbol,
                    },
                    facts::PlaceRoot::Symbol(source.symbol),
                );
                if expected_transfers.contains(&transfer) {
                    return None;
                }
                expected_transfers.push(transfer);
                Some(())
            })
        })?;
    }
    for transfer in transition_transfers {
        if expected_transfers.contains(transfer) {
            return None;
        }
        expected_transfers.push(*transfer);
    }
    // Multiplicity records each transition arm against a clone of the live
    // prefix. StateExit disposal consequently excludes prefix moves only;
    // the retained computation selections still determine each runtime edge.
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix_end = statements
        .iter()
        .position(|statement| {
            matches!(
                statement,
                typed_trees::statement::StatementNode::Transition(_)
            )
        })
        .unwrap_or(statements.len());
    // A record local may draw its whole initial value straight from an affine
    // owned parameter; the local's own custody ledger (which must succeed
    // before the graph publishes) then owns the follow-on disposition.
    for (statement_index, statement) in statements[..prefix_end].iter().enumerate() {
        let typed_trees::statement::StatementNode::LocalData(local) = statement else {
            continue;
        };
        let typed_trees::expression::ExpressionNode::Name(path) =
            program.expression_table.expression(local.initial_value)
        else {
            continue;
        };
        if !path.symbol.is_valid() || path.symbol != path.head_symbol || path.members.count() != 1 {
            continue;
        }
        let Some((position, source)) = program
            .state_parameters(state)
            .iter()
            .enumerate()
            .find(|(_, source)| source.symbol == path.symbol)
        else {
            continue;
        };
        if (source.is_const) || source.is_mutable {
            continue;
        }
        if !parameters.iter().any(|parameter| {
            parameter.position as usize == position
                && parameter.access == CheckedStructuralAccess::Owned
                && parameter.multiplicity == Multiplicity::Affine
        }) {
            continue;
        }
        let transfer = (
            PermissionEventSource::Statement { statement_index },
            facts::PlaceRoot::Symbol(source.symbol),
        );
        if expected_transfers.contains(&transfer) {
            return None;
        }
        expected_transfers.push(transfer);
    }
    // A selected source parameter keeps no exit drop: its conditional custody
    // and residual death ride the owned-selection receipt instead, so it joins
    // neither the expected discard roster nor the event stream.
    let selection_source_roots = ownership
        .owned_selections
        .iter()
        .filter(|(_, receipt)| receipt.machine == machine && receipt.state == state.symbol)
        .flat_map(|(_, receipt)| {
            ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .iter()
                .map(|source| facts::PlaceRoot::Symbol(source.symbol))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut expected_discards = Vec::new();
    for parameter in parameters.iter().rev() {
        if parameter.access != CheckedStructuralAccess::Owned
            || parameter.multiplicity != Multiplicity::Affine
        {
            continue;
        }
        let source = program
            .state_parameters(state)
            .get(parameter.position as usize)?;
        let root = facts::PlaceRoot::Symbol(source.symbol);
        if selection_source_roots.contains(&root) {
            continue;
        }
        let transferred_in_prefix = expected_transfers.iter().any(|(source, transferred)| {
            *transferred == root
                && matches!(source, PermissionEventSource::Call { statement_index, .. } | PermissionEventSource::Statement { statement_index } if *statement_index < prefix_end)
        });
        if !transferred_in_prefix {
            expected_discards.push(root);
        }
    }
    let mut actual_discards = Vec::new();
    let mut actual_transfers = Vec::new();
    // Graph discovery has already retained every record value in this prefix.
    // Its local ledger belongs to validation::record_local_disposition,
    // which must succeed before the completed graph is published. Keep that
    // local provenance separate from this whole-parameter transfer audit; an
    // unknown root still reaches the checks below and rejects.
    let local_roots = statements[..prefix_end]
        .iter()
        .filter_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
                if program
                    .primitive_type_reference(local.type_reference)
                    .is_none() =>
            {
                Some(facts::PlaceRoot::Symbol(local.symbol))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for (_, event) in ownership.permissions.iter().filter(|(_, event)| {
        event.machine_symbol == machine
            && event.state_symbol == state.symbol
            && event.access == PermissionAccess::Owned
            && !local_roots.contains(&event.root)
    }) {
        if event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != PermissionProvenance::Unknown
            || event.obligation_live
            || !ownership.segments.span(event.segments)?.is_empty()
        {
            return None;
        }
        match event.kind {
            PermissionEventKind::AffineDrop
                if event.source == PermissionEventSource::StateExit
                    && event.multiplicity == Multiplicity::Affine =>
            {
                actual_discards.push(event.root);
            }
            PermissionEventKind::Transfer if event.multiplicity == Multiplicity::Affine => {
                let transfer = (event.source, event.root);
                if !expected_transfers.contains(&transfer) || actual_transfers.contains(&transfer) {
                    return None;
                }
                actual_transfers.push(transfer);
            }
            _ => return None,
        }
    }
    (actual_transfers.len() == expected_transfers.len() && actual_discards == expected_discards)
        .then_some(())
}
