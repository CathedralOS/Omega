//! const-v0 (TASKS_TIME.md D15; design brief static_root_and_constants.md).
//!
//! Consts exist ONLY until symbol resolution: `const Type::NAME: T = <literal>;`
//! declares a named pure value, and every `Type::NAME` expression path
//! substitutes a FRESH COPY of the initializer right here in expression
//! lowering. Symbol-resolved trees, typed trees, validation, proofs, backends,
//! and the interpreter never grow a const concept -- each use IS the literal,
//! which is exactly the copied-at-each-use semantics the brief specifies (and
//! why interior mutability can never hide in one).
//!
//! v0 boundaries, enforced loudly:
//! - TYPE-SCOPED only. A free-floating `const NAME: ...` single-segment
//!   reference could silently win over a like-named local/field (substitution
//!   happens before scoped-name resolution); a two-segment `Type::NAME` path
//!   cannot collide with locals. Free-floating arrives with a proper
//!   shadowing walk.
//! - LITERAL-ONLY initializers (scalars, negated scalars -- already folded by
//!   the parser -- and struct/array literals of those). Richer const
//!   expressions are the build-time-evaluation arc.
//! - A const may not collide with a case of its scope type: `Type::NAME` must
//!   stay unambiguous against case-constructor paths, which substitution
//!   would otherwise shadow.
//! - The declared type is v0-DOCUMENTATION at the declaration; every USE is
//!   checked by the ordinary store/narrowing machinery after substitution.
//!   (Declaration-site conformance for unused consts joins build-time eval.)

use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{ConstDefinition, DataMember, Item};

/// Declaration-site checks, run when item lowering reaches the const.
pub(crate) fn validate_const_definition(
    syntax_trees: &SyntaxTrees,
    definition: &ConstDefinition,
) -> Result<(), Diagnostic> {
    if definition.scope.as_str().is_empty() {
        return Err(Diagnostic::error(format!(
            "free-floating `const {}` is not accepted yet: scope it to a type \
             (`const Type::{}: ... = ...;`) -- a bare-name const could silently \
             shadow a like-named local or field until the shadowing walk lands",
            definition.name.as_str(),
            definition.name.as_str(),
        )));
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

/// If `members` is a two-segment path naming a declared const, lower a fresh
/// copy of its initializer into `expressions` and return it. `None` = not a
/// const reference; the caller lowers the path normally.
pub(crate) fn try_lower_const_reference(
    syntax_trees: &SyntaxTrees,
    expressions: &mut ExpressionTable,
    members: &[Identifier],
) -> Option<Result<ExpressionHandle, Diagnostic>> {
    let [scope, name] = members else {
        return None;
    };
    let definition = syntax_trees.root_items().find_map(|item| match item {
        Item::Const(definition)
            if definition.scope.as_str() == scope.as_str()
                && definition.name.as_str() == name.as_str() =>
        {
            Some(definition)
        }
        _ => None,
    })?;
    // Item order is source order, so a use can lower before its declaration
    // validates -- re-check the initializer shape here (cheap) so an invalid
    // const can never substitute garbage.
    if let Err(diagnostic) = validate_literal_initializer(syntax_trees, definition, definition.value)
    {
        return Some(Err(diagnostic));
    }
    Some(crate::expression::lower_expression_into_table(
        syntax_trees,
        expressions,
        definition.value,
    ))
}

/// v0 initializers are literals all the way down. The parser already folds
/// `-5` into a single literal, so no operator node is legitimate here.
fn validate_literal_initializer(
    syntax_trees: &SyntaxTrees,
    definition: &ConstDefinition,
    value: omega_syntax_trees::expression::ExpressionHandle,
) -> Result<(), Diagnostic> {
    use omega_syntax_trees::expression::ExpressionNode;
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
        other => Err(Diagnostic::error(format!(
            "const `{}::{}` initializer must be a literal (a scalar, or a \
             struct/array literal of literals) in const-v0; `{}` is not -- \
             richer const expressions arrive with build-time evaluation",
            definition.scope.as_str(),
            definition.name.as_str(),
            other.display_name(&syntax_trees.expressions),
        ))),
    }
}
