//! Preserve the source classifier's type-role equations for generic body
//! checking. Application preparation still discharges every requires equation;
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

pub(super) struct MachineTypeEquations {
    pub(super) definitions: HandleSpan<DeclaredTypeEquation>,
    pub(super) requires_argument_recovery: bool,
    pub(super) declares_equations: bool,
}

pub(super) fn lower(
    lowerer: &mut Lowerer,
    syntax: &SyntaxTrees,
    machine: &Machine,
) -> Result<MachineTypeEquations, Diagnostic> {
    let parameters = syntax.items.type_parameters(machine.type_parameters);
    let equations = crate::preparation::machine_equations::machine_type_equations(syntax, machine);
    let declares_equations = !equations.is_empty();
    let requires_argument_recovery = equations
        .iter()
        .any(crate::preparation::type_equations::TypeEquation::requires_argument_recovery);
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
    Ok(MachineTypeEquations {
        definitions: lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .declared_type_equations
            .insert_many(lowered),
        requires_argument_recovery,
        declares_equations,
    })
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
            let referee = lowerer.declaration_child_type_references().append(referee);
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
            let element_type = lowerer.declaration_child_type_references().append(element);
            TypeReference::Slice(SliceTypeReference {
                storage: SliceTypeReferenceStorage {
                    supplied_operand_owner: Default::default(),
                    element_type,
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
