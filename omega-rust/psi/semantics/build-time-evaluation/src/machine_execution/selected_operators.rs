//! Admission of supplied operator semantics against the current typed graph.
//! Omega retains provider authority; Psi independently checks the exact authored
//! selection, operand carrier and operation before allowing semantic execution.
use crate::SelectedBuildTimeBinaryOperator;
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
};

/// One selected boundary-operator use whose meaning is the selected provider's
/// ordinary checked machine body. Omega derives the row from the retained
/// selection fact and the settled ProviderPlan; this service independently
/// rechecks the exact authored selection, operand custody, provider ownership,
/// and executable entry before binding it. Execution never fabricates a
/// result: [`apply_selected_provider_bodies`] rebinds the authored expression
/// to the exact provider entry state on a private evaluation copy -- the same
/// ordinary call shape checked-side settlement publishes -- so the checked
/// interpreter runs the real provider body and the caller's result is
/// installed only after the provider returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedBuildTimeProviderBody {
    /// The authored spelled-operator expression retaining the selected use.
    pub expression: ExpressionHandle,
    /// The checked value origin that owned the selection when it was derived.
    pub origin: checked_trees::CheckedValueOrigin,
    /// The selected boundary operator requirement symbol.
    pub requirement: symbols::SymbolHandle,
    /// The authored operand expressions forwarded to the provider entry state
    /// in their authored order.
    pub operands: Vec<ExpressionHandle>,
    /// The exact selected provider machine symbol.
    pub provider_machine: symbols::SymbolHandle,
    /// The provider's executable entry state symbol.
    pub provider_state: symbols::SymbolHandle,
    /// The nominal provider type the settled plan owns.
    pub provider_type: String,
    /// The settled ProviderPlan commitment; never a name or fingerprint alone.
    pub provider: checked_trees::CheckedProviderPlanCommitment,
}

/// Rebind each retained provider-body occurrence to its exact selected entry
/// state on a private evaluation copy. The caller's tree is never rewritten:
/// the private program is the only thing admission and interpretation see, so
/// the provider body joins the ordinary call closure (termination, suspension,
/// blocking, linear carriers, and selection authority all apply unchanged) and
/// executes through `ExpressionNode::Call` like any other machine call.
pub(crate) fn apply_selected_provider_bodies(
    program: &TypedTrees,
    selected: &[SelectedBuildTimeProviderBody],
) -> Result<TypedTrees, String> {
    validate_selected_provider_bodies(program, selected)?;
    let mut program = program.clone();
    for row in selected {
        let provider_name = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == row.provider_machine)
            .map(|machine| machine.name.as_str().to_owned())
            .ok_or_else(|| "selected build-time provider body lost its exact machine".to_owned())?;
        let arguments = program
            .expression_table
            .insert_expression_handles(row.operands.iter().copied());
        *program.expression_table.expression_mut(row.expression) =
            ExpressionNode::Call(typed_trees::expression::TableCallExpression {
                receiver: ExpressionHandle::invalid(),
                target_symbol: row.provider_state,
                static_machine_parameter: symbols::SymbolHandle::invalid(),
                target: typed_trees::name::Identifier::generated(provider_name),
                static_requirement_dispatch: None,
                machine_arguments: Box::new([]),
                quotient_operation: None,
                private_layout_operation: None,
                arguments,
                evidence_arguments: Box::new([]),
                operational_acknowledgement:
                    language_semantics::CallOperationalAcknowledgement {
                        origin: language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                        acknowledges_suspend: false,
                        acknowledges_block: false,
                    },
            });
    }
    Ok(program)
}

/// Independently check each supplied provider-body row against the current
/// typed graph. A stale, substituted, padded, or name-only row can never reach
/// execution: the authored use, the selected requirement, the exact provider
/// machine, its executable entry, its nominal owner, and its checked
/// conformance must all rejoin before the private call is formed.
pub fn validate_selected_provider_bodies(
    program: &TypedTrees,
    selected: &[SelectedBuildTimeProviderBody],
) -> Result<(), String> {
    if selected.is_empty() {
        return Ok(());
    }
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(program);
    for (index, row) in selected.iter().enumerate() {
        if row.provider.is_empty()
            || selected[..index].iter().any(|prior| {
                prior.expression == row.expression
                    && prior.origin.machine_symbol() == row.origin.machine_symbol()
            })
        {
            return Err(
                "selected build-time provider body lacks unique exact provider custody".into(),
            );
        }
        // A provider body retains exactly one current use: either a spelled
        // occurrence or a uniquely resolved named call at this expression and
        // owning machine. The two arenas are disjoint by node kind, so their
        // counts sum rather than compete.
        let spelled_occurrences = facts
            .uses
            .iter()
            .filter(|(_, fact)| {
                fact.expression == row.expression
                    && fact.occurrence == checked_trees::CheckedOperatorOccurrence::Expression
                    && fact.origin.machine_symbol() == row.origin.machine_symbol()
            })
            .count();
        let named_occurrences = facts
            .named_uses
            .iter()
            .filter(|(_, fact)| {
                fact.expression == row.expression
                    && fact.origin.machine_symbol() == row.origin.machine_symbol()
            })
            .count();
        if spelled_occurrences + named_occurrences != 1 {
            return Err(
                "selected build-time expression is shared by distinct current origins".into(),
            );
        }
        let resolved_spelled: Vec<_> = facts
            .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
            .filter(|fact| {
                fact.expression == row.expression
                    && fact.origin == row.origin
                    && fact.occurrence == checked_trees::CheckedOperatorOccurrence::Expression
            })
            .collect();
        let resolved_named: Vec<_> = facts
            .named_uses()
            .filter(|fact| fact.expression == row.expression && fact.origin == row.origin)
            .collect();
        // The spelled use additionally binds the authored token spelling; a
        // named call selected its requirement by exact path and arity, so it
        // carries no spelling to rejoin.
        let (selected_symbol, spelled_spelling) =
            match (resolved_spelled.as_slice(), resolved_named.as_slice()) {
                ([fact], []) => (fact.selected_operator_symbol, Some(fact.spelling)),
                ([], [fact]) => (fact.selected_operator_symbol, None),
                _ => {
                    return Err(
                        "selected build-time provider body has no unique current use".into(),
                    );
                }
            };
        if selected_symbol != row.requirement {
            return Err(
                "selected build-time provider body differs from current requirement".into(),
            );
        }
        let operands = match program.expression_table.expression(row.expression) {
            ExpressionNode::Binary(binary) => vec![binary.left, binary.right],
            ExpressionNode::Indexed(indexed) => {
                match program.expression_table.expression(indexed.index) {
                    ExpressionNode::Range(_) => {
                        return Err(
                            "selected build-time provider use retains a range operand shape with no fixed-token checked-adapter dispatch"
                                .into(),
                        );
                    }
                    _ => vec![indexed.collection, indexed.index],
                }
            }
            // A named operator use is an ordinary call expression; its
            // argument list is the authored operand tuple.
            ExpressionNode::Call(call) => program
                .expression_table
                .expression_handles(call.arguments)
                .to_vec(),
            _ => return Err("selected build-time provider expression disappeared".into()),
        };
        if operands != row.operands {
            return Err(
                "selected build-time provider operands differ from current authored selection"
                    .into(),
            );
        }
        let matching: Vec<_> = program
            .operators()
            .iter()
            .filter(|operator| operator.symbol == row.requirement)
            .collect();
        let [operator] = matching.as_slice() else {
            return Err("selected build-time requirement is not unique".into());
        };
        if !operator.is_boundary
            || spelled_spelling.is_some_and(|spelling| operator.spelling != Some(spelling))
        {
            return Err(
                "selected build-time provider requirement lost its boundary spelling".into(),
            );
        }
        if !program.operator_type_parameters(operator).is_empty() {
            return Err(
                "selected build-time provider requirement is generic; symbolic application coverage is not part of this execution"
                    .into(),
            );
        }
        if program
            .signature_contracts
            .span_or_empty(operator.contracts)
            .iter()
            .any(|contract| {
                matches!(
                    contract.kind,
                    typed_trees::signature::SignatureContractKind::Crashes { .. }
                )
            })
        {
            return Err(
                "selected operator crash invocations have no build-time execution support".into(),
            );
        }
        if program.operator_parameters(operator).len() != row.operands.len() {
            return Err(
                "selected build-time provider requirement arity differs from retained operands"
                    .into(),
            );
        }
        let providers: Vec<_> = program
            .machines()
            .iter()
            .filter(|machine| machine.symbol == row.provider_machine)
            .collect();
        let [provider] = providers.as_slice() else {
            return Err("selected build-time provider machine is not unique".into());
        };
        if provider.attached_data.as_ref().map(|owner| owner.as_str())
            != Some(row.provider_type.as_str())
        {
            return Err(
                "selected build-time provider does not belong to its nominal provider type".into(),
            );
        }
        if provider.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
            return Err("selected build-time provider is not a checked body".into());
        }
        if !program.machine_type_parameters(provider).is_empty() {
            return Err(
                "selected build-time provider is generic; closed substitution must settle the realization first"
                    .into(),
            );
        }
        let Some(entry) = program.machine_states(provider).first() else {
            return Err("selected build-time provider has no executable entry state".into());
        };
        if entry.symbol != row.provider_state {
            return Err(
                "selected build-time provider state is not the provider's executable entry".into(),
            );
        }
        let entry_parameters: Vec<_> = program
            .state_parameters(entry)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .collect();
        if entry_parameters.len() != row.operands.len()
            || program.normalized_type_identity(entry.return_type)
                != program.normalized_type_identity(operator.return_type)
            || !program
                .operator_parameters(operator)
                .iter()
                .zip(entry_parameters.iter())
                .all(|(requirement, parameter)| {
                    program.normalized_type_identity(requirement.type_reference)
                        == program.normalized_type_identity(parameter.type_reference)
                })
        {
            return Err(
                "selected build-time provider entry signature differs from its requirement".into(),
            );
        }
        if program
            .machine_contracts(provider)
            .iter()
            .chain(program.state_contracts(entry).iter())
            .any(|contract| {
                matches!(
                    contract.kind,
                    typed_trees::signature::SignatureContractKind::Crashes { .. }
                )
            })
        {
            return Err(
                "selected build-time provider body crash contracts have no build-time execution support"
                    .into(),
            );
        }
        let [namespace, requirement] = program.operator_path_members(operator.name) else {
            return Err(
                "selected build-time provider requirement has no exact namespace path".into(),
            );
        };
        let conformances = program
            .machine_trait_conformances(provider)
            .iter()
            .filter(|conformance| {
                conformance.external_binding.is_none()
                    && conformance.name.as_str() == namespace.as_str()
                    && conformance.requirement.as_ref().map(|name| name.as_str())
                        == Some(requirement.as_str())
                    && (typed_trees::operator::resolve_satisfied_checked_operator(
                        program,
                        provider,
                        namespace.as_str(),
                        requirement.as_str(),
                    )
                    .is_some_and(|resolved| resolved.symbol == operator.symbol)
                        || typed_trees::operator::resolve_specialized_checked_operator_application(
                            program,
                            provider,
                            namespace.as_str(),
                            requirement.as_str(),
                        )
                        .is_some_and(|(resolved, _)| resolved.symbol == operator.symbol))
            })
            .count();
        if conformances != 1 {
            return Err(
                "selected build-time provider does not bind its requirement through exactly one checked conformance"
                    .into(),
            );
        }
    }
    Ok(())
}

pub fn validate_selected_operators(
    program: &TypedTrees,
    selected: &[SelectedBuildTimeBinaryOperator],
) -> Result<(), String> {
    if selected
        .iter()
        .any(|operator| operator.has_crash_contract(program))
    {
        return Err(
            "selected operator crash invocations have no build-time execution support".into(),
        );
    }
    if selected.is_empty() {
        return Ok(());
    }
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(program);
    for (index, row) in selected.iter().enumerate() {
        if row.provider.is_empty()
            || selected[..index].iter().any(|prior| {
                prior.expression == row.expression
                    && prior.origin.machine_symbol() == row.origin.machine_symbol()
            })
        {
            return Err("selected build-time operator lacks unique exact provider custody".into());
        }
        // The evaluator frame addresses an expression within its machine.
        // Reject current cross-origin aliasing as well as duplicate supplied
        // rows, so another state cannot borrow this occurrence's execution.
        if facts
            .uses
            .iter()
            .filter(|(_, fact)| {
                fact.expression == row.expression
                    && fact.occurrence == checked_trees::CheckedOperatorOccurrence::Expression
                    && fact.origin.machine_symbol() == row.origin.machine_symbol()
            })
            .count()
            != 1
        {
            return Err(
                "selected build-time expression is shared by distinct current origins".into(),
            );
        }
        let matching: Vec<_> = facts
            .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
            .filter(|fact| {
                fact.expression == row.expression
                    && fact.origin == row.origin
                    && fact.occurrence == checked_trees::CheckedOperatorOccurrence::Expression
            })
            .collect();
        let [fact] = matching.as_slice() else {
            return Err("selected build-time operator has no unique current use".into());
        };
        if fact.selected_operator_symbol != row.requirement || fact.policy_adapter != row.policy {
            return Err("selected build-time operator differs from current requirement or arithmetic policy".into());
        }
        let ExpressionNode::Binary(binary) = program.expression_table.expression(row.expression)
        else {
            return Err("selected build-time binary expression disappeared".into());
        };
        if binary.operator != row.operation || [binary.left, binary.right] != row.operands {
            return Err(
                "selected build-time operation differs from current authored selection".into(),
            );
        }
        let matching: Vec<_> = program
            .operators()
            .iter()
            .filter(|operator| operator.symbol == row.requirement)
            .collect();
        let [operator] = matching.as_slice() else {
            return Err("selected build-time requirement is not unique".into());
        };
        if typed_trees::operator::primitive_float_binary_semantics(program, operator)
            != Some((row.operation, row.format))
        {
            return Err(
                "selected build-time operation has no matching sealed Float meaning".into(),
            );
        }
        let [left, right] = program.operator_parameters(operator) else {
            return Err("selected build-time binary requirement has wrong arity".into());
        };
        let primitive = match row.format {
            numerics::literals::FloatFormat::F32 => typed_trees::types::PrimitiveType::F32,
            numerics::literals::FloatFormat::F64 => typed_trees::types::PrimitiveType::F64,
        };
        if !operator.is_boundary
            || program.primitive_type_reference(left.type_reference) != Some(primitive)
            || program.primitive_type_reference(right.type_reference) != Some(primitive)
        {
            return Err(
                "selected build-time requirement has incompatible boundary or operand carriers"
                    .into(),
            );
        }
    }
    Ok(())
}
