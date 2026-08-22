//! Psi-owned type-surface, ownership, mutability, and bounded-value analysis.
//!
//! This crate currently owns the source-facing type surface report. Deeper
//! ownership, borrow, and proof-aware type solving can grow here without living
//! inside the compiler orchestration crate.

use psi_arena::Arena;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::item::{CapabilityMember, DataMember, Item};
use psi_syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

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
    Operator,
    Proposition,
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
            // Consts substitute away at symbol resolution; the declared type
            // is v0-documentation (uses are checked post-substitution).
            Item::Const(_) => {}
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
                collect_domain_operators(&mut report, syntax_trees, domain);
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
            Item::WireData(_)
            | Item::Export(_)
            | Item::Conformance(_)
            | Item::Module(_)
            | Item::Package(_)
            | Item::Use(_) => {}
            Item::Proposition(proposition) => {
                insert_declaration(
                    &mut report,
                    &proposition.name,
                    TypeDeclarationKind::Proposition,
                );
                for binder in syntax_trees
                    .items
                    .type_parameters(proposition.type_parameters)
                {
                    if let psi_syntax_trees::item::TypeParameterKind::Const { type_reference } =
                        binder.kind
                    {
                        collect_type_reference(
                            &mut report,
                            syntax_trees,
                            type_reference,
                            TypeReferenceUseKind::Parameter,
                            &format!(
                                "proposition `{}` const binder `{}`",
                                proposition.name, binder.name
                            ),
                        );
                    }
                }
                for parameter in syntax_trees.items.state_parameters(proposition.parameters) {
                    let parameter = syntax_trees.items.state_parameter(*parameter);
                    collect_type_reference(
                        &mut report,
                        syntax_trees,
                        parameter.type_reference,
                        TypeReferenceUseKind::Parameter,
                        &format!(
                            "proposition `{}` parameter `{}`",
                            proposition.name, parameter.name
                        ),
                    );
                }
                if let psi_syntax_trees::item::PropositionBody::Witness { evidence } =
                    proposition.body
                {
                    collect_type_reference(
                        &mut report,
                        syntax_trees,
                        evidence,
                        TypeReferenceUseKind::Constraint,
                        &format!("proposition `{}` evidence", proposition.name),
                    );
                }
            }
            Item::Machine(machine) => collect_machine(&mut report, syntax_trees, machine),
            Item::Measure(measure) => {
                insert_declaration(
                    &mut report,
                    &operator_name(syntax_trees, measure.name),
                    TypeDeclarationKind::Operator,
                );
                if measure.parameter.is_valid() {
                    let parameter = syntax_trees.items.state_parameter(measure.parameter);
                    collect_type_reference(
                        &mut report,
                        syntax_trees,
                        parameter.type_reference,
                        TypeReferenceUseKind::Parameter,
                        "measure declaration",
                    );
                }
                if measure.return_type.is_valid() {
                    collect_type_reference(
                        &mut report,
                        syntax_trees,
                        measure.return_type,
                        TypeReferenceUseKind::ReturnType,
                        "measure declaration",
                    );
                }
            }
            Item::Operator(operator) => {
                insert_declaration(
                    &mut report,
                    &operator_name(syntax_trees, operator.name),
                    TypeDeclarationKind::Operator,
                );
                collect_state_parts(
                    &mut report,
                    syntax_trees,
                    operator.parameters,
                    operator.return_type,
                    "operator declaration",
                );
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
                        &psi_syntax_trees::item::StateSignature {
                            name: signature.name.clone(),
                            spelling: signature.spelling,
                            lifetime_parameters: signature.lifetime_parameters.clone(),
                            type_parameters: signature.type_parameters,
                            is_default: signature.is_default,
                            parameters: signature.parameters,
                            return_type: signature.return_type,
                            service_reaches: signature.service_reaches,
                            invokes: signature.invokes,
                            suspends: signature.suspends,
                            blocks: signature.blocks,
                            contracts: signature.contracts,
                            default_body: signature.default_body,
                            terminates_guarantee: signature.terminates_guarantee,
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
        }
    }

    report
}

fn collect_machine(
    report: &mut TypeSurfaceReport,
    syntax_trees: &SyntaxTrees,
    machine: &psi_syntax_trees::item::Machine,
) {
    insert_declaration(report, &machine.name, TypeDeclarationKind::Machine);

    for state in syntax_trees.items.state_handles(machine.states) {
        collect_state(report, syntax_trees, machine, *state);
    }
}

fn collect_domain_operators(
    report: &mut TypeSurfaceReport,
    syntax_trees: &SyntaxTrees,
    domain: &psi_syntax_trees::item::DomainDefinition,
) {
    for operator in syntax_trees.items.operators(domain.operators) {
        let operator_name = operator_name(syntax_trees, operator.name);
        insert_declaration(
            report,
            &format!("{}::{}", domain.name, operator_name),
            TypeDeclarationKind::Operator,
        );
        collect_state_parts(
            report,
            syntax_trees,
            operator.parameters,
            operator.return_type,
            &format!("domain `{}` operator `{operator_name}`", domain.name),
        );
    }
}

fn collect_state(
    report: &mut TypeSurfaceReport,
    syntax_trees: &SyntaxTrees,
    machine: &psi_syntax_trees::item::Machine,
    state: psi_syntax_trees::item::StateHandle,
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
    state: &psi_syntax_trees::item::StateSignature,
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
    parameters: psi_arena::HandleSpan<psi_syntax_trees::item::StateParameterHandle>,
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
            ..
        } => {
            insert_reference(report, base_name.as_str(), kind, owner);

            for argument in syntax_trees
                .type_references
                .type_reference_handles(*arguments)
            {
                collect_type_reference(report, syntax_trees, *argument, kind, owner);
            }
        }
        TypeReferenceNode::DynamicTrait { name, .. } | TypeReferenceNode::Named(name) => {
            insert_reference(report, name.as_str(), kind, owner)
        }
        // Const generic arguments name values, not types. Their referenced
        // const symbols substitute during resolution and do not belong in the
        // source-facing type-reference inventory.
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::SelfType
        | TypeReferenceNode::Unit => {}
    }
}

fn collect_constraints(
    report: &mut TypeSurfaceReport,
    syntax_trees: &SyntaxTrees,
    constraints: psi_arena::HandleSpan<TypeConstraintNode>,
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
            // A declared domain (`[u8] in Utf8`) references the domain by name.
            TypeConstraintNode::Domain(name) => {
                insert_reference(
                    report,
                    name.name.as_str(),
                    TypeReferenceUseKind::Constraint,
                    owner,
                );
            }
            TypeConstraintNode::Range { minimum, maximum } => {
                insert_reference(
                    report,
                    &format!(
                        "{}..={}",
                        syntax_trees.expressions.display_name(*minimum),
                        syntax_trees.expressions.display_name(*maximum),
                    ),
                    TypeReferenceUseKind::RangeConstraint,
                    owner,
                );
            }
            // An arithmetic domain is a behaviour tag, not a referenced type name.
            TypeConstraintNode::ArithmeticDomain(_) => {}
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

fn operator_name(
    syntax_trees: &SyntaxTrees,
    name: psi_arena::HandleSpan<psi_syntax_trees::identifier::Identifier>,
) -> String {
    syntax_trees
        .items
        .identifier_path_members(name)
        .iter()
        .map(psi_syntax_trees::identifier::Identifier::as_str)
        .collect::<Vec<_>>()
        .join("::")
}

#[cfg(test)]
mod tests {
    use super::{TypeDeclarationKind, TypeReferenceUseKind, build_type_surface_report};
    use psi_arena::HandleSpan;
    use psi_syntax_trees::SyntaxTrees;
    use psi_syntax_trees::identifier::Identifier;
    use psi_syntax_trees::item::{
        DomainDefinition, Item, Machine, OperatorDefinition, State, StateParameterNode,
        TypeParameter, TypeParameterKind,
    };
    use psi_syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

    #[test]
    fn collects_operator_declarations_and_signature_types() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());

        let name = syntax_trees.items.insert_identifier_path_members([
            Identifier::generated("Slice"),
            Identifier::generated("index"),
        ]);
        let type_parameter = syntax_trees.items.append_type_parameter(TypeParameter {
            name: Identifier::generated("T"),
            kind: TypeParameterKind::Type,
            bounds: Default::default(),
        });
        let generic_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("T")));
        let slice_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Slice {
                element_type: generic_type,
            });
        let borrowed_slice_type =
            syntax_trees
                .type_references
                .insert(TypeReferenceNode::Reference {
                    referee: slice_type,
                    is_mutable: false,
                    lifetime: None,
                });
        let index_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("usize")));
        let items_parameter = syntax_trees
            .items
            .insert_state_parameter_node(StateParameterNode {
                name: Identifier::generated("items"),
                type_reference: borrowed_slice_type,
                is_const: false,
                is_mutable: false,
                is_self: false,
            });
        let items_parameter = syntax_trees
            .items
            .append_state_parameter_handle(items_parameter);
        let index_parameter = syntax_trees
            .items
            .insert_state_parameter_node(StateParameterNode {
                name: Identifier::generated("index"),
                type_reference: index_type,
                is_const: false,
                is_mutable: false,
                is_self: false,
            });
        let _index_parameter = syntax_trees
            .items
            .append_state_parameter_handle(index_parameter);
        syntax_trees.push_root_item(Item::Operator(OperatorDefinition {
            is_boundary: false,
            name,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::from_parts(type_parameter, 1),
            parameters: HandleSpan::from_parts(items_parameter, 2),
            return_type: generic_type,
            contracts: HandleSpan::empty(),
            spelling: None,
            token_count: 1,
        }));

        let report = build_type_surface_report(&syntax_trees);

        assert!(
            report.declarations.iter().any(|(_, declaration)| {
                declaration.name == "Slice::index"
                    && declaration.kind == TypeDeclarationKind::Operator
            }),
            "operator declaration should be recorded"
        );
        assert!(report.references.iter().any(|(_, reference)| {
            reference.name == "usize" && reference.kind == TypeReferenceUseKind::Parameter
        }));
        assert!(report.references.iter().any(|(_, reference)| {
            reference.name == "T" && reference.kind == TypeReferenceUseKind::ReturnType
        }));
    }

    #[test]
    fn collects_domain_declarations_and_target_types() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let target_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("Packet")));
        let operator_name = syntax_trees
            .items
            .insert_identifier_path_members([Identifier::generated("normalize")]);
        let parameter = syntax_trees
            .items
            .insert_state_parameter_node(StateParameterNode {
                name: Identifier::generated("value"),
                type_reference: target_type,
                is_const: false,
                is_mutable: false,
                is_self: false,
            });
        let parameter = syntax_trees.items.append_state_parameter_handle(parameter);
        let operator = syntax_trees.items.append_operator(OperatorDefinition {
            is_boundary: false,
            name: operator_name,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            parameters: HandleSpan::from_parts(parameter, 1),
            return_type: target_type,
            contracts: HandleSpan::empty(),
            spelling: None,
            token_count: 1,
        });

        syntax_trees.push_root_item(Item::Domain(DomainDefinition {
            name: Identifier::generated("NonEmpty"),
            type_parameters: HandleSpan::empty(),
            target_type,
            index_arguments: HandleSpan::empty(),
            is_public: false,
            alias: None,
            authored_routes: Vec::new(),
            classification: None,
            predicate_body: psi_language_semantics::DomainPredicateBody::Bodyless,
            facts: psi_arena::HandleSpan::empty(),
            operators: HandleSpan::from_parts(operator, 1),
            semantic_clause_token_count: 3,
        }));

        let report = build_type_surface_report(&syntax_trees);

        assert!(
            report.declarations.iter().any(|(_, declaration)| {
                declaration.name == "NonEmpty" && declaration.kind == TypeDeclarationKind::Domain
            }),
            "domain declaration should be recorded"
        );
        assert!(
            report.declarations.iter().any(|(_, declaration)| {
                declaration.name == "NonEmpty::normalize"
                    && declaration.kind == TypeDeclarationKind::Operator
            }),
            "domain operator declaration should be recorded"
        );
        assert!(
            report.references.iter().any(|(_, reference)| {
                reference.name == "Packet" && reference.kind == TypeReferenceUseKind::Storage
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
        let minimum =
            syntax_trees
                .expressions
                .insert(psi_syntax_trees::expression::ExpressionNode::Integer(
                    psi_numerics::literals::IntegerLiteral::from_value(0),
                ));
        let maximum =
            syntax_trees
                .expressions
                .insert(psi_syntax_trees::expression::ExpressionNode::Integer(
                    psi_numerics::literals::IntegerLiteral::from_value(100000),
                ));
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
            contracts: HandleSpan::empty(),
            statements: HandleSpan::empty(),
        });
        let state = syntax_trees.items.append_state_handle(state);

        syntax_trees.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("main"),
            attached_data: None,
            target: None,
            boundary: false,
            bodyless: false,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            conformance_bounds: Vec::new(),
            terminates_guarantee: false,
            ranking_subjects: HandleSpan::empty(),
            ranking_view: HandleSpan::empty(),
            ranking_view_arguments: HandleSpan::empty(),
            ranking_range: psi_syntax_trees::expression::ExpressionHandle::invalid(),
            service_reaches: HandleSpan::empty(),
            invokes: HandleSpan::empty(),
            suspends: false,
            blocks: false,
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
                reference.name == "0..=100000"
                    && reference.kind == TypeReferenceUseKind::RangeConstraint
            }),
            "range constraints should be recorded"
        );
        assert_eq!(report.references.len(), 4);
    }
}
