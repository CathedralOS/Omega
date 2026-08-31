//! Exact checked occurrence join for closed conformance static arguments.

use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

use super::ContractCallStaticParameterKind;
use crate::capture::contracts::facts::ContractProjectionContext;
use crate::capture::semantics::declarations::nominal_identity;
use crate::record::PackageReviewContractStaticArgument;

pub(crate) fn require_exact_conformance_static_argument_selections(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    expression: psi_typed_trees::expression::ExpressionHandle,
    arguments: &[psi_typed_trees::expression::StaticMachineArgument],
) -> Result<(), Vec<Diagnostic>> {
    use psi_language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelectionTarget,
    };

    let authored = arguments
        .iter()
        .filter(|argument| {
            argument.symbol.is_valid()
                && compilation.typed.symbols.get(argument.symbol).kind
                    == psi_symbols::SymbolKind::Conformance
        })
        .map(|argument| argument.symbol)
        .collect::<Vec<_>>();
    let mut retained = Vec::new();
    for occurrence in compilation
        .expression_table
        .authored_selection_occurrences(expression)
    {
        let Some(selection) = compilation
            .authored_declaration_selections()
            .get(occurrence)
        else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` retains an unknown conformance static-argument selection occurrence",
                context.subject_kind, context.subject_name,
            ))]);
        };
        if selection.kind() != AuthoredDeclarationSelectionKind::Conformance {
            continue;
        }
        let AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
            continue;
        };
        if compilation.typed.symbols.get(target.selected_symbol()).kind
            != psi_symbols::SymbolKind::Conformance
        {
            continue;
        }
        if selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface {
            return Err(vec![Diagnostic::error(format!(
                "reviewed {} `{}` conformance static argument is not retained as a public-interface selection",
                context.subject_kind, context.subject_name,
            ))]);
        }
        retained.push(target.selected_symbol());
    }
    if retained != authored {
        return Err(vec![Diagnostic::error(format!(
            "reviewed {} `{}` conformance static arguments do not match their exact authored selections",
            context.subject_kind, context.subject_name,
        ))]);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn project_contract_conformance_application(
    compilation: &CheckedCompilation,
    context: &ContractProjectionContext<'_>,
    binders: &[(SymbolHandle, String)],
    checked_fact: Option<psi_arena::Handle<psi_typed_trees::domain::ProofFact>>,
    expression: psi_typed_trees::expression::ExpressionHandle,
    static_argument_position: usize,
    argument: &psi_typed_trees::expression::StaticMachineArgument,
    parameter_kind: ContractCallStaticParameterKind,
    depth: usize,
) -> Result<PackageReviewContractStaticArgument, Vec<Diagnostic>> {
    let rejected = |reason: &str| {
        vec![Diagnostic::error(format!(
            "reviewed {} `{}` uses a static conformance application {reason}",
            context.subject_kind, context.subject_name,
        ))]
    };
    if depth != 0 || parameter_kind != ContractCallStaticParameterKind::Conformance {
        return Err(rejected(
            "outside the admitted top-level conformance-binder cohort",
        ));
    }
    let checked_fact = checked_fact
        .ok_or_else(|| rejected("without an exact checked proof-fact occurrence owner"))?;
    let matching = compilation
        .facts
        .proof
        .contract_expression_static_conformance_applications
        .iter()
        .filter(|candidate| {
            candidate.owner == context.owner
                && candidate.fact == checked_fact
                && candidate.expression == expression
                && candidate.static_argument_position == static_argument_position
        })
        .collect::<Vec<_>>();
    let [checked] = matching.as_slice() else {
        return Err(rejected(&format!(
            "with {} exact checked occurrence rows; expected one",
            matching.len()
        )));
    };
    let closed = psi_typed_trees_to_checked_trees::close_conformance_application(
        &compilation.typed,
        argument,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    if checked.application != closed {
        return Err(rejected(
            "whose retained checked occurrence disagrees with the authored application",
        ));
    }
    let declarations = compilation
        .conformances()
        .iter()
        .filter(|declaration| declaration.symbol == argument.symbol)
        .collect::<Vec<_>>();
    let [declaration] = declarations.as_slice() else {
        return Err(rejected(&format!(
            "that rejoins {} conformance declarations; expected one",
            declarations.len()
        )));
    };
    let parameters = compilation.conformance_type_parameters(declaration);
    if !declaration.lifetime_parameters.is_empty()
        || parameters.iter().any(|parameter| {
            !matches!(
                parameter.kind,
                psi_typed_trees::data::TypeParameterKind::Type
            )
        })
        || !closed.lifetime_arguments.is_empty()
        || !closed.const_arguments.is_empty()
        || !closed.machine_arguments.is_empty()
    {
        return Err(rejected(
            "outside the lifetime-free, type-only closed cohort",
        ));
    }
    let projected =
        crate::capture::semantics::conformances::project_selected_conformance_application(
            compilation,
            argument,
            binders,
            context.lifetime_binders,
            context.subject_kind,
            context.subject_name,
        )?;
    if !projected.lifetime_arguments.is_empty() || !projected.trait_lifetime_arguments.is_empty() {
        return Err(rejected(
            "whose declaration or target trait carries an erased lifetime",
        ));
    }
    let matching_traits = compilation
        .traits()
        .iter()
        .filter(|definition| definition.symbol == projected.trait_symbol)
        .collect::<Vec<_>>();
    let [trait_definition] = matching_traits.as_slice() else {
        return Err(rejected(&format!(
            "whose target rejoins {} trait declarations; expected one",
            matching_traits.len()
        )));
    };
    if !trait_definition.is_public {
        return Err(rejected("that exposes a non-public target trait"));
    }
    Ok(
        PackageReviewContractStaticArgument::ConformanceApplication {
            declaration: projected.declaration,
            arguments: projected.arguments,
            subject: Box::new(projected.subject),
            trait_identity: nominal_identity(compilation, projected.trait_symbol)?,
            trait_arguments: projected.trait_arguments,
        },
    )
}
