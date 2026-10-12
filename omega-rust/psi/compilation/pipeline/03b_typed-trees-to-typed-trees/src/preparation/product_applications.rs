//! Relocate non-executing Product descriptions into one typed payload owner.
//!
//! The prepared Build graph and the final Product graph share declaration
//! symbols, but not growing expression/type arenas. Copy actual payloads through
//! their table owners before forming static application bindings. This is tuple
//! derivation only: source checking owes category, bound and correspondence
//! admission. Neither a selected requirement nor a machine actual is executed.

use language_semantics::declaration_selection::BuildOperation;
use std::sync::Arc;
use symbols::{BUILTIN_TYPE_COUNT, JoinedPath, SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionNode, StaticMachineArgument};
use typed_trees::references::StaticArgumentCall;
use typed_trees::static_applications::StaticMachineLifetimeBinding;
use typed_trees::typed_trees::product_applications::{
    CapturedStaticLexicalScopeSlot, ProductDeclarationOperand, ProductStaticArgumentSelection,
};
use typed_trees::types::TypeReferenceNode;
use validation::compiler_operations::{
    ProductDeclarationKind, build_declaration_operand_kind, build_operation_for_call,
    product_application_declaration, resolve_product_declaration, resolve_product_operand,
    resolve_product_requirement,
};

pub(crate) fn retain(program: &mut TypedTrees) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let mut descriptions = Vec::new();
    for (expression, node) in program.expression_table.iter_expressions() {
        if let ExpressionNode::Call(call) = node
            && let Some(operation) =
                build_operation_for_call(program, StaticArgumentCall::Expression(expression, call))
            && build_declaration_operand_kind(operation, 0).is_some()
        {
            descriptions.push((
                call.authored_machine_application,
                validation::compiler_operations::build_declaration_expression_source_span(
                    program, expression,
                ),
                operation,
            ));
        }
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(call) = statement.as_call()
                    && let Some(operation) =
                        build_operation_for_call(program, StaticArgumentCall::Statement(call))
                    && build_declaration_operand_kind(operation, 0).is_some()
                {
                    descriptions.push((
                        call.authored_machine_application,
                        call.source_span,
                        operation,
                    ));
                }
            }
        }
    }
    // Repreparation derives rows from its own live applications; a previous
    // private graph's payload handles cannot be reused by numeric coincidence.
    program.product_declaration_operands.reset_retain_capacity();
    if descriptions.is_empty() {
        return Ok(());
    }
    // Every captured lexical scope borrows this exact immutable Build graph.
    // Only relocated operand payloads live in the Product application owner.
    let lexical_owner_capture = Arc::new(typed_trees::FrozenTypedTrees::from(program.clone()));
    let mut owner = program.product_declarations().clone();
    owner.product_declaration_frontier = None;
    owner.product_declaration_operands.reset_retain_capacity();
    owner.captured_static_lexical_scopes.reset_retain_capacity();
    owner.captured_static_value_carriers.reset_retain_capacity();
    owner
        .product_static_argument_selections
        .reset_retain_capacity();
    // Symbols use the shared source prefix and appended lexical declarations.
    // Captured scopes retain original custody separately from executable roots.
    owner.symbols = program.symbols.clone();
    let mut rows = Vec::new();
    for (source_application, occurrence, operation) in descriptions {
        if !source_application.is_valid() {
            continue;
        }
        let source = program
            .expression_table
            .static_machine_application(source_application);
        let lexical_owner = source.lexical_owner;
        let captured_lexical_scope =
            retain_lexical_scope(&mut owner, &lexical_owner_capture, lexical_owner)
                .map_err(|diagnostic| vec![diagnostic])?;
        for (argument_index, argument) in program
            .expression_table
            .static_machine_application_arguments(source_application)
            .iter()
            .enumerate()
        {
            let Some(kind) = build_declaration_operand_kind(operation, argument_index) else {
                continue;
            };
            // Applied subjects and requirements form checked tuples in their
            // exact Product role; family descriptions remain declaration-only.
            let (declaration, selected_interface) =
                if matches!(kind, ProductDeclarationKind::BoundaryRequirement) {
                    let Some(selection) =
                        resolve_product_requirement(program, argument, occurrence)
                    else {
                        continue;
                    };
                    (selection.declaration, selection.selected_interface)
                } else if matches!(kind, ProductDeclarationKind::ProviderSubject)
                    && argument.application.is_some()
                {
                    let Some(declaration) =
                        resolve_product_declaration(program, argument, occurrence, kind)
                    else {
                        continue;
                    };
                    if program
                        .product_declarations()
                        .trait_definition_by_symbol(declaration)
                        .is_none_or(|definition| !definition.is_boundary)
                    {
                        continue;
                    }
                    (declaration, declaration)
                } else {
                    let Some(declaration) =
                        resolve_product_operand(program, argument, occurrence, kind)
                    else {
                        continue;
                    };
                    (declaration, SymbolHandle::invalid())
                };
            if operation == BuildOperation::ServiceExclusion
                && !program
                    .product_declarations()
                    .traits()
                    .iter()
                    .any(|definition| definition.symbol == declaration && definition.is_boundary)
            {
                continue;
            }
            let actuals = argument
                .application
                .as_ref()
                .map_or(&[][..], |application| application.arguments.as_ref());
            let mut actuals = actuals.to_vec();
            let mut selections = Vec::new();
            for actual in &mut actuals {
                relocate_argument(&mut owner, program, actual, occurrence, &mut selections);
            }
            let declaration_selections = owner
                .product_static_argument_selections
                .insert_many(selections);
            let application = if matches!(kind, ProductDeclarationKind::BoundaryRequirement)
                || selected_interface.is_valid()
            {
                let application = owner.expression_table.retain_static_machine_application(
                    declaration,
                    argument.path.last().cloned().unwrap_or_default(),
                    &actuals,
                );
                let retained = owner
                    .expression_table
                    .static_machine_application_mut(application);
                retained.binding_declaration = declaration;
                retained.selected_interface = selected_interface;
                retained.lexical_owner = lexical_owner;
                retained.captured_lexical_scope = captured_lexical_scope;
                let Some(schema) = product_application_declaration(
                    &owner,
                    owner
                        .expression_table
                        .static_machine_application(application),
                    kind,
                ) else {
                    continue;
                };
                let parameters = schema
                    .parameters
                    .iter()
                    .map(|parameter| (*parameter).clone())
                    .collect::<Vec<_>>();
                let bindings = validation::static_arguments::derive_explicit_static_bindings(
                    &owner,
                    &parameters,
                    schema.conformance_bounds,
                    &actuals,
                );
                let supplied_lifetimes = argument
                    .application
                    .as_ref()
                    .map_or(&[][..], |application| {
                        application.lifetime_arguments.as_ref()
                    });
                let lifetime_parameters = schema
                    .captured_lifetimes
                    .iter()
                    .chain(schema.invocation_lifetimes)
                    .cloned()
                    .collect::<Vec<_>>();
                let lifetimes = lifetime_parameters
                    .iter()
                    .zip(supplied_lifetimes)
                    .map(|(parameter, argument)| StaticMachineLifetimeBinding {
                        parameter: parameter.clone(),
                        argument: argument.clone().into(),
                    })
                    .collect::<Vec<_>>();
                if let Some(bindings) = bindings {
                    owner
                        .expression_table
                        .set_static_machine_application_bindings(application, &bindings);
                }
                owner
                    .expression_table
                    .set_static_machine_application_lifetimes(application, &lifetimes);
                application
            } else {
                // Selection custody alone cannot stand in for a checked tuple.
                typed_trees::static_applications::StaticMachineApplicationHandle::invalid()
            };
            let receiver_type = if matches!(kind, ProductDeclarationKind::ProviderSubject)
                && application.is_valid()
            {
                let mut receiver = argument.clone();
                receiver.symbol = declaration;
                // The outer Build handle is not a Product type coordinate.
                // Construct the receiver from the exact declaration and the
                // already relocated original tuple instead of borrowing it.
                receiver.type_reference = typed_trees::types::TypeReferenceHandle::invalid();
                let Some(receiver_application) = receiver.application.as_mut() else {
                    continue;
                };
                receiver_application.arguments = actuals.into_boxed_slice();
                crate::monomorphization::application_arguments::static_argument_type_reference(
                    &mut owner, &receiver,
                )
                .unwrap_or_default()
            } else {
                typed_trees::types::TypeReferenceHandle::invalid()
            };
            rows.push((
                source_application,
                argument_index,
                occurrence,
                declaration,
                kind,
                application,
                receiver_type,
                declaration_selections,
            ));
        }
    }
    let owner = Arc::new(typed_trees::FrozenTypedTrees::from(owner));
    for (
        source_application,
        argument_index,
        source_span,
        declaration,
        kind,
        application,
        receiver_type,
        declaration_selections,
    ) in rows
    {
        program.product_declaration_operands.insert(
            ProductDeclarationOperand {
                source_application,
                argument_index,
                source_span,
                declaration,
                kind,
                owner: owner.clone(),
                application,
                receiver_type,
                declaration_selections,
            }
            .into(),
        );
    }
    Ok(())
}

fn retain_lexical_scope(
    owner: &mut TypedTrees,
    source: &Arc<typed_trees::FrozenTypedTrees>,
    symbol: SymbolHandle,
) -> Result<arena::Handle<CapturedStaticLexicalScopeSlot>, diagnostics::Diagnostic> {
    // Even a same-numbered Product machine is a different lexical owner if
    // the Build graph appended this declaration after the source frontier.
    // All references from this one capture reuse its original owner identity.
    let captured = owner.capture_static_lexical_scope(source.clone(), symbol);
    if captured.is_valid() {
        let scope = owner
            .captured_static_lexical_scope(captured)
            .expect("new capture retains its original scope")
            .clone();
        crate::monomorphization::captured_lexical_scopes::retain(owner, &scope)
    } else {
        Err(diagnostics::Diagnostic::error(
            "Product application lost its original lexical declaration",
        ))
    }
}

fn relocate_argument(
    owner: &mut TypedTrees,
    source: &TypedTrees,
    argument: &mut StaticMachineArgument,
    occurrence: source::SourceSpan,
    selections: &mut Vec<ProductStaticArgumentSelection>,
) {
    if !argument.path.is_empty()
        && argument.const_literal.is_none()
        && argument.evidence_projection.is_none()
    {
        let candidates = owner
            .data_definitions()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public))
            .chain(
                owner
                    .conformances()
                    .iter()
                    .map(|definition| (definition.symbol, definition.is_public)),
            )
            .chain(
                owner
                    .const_declarations()
                    .iter()
                    .map(|definition| (definition.symbol, definition.is_public)),
            )
            .chain(
                owner
                    .machines()
                    .iter()
                    .map(|definition| (definition.symbol, definition.is_public)),
            )
            .chain(
                owner
                    .symbols
                    .child_handles(owner.symbols.root())
                    .into_iter()
                    .flatten()
                    .take(BUILTIN_TYPE_COUNT)
                    .filter(|symbol| owner.symbols.builtin_type_atom(*symbol).is_some())
                    .map(|symbol| (symbol, true)),
            );
        let symbol = owner.symbols.find_build_subject_declaration_from_source(
            &argument.path.joined_path(),
            occurrence,
            candidates,
            &[],
        );
        argument.symbol = symbol.unwrap_or_default();
    }
    if argument.symbol.is_valid()
        && argument.const_literal.is_none()
        && argument.evidence_projection.is_none()
        && let Some(position) = argument.authored_position
    {
        selections.push(ProductStaticArgumentSelection {
            descendant_ordinal: position.descendant_ordinal,
            declaration: argument.symbol,
        });
    }
    if let Some(application) = &mut argument.application {
        for actual in application.arguments.iter_mut() {
            relocate_argument(owner, source, actual, occurrence, selections);
        }
    }
    // Original declaration custody and every descendant coordinate have now
    // been retained. A payload early return cannot erase a nominal occurrence
    // or shift the next sibling's original preorder coordinate.
    if argument.type_reference.is_valid() {
        let tables = &mut owner.tables;
        argument.type_reference = tables.type_reference_table.copy_from(
            &source.type_reference_table,
            &source.expression_table,
            &mut tables.expression_table,
            argument.type_reference,
        );
        if !matches!(
            owner
                .type_reference_table
                .type_reference(argument.type_reference),
            TypeReferenceNode::Named { .. }
        ) {
            return;
        }
        let Some(actual) =
            crate::monomorphization::application_arguments::static_argument_from_type_reference(
                owner,
                argument.type_reference,
            )
        else {
            return;
        };
        // A retained type/index graph can contain a closed const value. Rejoin
        // its original proposal before ordinary tuple formation; treating every
        // copied handle as a type skips const interning and binder admission.
        // Named declarations retain their symbol and declared carrier. Genuine
        // type operands stay in their copied graph, including structural types.
        if validation::static_arguments::spelling(owner, &actual).is_some()
            || (actual.symbol.is_valid()
                && owner.const_declaration_by_symbol(actual.symbol).is_some())
        {
            let authored_position = argument.authored_position;
            *argument = actual;
            argument.authored_position = authored_position;
        } else {
            return;
        }
    }
    // Convert a type application to typed payload custody. A conformance keeps
    // its named proposal for the ordinary conformance closure owner to admit.
    if argument.symbol.is_valid()
        && matches!(
            owner.symbols.get(argument.symbol).kind,
            SymbolKind::Data | SymbolKind::BuiltinType
        )
    {
        let actual = if let Some(application) = &argument.application {
            let actuals = application
                .arguments
                .iter()
                .map(|actual| {
                    crate::monomorphization::application_arguments::static_argument_type_reference(
                        owner, actual,
                    )
                })
                .collect::<Option<Vec<_>>>();
            actuals.map(|actuals| {
                let arguments = owner
                    .type_reference_table
                    .insert_type_reference_handles(actuals);
                owner
                    .type_reference_table
                    .insert(TypeReferenceNode::Generic {
                        base_symbol: argument.symbol,
                        base_name: argument.path.last().cloned().unwrap_or_default(),
                        lifetime_arguments: application.lifetime_arguments.to_vec(),
                        arguments,
                    })
            })
        } else {
            crate::monomorphization::application_arguments::static_argument_type_reference(
                owner, argument,
            )
        };
        if let Some(actual) = actual {
            argument.type_reference = actual;
            argument.path = Box::new([]);
            argument.symbol = SymbolHandle::invalid();
            argument.application = None;
        }
    } else if let Some(spelling) = validation::static_arguments::spelling(owner, argument) {
        let meaning = owner.static_const_integer_meaning(argument);
        crate::monomorphization::application_arguments::intern_closed_const_with_meaning(
            owner, &spelling, meaning,
        );
    }
}
