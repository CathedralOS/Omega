//! Typed authority for designated product operands of Build provider selection.

use super::intrinsic_calls::exact_build_prelude_data;
use language_semantics::declaration_selection::BuildOperation;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};

pub(crate) fn provider_selection_expressions(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<ExpressionHandle> {
    let mut expressions = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            crate::monomorphization::collect_statement_expression_trees(
                program,
                statement,
                &mut expressions,
            );
        }
    }
    expressions.retain(|expression| {
        matches!(program.expression_table.expression(*expression), ExpressionNode::Call(call)
            if BuildOperation::from_call_target(call.target.as_str())
                == Some(BuildOperation::ProviderSelection)
                && !call.target_symbol.is_valid())
    });
    expressions
}

pub(crate) fn is_build_provider_selection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    BuildOperation::from_call_target(call.target.as_str())
        == Some(BuildOperation::ProviderSelection)
        && !call.target_symbol.is_valid()
        && exact_mutable_build_receiver(program, call.receiver)
}

pub(crate) fn exact_mutable_build_receiver(
    program: &TypedTrees,
    receiver: ExpressionHandle,
) -> bool {
    if !receiver.is_valid() {
        return false;
    }
    if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(receiver) {
        return borrow.access == language_semantics::ReferenceAccess::Mutable
            && crate::flow::expression_type_symbol(program, borrow.target)
                .is_some_and(|symbol| exact_build_prelude_data(program, symbol, "Build"));
    }
    crate::flow::expression_place_type_reference(program, receiver, &[]).is_some_and(|reference| {
        matches!(program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::Reference { access, referee, .. }
                if *access == language_semantics::ReferenceAccess::Mutable
                    && exact_build_prelude_data(program, program.type_reference_table.type_symbol(*referee), "Build"))
    })
}

pub(crate) fn exact_mutable_build_statement_receiver(
    program: &TypedTrees,
    call: &typed_trees::statement::TableCall,
) -> bool {
    super::operator_targets::type_reference_for_symbol(program, call.receiver_symbol)
        .is_some_and(|reference| {
            matches!(program.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::Reference { access, referee, .. }
                    if *access == language_semantics::ReferenceAccess::Mutable
                        && exact_build_prelude_data(program, program.type_reference_table.type_symbol(*referee), "Build"))
        })
}

/// Resolve an already classified product operand, never an ordinary static use.
/// The collector of an executed selection independently rejoins the same path.
pub(crate) fn resolve_product_operand(
    program: &TypedTrees,
    argument: &StaticMachineArgument,
    occurrence: source::SourceSpan,
    subject: bool,
) -> Option<SymbolHandle> {
    if argument.type_reference.is_valid()
        || argument.application.is_some()
        || argument.const_literal.is_some()
        || argument.evidence_projection.is_some()
        || argument.path.is_empty()
    {
        return None;
    }
    let path = argument
        .path
        .iter()
        .map(|name| name.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let candidates = if subject {
        program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .map(|definition| (definition.symbol, definition.is_public))
            .chain(
                program
                    .machines()
                    .iter()
                    .filter(|machine| {
                        machine.supply_mode
                            == language_semantics::MachineSupplyMode::TopLevelRequirement
                    })
                    .map(|machine| (machine.symbol, machine.is_public)),
            )
            .chain(
                program
                    .operators()
                    .iter()
                    .chain(
                        program
                            .domain_definitions()
                            .iter()
                            .flat_map(|domain| program.domain_operators(domain)),
                    )
                    .filter(|operator| operator.is_boundary)
                    .map(|operator| (operator.symbol, operator.is_public)),
            )
            .collect::<Vec<_>>()
    } else {
        program
            .data_definitions()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public))
            .collect()
    };
    program
        .symbols
        .find_product_declaration_from_source(&path, occurrence, candidates)
}

#[cfg(test)]
mod tests {
    use super::is_build_provider_selection;
    use typed_trees::TypedTrees;
    use typed_trees::expression::ExpressionNode;

    fn fixture(text: &str, toolchain: bool) -> TypedTrees {
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                std::path::PathBuf::from("<build-prelude>"),
                text.to_owned(),
                std::path::PathBuf::from("."),
                None,
                if toolchain {
                    source::SourceOrigin::Toolchain
                } else {
                    source::SourceOrigin::User
                },
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text)
            .tokenize()
            .expect("tokenize");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).expect("parse");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                syntax: &syntax,
                sources: Some(std::sync::Arc::new(sources)),
                top_level_bindings: Vec::new(),
            },
        )
        .expect("resolve");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
    }

    #[test]
    fn product_selection_requires_exact_mutable_toolchain_build() {
        for (access, toolchain, expected) in [
            ("&mut", true, true),
            ("&", true, false),
            ("&mut", false, false),
        ] {
            let program = fixture(
                &format!(
                    "data Build {{}} boundary trait Reader {{ machine read(); }} data Provider {{}} machine build(builder: {access} Build) {{ builder.select_provider<Reader, Provider>(); }}"
                ),
                toolchain,
            );
            let (expression, _) = program.expression_table.iter_expressions().find(|(_, node)| {
                matches!(node, ExpressionNode::Call(call) if call.target.as_str() == "select_provider")
            }).expect("selection call");
            assert_eq!(is_build_provider_selection(&program, expression), expected);
        }
    }

    #[test]
    fn admitted_provider_call_has_unit_statement_semantics_but_not_value_semantics() {
        for (body, accepted) in [
            ("builder.select_provider<Reader, Provider>();", true),
            (
                "consume(builder.select_provider<Reader, Provider>());",
                false,
            ),
        ] {
            let mut program = fixture(
                &format!(
                    "data Build {{}} boundary trait Reader {{ machine read(); }} data Provider {{}} machine consume(value: i32) {{}} machine build(builder: &mut Build) {{ {body} }}"
                ),
                true,
            );
            crate::authored_selections::bind_pre_specialization_authored_selections(&mut program)
                .expect("exact Build operation binds before source validation");
            let result = validation::validate_program(&program);
            if accepted {
                result.expect("admitted Unit operation is a valid standalone statement");
            } else {
                let diagnostics = result.expect_err("Unit cannot supply an ordinary value operand");
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.to_string().contains("Unit cannot supply"))
                );
            }
        }
    }

    #[test]
    fn statement_spelled_provider_operands_retain_finalizable_occurrences() {
        let mut program = fixture(
            "data Build {} boundary trait Reader { machine read(); } data Provider {} machine build(builder: &mut Build) { builder.select_provider<Reader, Provider>(); }",
            true,
        );
        crate::authored_selections::finalize_checked_authored_selections(
            &mut program,
            &checked_trees::CheckFacts::default(),
        )
        .expect("call and both static operands must retain finalizable custody");
        let operands = program
            .authored_declaration_selections()
            .iter()
            .filter(|selection| {
                selection.kind() == typed_trees::AuthoredDeclarationSelectionKind::StaticArgument
            })
            .collect::<Vec<_>>();
        assert_eq!(operands.len(), 2);
        assert!(operands.iter().all(|selection| matches!(
            selection.target(),
            typed_trees::AuthoredDeclarationSelectionTarget::Resolved(_)
        )));
    }

    #[test]
    fn resolved_same_named_method_is_not_a_product_selection() {
        let program = fixture(
            "data Build {} boundary trait Reader { machine read(); } data Provider {} machine Build::select_provider(&mut self) {} machine build(builder: &mut Build) { builder.select_provider<Reader, Provider>(); }",
            true,
        );
        let (expression, _) = program.expression_table.iter_expressions().find(|(_, node)| {
            matches!(node, ExpressionNode::Call(call) if call.target.as_str() == "select_provider")
        }).expect("ordinary call");
        assert!(!is_build_provider_selection(&program, expression));
    }
}
