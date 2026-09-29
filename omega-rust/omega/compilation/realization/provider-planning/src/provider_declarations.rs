//! Provider-selection declarations retained from target defaults and Build execution.
//!
//! Static defaults enter through `harvest_provider_selections`; executed Build
//! choices enter through `collect_executed_provider_selections`. Both use one
//! decoder for exact subject, provider and composition identities. Package review
//! independently rejoins retained choices through
//! `validate_executed_provider_selection_declaration`, without rerunning Build.

use crate::{ProviderPlanningContext, ProviderSelection, ProviderSelectionIdentity};
use build_time_evaluation::is_exact_toolchain_build_prelude_data;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::BuildOperation;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;

/// Static target-default declarations have their own admission path. Authored
/// Build overrides instead enter through executed call receipts below.
pub fn harvest_provider_selections(
    context: ProviderPlanningContext<'_>,
    machine: &typed_trees::machine::Machine,
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let typed = context.typed;
    if !context.contains(machine.symbol) {
        return Err(vec![Diagnostic::error(format!(
            "provider-default declaration `{}` is outside the {:?} planning context",
            machine.name, context.scope,
        ))]);
    }
    let mut requests = Vec::new();
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            match statement {
                typed_trees::statement::StatementNode::Expression(expression) => {
                    if let typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                        && !call.target_symbol.is_valid()
                    {
                        requests.push(ProviderSelectionCall {
                            target: call.target.as_str(),
                            arguments: &call.machine_arguments,
                            value_arguments: typed
                                .expression_table
                                .expression_handles(call.arguments),
                            source_span: provider_selection_expression_source_span(
                                typed,
                                *expression,
                            ),
                            machine: machine.symbol,
                            composition_case: None,
                            product_operands: false,
                        });
                    }
                }
                typed_trees::statement::StatementNode::Call(call)
                    if !call.target_symbol.is_valid() =>
                {
                    requests.push(ProviderSelectionCall {
                        target: call.target.as_str(),
                        arguments: &call.machine_arguments,
                        value_arguments: typed.statement_table.expression_handles(call.arguments),
                        source_span: call.source_span,
                        machine: machine.symbol,
                        composition_case: None,
                        product_operands: false,
                    });
                }
                _ => {}
            }
        }
    }
    collect_provider_selections(context, requests)
}

struct ProviderSelectionCall<'a> {
    target: &'a str,
    arguments: &'a [typed_trees::expression::StaticMachineArgument],
    value_arguments: &'a [typed_trees::expression::ExpressionHandle],
    source_span: source::SourceSpan,
    machine: SymbolHandle,
    composition_case: Option<SymbolHandle>,
    product_operands: bool,
}

/// Rejoin only calls evaluated on the original Build cell. An unreachable
/// selection does not become configuration merely because its text exists.
pub fn collect_executed_provider_selections(
    context: ProviderPlanningContext<'_>,
    executed: &[checked_interpreter::ExecutedProviderSelection],
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let typed = context.typed;
    // Executed Build operations select product descriptions. The selecting
    // machine remains Build-owned; only its designated operands use Product.
    if context.scope != source::DependencyScope::Product {
        return Err(vec![Diagnostic::error(
            "executed provider selections require the Product planning context",
        )]);
    }
    let requests = executed
        .iter()
        .map(|row| {
            let request = match row.site {
                checked_interpreter::ExecutedProviderSelectionSite::Statement(handle) => {
                    match typed.statement_table.statement(handle) {
                        typed_trees::statement::StatementNode::Call(call)
                            if BuildOperation::from_call_target(call.target.as_str())
                                == Some(BuildOperation::ProviderSelection)
                                && !call.target_symbol.is_valid() =>
                        {
                            Some(ProviderSelectionCall {
                                target: call.target.as_str(),
                                arguments: &call.machine_arguments,
                                value_arguments: typed
                                    .statement_table
                                    .expression_handles(call.arguments),
                                source_span: call.source_span,
                                machine: row.machine,
                                composition_case: Some(row.composition_case),
                                product_operands: true,
                            })
                        }
                        _ => None,
                    }
                }
                checked_interpreter::ExecutedProviderSelectionSite::Expression(handle) => {
                    match typed.expression_table.expression(handle) {
                        typed_trees::expression::ExpressionNode::Call(call)
                            if BuildOperation::from_call_target(call.target.as_str())
                                == Some(BuildOperation::ProviderSelection)
                                && !call.target_symbol.is_valid() =>
                        {
                            Some(ProviderSelectionCall {
                                target: call.target.as_str(),
                                arguments: &call.machine_arguments,
                                value_arguments: typed
                                    .expression_table
                                    .expression_handles(call.arguments),
                                source_span: provider_selection_expression_source_span(
                                    typed, handle,
                                ),
                                machine: row.machine,
                                composition_case: Some(row.composition_case),
                                product_operands: true,
                            })
                        }
                        _ => None,
                    }
                }
            };
            request.ok_or_else(|| {
                vec![Diagnostic::error(
                    "executed provider selection did not rejoin its admitted call",
                )]
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    collect_provider_selections(context, requests)
}

fn provider_selection_expression_source_span(
    typed: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> source::SourceSpan {
    // The declaration occurrence names the call itself. An expression's
    // enclosing span can be synthetic after receiver/statement lowering.
    typed
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| typed.authored_declaration_selections().get(occurrence))
        .find(|selection| {
            selection.kind()
                == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
        })
        .map_or_else(
            || typed.expression_table.source_span(expression),
            |selection| selection.source_span(),
        )
}

fn collect_provider_selections<'a>(
    context: ProviderPlanningContext<'_>,
    requests: impl IntoIterator<Item = ProviderSelectionCall<'a>>,
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let typed = context.typed;
    let mut selections: Vec<ProviderSelection> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut record = |request: ProviderSelectionCall<'_>| {
        let ProviderSelectionCall {
            target,
            arguments,
            value_arguments,
            source_span,
            machine,
            composition_case,
            product_operands,
        } = request;
        if target != "select_provider" {
            return;
        }
        let [boundary_argument, provider_argument] = arguments else {
            diagnostics.push(Diagnostic::error(
                "provider selection must retain exactly two resolved type paths",
            ));
            return;
        };
        let project_identity = |argument: &typed_trees::expression::StaticMachineArgument,
                                is_provider: bool| {
            let authored_path = argument
                .path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            let symbol = if product_operands {
                typed_trees_to_checked_trees::typed_product_provider_selection_operand(
                    typed,
                    argument,
                    source_span,
                    !is_provider,
                )
                .unwrap_or_else(SymbolHandle::invalid)
            } else {
                argument.symbol
            };
            ProviderSelectionIdentity {
                symbol,
                package: typed.symbols.symbol_package_identity(symbol),
                canonical_path: typed.symbols.display_path(symbol, "::"),
                authored_path,
            }
        };
        let boundary_identity = project_identity(boundary_argument, false);
        let provider_type = project_identity(provider_argument, true);
        if !context.contains(boundary_identity.symbol) || !context.contains(provider_type.symbol) {
            diagnostics.push(Diagnostic::error(format!(
                "provider selection subject or provider is outside the {:?} planning context",
                context.scope,
            )));
            return;
        }
        let composition_mode = match composition_case.map_or_else(
            || provider_selection_composition_mode(typed, value_arguments),
            |case| provider_composition_case(typed, case),
        ) {
            Ok(mode) => mode,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                return;
            }
        };
        let subject = if boundary_identity.symbol.is_valid()
            && typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Trait
            && typed.traits().iter().any(|definition| {
                definition.symbol == boundary_identity.symbol && definition.is_boundary
            }) {
            crate::ProviderSelectionSubject::BoundaryTrait(boundary_identity)
        } else if boundary_identity.symbol.is_valid()
            && typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Machine
            && typed.machines().iter().any(|requirement| {
                requirement.symbol == boundary_identity.symbol
                    && requirement.spelling.is_none()
                    && requirement.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
            })
        {
            crate::ProviderSelectionSubject::BoundaryRequirement(boundary_identity)
        } else if boundary_identity.symbol.is_valid()
            && (typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Operator
                || typed.machine_token_bindings().iter().any(|binding| {
                    binding.is_boundary && binding.symbol == boundary_identity.symbol
                }))
        {
            // One resolved overload symbol names its complete package-
            // qualified family; provider-planning derives the canonical
            // roster so harvest and later replay share one definition.
            match crate::ProviderOperatorFamilySelection::derive(
                context,
                boundary_identity.symbol,
                boundary_identity.authored_path,
            ) {
                Ok(family) => crate::ProviderSelectionSubject::BoundaryOperatorFamily(family),
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(reason));
                    return;
                }
            }
        } else {
            diagnostics.push(Diagnostic::error(format!(
                "provider selection subject `{}` does not resolve to an exact boundary trait, top-level boundary requirement, or boundary-operator family",
                boundary_identity.authored_path
            )));
            return;
        };
        if !provider_type.symbol.is_valid()
            || typed.symbols.get(provider_type.symbol).kind != SymbolKind::Data
        {
            diagnostics.push(Diagnostic::error(format!(
                "provider selection type `{}` does not resolve to an exact data declaration",
                provider_type.authored_path
            )));
            return;
        }
        if let Some(existing) = selections
            .iter()
            .find(|selection| selection.subject.same_declaration_as(&subject))
        {
            if existing.provider_type.symbol != provider_type.symbol {
                diagnostics.push(Diagnostic::error(format!(
                    "build selects two provider types for slot `{}`: `{}` and `{}`",
                    subject.canonical_path(),
                    existing.provider_type.canonical_path,
                    provider_type.canonical_path,
                )));
                return;
            }
            if existing.composition_mode != composition_mode {
                diagnostics.push(Diagnostic::error(format!(
                    "build selects provider `{}` for slot `{}` with conflicting composition modes {:?} and {:?}",
                    provider_type.canonical_path,
                    subject.canonical_path(),
                    existing.composition_mode,
                    composition_mode,
                )));
                return;
            }
        }
        selections.push(ProviderSelection {
            subject,
            provider_type,
            composition_mode,
            selecting_machine: machine,
            source_span,
        });
    };
    for request in requests {
        record(request);
    }
    if diagnostics.is_empty() {
        Ok(selections)
    } else {
        Err(diagnostics)
    }
}

/// Recheck the declaration referenced by sealed executed-selection custody.
/// This validates its operands without rerunning Build or turning unexecuted
/// source calls into selections. Execution order and computed mode come from
/// the retained activation, not a second interpretation during package review.
pub fn validate_executed_provider_selection_declaration(
    typed: &TypedTrees,
    selection: &ProviderSelection,
) -> bool {
    let context = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let Some(machine) = typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == selection.selecting_machine)
    else {
        return false;
    };
    let mut matched = false;
    let mut validate =
        |target: &str,
         target_symbol: SymbolHandle,
         arguments: &[typed_trees::expression::StaticMachineArgument],
         values: &[typed_trees::expression::ExpressionHandle]| {
            if target != "select_provider" || target_symbol.is_valid() || values.len() > 1 {
                return false;
            }
            let [slot, provider] = arguments else {
                return false;
            };
            let slot_symbol =
                typed_trees_to_checked_trees::typed_product_provider_selection_operand(
                    typed,
                    slot,
                    selection.source_span,
                    true,
                );
            let provider_symbol =
                typed_trees_to_checked_trees::typed_product_provider_selection_operand(
                    typed,
                    provider,
                    selection.source_span,
                    false,
                );
            let subject_symbol = match &selection.subject {
                crate::ProviderSelectionSubject::BoundaryTrait(identity)
                | crate::ProviderSelectionSubject::BoundaryRequirement(identity) => identity.symbol,
                crate::ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                    // The representative may differ after declaration reordering;
                    // compare the complete derived family instead.
                    let Some(symbol) = slot_symbol else {
                        return false;
                    };
                    let path = slot
                        .path
                        .iter()
                        .map(|part| part.as_str())
                        .collect::<Vec<_>>()
                        .join("::");
                    if crate::ProviderOperatorFamilySelection::derive(context, symbol, path)
                        .as_ref()
                        != Ok(family)
                    {
                        return false;
                    }
                    symbol
                }
            };
            if slot_symbol != Some(subject_symbol)
                || provider_symbol != Some(selection.provider_type.symbol)
            {
                return false;
            }
            if values.is_empty() && selection.composition_mode != crate::CompositionMode::Fused {
                return false;
            }
            if let [value] = values
                && let Some(case) = exact_case_argument_symbol(typed, *value)
                && provider_composition_case(typed, case).ok() != Some(selection.composition_mode)
            {
                return false;
            }
            matched = true;
            true
        };
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            if let typed_trees::statement::StatementNode::Call(call) = statement
                && call.source_span == selection.source_span
                && !validate(
                    call.target.as_str(),
                    call.target_symbol,
                    &call.machine_arguments,
                    typed.statement_table.expression_handles(call.arguments),
                )
            {
                return false;
            }
        }
    }
    for handle in typed_trees_to_checked_trees::typed_provider_selection_expressions(typed, machine)
    {
        let expression = typed.expression_table.expression(handle);
        if provider_selection_expression_source_span(typed, handle) == selection.source_span
            && let typed_trees::expression::ExpressionNode::Call(call) = expression
            && (!typed_trees_to_checked_trees::typed_build_provider_selection(typed, handle)
                || !validate(
                    call.target.as_str(),
                    call.target_symbol,
                    &call.machine_arguments,
                    typed.expression_table.expression_handles(call.arguments),
                ))
        {
            return false;
        }
    }
    matched
}

fn provider_selection_composition_mode(
    typed: &TypedTrees,
    arguments: &[typed_trees::expression::ExpressionHandle],
) -> Result<crate::CompositionMode, Diagnostic> {
    let [] = arguments else {
        let [argument] = arguments else {
            return Err(Diagnostic::error(
                "provider selection must retain zero arguments for fused composition or one exact compiler-owned CompositionMode value",
            ));
        };
        let Some(case_symbol) = exact_case_argument_symbol(typed, *argument) else {
            return Err(Diagnostic::error(
                "provider selection composition mode must be the exact compiler-owned CompositionMode::Fused or CompositionMode::Independent case",
            ));
        };
        return provider_composition_case(typed, case_symbol);
    };
    Ok(crate::CompositionMode::Fused)
}

fn provider_composition_case(
    typed: &TypedTrees,
    case_symbol: SymbolHandle,
) -> Result<crate::CompositionMode, Diagnostic> {
    if !case_symbol.is_valid() {
        return Ok(crate::CompositionMode::Fused);
    }
    let exact_modes = typed
        .data_definitions()
        .iter()
        .filter(|definition| {
            is_exact_toolchain_build_prelude_data(typed, definition.symbol, "CompositionMode")
        })
        .collect::<Vec<_>>();
    let [modes] = exact_modes.as_slice() else {
        return Err(Diagnostic::error(
            "explicit provider composition mode requires exactly one compiler-owned CompositionMode declaration",
        ));
    };
    let selected = typed
        .data_members(modes)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Variant(variant)
                if variant.symbol == case_symbol
                    && typed.symbols.get(variant.symbol).parent == modes.symbol =>
            {
                Some(variant)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [selected] = selected.as_slice() else {
        return Err(Diagnostic::error(
            "provider selection composition mode does not name an exact compiler-owned CompositionMode case",
        ));
    };
    if !typed.data_payload_fields(selected).is_empty() {
        return Err(Diagnostic::error(
            "provider selection composition mode case unexpectedly carries a payload",
        ));
    }
    match selected.name.as_str() {
        "Fused" => Ok(crate::CompositionMode::Fused),
        "Independent" => Ok(crate::CompositionMode::Independent),
        other => Err(Diagnostic::error(format!(
            "compiler-owned CompositionMode contains unsupported case `{other}`"
        ))),
    }
}

/// The case symbol one build-declaration argument names, whichever typed
/// shape carries it. A payload-free case such as `CompositionMode::Independent`
/// or `CrashCause::Trap` is normalized to a constructor literal retaining the
/// exact case symbol; marker positions can instead retain the plain name path
/// or a member projection. Every shape must still resolve to an exact
/// toolchain case at the caller; any other expression is not a case.
fn exact_case_argument_symbol(
    typed: &TypedTrees,
    argument: typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    match typed.expression_table.expression(argument) {
        typed_trees::expression::ExpressionNode::Name(path) => Some(path.symbol),
        typed_trees::expression::ExpressionNode::Member(member) => Some(member.member_symbol),
        typed_trees::expression::ExpressionNode::StructLiteral(literal)
            if literal.fields.is_empty() =>
        {
            Some(literal.case_symbol.unwrap_or_else(SymbolHandle::invalid))
        }
        _ => None,
    }
}
