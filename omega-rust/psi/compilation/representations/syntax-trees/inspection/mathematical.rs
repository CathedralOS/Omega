use super::expression_and_type_snapshots::{
    snapshot_expression_handle, snapshot_identifier, snapshot_type_reference_handle,
};
use super::item_snapshots::{snapshot_binding_relevance, snapshot_type_parameters};
use super::{ExpressionSnapshot, IdentifierSnapshot, TypeParameterSnapshot, TypeReferenceSnapshot};
use crate::SyntaxTrees;
use crate::item::{
    MathematicalDefinition, MathematicalDefinitionBody, MathematicalTypeHandle,
    MathematicalTypeNode,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MathematicalDefinitionSnapshot {
    pub name: IdentifierSnapshot,
    pub is_public: bool,
    pub type_parameters: Vec<TypeParameterSnapshot>,
    pub parameters: Vec<MathematicalParameterSnapshot>,
    pub result: MathematicalTypeSnapshot,
    pub body: MathematicalDefinitionBodySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MathematicalParameterSnapshot {
    pub name: IdentifierSnapshot,
    pub relevance: &'static str,
    pub ty: MathematicalTypeSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MathematicalDefinitionBodySnapshot {
    Definition { expression: ExpressionSnapshot },
    Assumption,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MathematicalTypeSnapshot {
    Ordinary {
        reference: TypeReferenceSnapshot,
    },
    Arrow {
        binder: Option<IdentifierSnapshot>,
        domain: Box<Self>,
        codomain: Box<Self>,
    },
    Application {
        callee: Box<Self>,
        arguments: Vec<ExpressionSnapshot>,
    },
}

pub(super) fn snapshot_definition(
    trees: &SyntaxTrees,
    definition: &MathematicalDefinition,
) -> MathematicalDefinitionSnapshot {
    MathematicalDefinitionSnapshot {
        name: snapshot_identifier(trees, &definition.name),
        is_public: definition.is_public,
        type_parameters: snapshot_type_parameters(trees, definition.type_parameters),
        parameters: trees
            .items
            .mathematical_parameters(definition.parameters)
            .iter()
            .map(|handle| {
                let parameter = trees.items.mathematical_parameter(*handle);
                MathematicalParameterSnapshot {
                    name: snapshot_identifier(trees, &parameter.name),
                    relevance: snapshot_binding_relevance(parameter.relevance),
                    ty: snapshot_type(trees, parameter.ty),
                }
            })
            .collect(),
        result: snapshot_type(trees, definition.result),
        body: match definition.body {
            MathematicalDefinitionBody::Definition(value) => {
                MathematicalDefinitionBodySnapshot::Definition {
                    expression: snapshot_expression_handle(trees, value),
                }
            }
            MathematicalDefinitionBody::Assumption => {
                MathematicalDefinitionBodySnapshot::Assumption
            }
        },
    }
}

fn snapshot_type(trees: &SyntaxTrees, ty: MathematicalTypeHandle) -> MathematicalTypeSnapshot {
    match trees.items.mathematical_type(ty) {
        MathematicalTypeNode::Ordinary(reference) => MathematicalTypeSnapshot::Ordinary {
            reference: snapshot_type_reference_handle(trees, *reference),
        },
        MathematicalTypeNode::Arrow {
            binder,
            domain,
            codomain,
        } => MathematicalTypeSnapshot::Arrow {
            binder: binder.as_ref().map(|name| snapshot_identifier(trees, name)),
            domain: Box::new(snapshot_type(trees, *domain)),
            codomain: Box::new(snapshot_type(trees, *codomain)),
        },
        MathematicalTypeNode::Application { callee, arguments } => {
            MathematicalTypeSnapshot::Application {
                callee: Box::new(snapshot_type(trees, *callee)),
                arguments: trees
                    .expressions
                    .expression_handles(*arguments)
                    .iter()
                    .map(|handle| snapshot_expression_handle(trees, *handle))
                    .collect(),
            }
        }
    }
}
