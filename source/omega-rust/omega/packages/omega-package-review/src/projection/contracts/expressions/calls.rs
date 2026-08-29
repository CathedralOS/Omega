use crate::model::{PackageReviewByteSequencePredicate, PackageReviewContractCallTarget};
use crate::projection::contracts::ContractProjectionContext;
use crate::projection::exact_identity::nominal_identity;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(crate) fn exact_fact_call_projection<'compilation>(
    compilation: &'compilation CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    projection_expression: psi_typed_trees::expression::ExpressionHandle,
    call_expression: psi_typed_trees::expression::ExpressionHandle,
    member: &psi_typed_trees::expression::TableMemberExpression,
) -> Result<&'compilation psi_checked_trees::CheckedFactCallProjection, Vec<Diagnostic>> {
    use psi_typed_trees::expression::ExpressionNode;

    let ExpressionNode::Call(call) = compilation.expression_table.expression(call_expression)
    else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection does not rejoin a call expression",
            context.subject_kind, context.subject_name
        ))]);
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
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection rejoins {} exact eligibility certificates; expected one",
            context.subject_kind,
            context.subject_name,
            matching.len()
        ))]);
    };
    let target = compilation
        .machines()
        .iter()
        .find(|machine| machine.symbol == projection.target_machine)
        .and_then(|machine| {
            compilation
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == projection.target_state)
        });
    if target.is_none_or(|state| state.return_type != projection.result_type) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection certificate no longer rejoins its exact result type",
            context.subject_kind, context.subject_name
        ))]);
    }
    if member.case_variant.is_some() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection is not a direct record field",
            context.subject_kind, context.subject_name
        ))]);
    }
    let data_symbol = match compilation
        .type_reference_table
        .type_reference(projection.result_type)
    {
        psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
        psi_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } => {
            Some(*base_symbol)
        }
        _ => None,
    };
    let field_rejoins = data_symbol
        .and_then(|symbol| {
            compilation
                .data_definitions()
                .iter()
                .find(|data| data.symbol == symbol)
        })
        .and_then(|data| {
            compilation.data_members(data).iter().find_map(|candidate| {
                let psi_typed_trees::data::DataMember::Field(field) = candidate else {
                    return None;
                };
                (field.name.as_str() == member.member.as_str()).then_some(field.symbol)
            })
        })
        .is_some_and(|field| field == projection.field);
    if !field_rejoins {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` fact-call projection certificate no longer rejoins its exact field",
            context.subject_kind, context.subject_name
        ))]);
    }
    Ok(projection)
}

pub(crate) fn exact_checked_contract_call_target(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: psi_typed_trees::expression::ExpressionHandle,
    call: &psi_typed_trees::expression::TableCallExpression,
) -> Result<PackageReviewContractCallTarget, Vec<Diagnostic>> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelectionTarget,
    };

    let selections = compilation
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            compilation
                .authored_declaration_selections()
                .get(occurrence)
        })
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call has {} exact checked call-selection rows; expected one",
            context.subject_kind,
            context.subject_name,
            selections.len()
        ))]);
    };
    if selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call is not retained as a public-interface selection",
            context.subject_kind, context.subject_name
        ))]);
    }
    match selection.target() {
        AuthoredDeclarationSelectionTarget::Resolved(target)
            if target.selected_symbol() == call.target_symbol =>
        {
            if let Some(function) = compilation
                .typed
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
            {
                Ok(PackageReviewContractCallTarget::BuiltinFunction(function))
            } else {
                Ok(PackageReviewContractCallTarget::Nominal(nominal_identity(
                    compilation,
                    call.target_symbol,
                )?))
            }
        }
        AuthoredDeclarationSelectionTarget::Resolved(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract call target disagrees with its exact checked call-selection row",
            context.subject_kind, context.subject_name
        ))]),
        AuthoredDeclarationSelectionTarget::Intrinsic(
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::ByteSequencePredicate(
                predicate,
            ),
        ) if !call.target_symbol.is_valid()
            && !call.receiver.is_valid()
            && psi_language_semantics::byte_predicates::ByteSequencePredicate::from_name(
                call.target.as_str(),
            ) == Some(predicate) => Ok(PackageReviewContractCallTarget::ByteSequencePredicate(
                match predicate {
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::ValidUtf8 => {
                        PackageReviewByteSequencePredicate::ValidUtf8
                    }
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::NoNul => {
                        PackageReviewByteSequencePredicate::NoNul
                    }
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::AsciiOnly => {
                        PackageReviewByteSequencePredicate::AsciiOnly
                    }
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::NonEmpty => {
                        PackageReviewByteSequencePredicate::NonEmpty
                    }
                },
            )),
        AuthoredDeclarationSelectionTarget::Intrinsic(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` contract-call intrinsic identity disagrees with its exact checked call-selection row or is not yet represented by package review",
            context.subject_kind, context.subject_name
        ))]),
        AuthoredDeclarationSelectionTarget::LateBound(_) => Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` retains an unresolved contract call selection",
            context.subject_kind, context.subject_name
        ))]),
    }
}
