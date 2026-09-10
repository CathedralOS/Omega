//! Typed evaluation of closed integer expressions and Boolean indices.
//!
//! The probe owns no published layout or symbols. Only its canonical result and
//! exact authored selection custody return to the original syntax forest.
//! Named operands need this probe to preserve selected carriers and declarations.
//! Boolean destinations also need it without names: the syntax-only arithmetic
//! normalizer cannot produce Boolean identity or establish operator meaning.
//! Destination spelling only routes the probe; exact typed identity, admission
//! and operand checks still precede evaluation and publication of the result.
//! Every machine argument first needs its original lexical selection, including
//! structural destinations such as fixed arrays. A destination without a name
//! cannot bypass that obligation and let later canonicalization erase a runtime
//! operand. Nonscalar values retain their existing materialization path.

mod lexical_selection;
mod value;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_semantics::const_value::DecodedCanonicalConstValue;
use source::{SourceMap, SourceSpan};
use symbols::SourceScopedTopLevelBinding;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Item, Machine, State};
use syntax_trees::statement::{StatementNode, TableTransition, TransitionTargetNode};
use syntax_trees::types::{ConstArgumentOrigin, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn evaluate(
    mut syntax: SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
    bindings: &[SourceScopedTopLevelBinding],
    authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let mut arguments =
        syntax_trees_to_symbol_resolved_trees::closed_data_const_argument_expressions(&syntax);
    let machine_arguments =
        syntax_trees_to_symbol_resolved_trees::closed_machine_const_arguments(&syntax);
    let mut lexical_arguments = Vec::new();
    let mut aggregate_arguments = Vec::new();
    for (argument, destination, public) in machine_arguments {
        // This is probe routing, not builtin identity. The typed destination
        // must still resolve to the exact primitive before evaluation.
        let scalar = matches!(
            syntax.type_references.type_reference(destination),
            TypeReferenceNode::Named(destination_name)
                if matches!(destination_name.as_str(),
                    "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "bool")
        );
        let original = syntax.type_references.type_reference(argument).clone();
        if let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(argument) {
            let name = name.clone();
            let reference = name.source_span();
            let mut members = HandleSpan::empty();
            // Named type arguments retain one complete authored path span.
            // Restore its lexical segments without inventing member offsets
            // across source trivia; selection custody belongs to the whole use.
            for member in name.as_str().split("::") {
                syntax.expressions.append_identifier_path_member_to_span(
                    &mut members,
                    Identifier::new(member, reference),
                );
            }
            let expression = syntax.expressions.insert(ExpressionNode::Name(members));
            syntax.expressions.set_source_span(expression, reference);
            syntax
                .type_references
                .replace_type_reference(argument, TypeReferenceNode::ConstExpression(expression));
        }
        if !scalar {
            aggregate_arguments.push((argument, original));
            continue;
        }
        arguments.push((argument, destination, public));
        lexical_arguments.push(argument);
    }
    if !aggregate_arguments.is_empty() {
        // Aggregate substitution still uses the existing canonical-value route.
        // Resolve its authored path first so that route cannot capture a static
        // declaration through a runtime parameter or prior local binding.
        let resolved =
            syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_for_const_argument_selection(
                &syntax,
                sources.clone(),
                bindings.to_vec(),
            )?;
        for (argument, original) in aggregate_arguments {
            if let TypeReferenceNode::ConstExpression(expression) =
                syntax.type_references.type_reference(argument)
            {
                lexical_selection::retain(&syntax, &resolved, *expression).map_err(|reason| {
                    vec![
                        Diagnostic::error(format!("const argument expression: {reason}"))
                            .with_source_span(syntax.expressions.source_span(*expression)),
                    ]
                })?;
            }
            syntax
                .type_references
                .replace_type_reference(argument, original);
        }
    }
    let pending = arguments
        .iter()
        .filter_map(|(argument, destination, public)| {
            let TypeReferenceNode::ConstExpression(expression) =
                syntax.type_references.type_reference(*argument)
            else {
                return None;
            };
            let boolean_destination = matches!(
                syntax.type_references.type_reference(*destination),
                TypeReferenceNode::Named(name) if name.as_str() == "bool"
            );
            (boolean_destination || requires_typed_expression_probe(&syntax, *expression))
                .then_some((*argument, *expression, *destination, *public))
        })
        .collect::<Vec<_>>();
    if pending.is_empty() {
        return Ok(syntax);
    }

    let mut lexical_origins = Vec::new();
    if pending
        .iter()
        .any(|(argument, _, _, _)| lexical_arguments.contains(argument))
    {
        // Resolve the original machine owners before placeholder synthesis.
        // Their runtime bodies are neither typed as probes nor executed here.
        let resolved =
            syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_for_const_argument_selection(
                &syntax,
                sources.clone(),
                bindings.to_vec(),
            )?;
        for (argument, expression, _, _) in &pending {
            if lexical_arguments.contains(argument) {
                let origins = lexical_selection::retain(&syntax, &resolved, *expression).map_err(
                    |reason| {
                        vec![
                            Diagnostic::error(format!("const argument expression: {reason}"))
                                .with_source_span(syntax.expressions.source_span(*expression)),
                        ]
                    },
                )?;
                lexical_origins.push((*argument, origins));
            }
        }
    }

    // As in const-generic call evaluation, temporary arguments let the ordinary
    // frontend type the selected expression before any real instance is created.
    // Original expressions and their authored names remain unchanged in the probe.
    let mut probe = syntax.clone();
    for (argument, destination, _) in &arguments {
        let placeholder = match syntax.type_references.type_reference(*destination) {
            TypeReferenceNode::Named(name) if name.as_str() == "bool" => "false",
            _ => "0",
        };
        probe.type_references.replace_type_reference(
            *argument,
            TypeReferenceNode::Named(Identifier::generated(placeholder)),
        );
    }
    for (ordinal, (_, expression, destination, _)) in pending.iter().enumerate() {
        append_probe(&mut probe, ordinal, *expression, *destination);
    }
    let probe =
        crate::normalize_generic_data_with_optional_sources(probe, sources.clone(), bindings)?;
    let resolved = crate::lower_probe_with_optional_sources(&probe, sources, bindings)?;
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])?;

    for (argument, original, _, public) in pending {
        let reference = syntax.expressions.source_span(original);
        let failure = |reason| {
            vec![
                Diagnostic::error(format!("const argument expression: {reason}"))
                    .with_source_span(reference),
            ]
        };
        let mut machines = typed
            .machines()
            .iter()
            .filter(|machine| typed.symbols.symbol_source_span(machine.symbol) == Some(reference));
        let machine = machines
            .next()
            .ok_or_else(|| failure("typed probe lost its source custody".to_owned()))?;
        if machines.next().is_some() {
            return Err(failure(
                "typed probe source custody is ambiguous".to_owned(),
            ));
        }
        let [state] = typed.machine_states(machine) else {
            return Err(failure(
                "typed probe lost its single expression state".to_owned(),
            ));
        };
        let [typed_trees::statement::StatementNode::Transition(transition)] =
            typed.statement_table.statements(state.statement_nodes)
        else {
            return Err(failure("typed probe lost its expression return".to_owned()));
        };
        let typed_trees::statement::TransitionTargetNode::Value(expression) =
            typed.statement_table.transition_target(transition.target)
        else {
            return Err(failure("typed probe return is not a value".to_owned()));
        };
        let destination =
            exact_probe_destination(&typed, state.return_type).ok_or_else(|| {
                failure(
                    "index destination requires an unconstrained exact builtin integer or Boolean carrier"
                        .to_owned(),
                )
            })?;
        crate::admission::require_const_expression_selection(&typed, machine, reference, authority)
            .map_err(&failure)?;
        let (origins, operators) =
            expression_custody(&typed, machine, state, *expression, public).map_err(&failure)?;
        if let Some((_, expected)) = lexical_origins
            .iter()
            .find(|(original, _)| *original == argument)
            && (&origins != expected)
        {
            return Err(failure(
                "standalone probe changed the original machine's constant selection".to_owned(),
            ));
        }
        let (result, warnings) =
            value::evaluate(&typed, machine, state, *expression, destination).map_err(&failure)?;
        if result.type_name != destination.name() {
            return Err(failure(format!(
                "landed `{}` result cannot initialize `{}`",
                result.type_name,
                destination.name()
            )));
        }
        let replacement = match result.decode_encoding() {
            Some(DecodedCanonicalConstValue::Integer { value, .. }) => value.to_string(),
            Some(DecodedCanonicalConstValue::Boolean(_)) => result.atom(),
            _ => {
                return Err(failure(
                    "index probe returned an unsupported canonical value".to_owned(),
                ));
            }
        };
        for warning in warnings {
            eprintln!("{warning}");
        }
        syntax.type_references.retain_const_argument_normalization(
            argument,
            reference,
            result.encoding,
            origins,
            operators,
        );
        syntax.type_references.replace_type_reference(
            argument,
            TypeReferenceNode::Named(Identifier::generated(replacement)),
        );
    }
    Ok(syntax)
}

fn requires_typed_expression_probe(syntax: &SyntaxTrees, expression: ExpressionHandle) -> bool {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !expression.is_valid() || visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match syntax.expressions.expression(expression) {
            ExpressionNode::Match(_) => return true,
            ExpressionNode::Name(_) => return true,
            ExpressionNode::Binary(binary) => {
                pending.push(binary.right);
                pending.push(binary.left);
            }
            _ => {}
        }
    }
    false
}

fn exact_probe_destination(
    program: &typed_trees::TypedTrees,
    destination: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::types::PrimitiveType> {
    use symbols::BuiltinTypeAtom;
    use typed_trees::types::{PrimitiveType, TypeReferenceNode};
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(destination)
    else {
        return None;
    };
    Some(match program.symbols.builtin_type_atom(*symbol)? {
        BuiltinTypeAtom::I8 => PrimitiveType::I8,
        BuiltinTypeAtom::I16 => PrimitiveType::I16,
        BuiltinTypeAtom::I32 => PrimitiveType::I32,
        BuiltinTypeAtom::I64 => PrimitiveType::I64,
        BuiltinTypeAtom::U8 => PrimitiveType::U8,
        BuiltinTypeAtom::U16 => PrimitiveType::U16,
        BuiltinTypeAtom::U32 => PrimitiveType::U32,
        BuiltinTypeAtom::U64 => PrimitiveType::U64,
        BuiltinTypeAtom::Bool => PrimitiveType::Bool,
        _ => return None,
    })
}

fn append_probe(
    syntax: &mut SyntaxTrees,
    ordinal: usize,
    expression: ExpressionHandle,
    destination: TypeReferenceHandle,
) {
    let reference = syntax.expressions.source_span(expression);
    // This private name cannot be authored by the lexer. Its source coordinate
    // locates the owning package/module, not a manufactured declaration token.
    let name = Identifier::new(format!("@const-argument-{ordinal}"), reference);
    let target = syntax
        .statements
        .insert_transition_target(TransitionTargetNode::Value(expression));
    let statement = syntax
        .statements
        .insert(StatementNode::Transition(TableTransition {
            target,
            source_span: reference,
            ..Default::default()
        }));
    let statement = syntax.items.append_statement_handle(statement);
    let state = syntax.items.insert_state(&State {
        name: name.clone(),
        parameters: HandleSpan::empty(),
        return_type: destination,
        contracts: HandleSpan::empty(),
        statements: HandleSpan::from_parts(statement, 1),
    });
    let state = syntax.items.append_state_handle(state);
    syntax.push_root_item(Item::Machine(Machine {
        name,
        generic_data_template: Default::default(),
        attached_data: None,
        is_public: false,
        bodyless: false,
        target: None,
        boundary: false,
        is_top_level_boundary_requirement: false,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        satisfies: HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        terminates_guarantee: false,
        ranking_subjects: HandleSpan::empty(),
        ranking_view: HandleSpan::empty(),
        ranking_view_arguments: HandleSpan::empty(),
        ranking_range: ExpressionHandle::invalid(),
        service_reach_keyword_source_spans: Vec::new(),
        service_reaches: HandleSpan::empty(),
        service_reach_is_installation_bound: false,
        invokes: HandleSpan::empty(),
        suspends_keyword_source_spans: Vec::new(),
        blocks_keyword_source_spans: Vec::new(),
        suspends: false,
        blocks: false,
        contracts: HandleSpan::empty(),
        states: HandleSpan::from_parts(state, 1),
    }));
}

fn expression_custody(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    root: typed_trees::expression::ExpressionHandle,
    public: bool,
) -> Result<(Vec<ConstArgumentOrigin>, Vec<SourceSpan>), String> {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
    };
    use typed_trees::expression::ExpressionNode;
    let mut origins = Vec::new();
    let mut operators = Vec::new();
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        for occurrence in program
            .expression_table
            .authored_selection_occurrences(expression)
        {
            let selection = program
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or("evaluated expression lost an authored selection")?;
            if selection.kind() == Kind::Operator {
                let builtin = matches!(
                    selection.target(),
                    Target::Intrinsic(Intrinsic::BuiltinOperator)
                ) || (matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::Binary(_)
                ) && validation::has_builtin_binary_expression_meaning(
                    program,
                    machine,
                    Some(state),
                    expression,
                ));
                if !builtin {
                    return Err("evaluated operator has no checked builtin meaning".to_owned());
                }
                if !operators.contains(&selection.source_span()) {
                    operators.push(selection.source_span());
                }
                continue;
            }
            let Target::Resolved(selected) = selection.target() else {
                return Err(
                    "evaluated constant reference has no exact declaration selection".to_owned(),
                );
            };
            let declaration = program
                .const_declarations()
                .iter()
                .find(|declaration| declaration.symbol == selected.selected_symbol())
                .ok_or("evaluated reference did not select a constant declaration")?;
            let declaration_source = program
                .symbols
                .symbol_source_span(declaration.symbol)
                .ok_or("selected constant lost its declaration source")?;
            if !declaration.is_public
                && (public
                    || !program
                        .symbols
                        .same_source_package(selection.source_span(), declaration_source))
            {
                return Err(
                    "private constant declaration cannot be selected by this index owner"
                        .to_owned(),
                );
            }
            let origin = ConstArgumentOrigin {
                reference: selection.source_span(),
                declaration: declaration_source,
                initializer: declaration.initializer_source_span,
                canonical_value_encoding: declaration
                    .canonical_value_encoding
                    .clone()
                    .ok_or("selected constant has no canonical value")?,
            };
            if !origins.contains(&origin) {
                origins.push(origin);
            }
        }
        if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) {
            pending.push(binary.right);
            pending.push(binary.left);
        } else if let ExpressionNode::Match(dispatch) =
            program.expression_table.expression(expression)
        {
            for arm in program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .rev()
            {
                pending.push(arm.value);
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    pending.push(pattern);
                }
            }
            pending.push(dispatch.subject);
        }
    }
    Ok((origins, operators))
}
