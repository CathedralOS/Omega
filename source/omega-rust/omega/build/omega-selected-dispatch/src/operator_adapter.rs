//! Checked boundary-operator ProviderPlan execution bridge.
//!
//! Semantic checking and retained facts continue to name the public boundary
//! operator. After selection, a named or fixed-token use whose exact plan row
//! is a checked adapter redirects execution to that ordinary Omega machine
//! body. This is the operator analogue of boundary-trait adapter dispatch;
//! compiler intrinsics remain in `float_intrinsic_dispatch`.

mod application_realization;
mod spelled;
mod unit;

pub use application_realization::{
    CheckedNongenericOperatorApplicationRealization, CheckedOperatorAuthoredUseKind,
    derive_checked_nongeneric_operator_application_realizations,
};

use omega_effects::provider_plan::ProviderBinding;
use psi_checked_trees::{
    CheckedTrees, CheckedUnitEffectOperationPlan, CheckedValueOrigin, CheckedValueStatementRole,
};
use psi_diagnostics::Diagnostic;
use psi_language_core::CallOperationalAcknowledgementOrigin;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OperatorAdapterRewrite {
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    requirement_operator: psi_symbols::SymbolHandle,
    provider_plan_report_fingerprint: u64,
    provider_plan_commitment: psi_checked_trees::CheckedProviderPlanCommitment,
    machine_symbol: psi_symbols::SymbolHandle,
    machine: String,
    entry_symbol: psi_symbols::SymbolHandle,
    source: OperatorAdapterSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OperatorAdapterSource {
    NamedCall,
    Spelled(Box<[ExpressionHandle]>),
}

pub fn settle_selected_operator_adapter_dispatch(
    checked: &mut Arc<CheckedTrees>,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let rewrites = plan_selected_operator_adapter_rewrites(checked, selected_provider_plans)?;
    if rewrites.is_empty() {
        return Ok(());
    }

    let applications = rewrites
        .iter()
        .map(|rewrite| unit::selected_unit_application(checked, rewrite))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|diagnostic| vec![diagnostic])?;
    let mut staged = checked.as_ref().clone();
    psi_typed_trees_to_checked_trees::rebuild_checked_unit_effect_plans_with_selected_operators(
        &mut staged,
        &applications,
    );
    for rewrite in &rewrites {
        unit::validate_selected_unit_application(&staged, rewrite)
            .map_err(|diagnostic| vec![diagnostic])?;
    }
    apply_selected_operator_adapter_rewrites(&mut staged, &rewrites);
    *Arc::make_mut(checked) = staged;
    Ok(())
}

/// Rejoin every retained selected Unit call to the exact ProviderPlan still
/// owned by the compiler before Terminal production. Checked Psi deliberately
/// carries only the selected join; Omega retains the complete plan whose
/// strong identity authorizes that join.
pub fn validate_selected_operator_terminal_custody(
    checked: &CheckedTrees,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for machine in &checked.facts.flow.terminal_unit_effects.machines {
        for operation in &machine.operations {
            let CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                coordinate,
                requirement_operator,
                provider_plan_report_fingerprint,
                provider_plan_commitment,
                realization_machine,
                realization_state,
                ..
            } = operation
            else {
                continue;
            };
            let origin = CheckedValueOrigin::StateStatement {
                machine_symbol: machine.machine,
                state_symbol: machine.state,
                statement_index: match usize::try_from(coordinate.statement_index) {
                    Ok(statement_index) => statement_index,
                    Err(_) => {
                        diagnostics.push(Diagnostic::error(
                            "selected Unit operator statement coordinate exceeds usize",
                        ));
                        continue;
                    }
                },
                role: CheckedValueStatementRole::LocalInitializer,
            };
            let uses = checked
                .facts
                .operators
                .named_uses
                .iter()
                .map(|(_, operator_use)| {
                    (
                        operator_use.expression,
                        operator_use.origin,
                        operator_use.selected_operator_symbol,
                        operator_use.provider_plan_report_fingerprint,
                        operator_use.provider_plan_commitment,
                    )
                })
                .chain(
                    checked
                        .facts
                        .operators
                        .uses
                        .iter()
                        .map(|(_, operator_use)| {
                            (
                                operator_use.expression,
                                operator_use.origin,
                                operator_use.selected_operator_symbol,
                                operator_use.provider_plan_report_fingerprint,
                                operator_use.provider_plan_commitment,
                            )
                        }),
                )
                .filter(|(_, use_origin, operator, report, commitment)| {
                    *use_origin == origin
                        && *operator == *requirement_operator
                        && *report == *provider_plan_report_fingerprint
                        && *commitment == *provider_plan_commitment
                })
                .collect::<Vec<_>>();
            let [(expression, _, _, _, _)] = uses.as_slice() else {
                diagnostics.push(Diagnostic::error(format!(
                    "selected Unit operator call at {:?} retained {} matching authored uses; expected exactly one",
                    origin,
                    uses.len(),
                )));
                continue;
            };
            let plan = match resolve_exact_selected_plan(
                selected_provider_plans.plans(),
                *provider_plan_report_fingerprint,
                *provider_plan_commitment,
                "selected Unit operator call",
            ) {
                Ok(plan) => plan,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let operator =
                match exact_operator_definition(checked, *expression, *requirement_operator) {
                    Ok(operator) => operator,
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic);
                        continue;
                    }
                };
            match resolve_checked_adapter_for_operator(checked, operator, plan, *expression) {
                Ok(Some((expected_machine, _, expected_state)))
                    if expected_machine == *realization_machine
                        && expected_state == *realization_state => {}
                Ok(Some((expected_machine, expected_name, expected_state))) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "selected Unit operator call at {:?} retains realization {:?}/{:?}, but its exact ProviderPlan selects `{expected_name}` at {:?}/{:?}",
                        origin,
                        realization_machine,
                        realization_state,
                        expected_machine,
                        expected_state,
                    )));
                }
                Ok(None) => diagnostics.push(Diagnostic::error(format!(
                    "selected Unit operator call at {:?} is not backed by a checked-adapter ProviderPlan row",
                    origin,
                ))),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn plan_selected_operator_adapter_rewrites(
    checked: &CheckedTrees,
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
) -> Result<Vec<OperatorAdapterRewrite>, Vec<Diagnostic>> {
    let mut rewrites = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, operator_use) in checked.facts.operators.named_uses.iter() {
        if operator_use.provider_plan_report_fingerprint == 0
            && operator_use.provider_plan_commitment.is_empty()
        {
            continue;
        }
        let rewrite = match resolve_selected_operator_adapter_call(
            checked,
            selected_provider_plans.plans(),
            operator_use,
        ) {
            Ok(Some(rewrite)) => rewrite,
            Ok(None) => continue,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        stage_operator_adapter_rewrite(&mut rewrites, &mut diagnostics, rewrite);
    }

    for (_, operator_use) in checked.facts.operators.uses.iter() {
        if operator_use.provider_plan_report_fingerprint == 0
            && operator_use.provider_plan_commitment.is_empty()
        {
            continue;
        }
        let rewrite = match spelled::resolve_selected_spelled_operator_adapter_call(
            checked,
            selected_provider_plans.plans(),
            operator_use,
        ) {
            Ok(Some(rewrite)) => rewrite,
            Ok(None) => continue,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        stage_operator_adapter_rewrite(&mut rewrites, &mut diagnostics, rewrite);
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(rewrites)
}

fn stage_operator_adapter_rewrite(
    rewrites: &mut Vec<OperatorAdapterRewrite>,
    diagnostics: &mut Vec<Diagnostic>,
    rewrite: OperatorAdapterRewrite,
) {
    if let Some(selected) = rewrites
        .iter()
        .find(|selected| selected.expression == rewrite.expression)
    {
        if selected != &rewrite {
            diagnostics.push(Diagnostic::error(format!(
                "operator expression {:?} carries contradictory checked-adapter realizations",
                rewrite.expression,
            )));
        }
        return;
    }
    rewrites.push(rewrite);
}

fn apply_selected_operator_adapter_rewrites(
    checked: &mut CheckedTrees,
    rewrites: &[OperatorAdapterRewrite],
) {
    for rewrite in rewrites {
        let replacement = match &rewrite.source {
            OperatorAdapterSource::NamedCall => {
                let ExpressionNode::Call(mut call) = checked
                    .typed
                    .expression_table
                    .expression(rewrite.expression)
                    .clone()
                else {
                    unreachable!("validated named operator rewrite ceased to be a call")
                };
                call.receiver = ExpressionHandle::invalid();
                call.target = psi_typed_trees::name::Identifier::generated(rewrite.machine.clone());
                call.target_symbol = rewrite.entry_symbol;
                ExpressionNode::Call(call)
            }
            OperatorAdapterSource::Spelled(operands) => {
                let arguments = checked
                    .typed
                    .expression_table
                    .insert_expression_handles(operands.iter().copied());
                ExpressionNode::Call(TableCallExpression {
                    receiver: ExpressionHandle::invalid(),
                    target_symbol: rewrite.entry_symbol,
                    target: psi_typed_trees::name::Identifier::generated(rewrite.machine.clone()),
                    static_requirement_dispatch: None,
                    machine_arguments: Box::new([]),
                    quotient_operation: None,
                    private_layout_operation: None,
                    arguments,
                    evidence_arguments: Box::new([]),
                    operational_acknowledgement:
                        psi_language_core::CallOperationalAcknowledgement {
                            origin: CallOperationalAcknowledgementOrigin::CompilerSynthesized,
                            acknowledges_suspend: false,
                            acknowledges_block: false,
                        },
                })
            }
        };
        *checked
            .typed
            .expression_table
            .expression_mut(rewrite.expression) = replacement;
    }
}

fn resolve_selected_operator_adapter_call(
    checked: &CheckedTrees,
    selected_provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    operator_use: &psi_checked_trees::CheckedNamedOperatorUseFact,
) -> Result<Option<OperatorAdapterRewrite>, Diagnostic> {
    let plan = resolve_exact_selected_plan(
        selected_provider_plans,
        operator_use.provider_plan_report_fingerprint,
        operator_use.provider_plan_commitment,
        "named operator use",
    )?;

    resolve_operator_adapter_call(checked, operator_use, plan)
}

fn resolve_operator_adapter_call(
    checked: &CheckedTrees,
    operator_use: &psi_checked_trees::CheckedNamedOperatorUseFact,
    plan: &omega_effects::provider_plan::ProviderPlan,
) -> Result<Option<OperatorAdapterRewrite>, Diagnostic> {
    let operator = exact_operator_definition(
        checked,
        operator_use.expression,
        operator_use.selected_operator_symbol,
    )?;
    if !operator.is_boundary {
        return Err(Diagnostic::error(format!(
            "selected checked operator at expression {:?} does not name a boundary operator",
            operator_use.expression,
        )));
    }
    let ExpressionNode::Call(call) = checked
        .typed
        .expression_table
        .expression(operator_use.expression)
    else {
        return Err(Diagnostic::error(format!(
            "selected checked operator at expression {:?} is not a named call",
            operator_use.expression,
        )));
    };
    if psi_typed_trees::operator::resolve_named_expression_call(&checked.typed, call)
        .map(|resolved| resolved.symbol)
        != Some(operator.symbol)
    {
        return Err(Diagnostic::error(format!(
            "selected checked operator at expression {:?} no longer names its checked operator symbol",
            operator_use.expression,
        )));
    }

    let Some((machine_symbol, machine, entry_symbol)) =
        resolve_checked_adapter_for_operator(checked, operator, plan, operator_use.expression)?
    else {
        return Ok(None);
    };

    Ok(Some(OperatorAdapterRewrite {
        expression: operator_use.expression,
        origin: operator_use.origin,
        requirement_operator: operator_use.selected_operator_symbol,
        provider_plan_report_fingerprint: operator_use.provider_plan_report_fingerprint,
        provider_plan_commitment: operator_use.provider_plan_commitment,
        machine_symbol,
        machine,
        entry_symbol,
        source: OperatorAdapterSource::NamedCall,
    }))
}

pub(super) fn resolve_checked_adapter_for_operator(
    checked: &CheckedTrees,
    operator: &psi_typed_trees::operator::OperatorDefinition,
    plan: &omega_effects::provider_plan::ProviderPlan,
    expression: ExpressionHandle,
) -> Result<Option<(psi_symbols::SymbolHandle, String, psi_symbols::SymbolHandle)>, Diagnostic> {
    let overload_identity =
        psi_typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator);
    if overload_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked operator at expression {expression:?} has an empty canonical overload identity",
        )));
    }

    let [method] = plan.schema.methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-operator ProviderPlan `{}` must retain exactly one schema method",
            plan.name,
        )));
    };
    let [row] = plan.rows.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-operator ProviderPlan `{}` must retain exactly one realization row",
            plan.name,
        )));
    };
    if plan.schema.trait_name != overload_identity
        || method.name != "realize"
        || method.requirement_owner != overload_identity
        || method.requirement_identity != overload_identity
        || !plan.schema.row_binds_method(row, method)
    {
        return Err(Diagnostic::error(format!(
            "selected checked-operator ProviderPlan `{}` does not bind exact overload `{overload_identity}`",
            plan.name,
        )));
    }

    let ProviderBinding::CheckedAdapter {
        machine_identity, ..
    } = &row.binding
    else {
        return Ok(None);
    };

    if plan.provider_type.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-operator ProviderPlan `{}` has no nominal provider type",
            plan.name,
        )));
    }
    let provider =
        omega_provider_planning::plans::exact_checked_adapter(&checked.typed, plan, row)?;
    if provider.attached_data.as_ref().map(|owner| owner.as_str())
        != Some(plan.provider_type.as_str())
    {
        return Err(Diagnostic::error(format!(
            "selected checked-operator adapter `{machine_identity}` does not belong to nominal provider `{}`",
            plan.provider_type,
        )));
    }
    if provider.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody {
        return Err(Diagnostic::error(format!(
            "selected checked-operator adapter `{machine_identity}` is not a checked body",
        )));
    }
    let Some(entry) = checked.typed.machine_states(provider).first() else {
        return Err(Diagnostic::error(format!(
            "selected checked-operator adapter `{machine_identity}` has no executable entry state",
        )));
    };

    let [namespace, requirement] = checked.typed.operator_path_members(operator.name) else {
        return Err(Diagnostic::error(format!(
            "selected checked operator overload `{overload_identity}` has no exact namespace and requirement path",
        )));
    };
    let conformances = checked
        .typed
        .machine_trait_conformances(provider)
        .iter()
        .filter(|conformance| {
            conformance.external_binding.is_none()
                && conformance.name.as_str() == namespace.as_str()
                && conformance.requirement.as_ref().map(|name| name.as_str())
                    == Some(requirement.as_str())
                && psi_typed_trees::operator::resolve_satisfied_checked_operator(
                    &checked.typed,
                    provider,
                    namespace.as_str(),
                    requirement.as_str(),
                )
                .is_some_and(|resolved| resolved.symbol == operator.symbol)
        })
        .count();
    if conformances != 1 {
        return Err(Diagnostic::error(format!(
            "selected checked-operator adapter `{machine_identity}` binds exact overload `{overload_identity}` through {conformances} checked conformances",
        )));
    }

    Ok(Some((
        provider.symbol,
        provider.name.as_str().to_owned(),
        entry.symbol,
    )))
}

pub(super) fn exact_operator_definition<'program>(
    checked: &'program CheckedTrees,
    expression: ExpressionHandle,
    symbol: psi_symbols::SymbolHandle,
) -> Result<&'program psi_typed_trees::operator::OperatorDefinition, Diagnostic> {
    let operators = checked
        .typed
        .operators()
        .iter()
        .filter(|operator| operator.symbol == symbol)
        .collect::<Vec<_>>();
    let [operator] = operators.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked operator at expression {expression:?} resolves symbol {symbol:?} to {} operator definitions",
            operators.len(),
        )));
    };
    Ok(*operator)
}

pub(super) fn resolve_exact_selected_plan<'plans>(
    selected_provider_plans: &'plans [omega_effects::provider_plan::ProviderPlan],
    report_fingerprint: u64,
    commitment: psi_checked_trees::CheckedProviderPlanCommitment,
    use_label: &str,
) -> Result<&'plans omega_effects::provider_plan::ProviderPlan, Diagnostic> {
    if commitment.is_empty() {
        return Err(Diagnostic::error(format!(
            "{use_label} carries ProviderPlan report fingerprint {report_fingerprint:#018x} without an exact commitment",
        )));
    }
    let report_matches = selected_provider_plans
        .iter()
        .filter(|plan| plan.report_fingerprint() == report_fingerprint)
        .collect::<Vec<_>>();
    let plans = report_matches
        .iter()
        .copied()
        .filter(|plan| plan.identity_digest().as_bytes() == commitment.as_bytes())
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        return Err(Diagnostic::error(
            match (report_matches.len(), plans.len()) {
                (1, 0) => format!(
                    "{use_label} ProviderPlan report fingerprint {report_fingerprint:#018x} has an exact commitment that does not match the selected plan",
                ),
                (0, _) => format!(
                    "{use_label} carries unknown ProviderPlan report fingerprint {report_fingerprint:#018x}",
                ),
                (_, count) => format!(
                    "{use_label} ProviderPlan report fingerprint {report_fingerprint:#018x} and exact commitment match {count} selected plans",
                ),
            },
        ));
    };
    Ok(*plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
        data CheckedMath {}
        boundary operator CheckedMath::offset_zero(value: i32) -> i32
        requires value == value
        ensures result == value + 0 && value == value;

        data OtherMath {}
        boundary operator OtherMath::offset_zero(value: i32) -> i32
        requires value == value
        ensures result == value + 0 && value == value;

        data CheckedMathProvider {}
        machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
        satisfies CheckedMath::offset_zero
        requires input == input
        ensures result == input + 0 && input == input
        {
            transition { _ -> (input + 0) }
        }
        machine CheckedMathProvider::decoy_impl(input: i32) -> i32
        satisfies OtherMath::offset_zero
        requires input == input
        ensures result == input + 0 && input == input
        {
            transition { _ -> (input + 0) }
        }

        data AlternateMathProvider {}
        machine AlternateMathProvider::offset_zero_impl(input: i32) -> i32
        satisfies CheckedMath::offset_zero
        requires input == input
        ensures result == input + 0 && input == input
        {
            transition { _ -> (input + 0) }
        }

        machine run() -> i32 {
            transition { _ -> (CheckedMath::offset_zero(70)) }
        }

        data Main {}
        machine Main::main(&mut self) {
            let result: i32 = CheckedMath::offset_zero(70);
        }
    "#;

    struct Fixture {
        checked: CheckedTrees,
        checked_plan: omega_effects::provider_plan::ProviderPlan,
        other_plan: omega_effects::provider_plan::ProviderPlan,
        operator_use: psi_checked_trees::CheckedNamedOperatorUseFact,
    }

    fn fixture() -> Fixture {
        let tokens = psi_source_files_to_tokens::Lexer::new(SOURCE)
            .tokenize()
            .expect("tokenize checked-operator dispatch fixture");
        let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse checked-operator dispatch fixture");
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve checked-operator dispatch fixture");
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type checked-operator dispatch fixture");
        let plans = omega_provider_planning::plans::derive_satisfies_plans(&typed, None);
        let checked_plan = plans
            .iter()
            .find(|plan| {
                plan.schema.trait_name.contains("CheckedMath::offset_zero")
                    && plan.provider_type == "CheckedMathProvider"
            })
            .expect("CheckedMath provider plan")
            .clone();
        let other_plan = plans
            .iter()
            .find(|plan| plan.schema.trait_name.contains("OtherMath::offset_zero"))
            .expect("OtherMath provider plan")
            .clone();
        let checked = psi_typed_trees_to_checked_trees::lower_typed_trees(typed)
            .expect("check operator dispatch fixture");
        let operator_use = checked
            .facts
            .operators
            .named_uses
            .iter()
            .map(|(_, operator_use)| *operator_use)
            .find(|operator_use| {
                checked
                    .typed
                    .operators()
                    .iter()
                    .find(|operator| operator.symbol == operator_use.selected_operator_symbol)
                    .is_some_and(|operator| {
                        checked
                            .typed
                            .operator_path_members(operator.name)
                            .iter()
                            .map(|member| member.as_str())
                            .eq(["CheckedMath", "offset_zero"])
                    })
            })
            .expect("checked named operator use");
        Fixture {
            checked,
            checked_plan,
            other_plan,
            operator_use,
        }
    }

    fn settled_attached_unit_fixture() -> (
        Arc<CheckedTrees>,
        omega_effects::SelectedProviderPlanFacts,
        psi_checked_trees::CheckedNamedOperatorUseFact,
    ) {
        let mut fixture = fixture();
        let main = fixture
            .checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("Unit fixture machine");
        let main_symbol = main.symbol;
        let main_state = fixture.checked.typed.machine_states(main)[0].symbol;
        let (use_handle, mut operator_use) = fixture
            .checked
            .facts
            .operators
            .named_uses
            .iter()
            .map(|(handle, operator_use)| (handle, *operator_use))
            .find(|(_, operator_use)| {
                matches!(
                    operator_use.origin,
                    CheckedValueOrigin::StateStatement {
                        machine_symbol,
                        state_symbol,
                        statement_index: 0,
                        role: CheckedValueStatementRole::LocalInitializer,
                    } if machine_symbol == main_symbol && state_symbol == main_state
                )
            })
            .expect("selected operator local in Unit fixture");
        operator_use.provider_plan_report_fingerprint = fixture.checked_plan.report_fingerprint();
        operator_use.provider_plan_commitment =
            psi_checked_trees::CheckedProviderPlanCommitment::from_digest(
                *fixture.checked_plan.identity_digest().as_bytes(),
            );
        *fixture
            .checked
            .facts
            .operators
            .named_uses
            .get_mut(use_handle) = operator_use;
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&fixture.checked_plan),
            std::slice::from_ref(&fixture.checked_plan.name),
        )
        .expect("select exact checked-operator plan");
        let mut settled = Arc::new(fixture.checked);
        settle_selected_operator_adapter_dispatch(&mut settled, &selected)
            .expect("selected Unit operator application settles");
        (settled, selected, operator_use)
    }

    #[test]
    fn derives_canonical_empty_nongeneric_checked_body_application() {
        let (settled, selected, operator_use) = settled_attached_unit_fixture();

        let rows = derive_checked_nongeneric_operator_application_realizations(&settled, &selected)
            .expect("exact selected application joins");
        let [row] = rows.as_slice() else {
            panic!("fixture must derive one attached-Unit application row")
        };
        assert_eq!(row.authored_use_kind, CheckedOperatorAuthoredUseKind::Named,);
        assert_eq!(row.call_coordinate.statement_index, 0);
        assert_eq!(row.call_coordinate.call_ordinal, 0);
        assert_eq!(
            row.requirement_operator,
            operator_use.selected_operator_symbol
        );
        assert!(
            row.requirement_overload_identity
                .contains("CheckedMath::offset_zero")
        );
        assert_eq!(
            row.provider_plan_commitment,
            operator_use.provider_plan_commitment,
        );
        assert!(!row.realization_contract_commitment.is_zero());
    }

    #[test]
    fn application_realization_derivation_rejects_missing_or_substituted_joins() {
        let (settled, selected, _) = settled_attached_unit_fixture();

        let mut missing_application = settled.as_ref().clone();
        missing_application
            .facts
            .operators
            .boundary_applications
            .clear();
        let missing = derive_checked_nongeneric_operator_application_realizations(
            &missing_application,
            &selected,
        )
        .expect_err("selected call without its application demand must reject");
        assert_eq!(missing.len(), 1);
        assert!(
            missing[0]
                .message
                .contains("retained 0 exact application demands"),
            "unexpected diagnostic: {}",
            missing[0].message,
        );

        let mut substituted_contract = settled.as_ref().clone();
        let selected_call = substituted_contract
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.operations)
            .find_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                    realization_contract_commitment,
                    ..
                } => Some(realization_contract_commitment),
                _ => None,
            })
            .expect("selected attached-Unit call");
        *selected_call = psi_checked_trees::MachineContractCommitment::from_digest([0xa5; 32]);
        let substituted = derive_checked_nongeneric_operator_application_realizations(
            &substituted_contract,
            &selected,
        )
        .expect_err("substituted machine contract must reject");
        assert_eq!(substituted.len(), 1);
        assert!(
            substituted[0]
                .message
                .contains("does not rejoin its exact machine contract commitment"),
            "unexpected diagnostic: {}",
            substituted[0].message,
        );
    }

    #[derive(Clone, Copy, Debug)]
    enum Drift {
        None,
        UnknownPlan,
        MissingCommitment,
        WrongCommitment,
        DuplicatePlan,
        EmptyOverload,
        CrossOperatorPlan,
        AbsentAdapter,
        DuplicateAdapter,
        WrongOwner,
        NonCheckedAdapter,
        WrongConformance,
        NonCallExpression,
        SourceOperator,
        Intrinsic,
    }

    #[test]
    fn exact_resolver_rejects_every_identity_drift() {
        let cases = [
            (Drift::None, None),
            (
                Drift::UnknownPlan,
                Some("unknown ProviderPlan report fingerprint"),
            ),
            (
                Drift::MissingCommitment,
                Some("without an exact commitment"),
            ),
            (
                Drift::WrongCommitment,
                Some("exact commitment that does not match"),
            ),
            (Drift::DuplicatePlan, Some("match 2 selected plans")),
            (Drift::EmptyOverload, Some("does not bind exact overload")),
            (
                Drift::CrossOperatorPlan,
                Some("does not bind exact overload"),
            ),
            (Drift::AbsentAdapter, Some("is absent from typed machines")),
            (
                Drift::DuplicateAdapter,
                Some("resolves to 2 exact typed machines"),
            ),
            (
                Drift::WrongOwner,
                Some("does not belong to nominal provider"),
            ),
            (Drift::NonCheckedAdapter, Some("is not a checked body")),
            (
                Drift::WrongConformance,
                Some("through 0 checked conformances"),
            ),
            (Drift::NonCallExpression, Some("is not a named call")),
            (
                Drift::SourceOperator,
                Some("no longer names its checked operator symbol"),
            ),
            (Drift::Intrinsic, None),
        ];

        for (drift, expected_error) in cases {
            let mut fixture = fixture();
            let mut plan = fixture.checked_plan.clone();
            let mut plans = vec![plan.clone()];
            match drift {
                Drift::None => {}
                Drift::UnknownPlan => {
                    fixture.operator_use.provider_plan_report_fingerprint = u64::MAX
                }
                Drift::MissingCommitment => {}
                Drift::WrongCommitment => {}
                Drift::DuplicatePlan => plans.push(plan.clone()),
                Drift::EmptyOverload => {
                    plan.schema.trait_name.clear();
                    plan.schema.methods[0].requirement_owner.clear();
                    plan.schema.methods[0].requirement_identity.clear();
                    plan.rows[0].requirement_identity.clear();
                    plans = vec![plan.clone()];
                }
                Drift::CrossOperatorPlan => {
                    plan = fixture.other_plan.clone();
                    plans = vec![plan.clone()];
                }
                Drift::AbsentAdapter => {
                    plan.rows[0].binding = ProviderBinding::CheckedAdapter {
                        machine_identity: "CheckedMathProvider::absent".into(),
                        machine_package_identity: None,
                    };
                    plans = vec![plan.clone()];
                }
                Drift::DuplicateAdapter => {
                    let machine_identity = match &plan.rows[0].binding {
                        ProviderBinding::CheckedAdapter {
                            machine_identity, ..
                        } => machine_identity.clone(),
                        binding => panic!("unexpected fixture binding {binding:?}"),
                    };
                    let duplicate = fixture
                        .checked
                        .typed
                        .machine_by_normalized_overload_identity(&machine_identity)
                        .expect("selected adapter")
                        .clone();
                    fixture.checked.typed.push_machine(duplicate);
                }
                Drift::WrongOwner => {
                    plan.provider_type = "OtherProvider".into();
                    plans = vec![plan.clone()];
                }
                Drift::NonCheckedAdapter => {
                    fixture
                        .checked
                        .typed
                        .machines_mut()
                        .iter_mut()
                        .find(|machine| machine.name.as_str().ends_with("offset_zero_impl"))
                        .expect("checked adapter")
                        .supply_mode = psi_language_semantics::MachineSupplyMode::Boundary;
                }
                Drift::WrongConformance => {
                    let decoy = fixture
                        .checked
                        .typed
                        .machines()
                        .iter()
                        .find(|machine| machine.name.as_str().ends_with("decoy_impl"))
                        .expect("decoy adapter");
                    let machine_identity = fixture
                        .checked
                        .typed
                        .normalized_machine_overload_identity(decoy)
                        .expect("decoy adapter identity")
                        .identity();
                    plan.rows[0].binding = ProviderBinding::CheckedAdapter {
                        machine_identity,
                        machine_package_identity: None,
                    };
                    plans = vec![plan.clone()];
                }
                Drift::NonCallExpression => {
                    let ExpressionNode::Call(call) = fixture
                        .checked
                        .typed
                        .expression_table
                        .expression(fixture.operator_use.expression)
                    else {
                        panic!("fixture operator expression is not a call");
                    };
                    fixture.operator_use.expression = fixture
                        .checked
                        .typed
                        .expression_table
                        .expression_handles(call.arguments)[0];
                }
                Drift::SourceOperator => {
                    let other = fixture
                        .checked
                        .typed
                        .operators()
                        .iter()
                        .find(|operator| {
                            fixture
                                .checked
                                .typed
                                .operator_path_members(operator.name)
                                .iter()
                                .map(|member| member.as_str())
                                .eq(["OtherMath", "offset_zero"])
                        })
                        .expect("other operator");
                    fixture.operator_use.selected_operator_symbol = other.symbol;
                    plan = fixture.other_plan.clone();
                    plans = vec![plan.clone()];
                }
                Drift::Intrinsic => {
                    plan.rows[0].binding = ProviderBinding::CompilerIntrinsic {
                        machine: "CheckedMath::offset_zero".into(),
                    };
                    plans = vec![plan.clone()];
                }
            }
            if !matches!(drift, Drift::UnknownPlan) {
                fixture.operator_use.provider_plan_report_fingerprint = plan.report_fingerprint();
            }
            fixture.operator_use.provider_plan_commitment =
                if matches!(drift, Drift::MissingCommitment) {
                    psi_checked_trees::CheckedProviderPlanCommitment::default()
                } else {
                    psi_checked_trees::CheckedProviderPlanCommitment::from_digest(
                        if matches!(drift, Drift::WrongCommitment) {
                            [0xa5; 32]
                        } else {
                            *plan.identity_digest().as_bytes()
                        },
                    )
                };

            let result = resolve_selected_operator_adapter_call(
                &fixture.checked,
                &plans,
                &fixture.operator_use,
            );
            match expected_error {
                Some(expected) => {
                    let diagnostic = result.expect_err("drift must fail closed");
                    assert!(
                        diagnostic.message.contains(expected),
                        "{drift:?}: expected `{expected}`, got `{}`",
                        diagnostic.message,
                    );
                }
                None if matches!(drift, Drift::Intrinsic) => {
                    assert_eq!(result.expect("intrinsic remains delegated"), None);
                }
                None => {
                    let rewrite = result
                        .expect("exact realization resolves")
                        .expect("checked adapter stages a rewrite");
                    assert_eq!(rewrite.expression, fixture.operator_use.expression);
                    assert_eq!(rewrite.machine, "CheckedMathProvider::offset_zero_impl");
                }
            }
        }
    }

    #[test]
    fn shared_success_clones_only_after_complete_preflight() {
        let mut fixture = fixture();
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&fixture.checked_plan),
            std::slice::from_ref(&fixture.checked_plan.name),
        )
        .expect("select exact checked-operator plan");
        let (handle, mut retained) = fixture
            .checked
            .facts
            .operators
            .named_uses
            .iter()
            .map(|(handle, operator_use)| (handle, *operator_use))
            .find(|(_, operator_use)| operator_use.expression == fixture.operator_use.expression)
            .expect("fixture checked use");
        retained.provider_plan_report_fingerprint = fixture.checked_plan.report_fingerprint();
        retained.provider_plan_commitment =
            psi_checked_trees::CheckedProviderPlanCommitment::from_digest(
                *fixture.checked_plan.identity_digest().as_bytes(),
            );
        *fixture.checked.facts.operators.named_uses.get_mut(handle) = retained;
        let original_contents = fixture.checked.clone();
        let original = Arc::new(fixture.checked);
        let mut settled = Arc::clone(&original);

        settle_selected_operator_adapter_dispatch(&mut settled, &selected)
            .expect("exact selected adapter rewrites");

        assert!(
            !Arc::ptr_eq(&settled, &original),
            "a shared successful settlement must publish through a fresh Arc"
        );
        assert_eq!(
            original.as_ref(),
            &original_contents,
            "successful settlement must not mutate retained shared custody"
        );
        let rewritten = settled
            .typed
            .expression_table
            .expression(retained.expression);
        let ExpressionNode::Call(rewritten) = rewritten else {
            panic!("rewritten expression is not a call");
        };
        let provider = settled
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "CheckedMathProvider::offset_zero_impl")
            .expect("exact checked adapter");
        assert_eq!(rewritten.target.as_str(), provider.name.as_str());
        assert_eq!(
            rewritten.target_symbol,
            settled.typed.machine_states(provider)[0].symbol,
        );
        assert_eq!(
            settled.facts.operators.named_uses.get(handle),
            &retained,
            "execution redirection must not rewrite retained semantic evidence",
        );
    }

    #[test]
    fn selected_local_result_retains_exact_unit_realization_application() {
        let mut fixture = fixture();
        let main = fixture
            .checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("Unit fixture machine");
        let main_symbol = main.symbol;
        let main_state = fixture.checked.typed.machine_states(main)[0].symbol;
        let (use_handle, mut operator_use) = fixture
            .checked
            .facts
            .operators
            .named_uses
            .iter()
            .map(|(handle, operator_use)| (handle, *operator_use))
            .find(|(_, operator_use)| {
                matches!(
                    operator_use.origin,
                    CheckedValueOrigin::StateStatement {
                        machine_symbol,
                        state_symbol,
                        statement_index: 0,
                        role: CheckedValueStatementRole::LocalInitializer,
                    } if machine_symbol == main_symbol && state_symbol == main_state
                )
            })
            .expect("selected operator local in Unit fixture");
        operator_use.provider_plan_report_fingerprint = fixture.checked_plan.report_fingerprint();
        operator_use.provider_plan_commitment =
            psi_checked_trees::CheckedProviderPlanCommitment::from_digest(
                *fixture.checked_plan.identity_digest().as_bytes(),
            );
        *fixture
            .checked
            .facts
            .operators
            .named_uses
            .get_mut(use_handle) = operator_use;
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&fixture.checked_plan),
            std::slice::from_ref(&fixture.checked_plan.name),
        )
        .expect("select exact checked-operator plan");
        let mut settled = Arc::new(fixture.checked);

        settle_selected_operator_adapter_dispatch(&mut settled, &selected)
            .expect("selected Unit operator application settles");

        let unit_plan = settled
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .find(|plan| plan.machine == main_symbol && plan.state == main_state)
            .expect("selected Unit plan");
        assert!(unit_plan.operations.iter().any(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                    requirement_operator,
                    realization_machine,
                    ..
                } if *requirement_operator == operator_use.selected_operator_symbol
                    && settled
                        .typed
                        .machines()
                        .iter()
                        .find(|machine| machine.symbol == *realization_machine)
                        .is_some_and(|machine| {
                            machine.name.as_str() == "CheckedMathProvider::offset_zero_impl"
                        })
            )
        }));
        validate_selected_operator_terminal_custody(&settled, &selected)
            .expect("retained Unit call must rejoin its exact selected ProviderPlan");
        let missing = validate_selected_operator_terminal_custody(
            &settled,
            &omega_effects::SelectedProviderPlanFacts::default(),
        )
        .expect_err("retained Unit call cannot outlive its selected ProviderPlan closure");
        assert_eq!(missing.len(), 1);
        assert!(
            missing[0]
                .message
                .contains("unknown ProviderPlan report fingerprint"),
            "unexpected diagnostic: {}",
            missing[0].message,
        );
    }

    #[test]
    fn terminal_custody_rejects_another_conforming_realization() {
        let mut fixture = fixture();
        let main = fixture
            .checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("Unit fixture machine");
        let main_symbol = main.symbol;
        let main_state = fixture.checked.typed.machine_states(main)[0].symbol;
        let (use_handle, mut operator_use) = fixture
            .checked
            .facts
            .operators
            .named_uses
            .iter()
            .map(|(handle, operator_use)| (handle, *operator_use))
            .find(|(_, operator_use)| {
                matches!(
                    operator_use.origin,
                    CheckedValueOrigin::StateStatement {
                        machine_symbol,
                        state_symbol,
                        statement_index: 0,
                        role: CheckedValueStatementRole::LocalInitializer,
                    } if machine_symbol == main_symbol && state_symbol == main_state
                )
            })
            .expect("selected operator local in Unit fixture");
        operator_use.provider_plan_report_fingerprint = fixture.checked_plan.report_fingerprint();
        operator_use.provider_plan_commitment =
            psi_checked_trees::CheckedProviderPlanCommitment::from_digest(
                *fixture.checked_plan.identity_digest().as_bytes(),
            );
        *fixture
            .checked
            .facts
            .operators
            .named_uses
            .get_mut(use_handle) = operator_use;
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&fixture.checked_plan),
            std::slice::from_ref(&fixture.checked_plan.name),
        )
        .expect("select exact checked-operator plan");
        let mut settled = Arc::new(fixture.checked);
        settle_selected_operator_adapter_dispatch(&mut settled, &selected)
            .expect("selected Unit operator application settles");

        let alternate = settled
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "AlternateMathProvider::offset_zero_impl")
            .expect("alternate conforming adapter");
        let alternate_machine = alternate.symbol;
        let alternate_state = settled.typed.machine_states(alternate)[0].symbol;
        let alternate_contract = settled
            .facts
            .contract_plans
            .for_machine(alternate_machine)
            .expect("alternate contract");
        let alternate_contract_report_fingerprint = alternate_contract.report_fingerprint;
        let alternate_contract_commitment = alternate_contract.commitment;
        let alternate_reach = settled
            .facts
            .flow
            .control
            .states
            .iter()
            .find(|(_, state)| {
                state.machine_symbol == alternate_machine && state.state_symbol == alternate_state
            })
            .map(|(_, state)| state.service_reach)
            .expect("alternate service reach");
        let settled_mut = Arc::make_mut(&mut settled);
        let selected_call = settled_mut
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == main_symbol && plan.state == main_state)
            .and_then(|plan| {
                plan.operations.iter_mut().find(|operation| {
                    matches!(
                        operation,
                        CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
                    )
                })
            })
            .expect("selected Unit call");
        let CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
            realization_machine,
            realization_state,
            realization_contract_report_fingerprint,
            realization_contract_commitment,
            service_reach,
            ..
        } = selected_call
        else {
            unreachable!("selected operation kind was filtered above")
        };
        *realization_machine = alternate_machine;
        *realization_state = alternate_state;
        *realization_contract_report_fingerprint = alternate_contract_report_fingerprint;
        *realization_contract_commitment = alternate_contract_commitment;
        *service_reach = alternate_reach;

        let diagnostics = validate_selected_operator_terminal_custody(&settled, &selected)
            .expect_err("another conforming realization must not replace the selected plan row");
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("but its exact ProviderPlan selects"),
            "unexpected diagnostic: {}",
            diagnostics[0].message,
        );
    }

    #[test]
    fn shared_rejection_preserves_arc_identity_and_complete_contents() {
        let mut fixture = fixture();
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&fixture.checked_plan),
            std::slice::from_ref(&fixture.checked_plan.name),
        )
        .expect("select exact checked-operator plan");
        let handles = fixture
            .checked
            .facts
            .operators
            .named_uses
            .iter()
            .map(|(handle, operator_use)| (handle, *operator_use))
            .collect::<Vec<_>>();
        let (handle, mut valid) = handles
            .into_iter()
            .find(|(_, operator_use)| operator_use.expression == fixture.operator_use.expression)
            .expect("fixture checked use");
        valid.provider_plan_report_fingerprint = fixture.checked_plan.report_fingerprint();
        valid.provider_plan_commitment =
            psi_checked_trees::CheckedProviderPlanCommitment::from_digest(
                *fixture.checked_plan.identity_digest().as_bytes(),
            );
        *fixture.checked.facts.operators.named_uses.get_mut(handle) = valid;
        let mut invalid = valid;
        invalid.provider_plan_report_fingerprint = u64::MAX;
        fixture.checked.facts.operators.named_uses.append(invalid);
        let before = fixture.checked.clone();
        let original = Arc::new(fixture.checked);
        let mut rejected = Arc::clone(&original);

        let diagnostics = settle_selected_operator_adapter_dispatch(&mut rejected, &selected)
            .expect_err("one invalid use rejects the complete rewrite batch");
        assert!(
            diagnostics[0]
                .message
                .contains("unknown ProviderPlan report fingerprint")
        );
        assert_eq!(
            rejected.as_ref(),
            &before,
            "a later failure must not publish an earlier staged rewrite",
        );
        assert!(
            Arc::ptr_eq(&rejected, &original),
            "rejection must preserve exact shared program custody"
        );
    }

    #[test]
    fn empty_settlement_preserves_shared_arc_identity_and_contents() {
        let fixture = fixture();
        let original_contents = fixture.checked.clone();
        let original = Arc::new(fixture.checked);
        let mut settled = Arc::clone(&original);

        settle_selected_operator_adapter_dispatch(
            &mut settled,
            &omega_effects::SelectedProviderPlanFacts::default(),
        )
        .expect("a program without selected operator adapters is already settled");

        assert!(Arc::ptr_eq(&settled, &original));
        assert_eq!(settled.as_ref(), &original_contents);
    }
}
