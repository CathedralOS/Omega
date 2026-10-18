//! Decode source provider declarations without choosing a target or realization.
//!
//! Static defaults enter through `harvest_provider_selections`; executed Build
//! choices enter through `collect_executed_provider_selections`. Both use one
//! decoder for exact subject, provider and composition identities. Package review
//! independently rejoins retained choices without rerunning Build. Native schema,
//! ABI and provider-plan construction belong to Omega and are not dependencies.

mod context;
mod identities;
mod selection;

use crate::is_exact_toolchain_build_prelude_data;
use checked_trees::typed_trees::TypedTrees;
pub use context::ProviderPlanningContext;
use diagnostics::Diagnostic;
pub use identities::{eligible, is_named_provider, named_requirement_lifetime_arguments};
pub use selection::{
    CompositionMode, ProviderOperatorFamilySelection, ProviderSelection, ProviderSelectionIdentity,
    ProviderSelectionSubject,
};
use symbols::{JoinedPath, SymbolHandle, SymbolKind};

/// Normalized declaration effects retained for every authored deployment.
/// These source handles describe Build inputs, not installed runtime authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetProviderDefaults {
    /// None is a target-independent declaration.
    pub target: Option<String>,
    /// Includes empty declarations, whose phase must not depend on effects.
    pub machines: Vec<symbols::SymbolHandle>,
    pub selections: Vec<ProviderSelection>,
}

/// Harvest every Product default before choosing a realization. Build defaults
/// instead belong to its separately admitted execution, not foreign siblings.
pub fn retain_provider_defaults(
    context: ProviderPlanningContext<'_>,
) -> Result<Vec<TargetProviderDefaults>, Vec<Diagnostic>> {
    let mut rows: Vec<TargetProviderDefaults> = Vec::new();
    for machine in context.typed.machines().iter().filter(|machine| {
        machine.purpose == checked_trees::typed_trees::machine::MachinePurpose::ProviderDefaults
            // A generated interface contributes no authored declarations.
            // Harvest the complete member bodies, not a spurious targetless
            // defaults row for their shared callable head.
            && !machine.target_family_interface
            && context.contains(machine.symbol)
            && (context.scope == source::DependencyScope::Product || machine.target.is_none())
    }) {
        let target = machine
            .authored_target
            .as_ref()
            .map(|name| name.as_str().to_owned());
        let selections = harvest_provider_selections(context, machine)?;
        if let Some(row) = rows.iter_mut().find(|row| row.target == target) {
            row.machines.push(machine.symbol);
            row.selections.extend(selections);
        } else {
            rows.push(TargetProviderDefaults {
                target,
                machines: vec![machine.symbol],
                selections,
            });
        }
    }
    rows.sort_by(|left, right| left.target.cmp(&right.target));
    Ok(rows)
}

/// Static target-default declarations have their own admission path. Authored
/// Build overrides instead enter through executed call receipts below.
fn harvest_provider_selections(
    context: ProviderPlanningContext<'_>,
    machine: &checked_trees::typed_trees::machine::Machine,
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
                checked_trees::typed_trees::statement::StatementNode::Expression(expression) => {
                    if let checked_trees::typed_trees::expression::ExpressionNode::Call(call) =
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
                checked_trees::typed_trees::statement::StatementNode::Call(call)
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
    arguments: &'a [checked_trees::typed_trees::expression::StaticMachineArgument],
    value_arguments: &'a [checked_trees::typed_trees::expression::ExpressionHandle],
    source_span: source::SourceSpan,
    machine: SymbolHandle,
    composition_case: Option<SymbolHandle>,
    product_operands: bool,
}

/// Rejoin only calls evaluated on the original Build cell. An unreachable
/// selection does not become configuration merely because its text exists.
pub fn collect_executed_provider_selections(
    context: ProviderPlanningContext<'_>,
    executed: &[checked_interpreter::ExecutedBuildSelection],
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
        .filter(|row| row.operation == language_semantics::declaration_selection::BuildOperation::ProviderSelection)
        .map(|row| {
            let handle = row.expression;
            let request = match typed.expression_table.expression(handle) {
                checked_trees::typed_trees::expression::ExpressionNode::Call(call)
                    if language_semantics::declaration_selection::is_unresolved_provider_selection_call(
                        call.target.as_str(),
                        call.target_symbol,
                    ) =>
                {
                    Some(ProviderSelectionCall {
                        target: call.target.as_str(),
                        arguments: &call.machine_arguments,
                        value_arguments: typed.expression_table.expression_handles(call.arguments),
                        source_span: provider_selection_expression_source_span(typed, handle),
                        machine: row.machine,
                        composition_case: Some(row.composition_case),
                        product_operands: true,
                    })
                }
                _ => None,
            };
            request.ok_or_else(|| {
                vec![Diagnostic::error(
                    "executed provider selection did not rejoin its admitted call",
                )]
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    collect_provider_selections(
        ProviderPlanningContext::new(
            typed.product_declarations(),
            source::DependencyScope::Product,
        ),
        requests,
    )
}

fn provider_selection_expression_source_span(
    typed: &TypedTrees,
    expression: checked_trees::typed_trees::expression::ExpressionHandle,
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
        let project_identity =
            |argument: &checked_trees::typed_trees::expression::StaticMachineArgument,
             is_provider: bool| {
                let authored_path = argument.path.joined_path();
                let symbol = if product_operands {
                    checked_trees::typed_trees::product_operand::resolve_product_operand(
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
            ProviderSelectionSubject::BoundaryTrait(boundary_identity)
        } else if boundary_identity.symbol.is_valid()
            && typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Machine
            && typed.machines().iter().any(|requirement| {
                requirement.symbol == boundary_identity.symbol
                    && requirement.spelling.is_none()
                    && requirement.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
            })
        {
            ProviderSelectionSubject::BoundaryRequirement(boundary_identity)
        } else if boundary_identity.symbol.is_valid()
            && (typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Operator
                || typed.machine_token_bindings().iter().any(|binding| {
                    binding.is_boundary && binding.symbol == boundary_identity.symbol
                }))
        {
            // One resolved overload symbol names its complete package-
            // qualified family; provider-planning derives the canonical
            // roster so harvest and later replay share one definition.
            match ProviderOperatorFamilySelection::derive(
                context,
                boundary_identity.symbol,
                boundary_identity.authored_path,
            ) {
                Ok(family) => ProviderSelectionSubject::BoundaryOperatorFamily(family),
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
            || (typed.symbols.get(provider_type.symbol).kind != SymbolKind::Data
                && !identities::is_named_provider(typed, provider_type.symbol))
        {
            diagnostics.push(Diagnostic::error(format!(
                "provider selection type `{}` does not resolve to an exact provider data declaration or named implementation",
                provider_type.authored_path
            )));
            return;
        }
        if let Some(implementation) = typed.conformances().iter().find(|implementation| {
            implementation.symbol == provider_type.symbol && identities::eligible(implementation)
        }) {
            for row in &implementation.rows {
                if identities::named_requirement_lifetime_arguments(
                    typed,
                    implementation,
                    row.declaring_trait,
                )
                .is_none()
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "named provider `{}` requires an unsupported inherited declaring-trait lifetime application for `{}`; the exact parent substitution is not retained",
                        provider_type.authored_path,
                        row.requirement_name,
                    )));
                    return;
                }
            }
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

fn provider_selection_composition_mode(
    typed: &TypedTrees,
    arguments: &[checked_trees::typed_trees::expression::ExpressionHandle],
) -> Result<CompositionMode, Diagnostic> {
    let Some((_, arguments)) = arguments.split_first() else {
        return Err(Diagnostic::error(
            "provider selection requires an explicit activation argument",
        ));
    };
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
    Ok(CompositionMode::Fused)
}

fn provider_composition_case(
    typed: &TypedTrees,
    case_symbol: SymbolHandle,
) -> Result<CompositionMode, Diagnostic> {
    if !case_symbol.is_valid() {
        return Ok(CompositionMode::Fused);
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
            checked_trees::typed_trees::data::DataMember::Variant(variant)
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
        "Fused" => Ok(CompositionMode::Fused),
        "Independent" => Ok(CompositionMode::Independent),
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
    argument: checked_trees::typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    let symbol = match typed.expression_table.expression(argument) {
        checked_trees::typed_trees::expression::ExpressionNode::Name(path) => Some(path.symbol),
        checked_trees::typed_trees::expression::ExpressionNode::Member(member) => {
            Some(member.member_symbol)
        }
        checked_trees::typed_trees::expression::ExpressionNode::StructLiteral(literal)
            if literal.fields.is_empty() =>
        {
            Some(literal.case_symbol.unwrap_or_else(SymbolHandle::invalid))
        }
        _ => None,
    }?;
    // A local or parameter naming a computed mode is not a case literal.
    // Its value belongs to the retained Build execution; review may compare
    // a literal directly only when resolution selected an actual variant.
    (symbol.is_valid() && typed.symbols.get(symbol).kind == SymbolKind::Variant).then_some(symbol)
}
