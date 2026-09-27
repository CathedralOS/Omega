//! The proof surface report: every proposition, domain, capability contract
//! and bounded type site a program declares, collected from its syntax trees.

use arena::Arena;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, DataMember, Item, Machine, ProofFact,
};
use syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProofSurfaceReport {
    pub domains: Arena<DomainSurface>,
    pub propositions: Arena<PropositionSurface>,
    pub contracts: Arena<ContractSurface>,
    pub bounded_sites: Arena<BoundedTypeSite>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropositionSurface {
    pub name: String,
    pub binder_count: usize,
    pub parameter_count: usize,
    pub body: PropositionBodySurface,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PropositionBodySurface {
    #[default]
    Primitive,
    Witness,
    Transparent,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainSurface {
    pub name: String,
    pub target_type: String,
    pub predicate_body: language_semantics::DomainPredicateBody,
    pub fact_count: usize,
    pub membership_fact_count: usize,
    pub semantic_clause_token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContractSurface {
    pub owner: String,
    pub kind: ContractKindSurface,
    pub fact_count: usize,
    pub membership_fact_count: usize,
    pub token_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContractKindSurface {
    #[default]
    Requires,
    Ensures,
    Crashes,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundedTypeSite {
    pub owner: String,
    pub base_type: String,
    pub constraints: String,
}

pub fn build_proof_surface_report(syntax_trees: &SyntaxTrees) -> ProofSurfaceReport {
    let mut report = ProofSurfaceReport::default();

    for item in syntax_trees.root_items() {
        match item {
            Item::Capability(_) => {}
            // Consts substitute away at symbol resolution; their literal
            // initializers carry no bounded-type surface of their own.
            Item::Const(_) => {}
            Item::Data(data_definition) => {
                for member in syntax_trees.items.data_members(data_definition.members) {
                    if let DataMember::Field(field) = member {
                        collect_bounded_type_site_tree(
                            &mut report,
                            syntax_trees,
                            field.type_reference,
                            &format!("data `{}` field `{}`", data_definition.name, field.name),
                        );
                    }
                }
            }
            Item::Domain(domain) => {
                report.domains.insert(DomainSurface {
                    name: domain.name.to_string(),
                    target_type: type_reference_name(syntax_trees, domain.target_type),
                    predicate_body: domain.predicate_body,
                    fact_count: syntax_trees.items.proof_facts(domain.facts).len(),
                    membership_fact_count: syntax_trees
                        .items
                        .proof_facts(domain.facts)
                        .iter()
                        .filter(|fact| matches!(fact, ProofFact::Membership(_)))
                        .count(),
                    semantic_clause_token_count: domain.semantic_clause_token_count,
                });
                collect_bounded_type_site(
                    &mut report,
                    syntax_trees,
                    domain.target_type,
                    &format!("domain `{}` target type", domain.name),
                );
                for operator in syntax_trees.items.operators(domain.operators) {
                    collect_operator(
                        &mut report,
                        syntax_trees,
                        operator,
                        &format!(
                            "domain `{}` operator `{}`",
                            domain.name,
                            operator_name(syntax_trees, operator.name)
                        ),
                    );
                }
            }
            Item::Use(_)
            | Item::Measure(_)
            | Item::Conformance(_)
            | Item::Module(_)
            | Item::Package(_) => {}
            Item::Operator(operator) => {
                collect_operator(
                    &mut report,
                    syntax_trees,
                    operator,
                    &format!("operator `{}`", operator_name(syntax_trees, operator.name)),
                );
            }
            Item::Proposition(proposition) => {
                let body = match proposition.body {
                    syntax_trees::item::PropositionBody::Primitive => {
                        PropositionBodySurface::Primitive
                    }
                    syntax_trees::item::PropositionBody::Witness { evidence } => {
                        collect_bounded_type_site_tree(
                            &mut report,
                            syntax_trees,
                            evidence,
                            &format!("proposition `{}` evidence", proposition.name),
                        );
                        PropositionBodySurface::Witness
                    }
                    syntax_trees::item::PropositionBody::Transparent { .. } => {
                        PropositionBodySurface::Transparent
                    }
                };
                for parameter in syntax_trees.items.state_parameters(proposition.parameters) {
                    let parameter = syntax_trees.items.state_parameter(*parameter);
                    collect_bounded_type_site_tree(
                        &mut report,
                        syntax_trees,
                        parameter.type_reference,
                        &format!(
                            "proposition `{}` parameter `{}`",
                            proposition.name, parameter.name
                        ),
                    );
                }
                report.propositions.insert(PropositionSurface {
                    name: proposition.name.to_string(),
                    binder_count: proposition.type_parameters.len(),
                    parameter_count: proposition.parameters.len(),
                    body,
                });
            }
            Item::Machine(machine) => collect_machine(&mut report, syntax_trees, machine),
            Item::Trait(trait_definition) => {
                collect_trait_definition(&mut report, syntax_trees, trait_definition)
            }
        }
    }

    report
}

fn collect_machine(report: &mut ProofSurfaceReport, syntax_trees: &SyntaxTrees, machine: &Machine) {
    collect_contracts(
        report,
        syntax_trees,
        machine.contracts,
        &format!("machine `{}`", machine.name),
    );

    for state in syntax_trees.items.state_handles(machine.states) {
        let state = syntax_trees.items.state(*state);
        collect_state_node(report, syntax_trees, machine, state);
    }
}

fn collect_operator(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    operator: &syntax_trees::item::OperatorDefinition,
    owner: &str,
) {
    collect_contracts(report, syntax_trees, operator.contracts, owner);
    collect_signature_parts(
        report,
        syntax_trees,
        operator.parameters,
        operator.return_type,
        owner,
    );
}

fn collect_state_node(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    machine: &Machine,
    state: &syntax_trees::item::StateNode,
) {
    collect_contracts(
        report,
        syntax_trees,
        state.contracts,
        &format!("machine `{}` state `{}`", machine.name, state.name),
    );
    collect_signature_parts(
        report,
        syntax_trees,
        state.parameters,
        state.return_type,
        &format!("machine `{}` state `{}`", machine.name, state.name),
    );

    for statement in syntax_trees.items.statements(state.statements) {
        let syntax_trees::statement::StatementNode::LocalData(local_data) =
            syntax_trees.statements.statement(*statement)
        else {
            continue;
        };

        collect_bounded_type_site(
            report,
            syntax_trees,
            local_data.type_reference,
            &format!(
                "machine `{}` state `{}` local `{}`",
                machine.name, state.name, local_data.name
            ),
        );
    }
}

fn collect_trait_definition(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    trait_definition: &syntax_trees::item::TraitDefinition,
) {
    for machine in syntax_trees
        .items
        .state_signatures(trait_definition.machines)
    {
        let machine = syntax_trees.items.state_signature(*machine);
        collect_state_signature_node(
            report,
            syntax_trees,
            machine,
            &format!(
                "trait `{}` machine `{}`",
                trait_definition.name, machine.name
            ),
        );
    }
}

fn collect_state_signature_node(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    state: &syntax_trees::item::StateSignatureNode,
    owner: &str,
) {
    collect_contracts(report, syntax_trees, state.contracts, owner);
    collect_signature_parts(
        report,
        syntax_trees,
        state.parameters,
        state.return_type,
        owner,
    );
}

fn collect_signature_parts(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    parameters: arena::HandleSpan<syntax_trees::item::StateParameterHandle>,
    return_type: TypeReferenceHandle,
    owner: &str,
) {
    for parameter in syntax_trees.items.state_parameters(parameters) {
        let parameter = syntax_trees.items.state_parameter(*parameter);
        collect_bounded_type_site(
            report,
            syntax_trees,
            parameter.type_reference,
            &format!("{owner} parameter `{}`", parameter.name),
        );
    }

    if return_type.is_valid() {
        collect_bounded_type_site(
            report,
            syntax_trees,
            return_type,
            &format!("{owner} return type"),
        );
    }
}

fn collect_contracts(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    contracts: arena::HandleSpan<CapabilityContract>,
    owner: &str,
) {
    for contract in syntax_trees.items.capability_contracts(contracts) {
        report.contracts.insert(ContractSurface {
            owner: owner.to_owned(),
            kind: match &contract.kind {
                CapabilityContractKind::Requires => ContractKindSurface::Requires,
                CapabilityContractKind::Ensures => ContractKindSurface::Ensures,
                CapabilityContractKind::EnsuresForResultCase { .. } => ContractKindSurface::Ensures,
                CapabilityContractKind::Crashes { .. } => ContractKindSurface::Crashes,
            },
            fact_count: syntax_trees.items.proof_facts(contract.facts).len(),
            membership_fact_count: syntax_trees
                .items
                .proof_facts(contract.facts)
                .iter()
                .filter(|fact| matches!(fact, ProofFact::Membership(_)))
                .count(),
            token_count: contract.token_count,
        });
    }
}

fn collect_bounded_type_site(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    owner: &str,
) {
    match syntax_trees.type_references.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_bounded_type_site(report, syntax_trees, *referee, owner);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            report.bounded_sites.insert(BoundedTypeSite {
                owner: owner.to_owned(),
                base_type: type_reference_name(syntax_trees, *base_type),
                constraints: constraint_handle_name(syntax_trees, *constraints),
            });
            collect_bounded_type_site(report, syntax_trees, *base_type, owner);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_bounded_type_site(report, syntax_trees, *element_type, owner);
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_bounded_type_site(report, syntax_trees, *element_type, owner);
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in syntax_trees
                .type_references
                .type_reference_handles(*arguments)
            {
                collect_bounded_type_site(report, syntax_trees, *argument, owner);
            }
        }
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::DynamicTrait { .. } => {}
        TypeReferenceNode::Named(_) | TypeReferenceNode::SelfType | TypeReferenceNode::Unit => {}
    }
}

fn collect_bounded_type_site_tree(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    type_reference: TypeReferenceHandle,
    owner: &str,
) {
    match syntax_trees.type_references.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_bounded_type_site_tree(report, syntax_trees, *referee, owner);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            report.bounded_sites.insert(BoundedTypeSite {
                owner: owner.to_owned(),
                base_type: type_reference_name(syntax_trees, *base_type),
                constraints: constraint_handle_name(syntax_trees, *constraints),
            });
            collect_bounded_type_site_tree(report, syntax_trees, *base_type, owner);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            collect_bounded_type_site_tree(report, syntax_trees, *element_type, owner);
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_bounded_type_site_tree(report, syntax_trees, *element_type, owner);
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in syntax_trees
                .type_references
                .type_reference_handles(*arguments)
            {
                collect_bounded_type_site_tree(report, syntax_trees, *argument, owner);
            }
        }
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::DynamicTrait { .. } => {}
        TypeReferenceNode::Named(_) | TypeReferenceNode::SelfType | TypeReferenceNode::Unit => {}
    }
}

fn type_reference_name(syntax_trees: &SyntaxTrees, type_reference: TypeReferenceHandle) -> String {
    match syntax_trees.type_references.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee, access, ..
        } => {
            let qualifier = match access {
                language_semantics::ReferenceAccess::Shared => "",
                language_semantics::ReferenceAccess::Mutable => "mut ",
                language_semantics::ReferenceAccess::WriteOnly => "write ",
            };
            format!(
                "&{qualifier}{}",
                type_reference_name(syntax_trees, *referee)
            )
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_name(syntax_trees, *base_type)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            format!(
                "[{}; {length}]",
                type_reference_name(syntax_trees, *element_type)
            )
        }
        TypeReferenceNode::Slice { element_type } => {
            format!("[{}]", type_reference_name(syntax_trees, *element_type))
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let arguments = syntax_trees
                .type_references
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| type_reference_name(syntax_trees, *argument))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{base_name}<{arguments}>")
        }
        TypeReferenceNode::ConstExpression(expression) => {
            format!(
                "const {}",
                syntax_trees.expressions.display_name(*expression)
            )
        }
        TypeReferenceNode::DynamicTrait { name, conformance } => conformance
            .as_ref()
            .map(|selection| format!("dyn {name}::{selection}"))
            .unwrap_or_else(|| format!("dyn {name}")),
        TypeReferenceNode::Named(name) => name.to_string(),
        TypeReferenceNode::SelfType => "Self".to_owned(),
        TypeReferenceNode::Unit => "()".to_owned(),
    }
}

fn constraint_handle_name(
    syntax_trees: &SyntaxTrees,
    constraints: arena::HandleSpan<TypeConstraintNode>,
) -> String {
    let constraints = syntax_trees.type_references.constraints(constraints);
    if constraints.is_empty() {
        return "[]".to_owned();
    }

    let mut output = String::new();
    output.push('[');

    for (index, constraint) in constraints.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }

        output.push_str(&match constraint {
            TypeConstraintNode::Named(name) => name.to_string(),
            TypeConstraintNode::Domain(name) => format!("in {name}"),
            TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } => format!(
                "{}..{}{}",
                syntax_trees
                    .expressions
                    .expression(*minimum)
                    .display_name(&syntax_trees.expressions),
                if *end_inclusive { "=" } else { "" },
                syntax_trees
                    .expressions
                    .expression(*maximum)
                    .display_name(&syntax_trees.expressions)
            ),
            TypeConstraintNode::ArithmeticDomain(domain) => domain.name().to_owned(),
        });
    }

    output.push(']');
    output
}

fn operator_name(
    syntax_trees: &SyntaxTrees,
    name: arena::HandleSpan<syntax_trees::identifier::Identifier>,
) -> String {
    syntax_trees
        .items
        .identifier_path_members(name)
        .iter()
        .map(syntax_trees::identifier::Identifier::as_str)
        .collect::<Vec<_>>()
        .join("::")
}
