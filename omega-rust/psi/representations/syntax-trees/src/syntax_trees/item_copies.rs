//! Copying items between syntax trees: data, operators, measures, machines,
//! traits, capabilities and their member spans.

use crate::item::{
    CapabilityContract, CapabilityContractKind, CapabilityDefinition, CapabilityField,
    CapabilityMember, CapabilityState, DataDefinition, DataField, DataMember, DataVariant,
    DomainDefinition, Item, Machine, MathematicalDefinition, MathematicalDefinitionBody,
    MathematicalParameterNode, MathematicalTypeHandle, MathematicalTypeNode, MeasureDefinition,
    OperatorDefinition, ProofFact, ProofMembershipFact, StateParameterHandle, StateParameterNode,
    TraitDefinition, TypeParameter, UseItem,
};
use crate::syntax_trees::SyntaxTrees;
use arena::{Handle, HandleSpan};

impl SyntaxTrees {
    /// Deep-copy a single item from `other` into this tree's tables (the
    /// generic-instance desugar clones attached machines from a snapshot of
    /// the tree being extended). Returns the copied item; the caller pushes
    /// it as a root item after any post-copy rewrites.
    pub fn copy_item_from(&mut self, other: &SyntaxTrees, item: &Item) -> Item {
        self.copy_item(other, item)
    }

    /// Deep-copy one expression from a snapshot of this tree. Generic-instance
    /// synthesis rebuilds a template's constrained-type range endpoints and
    /// open domain-index expressions through this copy so the instance's
    /// const-binder rewrite lands on its own subtree, never on the template's
    /// shared nodes.
    pub fn copy_expression_from(
        &mut self,
        other: &SyntaxTrees,
        handle: crate::expression::ExpressionHandle,
    ) -> crate::expression::ExpressionHandle {
        self.copy_expression_handle(other, handle)
    }

    /// Deep-copy one proof fact from another syntax tree. Generic-instance
    /// synthesis uses this to retain field-dependent default-domain facts
    /// while discharging facts that depend only on concrete const arguments.
    pub fn copy_proof_fact_from(
        &mut self,
        other: &SyntaxTrees,
        source: Handle<ProofFact>,
    ) -> Handle<ProofFact> {
        let fact = other.items.proof_fact(source);
        let copied = match fact {
            ProofFact::Expression(expression) => {
                ProofFact::Expression(self.copy_expression_handle(other, *expression))
            }
            ProofFact::Membership(membership) => ProofFact::Membership(ProofMembershipFact {
                value: self.copy_expression_handle(other, membership.value),
                domain: self.copy_item_identifier_span(other, membership.domain),
                domain_arguments: self
                    .copy_type_reference_handle_span(other, membership.domain_arguments),
            }),
        };
        let copied = self.items.append_proof_fact(copied);
        if let Some(source_span) = other.items.proof_fact_source_span(source) {
            self.items.set_proof_fact_source_span(copied, source_span);
        }
        copied
    }

    /// Deep-copy one trait signature and its authored default body from a
    /// snapshot. Pre-resolution generic-default synthesis uses this to keep
    /// substitutions isolated from the source trait template.
    pub fn copy_state_signature_node_from(
        &mut self,
        other: &SyntaxTrees,
        signature: &crate::item::StateSignatureNode,
    ) -> crate::item::StateSignatureNode {
        let copied = self.copy_state_signature_node(other, signature);
        crate::item::StateSignatureNode {
            name: copied.name,
            spelling: copied.spelling,
            lifetime_parameters: copied.lifetime_parameters,
            type_parameters: copied.type_parameters,
            is_default: copied.is_default,
            parameters: copied.parameters,
            native_callback_parameters: copied.native_callback_parameters,
            return_type: copied.return_type,
            service_reach_is_installation_bound: copied.service_reach_is_installation_bound,
            service_reach_keyword_source_spans: copied.service_reach_keyword_source_spans,
            service_reaches: copied.service_reaches,
            invokes: copied.invokes,
            suspends_keyword_source_spans: copied.suspends_keyword_source_spans,
            blocks_keyword_source_spans: copied.blocks_keyword_source_spans,
            suspends: copied.suspends,
            blocks: copied.blocks,
            contracts: copied.contracts,
            default_body: copied.default_body,
            terminates_guarantee: copied.terminates_guarantee,
            where_facts: copied.where_facts,
        }
    }

    /// Deep-copy one top-level `let`/`boundary let` declaration from another
    /// syntax tree. Mirrors the item copies so `extend_from` keeps the exact
    /// telescope, arrow structure and body term the parser recorded.
    pub(crate) fn copy_mathematical_definition(
        &mut self,
        other: &SyntaxTrees,
        definition: &MathematicalDefinition,
    ) -> MathematicalDefinition {
        MathematicalDefinition {
            name: definition.name.clone(),
            is_public: definition.is_public,
            type_parameters: self.copy_type_parameter_span(other, definition.type_parameters),
            parameters: self.copy_mapped_span(
                other
                    .items
                    .mathematical_parameters(definition.parameters)
                    .to_vec(),
                |this, handle| {
                    let parameter = other.items.mathematical_parameter(handle);
                    let node = MathematicalParameterNode {
                        name: parameter.name.clone(),
                        relevance: parameter.relevance,
                        ty: this.copy_mathematical_type(other, parameter.ty),
                    };
                    this.items.insert_mathematical_parameter(node)
                },
                |this, handle| this.items.append_mathematical_parameter_handle(handle),
            ),
            result: self.copy_mathematical_type(other, definition.result),
            body: match definition.body {
                MathematicalDefinitionBody::Definition(term) => {
                    MathematicalDefinitionBody::Definition(self.copy_expression_handle(other, term))
                }
                MathematicalDefinitionBody::Assumption => MathematicalDefinitionBody::Assumption,
            },
        }
    }

    fn copy_mathematical_type(
        &mut self,
        other: &SyntaxTrees,
        handle: MathematicalTypeHandle,
    ) -> MathematicalTypeHandle {
        let node = match other.items.mathematical_type(handle) {
            MathematicalTypeNode::Ordinary(type_reference) => MathematicalTypeNode::Ordinary(
                self.copy_type_reference_handle(other, *type_reference),
            ),
            MathematicalTypeNode::Arrow {
                binder,
                domain,
                codomain,
            } => MathematicalTypeNode::Arrow {
                binder: binder.clone(),
                domain: self.copy_mathematical_type(other, *domain),
                codomain: self.copy_mathematical_type(other, *codomain),
            },
            MathematicalTypeNode::Application { callee, arguments } => {
                MathematicalTypeNode::Application {
                    callee: self.copy_mathematical_type(other, *callee),
                    arguments: self.copy_expression_handle_list(other, *arguments),
                }
            }
        };
        self.items.insert_mathematical_type(node)
    }

    pub(crate) fn copy_item(&mut self, other: &SyntaxTrees, item: &Item) -> Item {
        match item {
            Item::Capability(capability) => {
                Item::Capability(self.copy_capability_definition(other, capability))
            }
            Item::Conformance(conformance) => {
                let body = match &conformance.body {
                    crate::item::ConformanceBody::AttachedRequirementMachines => {
                        crate::item::ConformanceBody::AttachedRequirementMachines
                    }
                    crate::item::ConformanceBody::Closed { members } => {
                        let copied = other
                            .items
                            .conformance_members(*members)
                            .iter()
                            .map(|member| match member {
                                crate::item::ConformanceMember::Machine(machine) => {
                                    crate::item::ConformanceMember::Machine(
                                        self.copy_machine(other, machine),
                                    )
                                }
                                crate::item::ConformanceMember::TraitDefault {
                                    declaring_trait,
                                    requirement_ordinal,
                                    machine,
                                } => crate::item::ConformanceMember::TraitDefault {
                                    declaring_trait: declaring_trait.clone(),
                                    requirement_ordinal: *requirement_ordinal,
                                    machine: self.copy_machine(other, machine),
                                },
                                crate::item::ConformanceMember::Reference {
                                    declaring_trait,
                                    requirement,
                                    target,
                                } => crate::item::ConformanceMember::Reference {
                                    declaring_trait: declaring_trait.clone(),
                                    requirement: requirement.clone(),
                                    target: self.copy_item_identifier_span(other, *target),
                                },
                            })
                            .collect::<Vec<_>>();
                        let mut copied_start = arena::Handle::invalid();
                        let mut copied_count = 0u32;
                        for member in copied {
                            let handle = self.items.append_conformance_member(member);
                            if copied_count == 0 {
                                copied_start = handle;
                            }
                            copied_count = copied_count
                                .checked_add(1)
                                .expect("conformance member span count overflow");
                        }
                        crate::item::ConformanceBody::Closed {
                            members: if copied_count == 0 {
                                arena::HandleSpan::empty()
                            } else {
                                arena::HandleSpan::from_parts(copied_start, copied_count)
                            },
                        }
                    }
                };
                Item::Conformance(crate::item::ConformanceItem {
                    is_public: conformance.is_public,
                    lifetime_parameters: conformance.lifetime_parameters.clone(),
                    type_parameters: self
                        .copy_type_parameter_span(other, conformance.type_parameters),
                    subject: conformance.subject.clone(),
                    trait_name: conformance.trait_name.clone(),
                    trait_lifetime_arguments: conformance.trait_lifetime_arguments.clone(),
                    trait_arguments: self
                        .copy_type_reference_handle_span(other, conformance.trait_arguments),
                    alias: conformance.alias.clone(),
                    body,
                })
            }
            Item::Const(constant) => Item::Const(crate::item::ConstDefinition {
                scope: constant.scope.clone(),
                name: constant.name.clone(),
                is_public: constant.is_public,
                type_reference: self.copy_type_reference_handle(other, constant.type_reference),
                value: self.copy_expression_handle(other, constant.value),
                normalization: constant.normalization.as_ref().map(|normalization| {
                    crate::item::ConstInitializerNormalization {
                        authored_expression: self
                            .copy_expression_handle(other, normalization.authored_expression),
                        canonical_result_encoding: normalization.canonical_result_encoding.clone(),
                        selections: normalization.selections.clone(),
                        builtin_operators: normalization.builtin_operators.clone(),
                        call_selections: normalization.call_selections.clone(),
                    }
                }),
            }),
            Item::Data(data) => Item::Data(self.copy_data_definition(other, data)),
            Item::Domain(domain) => Item::Domain(DomainDefinition {
                name: domain.name.clone(),
                type_parameters: self.copy_type_parameter_span(other, domain.type_parameters),
                target_type: self.copy_type_reference_handle(other, domain.target_type),
                index_arguments: self
                    .copy_type_reference_handle_span(other, domain.index_arguments),
                is_public: domain.is_public,
                alias: domain
                    .alias
                    .as_ref()
                    .map(|alias| crate::item::DomainAliasDefinition {
                        constituents: alias
                            .constituents
                            .iter()
                            .map(|constituent| self.copy_item_identifier_span(other, *constituent))
                            .collect(),
                    }),
                authored_routes: domain.authored_routes.clone(),
                classification: domain.classification,
                predicate_body: domain.predicate_body,
                facts: self.copy_domain_fact_span(other, domain.facts),
                operators: self.copy_operator_definition_span(other, domain.operators),
                semantic_clause_token_count: domain.semantic_clause_token_count,
            }),
            Item::Measure(measure) => Item::Measure(self.copy_measure_definition(other, measure)),
            Item::Module(module) => Item::Module(crate::item::ModuleDeclaration {
                path: self.copy_item_identifier_span(other, module.path),
            }),
            Item::Operator(operator) => {
                Item::Operator(self.copy_operator_definition(other, operator))
            }
            Item::Package(package) => Item::Package(crate::item::PackageDeclaration {
                path: self.copy_item_identifier_span(other, package.path),
            }),
            Item::Proposition(proposition) => {
                Item::Proposition(crate::item::PropositionDefinition {
                    name: proposition.name.clone(),
                    is_public: proposition.is_public,
                    type_parameters: self
                        .copy_type_parameter_span(other, proposition.type_parameters),
                    parameters: self
                        .copy_state_parameter_handle_span(other, proposition.parameters),
                    transparent_formula_source_span: proposition.transparent_formula_source_span,
                    body: match proposition.body {
                        crate::item::PropositionBody::Primitive => {
                            crate::item::PropositionBody::Primitive
                        }
                        crate::item::PropositionBody::Witness { evidence } => {
                            crate::item::PropositionBody::Witness {
                                evidence: self.copy_type_reference_handle(other, evidence),
                            }
                        }
                        crate::item::PropositionBody::Transparent { proposition } => {
                            crate::item::PropositionBody::Transparent {
                                proposition: self.copy_expression_handle(other, proposition),
                            }
                        }
                    },
                })
            }
            Item::Use(use_item) => Item::Use(UseItem {
                path: self.copy_item_identifier_span(other, use_item.path),
            }),
            Item::Machine(machine) => Item::Machine(self.copy_machine(other, machine)),
            Item::Trait(trait_definition) => {
                Item::Trait(self.copy_trait_definition(other, trait_definition))
            }
        }
    }

    fn copy_capability_definition(
        &mut self,
        other: &SyntaxTrees,
        capability: &CapabilityDefinition,
    ) -> CapabilityDefinition {
        CapabilityDefinition {
            name: capability.name.clone(),
            members: self.copy_capability_member_span(other, capability.members),
        }
    }

    fn copy_data_definition(
        &mut self,
        other: &SyntaxTrees,
        data: &DataDefinition,
    ) -> DataDefinition {
        DataDefinition {
            name: data.name.clone(),
            is_public: data.is_public,
            supply_mode: data.supply_mode,
            lifetime_parameters: data.lifetime_parameters.clone(),
            type_parameters: self.copy_type_parameter_span(other, data.type_parameters),
            generic_instance: data
                .generic_instance
                .map(|origin| self.copy_type_reference_handle(other, origin)),
            properties: data.properties,
            quotient: data
                .quotient
                .as_ref()
                .map(|quotient| crate::item::QuotientDefinition {
                    carrier: self.copy_type_reference_handle(other, quotient.carrier),
                    relation: self.copy_item_identifier_span(other, quotient.relation),
                    equivalence: quotient.equivalence.as_ref().map(|selection| {
                        crate::item::QuotientEquivalenceSelection {
                            relation: self.copy_item_identifier_span(other, selection.relation),
                            trait_name: selection.trait_name.clone(),
                            trait_arguments: self
                                .copy_type_reference_handle_span(other, selection.trait_arguments),
                            conformance_name: selection.conformance_name.clone(),
                        }
                    }),
                }),
            where_facts: self.copy_domain_fact_span(other, data.where_facts),
            members: self.copy_data_member_span(other, data.members),
        }
    }

    fn copy_operator_definition(
        &mut self,
        other: &SyntaxTrees,
        operator: &OperatorDefinition,
    ) -> OperatorDefinition {
        OperatorDefinition {
            is_public: operator.is_public,
            is_boundary: operator.is_boundary,
            name: self.copy_item_identifier_span(other, operator.name),
            lifetime_parameters: operator.lifetime_parameters.clone(),
            type_parameters: self.copy_type_parameter_span(other, operator.type_parameters),
            parameters: self.copy_state_parameter_handle_span(other, operator.parameters),
            return_type: self.copy_type_reference_handle(other, operator.return_type),
            contracts: self.copy_capability_contract_span(other, operator.contracts),
            spelling: operator.spelling,
            token_count: operator.token_count,
        }
    }

    fn copy_measure_definition(
        &mut self,
        other: &SyntaxTrees,
        measure: &MeasureDefinition,
    ) -> MeasureDefinition {
        let parameter = if measure.parameter.is_valid() {
            let source_parameter = other.items.state_parameter(measure.parameter);
            let type_reference =
                self.copy_type_reference_handle(other, source_parameter.type_reference);
            self.items.insert_state_parameter_node(StateParameterNode {
                name: source_parameter.name.clone(),
                type_reference,
                is_const: source_parameter.is_const,
                is_mutable: source_parameter.is_mutable,
                is_self: source_parameter.is_self,
                relevance: source_parameter.relevance,
            })
        } else {
            StateParameterHandle::invalid()
        };

        MeasureDefinition {
            name: self.copy_item_identifier_span(other, measure.name),
            parameter,
            return_type: self.copy_type_reference_handle(other, measure.return_type),
            lexicographic: measure.lexicographic,
            body: self.copy_expression_handle_list(other, measure.body),
            token_count: measure.token_count,
        }
    }

    fn copy_operator_definition_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<OperatorDefinition>,
    ) -> HandleSpan<OperatorDefinition> {
        self.copy_mapped_span(
            other.items.operators(span),
            |this, operator| this.copy_operator_definition(other, operator),
            |this, operator| this.items.append_operator(operator),
        )
    }

    fn copy_machine(&mut self, other: &SyntaxTrees, machine: &Machine) -> Machine {
        Machine {
            name: machine.name.clone(),
            generic_data_template: machine.generic_data_template.clone(),
            where_facts: self.copy_domain_fact_span(other, machine.where_facts),
            attached_data: machine.attached_data.clone(),
            spelling: machine.spelling,
            is_public: machine.is_public,
            target: machine.target.clone(),
            boundary: machine.boundary,
            is_top_level_boundary_requirement: machine.is_top_level_boundary_requirement,
            bodyless: machine.bodyless,
            lifetime_parameters: machine.lifetime_parameters.clone(),
            type_parameters: self.copy_type_parameter_span(other, machine.type_parameters),
            satisfies: self.copy_mapped_span(
                other.items.satisfies_clauses(machine.satisfies).to_vec(),
                |this, clause| crate::item::SatisfiesClause {
                    trait_name: clause.trait_name,
                    lifetime_arguments: clause.lifetime_arguments,
                    arguments: this.copy_type_reference_handle_span(other, clause.arguments),
                    requirement: clause.requirement,
                    alias: clause.alias,
                    via: clause.via,
                    via_expression: if clause.via_expression.is_valid() {
                        this.copy_expression_handle(other, clause.via_expression)
                    } else {
                        crate::expression::ExpressionHandle::invalid()
                    },
                    via_keyword_source_span: clause.via_keyword_source_span,
                },
                |this, clause| this.items.append_satisfies_clause(clause),
            ),
            conformance_bounds: machine
                .conformance_bounds
                .iter()
                .map(|bound| crate::item::GenericConformanceBound {
                    binder: bound.binder.clone(),
                    subject: bound.subject.clone(),
                    carrier: bound.carrier.clone(),
                    arguments: self.copy_type_reference_handle_span(other, bound.arguments),
                    selected_conformance: bound
                        .selected_conformance
                        .as_ref()
                        .map(|argument| self.copy_static_machine_argument(other, argument)),
                })
                .collect(),
            terminates_guarantee: machine.terminates_guarantee,
            ranking_subjects: self.copy_expression_handle_list(other, machine.ranking_subjects),
            ranking_view: self.copy_item_identifier_span(other, machine.ranking_view),
            ranking_view_arguments: self
                .copy_expression_handle_list(other, machine.ranking_view_arguments),
            ranking_range: if machine.ranking_range.is_valid() {
                self.copy_expression_handle(other, machine.ranking_range)
            } else {
                crate::expression::ExpressionHandle::invalid()
            },
            service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
            service_reach_keyword_source_spans: machine.service_reach_keyword_source_spans.clone(),
            service_reaches: self.copy_item_identifier_span(other, machine.service_reaches),
            invokes: self.copy_item_identifier_span(other, machine.invokes),
            suspends_keyword_source_spans: machine.suspends_keyword_source_spans.clone(),
            blocks_keyword_source_spans: machine.blocks_keyword_source_spans.clone(),
            suspends: machine.suspends,
            blocks: machine.blocks,
            contracts: self.copy_capability_contract_span(other, machine.contracts),
            states: self.copy_state_handle_span(other, machine.states),
        }
    }

    fn copy_trait_definition(
        &mut self,
        other: &SyntaxTrees,
        trait_definition: &TraitDefinition,
    ) -> TraitDefinition {
        TraitDefinition {
            is_boundary: trait_definition.is_boundary,
            is_public: trait_definition.is_public,
            name: trait_definition.name.clone(),
            lifetime_parameters: trait_definition.lifetime_parameters.clone(),
            type_parameters: self.copy_type_parameter_span(other, trait_definition.type_parameters),
            conformance_bounds: trait_definition
                .conformance_bounds
                .iter()
                .map(|bound| crate::item::GenericConformanceBound {
                    binder: bound.binder.clone(),
                    subject: bound.subject.clone(),
                    carrier: bound.carrier.clone(),
                    arguments: self.copy_type_reference_handle_span(other, bound.arguments),
                    selected_conformance: bound
                        .selected_conformance
                        .as_ref()
                        .map(|argument| self.copy_static_machine_argument(other, argument)),
                })
                .collect(),
            parents: self.copy_type_reference_handle_span(other, trait_definition.parents),
            requires: self.copy_item_identifier_span(other, trait_definition.requires),
            machines: self.copy_state_signature_handle_span(other, trait_definition.machines),
        }
    }

    pub(crate) fn copy_type_parameter_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<TypeParameter>,
    ) -> HandleSpan<TypeParameter> {
        self.copy_mapped_span(
            other.items.type_parameters(span).to_vec(),
            |this, parameter| {
                let kind = match &parameter.kind {
                    crate::item::TypeParameterKind::Type => crate::item::TypeParameterKind::Type,
                    crate::item::TypeParameterKind::Const { type_reference } => {
                        crate::item::TypeParameterKind::Const {
                            type_reference: this.copy_type_reference_handle(other, *type_reference),
                        }
                    }
                    crate::item::TypeParameterKind::Value { type_reference } => {
                        crate::item::TypeParameterKind::Value {
                            type_reference: this.copy_type_reference_handle(other, *type_reference),
                        }
                    }
                    crate::item::TypeParameterKind::Machine { contract } => {
                        crate::item::TypeParameterKind::Machine {
                            contract: contract.as_ref().map(|contract| match contract {
                                crate::item::MachineParameterContract::RequirementIdentity => {
                                    crate::item::MachineParameterContract::RequirementIdentity
                                }
                                crate::item::MachineParameterContract::Structural(signature) => {
                                    crate::item::MachineParameterContract::Structural(
                                        this.copy_state_signature_value(other, signature),
                                    )
                                }
                                crate::item::MachineParameterContract::Nominal { requirement } => {
                                    crate::item::MachineParameterContract::Nominal {
                                        requirement: this
                                            .copy_item_identifier_span(other, *requirement),
                                    }
                                }
                            }),
                        }
                    }
                    crate::item::TypeParameterKind::Proposition { contract } => {
                        crate::item::TypeParameterKind::Proposition {
                            contract: contract.as_ref().map(|contract| {
                                crate::item::PropositionParameterSignature {
                                    name: contract.name.clone(),
                                    parameters: this.copy_state_parameter_handle_span(
                                        other,
                                        contract.parameters,
                                    ),
                                }
                            }),
                        }
                    }
                };
                TypeParameter {
                    name: parameter.name.clone(),
                    kind,
                    bounds: parameter.bounds,
                }
            },
            |this, parameter| this.items.append_type_parameter(parameter),
        )
    }

    fn copy_capability_member_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<CapabilityMember>,
    ) -> HandleSpan<CapabilityMember> {
        self.copy_mapped_span(
            other.items.capability_members(span),
            |this, member| match member {
                CapabilityMember::Field(field) => CapabilityMember::Field(CapabilityField {
                    name: field.name.clone(),
                    type_reference: this.copy_type_reference_handle(other, field.type_reference),
                }),
                CapabilityMember::State(state) => CapabilityMember::State(CapabilityState {
                    signature: this.copy_state_signature_value(other, &state.signature),
                    contracts: this.copy_capability_contract_span(other, state.contracts),
                }),
            },
            |this, member| this.items.append_capability_member(member),
        )
    }

    pub(crate) fn copy_capability_contract_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<CapabilityContract>,
    ) -> HandleSpan<CapabilityContract> {
        let contracts = other
            .items
            .capability_contracts(span)
            .iter()
            .map(|contract| CapabilityContract {
                kind: match &contract.kind {
                    CapabilityContractKind::Ensures => CapabilityContractKind::Ensures,
                    CapabilityContractKind::EnsuresForResultCase { result_case } => {
                        CapabilityContractKind::EnsuresForResultCase {
                            result_case: self.copy_item_identifier_span(other, *result_case),
                        }
                    }
                    CapabilityContractKind::Requires => CapabilityContractKind::Requires,
                    CapabilityContractKind::Crashes { cause } => {
                        CapabilityContractKind::Crashes { cause: *cause }
                    }
                },
                keyword_source_span: contract.keyword_source_span,
                binding: contract.binding.clone(),
                facts: self.copy_domain_fact_span(other, contract.facts),
                token_count: contract.token_count,
            })
            .collect::<Vec<_>>();
        self.copy_span(contracts, |this, contract| {
            this.items.append_capability_contract(contract)
        })
    }

    fn copy_data_member_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<DataMember>,
    ) -> HandleSpan<DataMember> {
        self.copy_mapped_span(
            other.items.data_members(span),
            |this, member| match member {
                DataMember::Field(field) => DataMember::Field(DataField {
                    identity: field.identity,
                    name: field.name.clone(),
                    relevance: field.relevance,
                    type_reference: this.copy_type_reference_handle(other, field.type_reference),
                }),
                DataMember::Variant(variant) => DataMember::Variant(DataVariant {
                    identity: variant.identity,
                    name: variant.name.clone(),
                    payload: this.copy_data_payload_field_span(other, variant.payload),
                    where_facts: this.copy_domain_fact_span(other, variant.where_facts),
                    retired_payload_identities: variant.retired_payload_identities.clone(),
                }),
                DataMember::Retired(identity) => DataMember::Retired(*identity),
            },
            |this, member| this.items.append_data_member(member),
        )
    }

    fn copy_data_payload_field_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<DataField>,
    ) -> HandleSpan<DataField> {
        self.copy_mapped_span(
            other.items.data_payload_fields(span),
            |this, field| DataField {
                identity: field.identity,
                name: field.name.clone(),
                relevance: field.relevance,
                type_reference: this.copy_type_reference_handle(other, field.type_reference),
            },
            |this, field| this.items.append_data_payload_field(field),
        )
    }

    pub(crate) fn copy_domain_fact_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<ProofFact>,
    ) -> HandleSpan<ProofFact> {
        let mut copied = HandleSpan::empty();
        for offset in 0..span.count() {
            let source = Handle::from_parts(
                span.start()
                    .arena_index()
                    .checked_add(offset)
                    .expect("proof fact copy handle index overflow"),
                span.start().generation(),
            );
            copied.push_contiguous(self.copy_proof_fact_from(other, source));
        }
        copied
    }
}
