//! Preserve the source classifier's type-role equations for generic body
//! checking. Application preparation still discharges the complete where clause;
//! current callers never create assumptions for a declaration's body.

use crate::lowering::{expression::lower_expression_into_table, type_reference};
use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbol_resolved_trees::machine::DeclaredTypeEquation;
use symbol_resolved_trees::types::{
    ConstrainedTypeReference, ConstrainedTypeReferenceStorage, ReferenceTypeReference,
    ReferenceTypeReferenceStorage, SliceTypeReference, SliceTypeReferenceStorage, TypeConstraint,
    TypeReference,
};
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::JoinedSyntaxPath;
use syntax_trees::{SyntaxTrees, item::Machine};

pub(super) fn lower(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    machine: &Machine,
) -> Result<HandleSpan<DeclaredTypeEquation>, Diagnostic> {
    let parameters = syntax.items.type_parameters(machine.type_parameters);
    let equations = crate::preparation::type_equations::classify_type_equations(
        syntax,
        parameters,
        syntax.items.proof_facts(machine.where_facts),
    );
    let mut lowered = Vec::with_capacity(equations.len());
    for equation in equations {
        equation.validate_kind("machine")?;
        let Some((parameter, definition)) = equation.structural_definition() else {
            continue;
        };
        lowered.push(DeclaredTypeEquation {
            parameter: TypeReference::Named {
                supplied_operand_owner: Default::default(),
                symbol: symbols::SymbolHandle::invalid(),
                name: crate::lowering::name::lower_name(syntax, &parameters[parameter].name),
            },
            definition: lower_type_operand(lowerer, syntax, definition)?,
        });
    }
    Ok(lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .declared_type_equations
        .insert_many(lowered))
}

/// These expression forms have type role only after the equation classifier
/// selected an opposite type binder. Their children join ordinary type storage.
fn lower_type_operand(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
) -> Result<TypeReference, Diagnostic> {
    Ok(match syntax.expressions.expression(expression) {
        ExpressionNode::TypeExpression(reference) => {
            type_reference::lower_type_reference_handle(lowerer, syntax, *reference)?
        }
        ExpressionNode::Name(path) => {
            let members = syntax.expressions.identifier_path_members(*path);
            TypeReference::Named {
                supplied_operand_owner: Default::default(),
                symbol: symbols::SymbolHandle::invalid(),
                name: symbol_resolved_trees::name::DiagnosticName::new(
                    members.joined_path_in(syntax),
                    syntax.expressions.source_span(expression),
                ),
            }
        }
        ExpressionNode::Borrow(borrow) => {
            let referee = lower_type_operand(lowerer, syntax, borrow.target)?;
            let referee = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .child_type_references
                .append(referee);
            TypeReference::Reference(ReferenceTypeReference {
                storage: ReferenceTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    referee,
                    access: borrow.access,
                    lifetime: None,
                },
            })
        }
        ExpressionNode::ArrayLiteral(elements) => {
            let [element] = syntax.expressions.expression_handles(*elements) else {
                return Err(Diagnostic::error(
                    "type equation requires exactly one slice element type",
                )
                .with_source_span(syntax.expressions.source_span(expression)));
            };
            let element = lower_type_operand(lowerer, syntax, *element)?;
            let element_type = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .child_type_references
                .append(element);
            TypeReference::Slice(SliceTypeReference {
                storage: SliceTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    element_type,
                },
            })
        }
        ExpressionNode::Indexed(indexed) => {
            let ExpressionNode::Range(range) = syntax.expressions.expression(indexed.index) else {
                return Err(Diagnostic::error("type equation requires a range shell")
                    .with_source_span(syntax.expressions.source_span(expression)));
            };
            let carrier = lower_type_operand(lowerer, syntax, indexed.collection)?;
            let base_type = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .child_type_references
                .append(carrier);
            let minimum = lower_expression_into_table(lowerer, syntax, range.start)?;
            let maximum = lower_expression_into_table(lowerer, syntax, range.end)?;
            let constraints = lowerer
                .symbol_resolved_trees
                .tables
                .types
                .constraints
                .insert_many([TypeConstraint::Range {
                    minimum,
                    maximum,
                    end_inclusive: range.end_inclusive,
                }]);
            TypeReference::Constrained(ConstrainedTypeReference {
                storage: ConstrainedTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    base_type,
                    constraints,
                },
            })
        }
        _ => {
            return Err(Diagnostic::error(
                "structural type equation lost its classified type operand",
            )
            .with_source_span(syntax.expressions.source_span(expression)));
        }
    })
}
