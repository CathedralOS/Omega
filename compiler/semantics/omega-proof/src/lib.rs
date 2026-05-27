//! Proof surface collection, proof obligation building, and invariant checking.

use omega_core::arena::Arena;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, DataMember, Item, Machine, Platform, ProofFact,
    StateSignature,
};
use omega_syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub mod checker;
pub mod obligations;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProofSurfaceReport {
    pub invariants: Arena<InvariantSurface>,
    pub domains: Arena<DomainSurface>,
    pub contracts: Arena<ContractSurface>,
    pub bounded_sites: Arena<BoundedTypeSite>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvariantSurface {
    pub name: String,
    pub constraints: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainSurface {
    pub name: String,
    pub target_type: String,
    pub fact_count: usize,
    pub membership_fact_count: usize,
    pub body_token_count: usize,
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
    Trusted,
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
                    fact_count: syntax_trees.items.proof_facts(domain.facts).len(),
                    membership_fact_count: syntax_trees
                        .items
                        .proof_facts(domain.facts)
                        .iter()
                        .filter(|fact| matches!(fact, ProofFact::Membership(_)))
                        .count(),
                    body_token_count: domain.body_token_count,
                });
                collect_bounded_type_site(
                    &mut report,
                    syntax_trees,
                    domain.target_type,
                    &format!("domain `{}` target type", domain.name),
                );
            }
            Item::Invariant(invariant) => {
                report.invariants.insert(InvariantSurface {
                    name: invariant.name.to_string(),
                    constraints: constraint_handle_name(syntax_trees, invariant.constraints),
                });
            }
            Item::Library(library) => {
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
            Item::Use(_) | Item::Export(_) | Item::Operator(_) => {}
            Item::Machine(machine) => collect_machine(&mut report, syntax_trees, machine),
            Item::Platform(platform) => collect_platform(&mut report, syntax_trees, platform),
            Item::Trait(trait_definition) => {
                collect_trait_definition(&mut report, syntax_trees, trait_definition)
            }
            Item::Target(_) | Item::TrustDefinition(_) => {}
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

fn collect_state_node(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    machine: &Machine,
    state: &omega_syntax_trees::item::StateNode,
) {
    collect_signature_parts(
        report,
        syntax_trees,
        state.parameters,
        state.return_type,
        &format!("machine `{}` state `{}`", machine.name, state.name),
    );

    for statement in syntax_trees.items.statements(state.statements) {
        let omega_syntax_trees::statement::StatementNode::LocalData(local_data) =
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

fn collect_platform(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    platform: &Platform,
) {
    for state in syntax_trees.items.state_signatures(platform.states) {
        let state = syntax_trees.items.state_signature(*state);
        collect_state_signature_node(
            report,
            syntax_trees,
            state,
            &format!("platform `{}` state `{}`", platform.name, state.name),
        );
    }
}

fn collect_trait_definition(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    trait_definition: &omega_syntax_trees::item::TraitDefinition,
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

fn collect_state_signature(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    state: &StateSignature,
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

fn collect_state_signature_node(
    report: &mut ProofSurfaceReport,
    syntax_trees: &SyntaxTrees,
    state: &omega_syntax_trees::item::StateSignatureNode,
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
    parameters: omega_core::arena::HandleSpan<omega_syntax_trees::item::StateParameterHandle>,
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
    contracts: omega_core::arena::HandleSpan<CapabilityContract>,
    owner: &str,
) {
    for contract in syntax_trees.items.capability_contracts(contracts) {
        report.contracts.insert(ContractSurface {
            owner: owner.to_owned(),
            kind: match contract.kind {
                CapabilityContractKind::Requires => ContractKindSurface::Requires,
                CapabilityContractKind::Ensures => ContractKindSurface::Ensures,
                CapabilityContractKind::Trusted(_) => ContractKindSurface::Trusted,
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
        TypeReferenceNode::Named(_) | TypeReferenceNode::SelfType | TypeReferenceNode::Unit => {}
    }
}

fn type_reference_name(syntax_trees: &SyntaxTrees, type_reference: TypeReferenceHandle) -> String {
    match syntax_trees.type_references.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            is_mutable,
        } => {
            let qualifier = if *is_mutable { "mut " } else { "" };
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
        TypeReferenceNode::Named(name) => name.to_string(),
        TypeReferenceNode::SelfType => "Self".to_owned(),
        TypeReferenceNode::Unit => "()".to_owned(),
    }
}

fn constraint_handle_name(
    syntax_trees: &SyntaxTrees,
    constraints: omega_core::arena::HandleSpan<TypeConstraintNode>,
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
            TypeConstraintNode::Range { minimum, maximum } => format!(
                "range<{}, {}>",
                syntax_trees
                    .expressions
                    .expression(*minimum)
                    .display_name(&syntax_trees.expressions),
                syntax_trees
                    .expressions
                    .expression(*maximum)
                    .display_name(&syntax_trees.expressions)
            ),
        });
    }

    output.push(']');
    output
}

#[cfg(test)]
mod tests {
    use omega_core::arena::HandleSpan;
    use omega_syntax_trees::SyntaxTrees;
    use omega_syntax_trees::identifier::Identifier;
    use omega_syntax_trees::item::{
        CapabilityContract, CapabilityContractKind, DomainDefinition, InvariantDefinition, Item,
        Machine, State, StateParameterNode,
    };
    use omega_syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

    use super::build_proof_surface_report;

    #[test]
    fn collects_domain_surface() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let target_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("String")));

        syntax_trees.push_root_item(Item::Domain(DomainDefinition {
            name: Identifier::generated("NonEmpty"),
            target_type,
            facts: HandleSpan::empty(),
            operators: HandleSpan::empty(),
            body_token_count: 3,
        }));

        let report = build_proof_surface_report(&syntax_trees);

        assert_eq!(report.domains.len(), 1);
        let (_, domain) = report.domains.iter().next().expect("domain surface");
        assert_eq!(domain.name, "NonEmpty");
        assert_eq!(domain.target_type, "String");
        assert_eq!(domain.fact_count, 0);
        assert_eq!(domain.membership_fact_count, 0);
        assert_eq!(domain.body_token_count, 3);
    }

    #[test]
    fn collects_invariants_and_bounded_type_sites() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let base_type = syntax_trees
            .type_references
            .insert_named(Identifier::generated("f32"));
        let constraint = syntax_trees
            .type_references
            .append_constraint(TypeConstraintNode::Named(Identifier::generated(
                "speed_range",
            )));
        let parameter_type = syntax_trees
            .type_references
            .insert_constrained(base_type, HandleSpan::from_parts(constraint, 1));
        let parameter = syntax_trees
            .items
            .insert_state_parameter_node(StateParameterNode {
                name: Identifier::generated("speed"),
                type_reference: parameter_type,
                is_const: false,
                is_mutable: false,
                is_self: false,
            });
        let parameter_handle = syntax_trees.items.append_state_parameter_handle(parameter);
        let state = syntax_trees.items.insert_state(&State {
            name: Identifier::generated("entry"),
            parameters: HandleSpan::from_parts(parameter_handle, 1),
            return_type: omega_syntax_trees::types::TypeReferenceHandle::invalid(),
            statements: HandleSpan::empty(),
        });
        let state_handle = syntax_trees.items.append_state_handle(state);
        let constraint = syntax_trees
            .type_references
            .append_constraint(TypeConstraintNode::Named(Identifier::generated("finite")));

        syntax_trees.push_root_item(Item::Invariant(InvariantDefinition {
            name: Identifier::generated("speed_range"),
            constraints: HandleSpan::from_parts(constraint, 1),
        }));
        syntax_trees.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("main"),
            attached_data: None,
            satisfies: HandleSpan::empty(),
            effects: HandleSpan::empty(),
            contracts: HandleSpan::empty(),
            states: HandleSpan::from_parts(state_handle, 1),
        }));

        let report = build_proof_surface_report(&syntax_trees);

        assert_eq!(report.invariants.len(), 1);
        assert_eq!(report.bounded_sites.len(), 1);

        let (_, bounded_site) = report
            .bounded_sites
            .iter()
            .next()
            .expect("bounded site should be collected");
        assert_eq!(bounded_site.base_type, "f32");
        assert_eq!(bounded_site.constraints, "[speed_range]");
    }

    #[test]
    fn collects_machine_contract_surface() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let requires = syntax_trees
            .items
            .append_capability_contract(CapabilityContract {
                kind: CapabilityContractKind::Requires,
                facts: HandleSpan::empty(),
                token_count: 3,
            });
        let ensures = syntax_trees
            .items
            .append_capability_contract(CapabilityContract {
                kind: CapabilityContractKind::Ensures,
                facts: HandleSpan::empty(),
                token_count: 3,
            });

        syntax_trees.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("distinct_indices"),
            attached_data: None,
            satisfies: HandleSpan::empty(),
            effects: HandleSpan::empty(),
            contracts: HandleSpan::from_parts(
                requires,
                ensures
                    .arena_index()
                    .checked_sub(requires.arena_index())
                    .expect("contracts should be contiguous")
                    + 1,
            ),
            states: HandleSpan::empty(),
        }));

        let report = build_proof_surface_report(&syntax_trees);

        assert_eq!(report.contracts.len(), 2);
        let contracts = report
            .contracts
            .iter()
            .map(|(_, contract)| contract.kind)
            .collect::<Vec<_>>();
        assert_eq!(
            contracts,
            vec![
                super::ContractKindSurface::Requires,
                super::ContractKindSurface::Ensures
            ]
        );
    }
}
