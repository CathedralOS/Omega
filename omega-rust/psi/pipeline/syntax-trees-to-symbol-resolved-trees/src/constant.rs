//! Constant substitution; value contract: wiki/spec/language/constants.md.
//!
//! Const VALUE semantics exist only until symbol resolution:
//! A constant declares a named pure value; each selected expression path gets
//! a fresh copy of its initializer before resolution publishes its result.
//! Symbol-resolved trees, typed trees,
//! validation, proofs, backends, and the interpreter never grow a const-value
//! concept -- each use IS the literal, which is exactly the copied-at-each-use
//! semantics the contract specifies (and why interior mutability can never hide in
//! one). The symbol table retains only declaration provenance so authored-
//! selection and package-authority checks cannot be erased by substitution.
//!
//! In module-constant source closures, scalar values substitute only after the
//! shared resolver has selected their namespace and lexical binding. Legacy
//! no-module lowering retains its conservative free-constant shadowing walk.
//!
//! Remaining boundaries, enforced loudly:
//! - LITERAL-ONLY initializers (scalars, negated scalars -- already folded by
//!   the parser -- payloadless cases, and struct/array literals of those).
//!   Richer const expressions are the build-time-evaluation arc.
//! - A const may not collide with a case of its scope type: `Type::NAME` must
//!   stay unambiguous against case-constructor paths, which substitution
//!   would otherwise shadow.
//! - The declared type is v0-DOCUMENTATION at the declaration; every USE is
//!   checked by the ordinary store/narrowing machinery after substitution.
//!   (Declaration-site conformance for unused consts joins build-time eval.)

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionRecordError,
};
use source::{SourceSpan, Span};
use symbol_resolved_trees::{SymbolResolvedTrees, expression::ExpressionHandle};
use symbols::SymbolKind;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{ConstDefinition, DataMember, Item};

/// Declaration-site checks, run when item lowering reaches the const.
pub(crate) fn validate_const_definition(
    lowerer: &crate::lowerer::Lowerer,
    syntax_trees: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<(), Diagnostic> {
    if lowerer.defer_const_substitution {
        // Namespace and lexical identities are available in the shared symbol
        // table after lowering, not in a whole-forest spelling collision walk.
        return validate_literal_initializer(syntax_trees, definition, definition.value);
    }
    if definition.scope.as_str().is_empty() {
        free_const_shadowing_walk(lowerer, syntax_trees, definition)?;
    }

    validate_literal_initializer(syntax_trees, definition, definition.value)?;

    for item in syntax_trees.root_items() {
        match item {
            // Duplicate `Type::NAME` declarations are ambiguous.
            Item::Const(other) => {
                if !std::ptr::eq(other, definition)
                    && other.scope.as_str() == definition.scope.as_str()
                    && other.name.as_str() == definition.name.as_str()
                    && declarations_share_resolution_scope(
                        lowerer,
                        definition.name.source_span(),
                        other.name.source_span(),
                    )
                {
                    return Err(Diagnostic::error(format!(
                        "duplicate const `{}::{}`",
                        definition.scope.as_str(),
                        definition.name.as_str(),
                    )));
                }
            }
            // `Type::NAME` must not shadow a case constructor of the scope type.
            Item::Data(data)
                if data.name.as_str() == definition.scope.as_str()
                    && declarations_share_resolution_scope(
                        lowerer,
                        definition.name.source_span(),
                        data.name.source_span(),
                    ) =>
            {
                for member in syntax_trees.items.data_members(data.members) {
                    if let DataMember::Variant(variant) = member
                        && variant.name.as_str() == definition.name.as_str()
                    {
                        return Err(Diagnostic::error(format!(
                            "const `{}::{}` collides with the case `{}` of data `{}`; \
                             pick a different const name",
                            definition.scope.as_str(),
                            definition.name.as_str(),
                            variant.name.as_str(),
                            data.name.as_str(),
                        )));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// The free-const SHADOWING WALK: a bare-name const substitutes BEFORE
/// scoped-name resolution, so its name must not be spellable as anything a
/// bare reference resolves to. Whole-program, refused at the const with
/// both sites named. Conservative by design: a collision anywhere refuses,
/// even if no bare use exists (fewer names, no silent-shadow class).
fn free_const_shadowing_walk(
    lowerer: &crate::lowerer::Lowerer,
    syntax_trees: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<(), Diagnostic> {
    let const_name = definition.name.as_str();
    let collision = |site: String| {
        Err(Diagnostic::error(format!(
            "free-floating `const {const_name}` collides with {site}: a bare `{const_name}` \
             would be ambiguous (the const substitutes before name resolution). Rename one, \
             or scope the const (`const Type::{const_name}: ... = ...;`)",
        )))
    };
    for item in syntax_trees.root_items() {
        match item {
            Item::Data(data) => {
                if data.name.as_str() == const_name
                    && declarations_share_resolution_scope(
                        lowerer,
                        definition.name.source_span(),
                        data.name.source_span(),
                    )
                {
                    return collision(format!("data `{}`", data.name.as_str()));
                }
                for member in syntax_trees.items.data_members(data.members) {
                    match member {
                        DataMember::Field(field)
                            if field.name.as_str() == const_name
                                && declarations_share_resolution_scope(
                                    lowerer,
                                    definition.name.source_span(),
                                    field.name.source_span(),
                                ) =>
                        {
                            return collision(format!(
                                "field `{}` of data `{}` (bare field reads spell the field name)",
                                field.name.as_str(),
                                data.name.as_str(),
                            ));
                        }
                        DataMember::Variant(variant)
                            if variant.name.as_str() == const_name
                                && declarations_share_resolution_scope(
                                    lowerer,
                                    definition.name.source_span(),
                                    variant.name.source_span(),
                                ) =>
                        {
                            return collision(format!(
                                "case `{}` of data `{}` (case constants are spelled bare)",
                                variant.name.as_str(),
                                data.name.as_str(),
                            ));
                        }
                        _ => {}
                    }
                }
            }
            Item::Measure(measure) => {
                if measure.parameter.is_valid() {
                    let parameter = syntax_trees.items.state_parameter(measure.parameter);
                    if parameter.name.as_str() == const_name
                        && declarations_share_resolution_scope(
                            lowerer,
                            definition.name.source_span(),
                            parameter.name.source_span(),
                        )
                    {
                        return collision(format!("measure parameter `{const_name}`"));
                    }
                }
            }
            Item::Machine(machine) => {
                if machine.name.as_str() == const_name
                    && declarations_share_resolution_scope(
                        lowerer,
                        definition.name.source_span(),
                        machine.name.source_span(),
                    )
                {
                    return collision(format!("machine `{}`", machine.name.as_str()));
                }
                for state_handle in syntax_trees.items.state_handles(machine.states) {
                    let state = syntax_trees.items.state(*state_handle);
                    if state.name.as_str() == const_name
                        && declarations_share_resolution_scope(
                            lowerer,
                            definition.name.source_span(),
                            state.name.source_span(),
                        )
                    {
                        return collision(format!(
                            "state `{}` of machine `{}`",
                            state.name.as_str(),
                            machine.name.as_str(),
                        ));
                    }
                    for parameter_handle in syntax_trees.items.state_parameters(state.parameters) {
                        let parameter = syntax_trees.items.state_parameter(*parameter_handle);
                        if parameter.name.as_str() == const_name
                            && declarations_share_resolution_scope(
                                lowerer,
                                definition.name.source_span(),
                                parameter.name.source_span(),
                            )
                        {
                            return collision(format!(
                                "parameter `{}` of state `{}` in machine `{}`",
                                parameter.name.as_str(),
                                state.name.as_str(),
                                machine.name.as_str(),
                            ));
                        }
                    }
                    for statement_handle in syntax_trees.items.statements(state.statements) {
                        if let syntax_trees::statement::StatementNode::LocalData(local) =
                            syntax_trees.statements.statement(*statement_handle)
                            && local.name.as_str() == const_name
                            && declarations_share_resolution_scope(
                                lowerer,
                                definition.name.source_span(),
                                local.name.source_span(),
                            )
                        {
                            return collision(format!(
                                "local `{}` in state `{}` of machine `{}`",
                                local.name.as_str(),
                                state.name.as_str(),
                                machine.name.as_str(),
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn declarations_share_resolution_scope(
    lowerer: &crate::lowerer::Lowerer,
    left: SourceSpan,
    right: SourceSpan,
) -> bool {
    lowerer.source_reference_can_see_declaration(left, right)
        == lowerer.source_reference_can_see_declaration(right, left)
}

/// If `members` is a two-segment path naming a declared const -- or a
/// SINGLE-segment path naming a FREE-FLOATING one (safe: the shadowing walk
/// refused every collidable name) -- lower a fresh copy of its initializer
/// into `expressions` and return it. `None` = not a const reference; the
/// caller lowers the path normally.
pub(crate) fn try_lower_const_reference(
    lowerer: &mut crate::lowerer::Lowerer,
    syntax_trees: &SyntaxTrees,
    members: &[Identifier],
) -> Option<Result<ExpressionHandle, Diagnostic>> {
    if lowerer.defer_const_substitution {
        return None;
    }
    let (scope_str, name) = match members {
        [scope, name] => (scope.as_str(), name),
        [name] => ("", name),
        _ => return None,
    };
    let reference_span = const_reference_span(members);
    let definition = syntax_trees.root_items().find_map(|item| match item {
        Item::Const(definition)
            if definition.scope.as_str() == scope_str
                && definition.name.as_str() == name.as_str()
                && lowerer.source_reference_can_see_declaration(
                    reference_span,
                    definition.name.source_span(),
                ) =>
        {
            Some(definition)
        }
        _ => None,
    })?;
    // Item order is source order, so a use can lower before its declaration
    // validates -- re-check the initializer shape here (cheap) so an invalid
    // const can never substitute garbage.
    if let Err(diagnostic) =
        validate_literal_initializer(syntax_trees, definition, definition.value)
    {
        return Some(Err(diagnostic));
    }
    let declaration_ordinal = syntax_trees
        .root_items()
        .filter_map(|item| match item {
            Item::Const(other) => Some(other),
            _ => None,
        })
        .position(|other| std::ptr::eq(other, definition))?;
    let lowered =
        crate::expression::lower_expression_into_table(lowerer, syntax_trees, definition.value);
    Some(lowered.inspect(|&expression| {
        if let Some(exposure) = lowerer.current_authored_expression_exposure {
            lowerer
                .pending_const_selections
                .push(crate::lowerer::PendingConstSelection {
                    expression,
                    source_span: reference_span,
                    declaration_ordinal,
                    exposure,
                });
        }
    }))
}

pub(crate) fn has_module_owned_constants(syntax: &SyntaxTrees) -> bool {
    syntax.root_items().any(|item| {
        let Item::Const(constant) = item else {
            return false;
        };
        syntax.root_items().any(|item| {
            let Item::Module(module) = item else {
                return false;
            };
            syntax
                .items
                .identifier_path_members(module.path)
                .first()
                .is_some_and(|member| {
                    member.source_span().source_id == constant.name.source_span().source_id
                })
        })
    })
}

pub(crate) fn retain_const_initializer(
    lowerer: &mut crate::lowerer::Lowerer,
    syntax: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<(), Diagnostic> {
    if !lowerer.defer_const_substitution {
        return Ok(());
    }
    use syntax_trees::expression::ExpressionNode;
    if !matches!(
        syntax.expressions.expression(definition.value),
        ExpressionNode::Boolean(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_)
    ) {
        return Ok(());
    }
    let initializer =
        crate::expression::lower_expression_into_table(lowerer, syntax, definition.value)?;
    lowerer
        .pending_const_values
        .push((lowerer.pending_const_declarations.len(), initializer));
    Ok(())
}

pub(crate) fn substitute_resolved_constants(
    program: &mut SymbolResolvedTrees,
    initializers: &[(usize, ExpressionHandle)],
    authored: &[crate::lowerer::PendingAuthoredExpression],
    selections: &mut Vec<crate::lowerer::PendingConstSelection>,
    retained_const_count: usize,
) -> Result<(), Diagnostic> {
    use symbol_resolved_trees::expression::ExpressionNode;
    let declarations = program
        .roots
        .const_declarations
        .iter()
        .map(|declaration| declaration.symbol)
        .collect::<Vec<_>>();
    for (ordinal, symbol) in declarations.iter().enumerate() {
        if declarations[..ordinal].iter().any(|other| {
            program.symbols.name(*other) == program.symbols.name(*symbol)
                && program.symbols.same_symbol_source_package(*other, *symbol)
                && !program.symbols.source_scopes_separate(*other, *symbol)
        }) {
            return Err(Diagnostic::error(format!(
                "duplicate const `{}`",
                program.symbols.display_path(*symbol, "::")
            ))
            .with_source_span(
                program
                    .symbols
                    .symbol_source_span(*symbol)
                    .unwrap_or_default(),
            ));
        }
        if let Some((carrier, case)) = program.symbols.name(*symbol).rsplit_once("::") {
            let reference = program
                .symbols
                .symbol_source_span(*symbol)
                .unwrap_or_default();
            if let Some(data) = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    carrier,
                    &[SymbolKind::Data],
                    reference,
                )
                && program
                    .symbols
                    .find_child_by_name_and_kind(data, case, SymbolKind::Variant)
                    .is_some()
            {
                return Err(Diagnostic::error(format!(
                    "const `{}` collides with the case `{case}`",
                    program.symbols.display_path(*symbol, "::")
                ))
                .with_source_span(reference));
            }
        } else if program
            .symbols
            .child_handles(program.symbols.root())
            .into_iter()
            .flatten()
            .any(|other| {
                matches!(
                    program.symbols.get(other).kind,
                    SymbolKind::Data | SymbolKind::Machine
                ) && program.symbols.name(other) == program.symbols.name(*symbol)
                    && program.symbols.same_symbol_source_package(other, *symbol)
                    && !program.symbols.source_scopes_separate(other, *symbol)
            })
        {
            return Err(Diagnostic::error(format!(
                "free-floating const `{}` collides with a declaration in the same namespace",
                program.symbols.display_path(*symbol, "::")
            ))
            .with_source_span(
                program
                    .symbols
                    .symbol_source_span(*symbol)
                    .unwrap_or_default(),
            ));
        }
    }
    for occurrence in authored {
        let ExpressionNode::Name(path) = program
            .tables
            .bodies
            .expressions
            .expression(occurrence.expression)
        else {
            continue;
        };
        if path.is_self_value
            || matches!(
                program.symbols.get(path.head_symbol).kind,
                SymbolKind::Local
                    | SymbolKind::Parameter
                    | SymbolKind::MachineParameter
                    | SymbolKind::TypeParameter
                    | SymbolKind::ConformanceParameter
            )
        {
            continue;
        }
        let members = program
            .tables
            .bodies
            .expressions
            .name_path_members(path.members);
        let Some(first) = members.first() else {
            continue;
        };
        let Some(last) = members.last() else {
            continue;
        };
        let reference = SourceSpan::new(
            first.source_span().source_id,
            Span::new(first.source_span().span.start, last.source_span().span.end),
        );
        let name = members
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let Some(selected) = program
            .symbols
            .find_top_level_by_name_and_kinds_from_source(&name, &[SymbolKind::Const], reference)
        else {
            continue;
        };
        let Some(declaration_ordinal) = declarations.iter().position(|symbol| *symbol == selected)
        else {
            return Err(Diagnostic::error(
                "resolved constant selection has no retained declaration",
            ));
        };
        let Some((_, initializer)) = initializers
            .iter()
            .find(|(ordinal, _)| *ordinal == declaration_ordinal)
        else {
            let message = if declaration_ordinal < retained_const_count {
                "seeded constant references require retained initializer substitution"
            } else {
                "aggregate constant references in a module source closure require namespace-aware aggregate substitution"
            };
            return Err(Diagnostic::error(message).with_source_span(reference));
        };
        let value = program
            .tables
            .bodies
            .expressions
            .expression(*initializer)
            .clone();
        *program
            .tables
            .bodies
            .expressions
            .expression_mut(occurrence.expression) = value;
        selections.push(crate::lowerer::PendingConstSelection {
            expression: occurrence.expression,
            source_span: reference,
            declaration_ordinal,
            exposure: occurrence.exposure,
        });
    }
    Ok(())
}

pub(crate) fn semantic_const_name(definition: &ConstDefinition) -> String {
    if definition.scope.as_str().is_empty() {
        definition.name.as_str().to_owned()
    } else {
        format!(
            "{}::{}",
            definition.scope.as_str(),
            definition.name.as_str()
        )
    }
}

fn const_reference_span(members: &[Identifier]) -> SourceSpan {
    let Some(first) = members.first() else {
        return SourceSpan::default();
    };
    let Some(last) = members.last() else {
        return first.source_span();
    };
    if first.source_span().source_id == last.source_span().source_id {
        SourceSpan::new(
            first.source_span().source_id,
            Span::new(first.source_span().span.start, last.source_span().span.end),
        )
    } else {
        first.source_span()
    }
}

/// Attach the authored const selection to the substituted expression. The
/// value remains fully erased; only declaration custody survives.
pub(crate) fn finalize_const_selections(
    program: &mut SymbolResolvedTrees,
    pending: &[crate::lowerer::PendingConstSelection],
) -> Result<(), Diagnostic> {
    let const_symbols = program
        .symbols
        .child_handles(program.symbols.root())
        .into_iter()
        .flatten()
        .filter(|symbol| program.symbols.get(*symbol).kind == SymbolKind::Const)
        .collect::<Vec<_>>();

    for selection in pending {
        let Some(symbol) = const_symbols.get(selection.declaration_ordinal).copied() else {
            return Err(Diagnostic::error(
                "failed to retain const declaration selection provenance",
            ));
        };
        let occurrence = program
            .record_resolved_authored_declaration_selection(
                selection.source_span,
                selection.exposure,
                AuthoredDeclarationSelectionKind::StaticPathSegment,
                symbol,
            )
            .map_err(const_selection_record_diagnostic)?;
        program
            .tables
            .bodies
            .expressions
            .attach_authored_selection_occurrences(selection.expression, [occurrence]);
    }

    Ok(())
}

/// Bind retained const declarations to the symbols minted from the parallel
/// pending declaration list. Value substitution is independent of this root:
/// only source identity and visibility survive here.
pub(crate) fn finalize_const_declarations(
    program: &mut SymbolResolvedTrees,
    pending: &[crate::lowerer::PendingConstDeclaration],
) -> Result<(), Diagnostic> {
    let const_symbols = program
        .symbols
        .child_handles(program.symbols.root())
        .into_iter()
        .flatten()
        .filter(|symbol| program.symbols.get(*symbol).kind == SymbolKind::Const)
        .collect::<Vec<_>>();
    if const_symbols.len() != pending.len()
        || program.roots.const_declarations.len() != pending.len()
    {
        return Err(Diagnostic::error(
            "failed to retain const declaration visibility provenance",
        ));
    }
    let mut ordinal = 0usize;
    let mut visibility_drifted = false;
    program
        .roots
        .const_declarations
        .for_each_mut(|declaration| {
            declaration.symbol = const_symbols[ordinal];
            visibility_drifted |= declaration.is_public != pending[ordinal].is_public;
            ordinal += 1;
        });
    if visibility_drifted {
        return Err(Diagnostic::error(
            "const declaration visibility drifted before symbol assignment",
        ));
    }
    Ok(())
}

fn const_selection_record_diagnostic(error: AuthoredDeclarationSelectionRecordError) -> Diagnostic {
    Diagnostic::error(format!(
        "failed to retain const declaration selection: {error:?}"
    ))
}

/// v0 initializers are literals all the way down. Payloadless case names are
/// nullary structural literals. The parser already folds `-5` into a single
/// literal, so no operator node is legitimate here.
fn validate_literal_initializer(
    syntax_trees: &SyntaxTrees,
    definition: &ConstDefinition,
    value: syntax_trees::expression::ExpressionHandle,
) -> Result<(), Diagnostic> {
    use syntax_trees::expression::ExpressionNode;
    match syntax_trees.expressions.expression(value) {
        ExpressionNode::Boolean(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_) => Ok(()),
        ExpressionNode::ArrayLiteral(values) => {
            for element in syntax_trees.expressions.expression_handles(*values) {
                validate_literal_initializer(syntax_trees, definition, *element)?;
            }
            Ok(())
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in syntax_trees.expressions.struct_fields(literal.fields) {
                validate_literal_initializer(syntax_trees, definition, field.value)?;
            }
            Ok(())
        }
        ExpressionNode::Name(path) => {
            let members = syntax_trees.expressions.identifier_path_members(*path);
            let [type_name, case_name] = members else {
                return invalid_literal_initializer(syntax_trees, definition, value);
            };
            let is_payloadless_case = syntax_trees.root_items().any(|item| {
                let Item::Data(data) = item else {
                    return false;
                };
                data.name.as_str() == type_name.as_str()
                    && syntax_trees
                        .items
                        .data_members(data.members)
                        .iter()
                        .any(|member| {
                            matches!(
                                member,
                                DataMember::Variant(variant)
                                    if variant.name.as_str() == case_name.as_str()
                                        && variant.payload.is_empty()
                            )
                        })
            });
            if is_payloadless_case {
                Ok(())
            } else {
                invalid_literal_initializer(syntax_trees, definition, value)
            }
        }
        _ => invalid_literal_initializer(syntax_trees, definition, value),
    }
}

fn invalid_literal_initializer(
    syntax_trees: &SyntaxTrees,
    definition: &ConstDefinition,
    value: syntax_trees::expression::ExpressionHandle,
) -> Result<(), Diagnostic> {
    Err(Diagnostic::error(format!(
        "const `{}::{}` initializer must be a literal (a scalar, a payloadless \
         case, or a struct/array literal of literals) in const-v0; `{}` is not \
         -- richer const expressions arrive with build-time evaluation",
        definition.scope.as_str(),
        definition.name.as_str(),
        syntax_trees
            .expressions
            .expression(value)
            .display_name(&syntax_trees.expressions),
    )))
}

#[cfg(test)]
mod module_tests {
    use super::*;
    use source::SourceId;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees::expression::ExpressionNode;
    use symbol_resolved_trees::statement::StatementNode;
    use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

    fn resolve(sources: &[&str]) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
        let mut syntax = SyntaxTrees::default();
        for (ordinal, text) in sources.iter().enumerate() {
            let tokens = Lexer::new(text)
                .tokenize()
                .expect("tokenize module constants");
            parse_syntax_trees_into_with_id(&mut syntax, SourceId(ordinal), &tokens)
                .expect("parse module constants");
        }
        crate::lower_syntax_trees(&syntax)
    }

    fn local_value(
        program: &SymbolResolvedTrees,
        machine_name: &str,
        local_name: &str,
    ) -> ExpressionHandle {
        let machine = program
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .expect("constant test machine");
        let state = program.machine_state(program.machine_state_handles(machine.states)[0]);
        program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.name.as_str() == local_name => {
                    Some(local.initial_value)
                }
                _ => None,
            })
            .expect("constant test local")
    }

    #[test]
    fn module_constants_select_exact_local_qualified_and_imported_values() {
        let program = resolve(&[
            "module combat; pub const DAMAGE: u64 = 7; machine combat_value() -> u64 { let observed: u64 = DAMAGE; observed }",
            "module rooms; pub const DAMAGE: u64 = 9; machine room_value() -> u64 { let observed: u64 = DAMAGE; observed }",
            "use combat::DAMAGE; use rooms; machine imported() -> u64 { let observed: u64 = DAMAGE; observed } machine qualified() -> u64 { let observed: u64 = rooms::DAMAGE; observed }",
        ]).expect("resolve distinct module constants");
        for (machine, expected) in [
            ("combat_value", 7),
            ("room_value", 9),
            ("imported", 7),
            ("qualified", 9),
        ] {
            let expression = local_value(&program, machine, "observed");
            let ExpressionNode::Integer(value) =
                program.tables.bodies.expressions.expression(expression)
            else {
                panic!("substituted scalar constant");
            };
            assert_eq!(value.value_u64(), Some(expected));
            assert_eq!(
                program
                    .tables
                    .bodies
                    .expressions
                    .authored_selection_occurrences(expression)
                    .count(),
                1
            );
        }
        let declarations = program
            .roots
            .const_declarations
            .iter()
            .map(|declaration| program.symbols.display_path(declaration.symbol, "::"))
            .collect::<Vec<_>>();
        assert_eq!(declarations, ["combat::DAMAGE", "rooms::DAMAGE"]);
    }

    #[test]
    fn module_constant_yields_to_state_parameter_and_preceding_local() {
        let program = resolve(&["module combat; const DAMAGE: u64 = 7;
            machine parameter(DAMAGE: u64) -> u64 { let observed: u64 = DAMAGE; observed }
            machine local() -> u64 { let before: u64 = DAMAGE; let DAMAGE: u64 = 12; let after: u64 = DAMAGE; after }
            machine initialize() -> u64 { let DAMAGE: u64 = DAMAGE; let observed: u64 = DAMAGE; observed }"]).expect("lexical bindings shadow constants");
        for (machine, local) in [
            ("parameter", "observed"),
            ("local", "after"),
            ("initialize", "observed"),
        ] {
            let expression = local_value(&program, machine, local);
            let ExpressionNode::Name(path) =
                program.tables.bodies.expressions.expression(expression)
            else {
                panic!("lexical value remains a Name");
            };
            assert!(matches!(
                program.symbols.get(path.symbol).kind,
                SymbolKind::Local | SymbolKind::Parameter
            ));
        }
        let ExpressionNode::Integer(value) = program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "local", "before"))
        else {
            panic!("earlier constant use");
        };
        assert_eq!(value.value_u64(), Some(7));
        let ExpressionNode::Integer(value) = program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "initialize", "DAMAGE"))
        else {
            panic!("initializer selects preceding constant");
        };
        assert_eq!(value.value_u64(), Some(7));
    }

    #[test]
    fn module_constant_yields_to_parser_generated_inferred_local() {
        let tokens = Lexer::new("module combat; const DAMAGE: u64 = 7; machine inferred() -> u64 { let DAMAGE: u64 = 12; let observed: u64 = DAMAGE; observed }")
            .tokenize().expect("tokenize inferred-local control");
        let mut syntax = SyntaxTrees::default();
        parse_syntax_trees_into_with_id(&mut syntax, SourceId(0), &tokens)
            .expect("parse inferred-local control");
        let machine = syntax
            .root_items()
            .find_map(|item| match item {
                Item::Machine(machine) => Some(machine),
                _ => None,
            })
            .expect("inferred machine");
        let state = syntax
            .items
            .state(syntax.items.state_handles(machine.states)[0]);
        let statement = syntax.items.statements(state.statements)[0];
        let syntax_trees::statement::StatementNode::LocalData(mut local) =
            syntax.statements.statement(statement).clone()
        else {
            panic!("first local");
        };
        // Parser-generated destructuring locals use this same absent-type
        // representation; no new inferred-let source syntax is introduced.
        local.type_reference = syntax_trees::types::TypeReferenceHandle::invalid();
        syntax.statements.replace_statement(
            statement,
            syntax_trees::statement::StatementNode::LocalData(local),
        );
        let program = crate::lower_syntax_trees(&syntax).expect("resolve inferred local shadow");
        let ExpressionNode::Name(path) = program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "inferred", "observed"))
        else {
            panic!("inferred local reference remains lexical");
        };
        assert_eq!(program.symbols.get(path.symbol).kind, SymbolKind::Local);
    }

    #[test]
    fn same_module_duplicate_constants_reject_across_files() {
        let errors = resolve(&[
            "module combat; const DAMAGE: u64 = 7;",
            "module combat; const DAMAGE: u64 = 9;",
        ])
        .expect_err("duplicate exact module constant");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("duplicate const"))
        );
    }

    #[test]
    fn ordinary_module_presence_keeps_root_aggregate_substitution() {
        let program = resolve(&[
            "module other; data Marker {}",
            "data Pair { value: u64; } const PAIR: Pair = Pair { value: 7 }; machine read() -> Pair { let observed: Pair = PAIR; observed }",
        ]).expect("unrelated module declarations do not change aggregate substitution");
        assert!(matches!(
            program
                .tables
                .bodies
                .expressions
                .expression(local_value(&program, "read", "observed")),
            ExpressionNode::StructLiteral(_)
        ));
    }

    #[test]
    fn explicit_receiver_field_is_not_a_module_constant_reference() {
        let program = resolve(&[
            "module combat; const DAMAGE: u64 = 7; data Main { DAMAGE: u64; }
             machine Main::field(&self) -> u64 { let observed: u64 = self.DAMAGE; observed }
             machine Main::value(&self) -> u64 { let observed: u64 = DAMAGE; observed }",
        ])
        .expect("explicit receiver projection and bare constant have distinct meanings");
        assert!(matches!(
            program.tables.bodies.expressions.expression(local_value(
                &program,
                "Main::field",
                "observed"
            )),
            ExpressionNode::Member(_)
        ));
        let ExpressionNode::Integer(value) = program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "Main::value", "observed"))
        else {
            panic!("bare spelling selects module constant, not implicit field");
        };
        assert_eq!(value.value_u64(), Some(7));
    }

    #[test]
    fn conformance_parameter_cannot_be_replaced_by_module_constant() {
        let program = resolve(&[
            "trait Ranked {}",
            "module combat; const Order: u64 = 7; machine sort<Element, Order: Element satisfies Ranked>(values: &mut [Element]) -> u64 { let observed: u64 = Order; observed }",
        ]).expect("resolution preserves the proof-static machine binder");
        let ExpressionNode::Name(path) = program
            .tables
            .bodies
            .expressions
            .expression(local_value(&program, "sort", "observed"))
        else {
            panic!("conformance binder is not a constant value");
        };
        assert_eq!(
            program.symbols.get(path.symbol).kind,
            SymbolKind::ConformanceParameter
        );
    }

    #[test]
    fn module_scalar_substitution_preserves_boolean_float_and_text_kinds() {
        let program = resolve(&[
            "module boolean; const VALUE: bool = true; machine boolean_value() -> bool { let observed: bool = VALUE; observed }",
            "module floating; const VALUE: f64 = 1.5; machine float_value() -> f64 { let observed: f64 = VALUE; observed }",
            "module text; const VALUE: string = \"value\"; machine text_value() -> string { let observed: string = VALUE; observed }",
        ]).expect("primitive literal kinds retain their module selections");
        assert!(matches!(
            program.tables.bodies.expressions.expression(local_value(
                &program,
                "boolean_value",
                "observed"
            )),
            ExpressionNode::Boolean(true)
        ));
        assert!(matches!(
            program.tables.bodies.expressions.expression(local_value(
                &program,
                "float_value",
                "observed"
            )),
            ExpressionNode::Float(_)
        ));
        assert!(matches!(
            program.tables.bodies.expressions.expression(local_value(
                &program,
                "text_value",
                "observed"
            )),
            ExpressionNode::String(_)
        ));
    }

    #[test]
    fn root_case_collisions_still_reject_in_module_constant_closures() {
        let errors = resolve(&[
            "module combat; const DAMAGE: u64 = 7;",
            "data Choice { case None; } const Choice::None: u64 = 1;",
        ])
        .expect_err("const cannot shadow an exact case constructor");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("collides with the case"))
        );
    }

    #[test]
    fn colliding_imported_constant_leaves_do_not_choose_traversal_order() {
        let program = resolve(&[
            "module combat; pub const DAMAGE: u64 = 7;",
            "module rooms; pub const DAMAGE: u64 = 9;",
            "use combat::DAMAGE; use rooms::DAMAGE; machine ambiguous() -> u64 { let observed: u64 = DAMAGE; observed }",
        ]).expect("resolution preserves an unresolved ambiguous value for checking");
        assert!(matches!(
            program.tables.bodies.expressions.expression(local_value(
                &program,
                "ambiguous",
                "observed"
            )),
            ExpressionNode::Name(_)
        ));
    }

    fn resolve_seeded(
        extension: &str,
    ) -> Result<crate::lowerer::SeededSymbolResolvedTrees, Vec<Diagnostic>> {
        use std::{path::PathBuf, sync::Arc};
        let mut sources = source::SourceMap::default();
        let base_text = "module combat; pub const DAMAGE: u64 = 7;";
        let base_source = sources
            .add(PathBuf::from("combat.omg"), base_text.to_owned())
            .source_id;
        let tokens = Lexer::new(base_text)
            .tokenize()
            .expect("tokenize retained constant");
        let mut syntax = SyntaxTrees::default();
        parse_syntax_trees_into_with_id(&mut syntax, base_source, &tokens)
            .expect("parse retained constant");
        let base = crate::lower_syntax_trees_with_sources(&syntax, Arc::new(sources.clone()))
            .expect("resolve retained constant");
        let extension_source = sources
            .add(PathBuf::from("extension.omg"), extension.to_owned())
            .source_id;
        let tokens = Lexer::new(extension)
            .tokenize()
            .expect("tokenize constant extension");
        let mut syntax = SyntaxTrees::default();
        parse_syntax_trees_into_with_id(&mut syntax, extension_source, &tokens)
            .expect("parse constant extension");
        crate::lowerer::lower_syntax_extension_with_authored_selection_frontier(
            base,
            &syntax,
            Arc::new(sources),
            Vec::new(),
        )
    }

    #[test]
    fn seeded_module_constant_references_identify_missing_retained_value() {
        let errors = resolve_seeded(
            "machine read() -> u64 { let observed: u64 = combat::DAMAGE; observed }",
        )
        .expect_err("retained base has no value initializer");
        assert!(errors.iter().any(|error| {
            error
                .message
                .contains("seeded constant references require retained initializer substitution")
        }));
    }

    #[test]
    fn seeded_raw_constant_spelling_cannot_erase_module_ambiguity() {
        let extension = resolve_seeded("const combat::DAMAGE: u64 = 9; machine read() -> u64 { let observed: u64 = combat::DAMAGE; observed }").expect("ambiguous path remains unresolved");
        let program = extension.trees();
        assert!(matches!(
            program
                .tables
                .bodies
                .expressions
                .expression(local_value(program, "read", "observed")),
            ExpressionNode::Name(_)
        ));
    }
}
