//! Capture nominal tag observation independently of value equality.
//!
//! Authored membership has carrier/case selections, not an operator selection;
//! synthesized tag tests have an explicit typed operation. Rejoin either form
//! with Psi's exact subject/carrier/case validator before recording the nominal
//! classifier. The RHS is not evaluated or projected as a constructor. Ordinary
//! structural equality in proof facts retains its authored operator and does not
//! require exporting a generated Boolean expansion into the review format.

use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::facts::exactly_one;
use crate::record::PackageReviewNominalIdentity;
use checked_trees::ContractProofFactOwner;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
use typed_trees::AuthoredDeclarationSelectionKind;
use typed_trees::AuthoredDeclarationSelectionTarget;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};

pub(super) fn is_case_membership(
    compilation: &CheckedCompilation,
    expression: ExpressionHandle,
    binary: &TableBinaryExpression,
) -> bool {
    binary.operator == BinaryOperator::CaseMembership
        || compilation
            .expression_table
            .authored_selection_occurrences(expression)
            .any(|occurrence| {
                compilation
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_some_and(|selection| {
                        matches!(
                            selection.kind(),
                            AuthoredDeclarationSelectionKind::CaseMembership
                                | AuthoredDeclarationSelectionKind::CaseReference
                        )
                    })
            })
}

pub(super) fn checked_case_classifier(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: ExpressionHandle,
    binary: &TableBinaryExpression,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let rejected = || {
        vec![Diagnostic::error(format!(
            "reviewed {} `{}` case membership does not retain its exact subject, carrier, case and selection exposure",
            context.subject_kind, context.subject_name,
        ))]
    };
    let machine_owner = match context.owner {
        ContractProofFactOwner::Machine { machine_symbol } => Some((machine_symbol, None)),
        ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => Some((machine_symbol, Some(state_symbol))),
        _ => None,
    };
    // Declarations and abstract signatures have their own subject custody
    // without an executable machine. Pass that exact scope to Psi instead of
    // inventing an attachment or borrowing a same-spelled subject elsewhere.
    let exact_meaning = if let Some((machine_symbol, state_symbol)) = machine_owner {
        let machine = exactly_one(
            compilation
                .machines()
                .iter()
                .filter(|machine| machine.symbol == machine_symbol),
            context.subject_name,
            "case-membership machine owner",
        )?;
        let state = state_symbol
            .map(|symbol| {
                exactly_one(
                    compilation
                        .machine_states(machine)
                        .iter()
                        .filter(|state| state.symbol == symbol),
                    context.subject_name,
                    "case-membership state owner",
                )
            })
            .transpose()?;
        validation::has_exact_case_membership_meaning(
            &compilation.typed,
            machine,
            state,
            expression,
            binary,
        )
    } else if let Some(domain_symbol) = context.domain_symbol {
        let domain = exactly_one(
            compilation
                .domain_definitions()
                .iter()
                .filter(|domain| domain.symbol == domain_symbol),
            context.subject_name,
            "case-membership domain owner",
        )?;
        validation::has_exact_domain_case_membership_meaning(
            &compilation.typed,
            domain,
            expression,
            binary,
        )
    } else if let Some(data_symbol) = context.data_symbol {
        let data = exactly_one(
            compilation
                .data_definitions()
                .iter()
                .filter(|data| data.symbol == data_symbol),
            context.subject_name,
            "case-membership data owner",
        )?;
        validation::has_exact_data_case_membership_meaning(
            &compilation.typed,
            data,
            expression,
            binary,
        )
    } else {
        validation::has_exact_parameter_case_membership_meaning(
            &compilation.typed,
            context.parameters,
            expression,
            binary,
        )
    };
    if !exact_meaning
        || compilation
            .expression_table
            .authored_selection_occurrences(expression)
            .any(|occurrence| {
                compilation
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_none_or(|selection| {
                        selection.exposure() != context.selection_exposure
                        // Generated tag tests may retain source-origin builtin rows,
                        // but must never erase a selected declared overload.
                        || (binary.operator == BinaryOperator::CaseMembership
                            && selection.kind() == AuthoredDeclarationSelectionKind::Operator
                            && selection.target()
                                != AuthoredDeclarationSelectionTarget::Intrinsic(
                                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                                ))
                    })
            })
    {
        return Err(rejected());
    }
    let ExpressionNode::Name(case) = compilation.expression_table.expression(binary.right) else {
        return Err(rejected());
    };
    let carrier = validation::exact_case_reference_owner(&compilation.typed, binary.right)
        .ok_or_else(rejected)?;
    if context.requires_public_nominals() && !carrier.is_public {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` exposes non-public data `{}` through case membership",
            context.subject_kind, context.subject_name, carrier.name,
        ))]);
    }
    nominal_identity(compilation, case.symbol)
}
