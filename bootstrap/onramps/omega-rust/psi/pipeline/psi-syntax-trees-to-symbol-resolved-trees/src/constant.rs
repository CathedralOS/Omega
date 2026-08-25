//! const-v0 (TASKS_TIME.md D15; design brief static_root_and_constants.md).
//!
//! Const VALUE semantics exist only until symbol resolution:
//! `const Type::NAME: T = <literal>;` declares a named pure value, and every
//! `Type::NAME` expression path substitutes a FRESH COPY of the initializer
//! right here in expression lowering. Symbol-resolved trees, typed trees,
//! validation, proofs, backends, and the interpreter never grow a const-value
//! concept -- each use IS the literal, which is exactly the copied-at-each-use
//! semantics the brief specifies (and why interior mutability can never hide in
//! one). The symbol table retains only declaration provenance so authored-
//! selection and package-authority checks cannot be erased by substitution.
//!
//! v0 boundaries, enforced loudly:
//! - TYPE-SCOPED only. A free-floating `const NAME: ...` single-segment
//!   reference could silently win over a like-named local/field (substitution
//!   happens before scoped-name resolution); a two-segment `Type::NAME` path
//!   cannot collide with locals. Free-floating arrives with a proper
//!   shadowing walk.
//! - LITERAL-ONLY initializers (scalars, negated scalars -- already folded by
//!   the parser -- payloadless cases, and struct/array literals of those).
//!   Richer const expressions are the build-time-evaluation arc.
//! - A const may not collide with a case of its scope type: `Type::NAME` must
//!   stay unambiguous against case-constructor paths, which substitution
//!   would otherwise shadow.
//! - The declared type is v0-DOCUMENTATION at the declaration; every USE is
//!   checked by the ordinary store/narrowing machinery after substitution.
//!   (Declaration-site conformance for unused consts joins build-time eval.)

use psi_diagnostics::Diagnostic;
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionRecordError,
};
use psi_source::{SourceSpan, Span};
use psi_symbol_resolved_trees::{SymbolResolvedTrees, expression::ExpressionHandle};
use psi_symbols::SymbolKind;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{ConstDefinition, DataMember, Item};

/// Declaration-site checks, run when item lowering reaches the const.
pub(crate) fn validate_const_definition(
    syntax_trees: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<(), Diagnostic> {
    if definition.scope.as_str().is_empty() {
        free_const_shadowing_walk(syntax_trees, definition)?;
    }

    validate_literal_initializer(syntax_trees, definition, definition.value)?;

    for item in syntax_trees.root_items() {
        match item {
            // Duplicate `Type::NAME` declarations are ambiguous.
            Item::Const(other) => {
                if !std::ptr::eq(other, definition)
                    && other.scope.as_str() == definition.scope.as_str()
                    && other.name.as_str() == definition.name.as_str()
                {
                    return Err(Diagnostic::error(format!(
                        "duplicate const `{}::{}`",
                        definition.scope.as_str(),
                        definition.name.as_str(),
                    )));
                }
            }
            // `Type::NAME` must not shadow a case constructor of the scope type.
            Item::Data(data) if data.name.as_str() == definition.scope.as_str() => {
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
                if data.name.as_str() == const_name {
                    return collision(format!("data `{}`", data.name.as_str()));
                }
                for member in syntax_trees.items.data_members(data.members) {
                    match member {
                        DataMember::Field(field) if field.name.as_str() == const_name => {
                            return collision(format!(
                                "field `{}` of data `{}` (bare field reads spell the field name)",
                                field.name.as_str(),
                                data.name.as_str(),
                            ));
                        }
                        DataMember::Variant(variant) if variant.name.as_str() == const_name => {
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
            Item::Machine(machine) => {
                if machine.name.as_str() == const_name {
                    return collision(format!("machine `{}`", machine.name.as_str()));
                }
                for state_handle in syntax_trees.items.state_handles(machine.states) {
                    let state = syntax_trees.items.state(*state_handle);
                    if state.name.as_str() == const_name {
                        return collision(format!(
                            "state `{}` of machine `{}`",
                            state.name.as_str(),
                            machine.name.as_str(),
                        ));
                    }
                    for parameter_handle in syntax_trees.items.state_parameters(state.parameters) {
                        let parameter = syntax_trees.items.state_parameter(*parameter_handle);
                        if parameter.name.as_str() == const_name {
                            return collision(format!(
                                "parameter `{}` of state `{}` in machine `{}`",
                                parameter.name.as_str(),
                                state.name.as_str(),
                                machine.name.as_str(),
                            ));
                        }
                    }
                    for statement_handle in syntax_trees.items.statements(state.statements) {
                        if let psi_syntax_trees::statement::StatementNode::LocalData(local) =
                            syntax_trees.statements.statement(*statement_handle)
                            && local.name.as_str() == const_name
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
    let (scope_str, name) = match members {
        [scope, name] => (scope.as_str(), name),
        [name] => ("", name),
        _ => return None,
    };
    let definition = syntax_trees.root_items().find_map(|item| match item {
        Item::Const(definition)
            if definition.scope.as_str() == scope_str
                && definition.name.as_str() == name.as_str() =>
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
    Some(lowered.map(|expression| {
        if let Some(exposure) = lowerer.current_authored_expression_exposure {
            lowerer
                .pending_const_selections
                .push(crate::lowerer::PendingConstSelection {
                    expression,
                    source_span: const_reference_span(members),
                    declaration_ordinal,
                    exposure,
                });
        }
        expression
    }))
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
    if const_symbols.len() != pending.len() {
        return Err(Diagnostic::error(
            "failed to retain const declaration visibility provenance",
        ));
    }
    for (symbol, declaration) in const_symbols.into_iter().zip(pending) {
        program.roots.const_declarations.push(
            psi_symbol_resolved_trees::constant::ConstDeclaration {
                symbol,
                is_public: declaration.is_public,
            },
        );
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
    value: psi_syntax_trees::expression::ExpressionHandle,
) -> Result<(), Diagnostic> {
    use psi_syntax_trees::expression::ExpressionNode;
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
    value: psi_syntax_trees::expression::ExpressionHandle,
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
