//! Proof surface collection, proof obligation building, and invariant checking.

use psi_arena::Arena;
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::item::{
    CapabilityContract, CapabilityContractKind, DataMember, Item, Machine, ProofFact,
    StateSignature,
};
use psi_syntax_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub mod boundary;
pub mod checker;
pub mod lemmas;
pub mod obligations;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProofSurfaceReport {
    pub invariants: Arena<InvariantSurface>,
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
pub struct InvariantSurface {
    pub name: String,
    pub constraints: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DomainSurface {
    pub name: String,
    pub target_type: String,
    pub predicate_body: psi_language_semantics::DomainPredicateBody,
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
    Boundary,
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
            Item::WireData(_)
            | Item::Use(_)
            | Item::Export(_)
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
                    psi_syntax_trees::item::PropositionBody::Primitive => {
                        PropositionBodySurface::Primitive
                    }
                    psi_syntax_trees::item::PropositionBody::Witness { evidence } => {
                        collect_bounded_type_site_tree(
                            &mut report,
                            syntax_trees,
                            evidence,
                            &format!("proposition `{}` evidence", proposition.name),
                        );
                        PropositionBodySurface::Witness
                    }
                    psi_syntax_trees::item::PropositionBody::Transparent { .. } => {
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
            Item::Target(_) => {}
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
    operator: &psi_syntax_trees::item::OperatorDefinition,
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
    state: &psi_syntax_trees::item::StateNode,
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
        let psi_syntax_trees::statement::StatementNode::LocalData(local_data) =
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
    trait_definition: &psi_syntax_trees::item::TraitDefinition,
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
    state: &psi_syntax_trees::item::StateSignatureNode,
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
    parameters: psi_arena::HandleSpan<psi_syntax_trees::item::StateParameterHandle>,
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
    contracts: psi_arena::HandleSpan<CapabilityContract>,
    owner: &str,
) {
    for contract in syntax_trees.items.capability_contracts(contracts) {
        report.contracts.insert(ContractSurface {
            owner: owner.to_owned(),
            kind: match &contract.kind {
                CapabilityContractKind::Requires => ContractKindSurface::Requires,
                CapabilityContractKind::Ensures => ContractKindSurface::Ensures,
                CapabilityContractKind::Boundary(_) => ContractKindSurface::Boundary,
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
                psi_language_semantics::ReferenceAccess::Shared => "",
                psi_language_semantics::ReferenceAccess::Mutable => "mut ",
                psi_language_semantics::ReferenceAccess::WriteOnly => "write ",
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
    constraints: psi_arena::HandleSpan<TypeConstraintNode>,
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
            TypeConstraintNode::Range { minimum, maximum } => format!(
                "{}..={}",
                syntax_trees
                    .expressions
                    .expression(*minimum)
                    .display_name(&syntax_trees.expressions),
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
    use psi_arena::HandleSpan;
    use psi_syntax_trees::SyntaxTrees;
    use psi_syntax_trees::identifier::Identifier;
    use psi_syntax_trees::item::{
        CapabilityContract, CapabilityContractKind, DomainDefinition, InvariantDefinition, Item,
        Machine, OperatorDefinition, State, StateParameterNode,
    };
    use psi_syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

    use super::build_proof_surface_report;

    #[test]
    fn collects_domain_surface() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let target_type = syntax_trees
            .type_references
            .insert(TypeReferenceNode::Named(Identifier::generated("String")));

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
            facts: HandleSpan::empty(),
            operators: HandleSpan::empty(),
            semantic_clause_token_count: 3,
        }));

        let report = build_proof_surface_report(&syntax_trees);

        assert_eq!(report.domains.len(), 1);
        let (_, domain) = report.domains.iter().next().expect("domain surface");
        assert_eq!(domain.name, "NonEmpty");
        assert_eq!(domain.target_type, "String");
        assert_eq!(
            domain.predicate_body,
            psi_language_semantics::DomainPredicateBody::Bodyless
        );
        assert_eq!(domain.fact_count, 0);
        assert_eq!(domain.membership_fact_count, 0);
        assert_eq!(domain.semantic_clause_token_count, 3);
    }

    #[test]
    fn collects_operator_contract_surface() {
        let mut syntax_trees = SyntaxTrees::new(Default::default());
        let operator_name = syntax_trees.items.insert_identifier_path_members([
            Identifier::generated("Slice"),
            Identifier::generated("index"),
        ]);
        let requires = syntax_trees
            .items
            .append_capability_contract(CapabilityContract {
                kind: CapabilityContractKind::Requires,
                binding: None,
                facts: HandleSpan::empty(),
                token_count: 3,
            });
        let ensures = syntax_trees
            .items
            .append_capability_contract(CapabilityContract {
                kind: CapabilityContractKind::Ensures,
                binding: None,
                facts: HandleSpan::empty(),
                token_count: 3,
            });
        let return_type = syntax_trees
            .type_references
            .insert_named(Identifier::generated("T"));
        syntax_trees.push_root_item(Item::Operator(OperatorDefinition {
            is_boundary: false,
            name: operator_name,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            parameters: HandleSpan::empty(),
            return_type,
            contracts: HandleSpan::from_parts(
                requires,
                ensures
                    .arena_index()
                    .checked_sub(requires.arena_index())
                    .expect("contracts should be contiguous")
                    + 1,
            ),
            spelling: None,
            token_count: 1,
        }));

        let report = build_proof_surface_report(&syntax_trees);

        assert_eq!(report.contracts.len(), 2);
        assert!(
            report
                .contracts
                .iter()
                .all(|(_, contract)| { contract.owner == "operator `Slice::index`" })
        );
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
            return_type: psi_syntax_trees::types::TypeReferenceHandle::invalid(),
            contracts: HandleSpan::empty(),
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
            service_reach_is_installation_bound: false,
            service_reaches: HandleSpan::empty(),
            invokes: HandleSpan::empty(),
            suspends: false,
            blocks: false,
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
                binding: None,
                facts: HandleSpan::empty(),
                token_count: 3,
            });
        let ensures = syntax_trees
            .items
            .append_capability_contract(CapabilityContract {
                kind: CapabilityContractKind::Ensures,
                binding: None,
                facts: HandleSpan::empty(),
                token_count: 3,
            });

        syntax_trees.push_root_item(Item::Machine(Machine {
            name: Identifier::generated("distinct_indices"),
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
            service_reach_is_installation_bound: false,
            service_reaches: HandleSpan::empty(),
            invokes: HandleSpan::empty(),
            suspends: false,
            blocks: false,
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
