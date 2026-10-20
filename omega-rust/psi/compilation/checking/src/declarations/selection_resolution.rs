//! Authored declaration-selection resolutions shared by typed preparation
//! and checking. Collect exact occurrence meanings before publishing them;
//! copies must agree, and a checked ledger never repairs a typed table.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionFinalizationError, AuthoredDeclarationSelectionIntrinsic,
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionTarget,
};
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthoredSelectionResolution {
    pub occurrence: AuthoredDeclarationSelectionOccurrenceId,
    pub binding: AuthoredDeclarationSelectionLateBinding,
    pub target: AuthoredSelectionResolutionTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredSelectionResolutionTarget {
    Declaration(SymbolHandle),
    Intrinsic(AuthoredDeclarationSelectionIntrinsic),
}

pub fn push_consistent_resolution(
    resolutions: &mut Vec<AuthoredSelectionResolution>,
    candidate: AuthoredSelectionResolution,
) -> Result<(), Diagnostic> {
    if let Some(existing) = resolutions
        .iter()
        .find(|resolution| resolution.occurrence == candidate.occurrence)
    {
        if *existing != candidate {
            return Err(Diagnostic::error(format!(
                "authored declaration selection occurrence {} resolved inconsistently across compiler-derived copies: {:?} selected {:?} and {:?}",
                candidate.occurrence.ordinal(),
                candidate.binding,
                existing.target,
                candidate.target
            )));
        }
        return Ok(());
    }
    resolutions.push(candidate);
    Ok(())
}

pub fn finalization_diagnostic(
    resolution: AuthoredSelectionResolution,
    error: AuthoredDeclarationSelectionFinalizationError,
) -> Diagnostic {
    Diagnostic::error(format!(
        "failed to finalize authored declaration selection occurrence {}: {error:?}",
        resolution.occurrence.ordinal()
    ))
}

/// `machine` is a specialization instance (template rewritten to a concrete
/// copy), not a canonical template — template-check walks skip it.
pub(crate) fn machine_is_template_instance(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> bool {
    program
        .machine_specializations
        .iter()
        .any(|specialization| {
            specialization.instance == machine.symbol
                && specialization.template != specialization.instance
        })
}

pub fn collect_checked_transition_target_selections(
    program: &dyn crate::CheckingInput,
    resolutions: &mut Vec<AuthoredSelectionResolution>,
) -> Result<(), Diagnostic> {
    for machine in program.machines() {
        if machine_is_template_instance(program, machine) {
            continue;
        }
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                for target in transition.targets() {
                    let typed_trees::statement::TransitionTargetNode::Named {
                        path,
                        source_span,
                        authored_call_selection: Some(occurrence),
                        ..
                    } = program.statement_table.transition_target(target)
                    else {
                        continue;
                    };
                    let Some(selection) =
                        program.authored_declaration_selections().get(*occurrence)
                    else {
                        return Err(Diagnostic::error(format!(
                            "transition target retains unknown authored selection occurrence {}",
                            occurrence.ordinal(),
                        ))
                        .with_source_span(*source_span));
                    };
                    if selection.kind() != AuthoredDeclarationSelectionKind::Call
                        || selection.source_span() != *source_span
                    {
                        return Err(Diagnostic::error(
                            "transition target retains mismatched authored call-selection evidence",
                        )
                        .with_source_span(*source_span));
                    }
                    if selection.target()
                        == AuthoredDeclarationSelectionTarget::LateBound(
                            AuthoredDeclarationSelectionLateBinding::CheckedCall,
                        )
                    {
                        let target_symbol = if path.symbol.is_valid() {
                            path.symbol
                        } else {
                            let Some(target_name) = program
                                .statement_table
                                .name_path_members(path.members)
                                .last()
                            else {
                                continue;
                            };
                            let matching = program
                                .machine_type_parameters(machine)
                                .iter()
                                .filter(|parameter| {
                                    parameter.name == *target_name
                                        && matches!(
                                            parameter.kind,
                                            typed_trees::data::TypeParameterKind::Machine { .. }
                                        )
                                })
                                .map(|parameter| parameter.symbol)
                                .collect::<Vec<_>>();
                            let [target] = matching.as_slice() else {
                                continue;
                            };
                            *target
                        };
                        if !target_symbol.is_valid() {
                            continue;
                        }
                        push_consistent_resolution(
                            resolutions,
                            AuthoredSelectionResolution {
                                occurrence: *occurrence,
                                binding: AuthoredDeclarationSelectionLateBinding::CheckedCall,
                                target: AuthoredSelectionResolutionTarget::Declaration(
                                    target_symbol,
                                ),
                            },
                        )?;
                    }
                }
            }
        }
    }
    Ok(())
}
/// The source-selection ledger names the template declaration, while typed
/// expressions keep the instantiated field and its substituted type. Rejoin
/// only through the exact instance's retained generic origin and member slot;
/// same-spelled fields belonging to other declarations remain distinct.
pub fn authored_data_member(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> SymbolHandle {
    let parent = program.symbols.get(symbol).parent;
    let owner = if program.symbols.get(parent).kind == symbols::SymbolKind::Variant {
        program.symbols.get(parent).parent
    } else {
        parent
    };
    let Some(instance) = program.data_definition_by_symbol(owner) else {
        return symbol;
    };
    let Some(origin) = instance.generic_instance else {
        return symbol;
    };
    let typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } =
        program.type_reference_table.type_reference(origin)
    else {
        return symbol;
    };
    let Some(template) = program.data_definition_by_symbol(*base_symbol) else {
        return symbol;
    };
    // The ledger reverses the same exact correspondence used by executable
    // projections. Case payloads are declaration children too; their immediate
    // parent is the variant, not the specialized data declaration.
    for member in program.data_members(template) {
        let fields = match member {
            typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
            typed_trees::data::DataMember::Variant(variant) => program.data_payload_fields(variant),
        };
        for field in fields {
            if program
                .closed_data_constructor_field(origin, template.symbol, field.symbol)
                .is_some_and(|selected| selected.symbol == symbol)
            {
                return field.symbol;
            }
        }
    }
    symbol
}

pub fn exact_named_operator_call<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<&'program typed_trees::operator::OperatorDefinition> {
    typed_trees::operator::resolve_named_expression_call(program, call).or_else(|| {
        let path = program.expression_table.name_path_node(call.receiver)?;
        let static_segments = program
            .expression_table
            .name_path_members(path.members)
            .iter()
            .map(|segment| segment.as_str())
            .collect::<Vec<_>>();
        (!static_segments.is_empty()).then_some(())?;
        typed_trees::operator::resolve_named_call(
            program,
            call.target_symbol,
            Some(&static_segments),
            call.target.as_str(),
            program
                .expression_table
                .expression_handles(call.arguments)
                .len(),
            false,
        )
    })
}

pub fn declaration_target(symbol: SymbolHandle) -> Option<AuthoredSelectionResolutionTarget> {
    symbol
        .is_valid()
        .then_some(AuthoredSelectionResolutionTarget::Declaration(symbol))
}
