use psi_checked_trees::CheckFacts;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionFinalizationError, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionTarget,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::{TypedTrees, expression::ExpressionNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CheckedResolution {
    occurrence: AuthoredDeclarationSelectionOccurrenceId,
    binding: AuthoredDeclarationSelectionLateBinding,
    selected: SymbolHandle,
}

pub(crate) fn finalize_checked_authored_selections(
    program: &mut TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Diagnostic> {
    let mut resolutions = Vec::new();
    let expressions = &program.tables.expression_table;

    for (expression, node) in expressions.iter_expressions() {
        let occurrences = expressions
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        for (occurrence_offset, occurrence) in occurrences.iter().copied().enumerate() {
            let Some(selection) = program.authored_declaration_selections().get(occurrence) else {
                return Err(Diagnostic::error(format!(
                    "expression retains unknown authored declaration selection occurrence {}",
                    occurrence.ordinal()
                )));
            };
            let AuthoredDeclarationSelectionTarget::LateBound(binding) = selection.target() else {
                continue;
            };

            let selected = match (binding, node) {
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    ExpressionNode::Call(call),
                ) => call.target_symbol,
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedMember,
                    ExpressionNode::Member(member),
                ) => member.member_symbol,
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStaticPathSegment,
                    ExpressionNode::Name(path),
                ) => expressions
                    .name_path_member_symbols(path.member_symbols)
                    .get(late_binding_ordinal(
                        program,
                        &occurrences[..occurrence_offset],
                        binding,
                    ))
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStructLiteralType,
                    ExpressionNode::StructLiteral(literal),
                ) => literal.type_symbol,
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStructLiteralCase,
                    ExpressionNode::StructLiteral(literal),
                ) => literal.case_symbol.unwrap_or_else(SymbolHandle::invalid),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedStructLiteralField,
                    ExpressionNode::StructLiteral(literal),
                ) => expressions
                    .struct_fields(literal.fields)
                    .get(late_binding_ordinal(
                        program,
                        &occurrences[..occurrence_offset],
                        binding,
                    ))
                    .map(|field| field.field_symbol)
                    .unwrap_or_else(SymbolHandle::invalid),
                (AuthoredDeclarationSelectionLateBinding::CheckedOperator, _)
                    if matches!(node, ExpressionNode::Binary(_) | ExpressionNode::Unary(_)) =>
                {
                    facts
                        .operators
                        .expression_use(expression)
                        .map(|operator_use| operator_use.selected_operator_symbol)
                        .unwrap_or_else(SymbolHandle::invalid)
                }
                _ => SymbolHandle::invalid(),
            };

            if selected.is_valid() {
                push_consistent_resolution(
                    &mut resolutions,
                    CheckedResolution {
                        occurrence,
                        binding,
                        selected,
                    },
                )?;
            }
        }
    }

    let mut selections = program.authored_declaration_selections().clone();
    for resolution in resolutions {
        selections
            .finalize_late_bound(
                resolution.occurrence,
                resolution.binding,
                resolution.selected,
            )
            .map_err(|error| finalization_diagnostic(resolution, error))?;
    }
    program.retain_authored_declaration_selections(selections);
    Ok(())
}

fn late_binding_ordinal(
    program: &TypedTrees,
    prior_occurrences: &[AuthoredDeclarationSelectionOccurrenceId],
    binding: AuthoredDeclarationSelectionLateBinding,
) -> usize {
    prior_occurrences
        .iter()
        .filter(|occurrence| {
            program
                .authored_declaration_selections()
                .get(**occurrence)
                .is_some_and(|selection| {
                    selection.target() == AuthoredDeclarationSelectionTarget::LateBound(binding)
                })
        })
        .count()
}

fn push_consistent_resolution(
    resolutions: &mut Vec<CheckedResolution>,
    candidate: CheckedResolution,
) -> Result<(), Diagnostic> {
    if let Some(existing) = resolutions
        .iter()
        .find(|resolution| resolution.occurrence == candidate.occurrence)
    {
        if *existing != candidate {
            return Err(Diagnostic::error(format!(
                "authored declaration selection occurrence {} resolved inconsistently across compiler-derived copies",
                candidate.occurrence.ordinal()
            )));
        }
        return Ok(());
    }
    resolutions.push(candidate);
    Ok(())
}

fn finalization_diagnostic(
    resolution: CheckedResolution,
    error: AuthoredDeclarationSelectionFinalizationError,
) -> Diagnostic {
    Diagnostic::error(format!(
        "failed to finalize authored declaration selection occurrence {}: {error:?}",
        resolution.occurrence.ordinal()
    ))
}
