use super::targets::contract_call_target_telescopes;
use crate::capture::PackageReviewInput;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::{PackageReviewByteSequencePredicate, PackageReviewContractCallTarget};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(crate) fn resolved_contract_call_symbol(
    compilation: &PackageReviewInput<'_>,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<SymbolHandle> {
    call.target_symbol
        .is_valid()
        .then_some(call.target_symbol)
        .or_else(|| {
            typed_trees::operator::resolve_named_expression_call(compilation.checking_input, call)
                .map(|operator| operator.symbol)
        })
}

pub(crate) fn contract_call_value_receiver(
    compilation: &PackageReviewInput<'_>,
    call: &typed_trees::expression::TableCallExpression,
    target: Option<SymbolHandle>,
) -> Option<typed_trees::expression::ExpressionHandle> {
    if !call.receiver.is_valid() {
        return None;
    }
    if target
        .is_some_and(|target| contract_call_target_parameter_sets(compilation, target).len() == 1)
    {
        return None;
    }
    if !call.target_symbol.is_valid()
        && let Some(operator) =
            typed_trees::operator::resolve_named_expression_call(compilation.checking_input, call)
        && let typed_trees::expression::ExpressionNode::Name(path) =
            compilation.expression_table.expression(call.receiver)
    {
        let receiver = compilation.expression_table.name_path_members(path.members);
        let operator_path = compilation.operator_path_members(operator.name);
        if operator_path.split_last().is_some_and(|(_, namespace)| {
            namespace.len() == receiver.len()
                && namespace
                    .iter()
                    .zip(receiver)
                    .all(|(expected, actual)| expected == actual)
        }) {
            return None;
        }
    }
    Some(call.receiver)
}

fn contract_call_target_parameter_sets<'a>(
    compilation: &'a PackageReviewInput<'_>,
    target: SymbolHandle,
) -> Vec<&'a [typed_trees::signature::StateParameter]> {
    contract_call_target_telescopes(compilation, target)
        .into_iter()
        .map(|telescope| telescope.value_parameters)
        .collect()
}

pub(crate) fn require_exact_contract_call_reference_arguments(
    compilation: &PackageReviewInput<'_>,
    context: &ContractProjectionContext<'_>,
    target: SymbolHandle,
    call: &typed_trees::expression::TableCallExpression,
) -> Result<(), Vec<Diagnostic>> {
    let candidates = contract_call_target_parameter_sets(compilation, target);
    let [parameters] = candidates.as_slice() else {
        return Err(context.rejected(format!(
            "contract call target rejoins {} value telescopes; expected exactly one",
            candidates.len()
        )));
    };
    let arguments = compilation
        .expression_table
        .expression_handles(call.arguments);
    let parameters = parameters.iter().collect::<Vec<_>>();
    if arguments.len() != parameters.len() {
        return Err(context.rejected("contract call has inconsistent checked value arity"));
    }
    for (argument, parameter) in arguments.iter().zip(parameters) {
        if matches!(
            compilation.expression_table.expression(*argument),
            typed_trees::expression::ExpressionNode::Borrow(_)
        ) && !validation::checked_argument_matches_type_reference(
            compilation.checking_input,
            *argument,
            parameter.type_reference,
        ) {
            return Err(context
                .rejected("reference argument does not match its contract-call parameter type"));
        }
    }
    Ok(())
}

pub(crate) fn exact_fact_call_projection<'compilation>(
    compilation: &'compilation PackageReviewInput<'compilation>,
    context: &ContractProjectionContext<'_>,
    projection_expression: typed_trees::expression::ExpressionHandle,
    call_expression: typed_trees::expression::ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Result<&'compilation checked_trees::CheckedFactCallProjection, Vec<Diagnostic>> {
    let Some(call) = compilation.expression_table.call_node(call_expression) else {
        return Err(context.rejected("fact-call projection does not rejoin a call expression"));
    };
    let matching = compilation
        .facts
        .fact_call_projections
        .iter()
        .filter(|projection| {
            projection.projection_expression == projection_expression
                && projection.call_expression == call_expression
                && projection.target_state == call.target_symbol
                && projection.machine_arguments == call.machine_arguments
        })
        .collect::<Vec<_>>();
    let [projection] = matching.as_slice() else {
        return Err(context.rejected(format!(
            "fact-call projection rejoins {} exact eligibility certificates; expected one",
            matching.len()
        )));
    };
    let target = compilation
        .machine(projection.target_machine)
        .and_then(|machine| compilation.state_in_machine(machine, projection.target_state));
    if target.is_none_or(|state| state.return_type != projection.result_type) {
        return Err(context
            .rejected("fact-call projection certificate no longer rejoins its exact result type"));
    }
    if member.case_variant.is_some() {
        return Err(context.rejected("fact-call projection is not a direct record field"));
    }
    let data_symbol = match compilation
        .type_reference_table
        .type_reference(projection.result_type)
    {
        typed_trees::types::TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
        _ => None,
    };
    let field_rejoins = data_symbol
        .and_then(|symbol| compilation.data_definition_by_symbol(symbol))
        .and_then(|data| {
            compilation.data_members(data).iter().find_map(|candidate| {
                let field = candidate.as_field()?;
                (field.name.as_str() == member.member.as_str()).then_some(field.symbol)
            })
        })
        .is_some_and(|field| field == projection.field);
    if !field_rejoins {
        return Err(
            context.rejected("fact-call projection certificate no longer rejoins its exact field")
        );
    }
    Ok(projection)
}

pub(crate) fn exact_checked_contract_call_target(
    compilation: &PackageReviewInput<'_>,
    context: &ContractProjectionContext<'_>,
    expression: typed_trees::expression::ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
) -> Result<PackageReviewContractCallTarget, Vec<Diagnostic>> {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
    };

    let selections = compilation
        .authored_selections(expression)
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(context.rejected(format!(
            "contract call has {} exact checked call-selection rows; expected one",
            selections.len()
        )));
    };
    if selection.exposure() != context.selection_exposure {
        return Err(context.rejected("contract call has the wrong retained selection exposure"));
    }
    let retained_symbol = resolved_contract_call_symbol(compilation, call);
    let owner_derived_symbol = compilation.derive_checked_nominal_call_target(expression);
    if retained_symbol.is_some()
        && owner_derived_symbol.is_some()
        && retained_symbol != owner_derived_symbol
    {
        return Err(context.rejected(
            "contract call retained target disagrees with its exact checked owner derivation",
        ));
    }
    let resolved_symbol = retained_symbol.or(owner_derived_symbol);
    match selection.target() {
        AuthoredDeclarationSelectionTarget::Resolved(target)
            if Some(target.selected_symbol()) == resolved_symbol =>
        {
            if let Some(function) = compilation
                .typed
                .symbols
                .builtin_function_for_symbol(target.selected_symbol())
            {
                Ok(PackageReviewContractCallTarget::BuiltinFunction(function))
            } else {
                Ok(PackageReviewContractCallTarget::Nominal(nominal_identity(
                    compilation,
                    target.selected_symbol(),
                )?))
            }
        }
        AuthoredDeclarationSelectionTarget::Resolved(_) => Err(context.rejected(
            "contract call target disagrees with its exact checked call-selection row",
        )),
        AuthoredDeclarationSelectionTarget::Intrinsic(
            language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::ByteSequencePredicate(
                predicate,
            ),
        ) if !call.target_symbol.is_valid()
            && !call.receiver.is_valid()
            && language_semantics::byte_predicates::ByteSequencePredicate::from_name(
                call.target.as_str(),
            ) == Some(predicate) => Ok(PackageReviewContractCallTarget::ByteSequencePredicate(
                match predicate {
                    language_semantics::byte_predicates::ByteSequencePredicate::ValidUtf8 => {
                        PackageReviewByteSequencePredicate::ValidUtf8
                    }
                    language_semantics::byte_predicates::ByteSequencePredicate::NoNul => {
                        PackageReviewByteSequencePredicate::NoNul
                    }
                    language_semantics::byte_predicates::ByteSequencePredicate::AsciiOnly => {
                        PackageReviewByteSequencePredicate::AsciiOnly
                    }
                    language_semantics::byte_predicates::ByteSequencePredicate::NonEmpty => {
                        PackageReviewByteSequencePredicate::NonEmpty
                    }
                },
            )),
        // A bare receiverless contract call whose spelled callee selects no
        // package declaration is admitted by checking as an opaque proof-view
        // atom (`Bag(items)`-style schematic views); the finalized ledger row
        // carries the compiler-owned ProofView intrinsic. Package review
        // rejoins that admission shape: the callee stays unresolved and
        // receiverless, and the retained name is the atom's whole identity.
        AuthoredDeclarationSelectionTarget::Intrinsic(
            language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::ProofView,
        ) if !call.target_symbol.is_valid()
            && !call.receiver.is_valid()
            && matches!(
                compilation.typed.symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
                    call.target.as_str(),
                    &[
                        symbols::SymbolKind::Module,
                        symbols::SymbolKind::BuiltinType,
                        symbols::SymbolKind::BuiltinFunction,
                        symbols::SymbolKind::Data,
                        symbols::SymbolKind::Domain,
                        symbols::SymbolKind::Machine,
                        symbols::SymbolKind::Operator,
                        symbols::SymbolKind::Measure,
                        symbols::SymbolKind::Proposition,
                        symbols::SymbolKind::MathematicalDefinition,
                        symbols::SymbolKind::Trait,
                        symbols::SymbolKind::Conformance,
                        symbols::SymbolKind::Const,
                        symbols::SymbolKind::ConformanceParameter,
                        symbols::SymbolKind::WireSchema,
                    ],
                    selection.source_span(),
                    |_| true,
                ),
                symbols::SymbolLookup::NotFound
            ) =>
        {
            Ok(PackageReviewContractCallTarget::ProofView {
                name: call.target.as_str().to_owned(),
            })
        }
        AuthoredDeclarationSelectionTarget::Intrinsic(_) => Err(context.rejected(
            "contract-call intrinsic identity disagrees with its exact checked call-selection row or is not yet represented by package review",
        )),
        AuthoredDeclarationSelectionTarget::LateBound(_) => Err(context.rejected(
            "retains an unresolved contract call selection",
        )),
    }
}
