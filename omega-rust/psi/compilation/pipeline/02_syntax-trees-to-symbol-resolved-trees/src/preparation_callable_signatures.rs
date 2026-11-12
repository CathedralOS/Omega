//! Keep a callable's type inputs and facts with the binders that own them.
//!
//! A descriptor adds no inline storage edges. Its signature still contributes
//! type and proof dependencies, and a template substitution must not replace a
//! signature's own formals with an equally spelled enclosing constant.

use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionHandle;
use syntax_trees::item::{MachineParameterContract, ProofFact, StateSignature, TypeParameterKind};
use syntax_trees::types::TypeReferenceHandle;

#[derive(Debug)]
pub(in crate::preparation) struct InputScope {
    parent: Option<usize>,
    type_names: Vec<String>,
    value_names: Vec<String>,
}

#[derive(Debug, Default)]
pub(in crate::preparation) struct SignatureInputs {
    scopes: Vec<InputScope>,
    pub types: Vec<(usize, TypeReferenceHandle)>,
    pub expressions: Vec<(usize, ExpressionHandle)>,
}

impl SignatureInputs {
    pub(in crate::preparation) fn scope_count(&self) -> usize {
        self.scopes.len()
    }

    pub(in crate::preparation) fn of(syntax: &SyntaxTrees, signature: &StateSignature) -> Self {
        let mut inputs = Self::default();
        inputs.signature(syntax, signature, None);
        inputs
    }

    pub(in crate::preparation) fn hides_type(&self, mut scope: usize, name: &str) -> bool {
        loop {
            let current = &self.scopes[scope];
            if current.type_names.iter().any(|bound| bound == name) {
                return true;
            }
            let Some(parent) = current.parent else {
                return false;
            };
            scope = parent;
        }
    }

    pub(in crate::preparation) fn hides_value(&self, mut scope: usize, name: &str) -> bool {
        loop {
            let current = &self.scopes[scope];
            if current.value_names.iter().any(|bound| bound == name) {
                return true;
            }
            let Some(parent) = current.parent else {
                return false;
            };
            scope = parent;
        }
    }

    pub(in crate::preparation) fn bound_values(&self, mut scope: usize) -> Vec<String> {
        let mut names = Vec::new();
        loop {
            let current = &self.scopes[scope];
            names.extend(current.value_names.iter().cloned());
            let Some(parent) = current.parent else {
                return names;
            };
            scope = parent;
        }
    }

    pub(in crate::preparation) fn bound_types(&self, mut scope: usize) -> Vec<String> {
        let mut names = Vec::new();
        loop {
            let current = &self.scopes[scope];
            names.extend(current.type_names.iter().cloned());
            let Some(parent) = current.parent else {
                return names;
            };
            scope = parent;
        }
    }

    fn signature(
        &mut self,
        syntax: &SyntaxTrees,
        signature: &StateSignature,
        parent: Option<usize>,
    ) {
        let mut type_names = syntax
            .items
            .type_parameters(signature.type_parameters)
            .iter()
            .map(|parameter| parameter.name.as_str(syntax).to_owned())
            .collect::<Vec<_>>();
        let mut value_names = type_names.clone();
        type_names.extend(
            signature
                .lifetime_parameters
                .iter()
                .map(|name| name.as_str(syntax).to_owned()),
        );
        value_names.extend(
            syntax
                .items
                .state_parameters(signature.parameters)
                .iter()
                .map(|parameter| {
                    syntax
                        .items
                        .state_parameter(*parameter)
                        .name
                        .as_str(syntax)
                        .to_owned()
                }),
        );
        // Syntax has not selected type versus value roles for a named generic
        // operand yet. A dependent formal in `Buffer<n>` must remain owned here.
        type_names.extend(value_names.iter().cloned());
        let scope = self.scopes.len();
        self.scopes.push(InputScope {
            parent,
            type_names,
            value_names,
        });
        self.types.extend(
            syntax
                .items
                .state_parameters(signature.parameters)
                .iter()
                .map(|parameter| {
                    (
                        scope,
                        syntax.items.state_parameter(*parameter).type_reference,
                    )
                }),
        );
        if signature.return_type.is_valid() {
            self.types.push((scope, signature.return_type));
        }
        for bound in &signature.conformance_bounds {
            self.types.extend(
                syntax
                    .type_references
                    .type_reference_handles(bound.arguments)
                    .iter()
                    .map(|reference| (scope, *reference)),
            );
        }
        for parameter in syntax.items.type_parameters(signature.type_parameters) {
            match &parameter.kind {
                TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference } => {
                    self.types.push((scope, *type_reference))
                }
                TypeParameterKind::Machine {
                    contract: Some(MachineParameterContract::Structural(required)),
                } => {
                    self.signature(syntax, required, Some(scope));
                }
                TypeParameterKind::Machine {
                    contract:
                        Some(MachineParameterContract::Nominal {
                            trait_arguments, ..
                        }),
                } => {
                    self.types.extend(
                        syntax
                            .type_references
                            .type_reference_handles(*trait_arguments)
                            .iter()
                            .map(|reference| (scope, *reference)),
                    );
                }
                TypeParameterKind::Proposition {
                    contract: Some(contract),
                } => {
                    let names = syntax
                        .items
                        .state_parameters(contract.parameters)
                        .iter()
                        .map(|parameter| {
                            syntax
                                .items
                                .state_parameter(*parameter)
                                .name
                                .as_str(syntax)
                                .to_owned()
                        })
                        .collect();
                    let nested = self.scopes.len();
                    self.scopes.push(InputScope {
                        parent: Some(scope),
                        type_names: Vec::new(),
                        value_names: names,
                    });
                    self.types.extend(
                        syntax
                            .items
                            .state_parameters(contract.parameters)
                            .iter()
                            .map(|parameter| {
                                (
                                    nested,
                                    syntax.items.state_parameter(*parameter).type_reference,
                                )
                            }),
                    );
                }
                _ => {}
            }
        }
        for contract in syntax.items.capability_contracts(signature.contracts) {
            for fact in syntax.items.proof_facts(contract.facts) {
                match fact {
                    ProofFact::Expression(expression) => {
                        self.expressions.push((scope, *expression))
                    }
                    ProofFact::Membership(membership) => {
                        self.expressions.push((scope, membership.value));
                        self.types.extend(
                            syntax
                                .type_references
                                .type_reference_handles(membership.domain_arguments)
                                .iter()
                                .map(|reference| (scope, *reference)),
                        );
                    }
                }
            }
        }
    }
}

/// Locate copied type occurrences hidden by a signature's own binder frames.
/// These occurrences keep their declaration meaning during enclosing application.
pub(in crate::preparation) fn bound_type_occurrences(
    syntax: &SyntaxTrees,
    signature: &StateSignature,
) -> std::collections::HashSet<TypeReferenceHandle> {
    use syntax_trees::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};
    let mut bound = std::collections::HashSet::new();
    let inputs = SignatureInputs::of(syntax, signature);
    let mut pending = inputs
        .types
        .iter()
        .map(|(scope, reference)| (*reference, inputs.bound_types(*scope)))
        .collect::<Vec<_>>();
    for (scope, expression) in &inputs.expressions {
        let hidden = inputs.bound_types(*scope);
        let mut expressions = std::collections::HashSet::new();
        crate::preparation::generic_data::collect_expression_handles(
            syntax,
            *expression,
            &mut expressions,
        );
        for expression in expressions {
            crate::preparation::generic_data::visit_expression_type_inputs(
                syntax,
                expression,
                &mut |reference| pending.push((reference, hidden.clone())),
            );
        }
    }
    while let Some((reference, hidden)) = pending.pop() {
        match syntax.type_references.type_reference(reference) {
            TypeReferenceNode::Named(name) => {
                if hidden.iter().any(|bound| bound == name.as_str(syntax)) {
                    bound.insert(reference);
                }
            }
            TypeReferenceNode::Reference { referee, .. } => pending.push((*referee, hidden)),
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                pending.push((*element_type, hidden.clone()));
                if let FixedArrayLength::Expression(length) = length {
                    pending.push((*length, hidden));
                }
            }
            TypeReferenceNode::Slice { element_type } => pending.push((*element_type, hidden)),
            TypeReferenceNode::Generic { arguments, .. }
            | TypeReferenceNode::DynamicTrait {
                trait_arguments: arguments,
                ..
            } => {
                pending.extend(
                    syntax
                        .type_references
                        .type_reference_handles(*arguments)
                        .iter()
                        .map(|reference| (*reference, hidden.clone())),
                );
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                pending.push((*base_type, hidden.clone()));
                for constraint in syntax.type_references.constraints(*constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        pending.extend(
                            syntax
                                .type_references
                                .type_reference_handles(domain.arguments)
                                .iter()
                                .map(|reference| (*reference, hidden.clone())),
                        );
                    }
                }
            }
            TypeReferenceNode::Callable { signature } => {
                let nested = SignatureInputs::of(syntax, syntax.items.state_signature(*signature));
                for (scope, reference) in &nested.types {
                    let mut hidden = hidden.clone();
                    hidden.extend(nested.bound_types(*scope));
                    pending.push((*reference, hidden));
                }
                for (scope, expression) in &nested.expressions {
                    let mut hidden = hidden.clone();
                    hidden.extend(nested.bound_types(*scope));
                    let mut expressions = std::collections::HashSet::new();
                    crate::preparation::generic_data::collect_expression_handles(
                        syntax,
                        *expression,
                        &mut expressions,
                    );
                    for expression in expressions {
                        crate::preparation::generic_data::visit_expression_type_inputs(
                            syntax,
                            expression,
                            &mut |reference| pending.push((reference, hidden.clone())),
                        );
                    }
                }
            }
            TypeReferenceNode::ConstExpression(expression) => {
                let mut expressions = std::collections::HashSet::new();
                crate::preparation::generic_data::collect_expression_handles(
                    syntax,
                    *expression,
                    &mut expressions,
                );
                for expression in expressions {
                    crate::preparation::generic_data::visit_expression_type_inputs(
                        syntax,
                        expression,
                        &mut |reference| pending.push((reference, hidden.clone())),
                    );
                }
            }
            TypeReferenceNode::StaticSelection(selection) => {
                crate::preparation::generic_data::visit_static_argument_type_inputs(
                    syntax,
                    std::slice::from_ref(selection),
                    &mut |reference| pending.push((reference, hidden.clone())),
                )
            }
            TypeReferenceNode::Unit => {}
        }
    }
    bound
}
