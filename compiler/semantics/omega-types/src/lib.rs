//! Type, ownership, mutability, and bounded-value analysis.
//!
//! This crate currently owns the source-facing type surface report. Deeper
//! ownership, borrow, and proof-aware type solving can grow here without living
//! inside the compiler orchestration crate.

use omega_core::arena::Arena;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{CapabilityMember, DataMember, Item};
use omega_syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeSurfaceReport {
    pub declarations: Arena<TypeDeclaration>,
    pub references: Arena<TypeReferenceUse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeDeclaration {
    pub name: String,
    pub kind: TypeDeclarationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypeDeclarationKind {
    Capability,
    Data,
    Domain,
    Invariant,
    Library,
    Machine,
    Platform,
    State,
    Target,
    Trait,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeReferenceUse {
    pub name: String,
    pub kind: TypeReferenceUseKind,
    pub owner: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypeReferenceUseKind {
    Constraint,
    RangeConstraint,
    ReturnType,
    Storage,
    Parameter,
    #[default]
    Unknown,
}

pub fn build_type_surface_report(syntax_trees: &SyntaxTrees) -> TypeSurfaceReport {
    let mut report = TypeSurfaceReport::default();

    for item in syntax_trees.root_items() {
        match item {
            Item::Capability(capability) => {
                insert_declaration(
                    &mut report,
                    &capability.name,
                    TypeDeclarationKind::Capability,
                );

                for member in syntax_trees.items.capability_members(capability.members) {
                    match member {
                        CapabilityMember::Field(field) => {
                            collect_type_reference(
                                &mut report,
                                syntax_trees,
                                field.type_reference,
                                TypeReferenceUseKind::Storage,
                                &format!("capability `{}` field `{}`", capability.name, field.name),
                            );
                        }
                        CapabilityMember::State(state) => {
                            collect_state_signature(
                                &mut report,
                                syntax_trees,
                                &state.signature,
                                &format!(
                                    "capability `{}` state `{}`",
                                    capability.name, state.signature.name
                                ),
                            );
                        }
                    }
                }
            }
            Item::Data(data_definition) => {
                insert_declaration(
                    &mut report,
                    &data_definition.name,
                    TypeDeclarationKind::Data,
                );

                for member in syntax_trees.items.data_members(data_definition.members) {
                    if let DataMember::Field(field) = member {
                        collect_type_reference(
                            &mut report,
                            syntax_trees,
                            field.type_reference,
                            TypeReferenceUseKind::Storage,
                            &format!("data `{}` field `{}`", data_definition.name, field.name),
                        );
                    }
                }
            }
            Item::Domain(domain) => {
                insert_declaration(&mut report, &domain.name, TypeDeclarationKind::Domain);
                collect_type_reference(
                    &mut report,
                    syntax_trees,
                    domain.target_type,
                    TypeReferenceUseKind::Storage,
                    &format!("domain `{}` target type", domain.name),
                );
            }
            Item::Invariant(invariant) => {
                insert_declaration(&mut report, &invariant.name, TypeDeclarationKind::Invariant);
                collect_constraints(
                    &mut report,
                    syntax_trees,
                    invariant.constraints,
                    &format!("invariant `{}`", invariant.name),
                );
            }
            Item::Library(library) => {
                if let Some(name) = &library.name {
                    insert_declaration(&mut report, name, TypeDeclarationKind::Library);
                }

                for function in syntax_trees.items.library_functions(library.functions) {
                    collect_state_signature(
                        &mut report,
                        syntax_trees,
                        &function.signature,
                        &format!(
                            "library `{}` function `{}`",
                            library
                                .name
                                .as_ref()
                                .map(ToString::to_string)
                                .unwrap_or_else(|| library.path.clone()),
                            function.signature.name
                        ),
                    );
                }
            }
            Item::Export(_) | Item::Use(_) => {}
            Item::Machine(machine) => collect_machine(&mut report, syntax_trees, machine),
            Item::Operator(operator) => collect_state_parts(
                &mut report,
                syntax_trees,
                operator.parameters,
                operator.return_type,
                "operator declaration",
            ),
            Item::Platform(platform) => {
                insert_declaration(&mut report, &platform.name, TypeDeclarationKind::Platform);

                for state in syntax_trees.items.state_signatures(platform.states) {
                    let signature = syntax_trees.items.state_signature(*state);
                    collect_state_signature(
                        &mut report,
                        syntax_trees,
                        &omega_syntax_trees::item::StateSignature {
                            name: signature.name.clone(),
                            parameters: signature.parameters,
                            return_type: signature.return_type,
                            effects: signature.effects,
                            contracts: signature.contracts,
                        },
                        &format!("platform `{}` state `{}`", platform.name, signature.name),
                    );
                }
            }
            Item::Trait(trait_definition) => {
                insert_declaration(
                    &mut report,
                    &trait_definition.name,
                    TypeDeclarationKind::Trait,
                );

                for machine in syntax_trees
                    .items
                    .state_signatures(trait_definition.machines)
                {
                    let signature = syntax_trees.items.state_signature(*machine);
                    collect_state_signature(
                        &mut report,
                        syntax_trees,
                        &omega_syntax_trees::item::StateSignature {
                            name: signature.name.clone(),
                            parameters: signature.parameters,
                            return_type: signature.return_type,
                            effects: signature.effects,
                            contracts: signature.contracts,
                        },
                        &format!(
                            "trait `{}` machine `{}`",
                            trait_definition.name, signature.name
                        ),
                    );
                }
            }
            Item::Target(target) => {
                insert_declaration(&mut report, &target.name, TypeDeclarationKind::Target);
            }
            Item::TrustDefinition(_) => {}
        }
    }

    report
}

fn collect_machine(
    report: &mut TypeSurfaceReport,
    syntax_trees: &SyntaxTrees,
    machine: &omega_syntax_trees::item::Machine,
) {
    insert_declaration(report, &machine.name, TypeDeclarationKind::Machine);

    for state in syntax_trees.items.state_handles(machine.states) {
        collect_state(report, syntax_trees, machine, *state);
    }
}

fn collect_state(
    report: &mut TypeSurfaceReport,
    syntax_trees: &SyntaxTrees,
    machine: &omega_syntax_trees::item::Machine,
    state: omega_syntax_trees::item::StateHandle,
) {
    let state = syntax_trees.items.state(state);
    insert_declaration(
        report,
        &format!("{}::{}", machine.name, state.name),
        TypeDeclarationKind::State,
    );

    collect_state_parts(
        report,
        syntax_trees,
        state.parameters,
        state.return_type,
        &format!("machine `{}` state `{}`", machine.name, state.name),
    );
}

fn collect_state_signature(
    report: &mut TypeSurfaceReport,
    syntax_trees: &SyntaxTrees,
    state: &omega_syntax_trees::item::StateSignature,
    owner: &str,
) {
    collect_state_parts(
        report,
        syntax_trees,
        state.parameters,
        state.return_type,
        owner,
    );
}

fn collect_state_parts(
    report: &mut TypeSurfaceReport,
    syntax_trees: &SyntaxTrees,
    parameters: omega_core::arena::HandleSpan<omega_syntax_trees::item::StateParameterHandle>,
    return_type: TypeReferenceHandle,
    owner: &str,
) {
    for parameter in syntax_trees.items.state_parameters(parameters) {
        let parameter = syntax_trees.items.state_parameter(*parameter);
        collect_type_reference(
            report,
            syntax_trees,
            parameter.type_reference,
            TypeReferenceUseKind::Parameter,
            &format!("{owner} parameter `{}`", parameter.name),
        );
    }

    if return_type.is_valid() {
        collect_type_reference(
            report,
            syntax_trees,
            return_type,
            TypeReferenceUseKind::ReturnType,
            &format!("{owner} return type"),
        );
    }
}

fn collect_type_reference(
    report: &mut TypeSurfaceReport,
    syntax_trees: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    kind: TypeReferenceUseKind,
    owner: &str,
) {
    if !type_reference.is_valid() {
        return;
    }

    match syntax_trees.type_references.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_reference(report, syntax_trees, *referee, kind, owner);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_reference(report, syntax_trees, *base_type, kind, owner);
            collect_constraints(report, syntax_trees, *constraints, owner);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_type_reference(report, syntax_trees, *element_type, kind, owner);
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_type_reference(report, syntax_trees, *element_type, kind, owner);
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
        } => {
            insert_reference(report, base_name.as_str(), kind, owner);

            for argument in syntax_trees
                .type_references
                .type_reference_handles(*arguments)
            {
                collect_type_reference(report, syntax_trees, *argument, kind, owner);
            }
        }
        TypeReferenceNode::Named(name) => insert_reference(report, name.as_str(), kind, owner),
        TypeReferenceNode::SelfType | TypeReferenceNode::Unit => {}
    }
}

fn collect_constraints(
    report: &mut TypeSurfaceReport,
    syntax_trees: &SyntaxTrees,
    constraints: omega_core::arena::HandleSpan<TypeConstraintNode>,
    owner: &str,
) {
    for constraint in syntax_trees.type_references.constraints(constraints) {
        match constraint {
            TypeConstraintNode::Named(name) => {
                insert_reference(
                    report,
                    name.as_str(),
                    TypeReferenceUseKind::Constraint,
                    owner,
                );
            }
            TypeConstraintNode::Range { minimum, maximum } => {
                insert_reference(
                    report,
                    &format!(
                        "range<{}, {}>",
                        syntax_trees.expressions.display_name(*minimum),
                        syntax_trees.expressions.display_name(*maximum),
                    ),
                    TypeReferenceUseKind::RangeConstraint,
                    owner,
                );
            }
        }
    }
}

fn insert_declaration(report: &mut TypeSurfaceReport, name: &str, kind: TypeDeclarationKind) {
    report.declarations.insert(TypeDeclaration {
        name: name.to_owned(),
        kind,
    });
}

fn insert_reference(
    report: &mut TypeSurfaceReport,
    name: &str,
    kind: TypeReferenceUseKind,
    owner: &str,
) {
    report.references.insert(TypeReferenceUse {
        name: name.to_owned(),
        kind,
        owner: owner.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::{TypeDeclarationKind, TypeReferenceUseKind, build_type_surface_report};
    use omega_core::arena::HandleSpan;
    use omega_syntax_trees::SyntaxTrees;
    use omega_syntax_trees::identifier::Identifier;
    use omega_syntax_trees::item::{DomainDefinition, Item, Machine, State, StateParameterNode};
    use omega_syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

    #[test]
    fn collects_domain_declarations_and_target_types() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let target_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("String")));

        syntax_trees.push_root_item(Item::Domain(DomainDefinition {
            name: Identifier::generated("NonEmpty"),
            target_type,
            facts: omega_core::arena::HandleSpan::empty(),
            operators: omega_core::arena::HandleSpan::empty(),
            body_token_count: 3,
        }));

        let report = build_type_surface_report(&syntax_trees);

        assert!(
            report.declarations.iter().any(|(_, declaration)| {
                declaration.name == "NonEmpty" && declaration.kind == TypeDeclarationKind::Domain
            }),
            "domain declaration should be recorded"
        );
        assert!(
            report.references.iter().any(|(_, reference)| {
                reference.name == "String" && reference.kind == TypeReferenceUseKind::Storage
            }),
            "domain target type should be recorded"
        );
    }

    #[test]
    fn collects_state_signatures_and_constraints() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());

        let base_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("f32")));
        let minimum = syntax_trees
            .expressions
            .insert(omega_syntax_trees::expression::ExpressionNode::Integer(0));
        let maximum = syntax_trees.expressions.insert(
            omega_syntax_trees::expression::ExpressionNode::Integer(100000),
        );
        let finite = syntax_trees
            .type_references
            .append_constraint(TypeConstraintNode::Named(Identifier::generated("finite")));
        let range = syntax_trees
            .type_references
            .append_constraint(TypeConstraintNode::Range { minimum, maximum });
        let parameter_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Constrained {
                base_type,
                constraints: HandleSpan::from_parts(finite, 2),
            });
        let return_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("i32")));

        let parameter = syntax_trees
            .items
            .insert_state_parameter_node(StateParameterNode {
                name: Identifier::generated("value"),
                type_reference: parameter_type,
                is_const: false,
                is_mutable: false,
                is_self: false,
            });
        let parameter = syntax_trees.items.append_state_parameter_handle(parameter);

        let state = syntax_trees.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::from_parts(parameter, 1),
            return_type,
            statements: HandleSpan::empty(),
        });
        let state = syntax_trees.items.append_state_handle(state);

        syntax_trees.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("main"),
            attached_data: None,
            satisfies: HandleSpan::empty(),
            effects: HandleSpan::empty(),
            contracts: HandleSpan::empty(),
            states: HandleSpan::from_parts(state, 1),
        }));

        let report = build_type_surface_report(&syntax_trees);

        let _ = range;

        assert!(
            report.declarations.iter().any(|(_, declaration)| {
                declaration.name == "main::entry" && declaration.kind == TypeDeclarationKind::State
            }),
            "state declaration should be recorded"
        );
        assert!(
            report.references.iter().any(|(_, reference)| {
                reference.name == "finite" && reference.kind == TypeReferenceUseKind::Constraint
            }),
            "constraint references should be recorded"
        );
        assert!(
            report.references.iter().any(|(_, reference)| {
                reference.name == "range<0, 100000>"
                    && reference.kind == TypeReferenceUseKind::RangeConstraint
            }),
            "range constraints should be recorded"
        );
        assert_eq!(report.references.len(), 4);
    }
}
