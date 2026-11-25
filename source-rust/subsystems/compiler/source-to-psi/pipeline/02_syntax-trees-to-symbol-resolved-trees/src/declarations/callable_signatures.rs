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
pub(in crate::resolution::declarations) struct InputScope {
    parent: Option<usize>,
    type_names: Vec<String>,
    value_names: Vec<String>,
    lifetimes: Vec<String>,
}

#[derive(Debug, Default)]
pub(in crate::resolution::declarations) struct SignatureInputs {
    scopes: Vec<InputScope>,
    pub types: Vec<(usize, TypeReferenceHandle)>,
    pub expressions: Vec<(usize, ExpressionHandle)>,
}

impl SignatureInputs {
    pub(in crate::resolution::declarations) fn scope_count(&self) -> usize {
        self.scopes.len()
    }

    pub(in crate::resolution::declarations) fn of(
        syntax: &SyntaxTrees,
        signature: &StateSignature,
    ) -> Self {
        let mut inputs = Self::default();
        inputs.signature(syntax, signature, None);
        inputs
    }

    pub(in crate::resolution::declarations) fn hides_type(
        &self,
        mut scope: usize,
        name: &str,
    ) -> bool {
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

    pub(in crate::resolution::declarations) fn hides_lifetime(
        &self,
        mut scope: usize,
        name: &str,
    ) -> bool {
        loop {
            let current = &self.scopes[scope];
            if current.lifetimes.iter().any(|bound| bound == name) {
                return true;
            }
            let Some(parent) = current.parent else {
                return false;
            };
            scope = parent;
        }
    }

    pub(in crate::resolution::declarations) fn hides_value(
        &self,
        mut scope: usize,
        name: &str,
    ) -> bool {
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

    pub(in crate::resolution::declarations) fn bound_values(
        &self,
        mut scope: usize,
    ) -> Vec<String> {
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

    pub(in crate::resolution::declarations) fn bound_types(&self, mut scope: usize) -> Vec<String> {
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
        let lifetimes = signature
            .lifetime_parameters
            .iter()
            .map(|name| name.as_str(syntax).to_owned())
            .collect::<Vec<_>>();
        type_names.extend(lifetimes.iter().cloned());
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
            lifetimes,
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
                }
                | TypeParameterKind::Proof { contract: required } => {
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
                        lifetimes: Vec::new(),
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
                    self.types.push((scope, contract.result));
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
