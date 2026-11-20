//! A stage crossing owns one typed home per live scalar, irrespective of the
//! number of names borrowing it. A loan records its defining local and home.
//! Only a direct borrow of a known home can be restored without running code.

use super::BuildStage;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use language_core::{LocalBindingRole, ReferenceAccess};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    ExpressionHandle, ExpressionNode, TableUnaryExpression, UnaryOperator,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{StateHandle, StateParameterNode};
use syntax_trees::statement::{StatementHandle, StatementNode, TableAssignment, TableLocalData};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Cut {
    pub end: usize,
    pub wait: usize,
    pub bindings: Vec<Binding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Binding {
    definition: usize,
    pub home: usize,
}

impl Binding {
    pub(super) fn classify(
        syntax: &SyntaxTrees,
        local: &TableLocalData,
        definition: usize,
        earlier: impl FnOnce(&str) -> Option<Self>,
    ) -> Option<Self> {
        if local.role != LocalBindingRole::Value
            || local.relevance.is_erased()
            || !local.type_reference.is_valid()
        {
            return None;
        }
        let ty = syntax.type_references.type_reference(local.type_reference);
        if let Some(name) = ty.as_named() {
            let name = name.as_str(syntax);
            if scalar_type(name) {
                return Some(Self {
                    definition,
                    home: definition,
                });
            }
        }
        if local.is_mutable || !local.initial_value.is_valid() {
            return None;
        }
        let ExpressionNode::Borrow(loan) = syntax.expressions.expression(local.initial_value)
        else {
            return None;
        };
        let syntax_trees::types::TypeReferenceNode::Reference {
            referee,
            access,
            lifetime: None,
        } = ty
        else {
            return None;
        };
        if *access != loan.access
            || !syntax
                .type_references
                .type_reference(*referee)
                .as_named()
                .is_some_and(|name| scalar_type(name.as_str(syntax)))
        {
            return None;
        }
        let ExpressionNode::Name {
            members,
            application: None,
        } = syntax.expressions.expression(loan.target)
        else {
            return None;
        };
        let [root] = syntax.expressions.identifier_path_members(*members) else {
            return None;
        };
        let root = earlier(root.as_str(syntax))?;
        (root.definition == root.home).then_some(Self {
            definition,
            home: root.home,
        })
    }
}

fn scalar_type(name: &str) -> bool {
    matches!(name, "bool" | "f32" | "f64")
        || numeric_literals::LandedIntegerType::from_name(name).is_some()
}

pub(super) fn local(syntax: &SyntaxTrees, statement: StatementHandle) -> &TableLocalData {
    let StatementNode::LocalData(local) = syntax.statements.statement(statement) else {
        unreachable!("only an ordinary local supplies a frame binding")
    };
    local
}

fn parameter(local: &TableLocalData) -> Identifier {
    let span = local.name.source_span();
    Identifier::new(BuildStage::frame_parameter_name(span.span), span)
}

fn read(syntax: &mut SyntaxTrees, name: Identifier, indirect: bool) -> ExpressionHandle {
    let span = name.source_span();
    let members = HandleSpan::from_parts(syntax.expressions.append_identifier_path_member(name), 1);
    let value = syntax.expressions.insert(ExpressionNode::Name {
        members,
        application: None,
    });
    syntax.expressions.set_source_span(value, span);
    if !indirect {
        return value;
    }
    let value = syntax
        .expressions
        .insert(ExpressionNode::Unary(TableUnaryExpression {
            operator: UnaryOperator::Dereference,
            operand: value,
        }));
    syntax.expressions.set_source_span(value, span);
    value
}

pub(super) fn project(
    syntax: &mut SyntaxTrees,
    state: StateHandle,
    statements: &[StatementHandle],
    homes: &[usize],
    incoming: Option<&Cut>,
    outgoing: Option<&Cut>,
) -> Result<(), Vec<Diagnostic>> {
    let entry = syntax.items.state(state);
    let [receiver] = *syntax.items.state_parameters(entry.parameters) else {
        return Err(vec![Diagnostic::error(
            "Build activation requires its one original Build parameter",
        )]);
    };
    let body = syntax.items.statements(entry.statements).to_vec();
    let mut params =
        HandleSpan::from_parts(syntax.items.append_state_parameter_handle(receiver), 1);
    for &home in homes {
        let local = local(syntax, statements[home]);
        let name = parameter(local);
        let ty = local.type_reference;
        let type_reference = syntax
            .type_references
            .insert_reference(ty, ReferenceAccess::Mutable);
        let param = syntax
            .items
            .insert_state_parameter_node(StateParameterNode {
                name,
                type_reference,
                is_mutable: true,
                ..Default::default()
            });
        params.push_contiguous(syntax.items.append_state_parameter_handle(param));
    }
    syntax.items.state_mut(state).parameters = params;
    let mut projected = HandleSpan::empty();
    if let Some(cut) = incoming {
        for &home in homes {
            if !cut.bindings.iter().any(|binding| binding.home == home) {
                continue;
            }
            let mut binding = local(syntax, statements[home]).clone();
            binding.initial_value = read(syntax, parameter(&binding), true);
            let restored = syntax.statements.insert(StatementNode::LocalData(binding));
            projected.push_contiguous(syntax.items.append_statement_handle(restored));
        }
        for loan in cut
            .bindings
            .iter()
            .filter(|binding| binding.definition != binding.home)
        {
            projected.push_contiguous(
                syntax
                    .items
                    .append_statement_handle(statements[loan.definition]),
            );
        }
    }
    for statement in body {
        projected.push_contiguous(syntax.items.append_statement_handle(statement));
    }
    if let Some(cut) = outgoing {
        // Read through every live loan. This keeps its ownership obligations
        // live through the wait, including when the owner has no further direct use.
        for binding in &cut.bindings {
            let loan = binding.definition != binding.home;
            if !loan
                && cut
                    .bindings
                    .iter()
                    .any(|other| other.home == binding.home && other.definition != other.home)
            {
                continue;
            }
            let target = parameter(local(syntax, statements[binding.home]));
            let name = local(syntax, statements[binding.definition]).name.clone();
            let target = read(syntax, target, true);
            let value = read(syntax, name, loan);
            let save = syntax
                .statements
                .insert(StatementNode::Assignment(TableAssignment { target, value }));
            projected.push_contiguous(syntax.items.append_statement_handle(save));
        }
    }
    syntax.items.state_mut(state).statements = projected;
    Ok(())
}
