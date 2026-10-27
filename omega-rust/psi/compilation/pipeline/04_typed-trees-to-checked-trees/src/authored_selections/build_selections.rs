//! Source authority for designated product operands, independent of execution.
//!
//! The host preparation only observes identity and therefore borrows shared.
//! The authored bind still requires mutable Build authority; this check retains
//! that source obligation before the ordinary call weakens its implementation
//! access. An integer ticket or a declared description type never proves issuance.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{BuildSelectionRequest, RootBindingOperand};
use typed_trees::expression::ExpressionNode;

pub(super) fn validate(
    program: &dyn validation::CheckingInput,
    owners: &validation::expression_owners::OwnerEnvironmentIndex,
) -> Result<(), Diagnostic> {
    let expressions = &program.expression_table;
    let operations: Vec<_> = program
        .traits()
        .iter()
        .filter(|definition| {
            definition.is_boundary && definition.name.as_str() == "BuildBindingOperations"
        })
        .filter(|definition| {
            program
                .symbols
                .symbol_source_span(definition.symbol)
                .and_then(|span| program.symbols.source_file(span))
                .is_some_and(|file| {
                    file.origin == source::SourceOrigin::Toolchain
                        && file.path == std::path::Path::new("<build-prelude>")
                })
        })
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .map(|signature| (signature.symbol, signature.name.as_str()))
        .collect();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let typed_trees::statement::StatementNode::Call(call) = statement
                    && operations
                        .iter()
                        .any(|(symbol, _)| *symbol == call.target_symbol)
                {
                    return Err(Diagnostic::error(
                        "Build binding protocol requires source-issued designated product operands",
                    ));
                }
            }
        }
    }
    for (expression, node) in expressions.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        let operation = operations
            .iter()
            .find(|(symbol, _)| *symbol == call.target_symbol)
            .map(|(_, name)| *name);
        if !call.build_selection.is_valid() {
            if operation.is_some() {
                return Err(Diagnostic::error(
                    "Build binding protocol requires source-issued designated product operands",
                ));
            }
            continue;
        }
        let request = expressions
            .build_selection_request(call.build_selection)
            .ok_or_else(|| Diagnostic::error("Build selection lost its designated operands"))?;
        let span = request.source_span();
        let operation = operation.ok_or_else(|| {
            Diagnostic::error("Build selection lost its exact toolchain boundary requirement")
                .with_source_span(span)
        })?;
        let arguments = expressions.expression_handles(call.arguments);
        let expected = match request {
            BuildSelectionRequest::Exclusion(_) => match operation {
                "prepare" => 1,
                "exclude" => 2,
                _ => {
                    return Err(Diagnostic::error(
                        "behavior exclusion lost its checked value protocol",
                    )
                    .with_source_span(span));
                }
            },
            BuildSelectionRequest::Entry(binding) => {
                if binding.slot.is_empty()
                    || matches!(&binding.implementation, RootBindingOperand::AuthoredPath(path) if path.is_empty())
                {
                    return Err(Diagnostic::error("root-slot binding requires exactly one slot path and one implementation path or description expression").with_source_span(span));
                }
                match (&binding.implementation, operation) {
                    (_, "prepare") | (RootBindingOperand::AuthoredPath(_), "bind") => 1,
                    (RootBindingOperand::Description, "bind_description") => 2,
                    _ => {
                        return Err(Diagnostic::error(
                            "root binding operation disagrees with its designated product operands",
                        )
                        .with_source_span(span));
                    }
                }
            }
            BuildSelectionRequest::Declaration(binding) => {
                use language_semantics::declaration_selection::BuildOperation;
                if !matches!(
                    binding.operation,
                    BuildOperation::ProviderSelection
                        | BuildOperation::RepresentationSelection
                        | BuildOperation::ServiceExclusion
                        | BuildOperation::BoundaryAcceptance
                ) || binding.operands.len()
                    != if matches!(
                        binding.operation,
                        BuildOperation::ServiceExclusion | BuildOperation::BoundaryAcceptance
                    ) {
                        1
                    } else {
                        2
                    }
                    || binding.operands.iter().any(|operand| {
                        operand.path.is_empty()
                            || operand.source_span.span.start >= operand.source_span.span.end
                    })
                {
                    return Err(Diagnostic::error(
                        "Product selection lost its admitted declaration operand shape",
                    )
                    .with_source_span(span));
                }
                match operation {
                    "prepare" | "select" => 1,
                    "select_mode" if binding.operation == BuildOperation::ProviderSelection => 2,
                    _ => {
                        return Err(Diagnostic::error(
                            "Product selection commit disagrees with its declaration operation",
                        )
                        .with_source_span(span));
                    }
                }
            }
        };
        if arguments.len() != expected {
            return Err(
                Diagnostic::error("root binding operation lost its ordinary arguments")
                    .with_source_span(span),
            );
        }
        for (machine, state, statement) in owners.containing_statement_sites(expression) {
            let machine = &program.machines()[machine];
            let state = &program.machine_states(machine)[state];
            let argument_type = |argument| {
                validation::expression_type_reference_in_state(
                    program,
                    state.symbol,
                    statement,
                    argument,
                )
            };
            if operation == "prepare" {
                let mut receiver = arguments[0];
                let mut explicit_mutable_borrow = false;
                while let ExpressionNode::Borrow(borrow) = expressions.expression(receiver) {
                    if borrow.access != language_semantics::ReferenceAccess::Mutable {
                        return Err(Diagnostic::error(
                            "root binding requires a compiler-issued &mut Build place",
                        )
                        .with_source_span(span));
                    }
                    explicit_mutable_borrow = true;
                    receiver = borrow.target;
                }
                let has_authority = argument_type(receiver).is_some_and(|reference| {
                    let referee = match program.type_reference_table.type_reference(reference) {
                        typed_trees::types::TypeReferenceNode::Reference {
                            access,
                            referee,
                            ..
                        } if *access == language_semantics::ReferenceAccess::Mutable => *referee,
                        _ if explicit_mutable_borrow => reference,
                        _ => return false,
                    };
                    validation::compiler_operations::exact_build_prelude_data(
                        program,
                        program.type_reference_table.type_symbol(referee),
                        "Build",
                    )
                });
                if !has_authority {
                    return Err(Diagnostic::error(
                        "root binding requires a compiler-issued &mut Build receiver",
                    )
                    .with_source_span(span));
                }
            } else if operation == "bind_description"
                && !argument_type(arguments[1]).is_some_and(|reference| {
                    validation::compiler_operations::type_reference_names_exact_prelude_data(
                        program,
                        reference,
                        "ProductEntryRef",
                    )
                })
            {
                return Err(Diagnostic::error("delegated root binding requires an operand of the compiler-owned ProductEntryRef description type").with_source_span(span));
            }
        }
    }
    Ok(())
}
