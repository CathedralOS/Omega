use crate::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableBinaryExpression, TableCallExpression,
    TableCastExpression, TableIndexedExpression, TableMemberExpression, TableStructLiteral,
    TableStructLiteralField,
};
use crate::identifier::Identifier;
use crate::item::{
    BoundaryLevel, BoundaryMode, BoundaryPolicy, CapabilityContract, CapabilityContractKind,
    CapabilityDefinition, CapabilityField, CapabilityMember, CapabilityState, DataDefinition,
    DataField, DataMember, DataVariant, DomainDefinition, Item, ItemHandle, ItemTable,
    LibraryDefinition, LibraryFunction, Machine, MeasureDefinition, OperatorDefinition, ProofFact,
    ProofMembershipFact, State, StateHandle, StateParameterHandle, StateParameterNode,
    StateSignature, StateSignatureHandle, TargetDefinition, TargetHost, TargetHostSetting,
    TargetHostSettingValue, TraitDefinition, TypeParameter, UseItem, WireDataDefinition,
    WireDataField, WireDataMember, WireDataReserved, WireDataVersion,
};
use crate::statement::{
    StatementHandle, StatementNode, StatementTable, TableAssemblyFact, TableAssignment, TableCall,
    TableLocalData, TableTransition, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};
use crate::types::{
    TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode, TypeReferenceTable,
};
use psi_arena::{Arena, Handle, HandleSpan};
use psi_source::SourceId;
use std::ops::{Deref, DerefMut};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTrees {
    pub source_id: SourceId,
    pub roots: SyntaxTreeRoots,
    pub tables: SyntaxTreeTables,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTreeTables {
    pub items: ItemTable,
    pub expressions: ExpressionTable,
    pub statements: StatementTable,
    pub type_references: TypeReferenceTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyntaxTreeRoots {
    pub items: Arena<ItemHandle>,
}

impl SyntaxTrees {
    pub fn new(source_id: SourceId) -> Self {
        Self {
            source_id,
            roots: SyntaxTreeRoots::default(),
            tables: SyntaxTreeTables::new(),
        }
    }

    pub fn from_root_items(source_id: SourceId, items: impl IntoIterator<Item = Item>) -> Self {
        let mut syntax_trees = Self::new(source_id);

        for item in items {
            syntax_trees.push_root_item(item);
        }

        syntax_trees
    }

    pub fn push_root_item(&mut self, item: Item) -> ItemHandle {
        let handle = self.insert_item(item);
        self.roots.items.append(handle);
        handle
    }

    pub fn root_item_handles(&self) -> &[ItemHandle] {
        self.roots.items.storage_slice()
    }

    pub fn root_item(&self, handle: ItemHandle) -> &Item {
        self.items.item(handle)
    }

    pub fn root_items(&self) -> impl Iterator<Item = &Item> {
        self.root_item_handles()
            .iter()
            .map(|handle| self.root_item(*handle))
    }

    pub fn root_item_count(&self) -> usize {
        self.roots.items.len()
    }

    pub fn extend_from(&mut self, other: &SyntaxTrees) {
        for handle in other.root_item_handles() {
            self.push_copied_root_item(other, *handle);
        }
    }

    fn insert_item(&mut self, item: Item) -> ItemHandle {
        match &item {
            Item::Machine(machine) => self.insert_machine(machine),
            Item::Trait(trait_definition) => self.insert_trait_definition(trait_definition),
            Item::Capability(_)
            | Item::Conformance(_)
            | Item::Const(_)
            | Item::Data(_)
            | Item::Domain(_)
            | Item::Export(_)
            | Item::Invariant(_)
            | Item::Library(_)
            | Item::Measure(_)
            | Item::Module(_)
            | Item::Operator(_)
            | Item::Package(_)
            | Item::Proposition(_)
            | Item::Target(_)
            | Item::WireData(_)
            | Item::Use(_) => {}
        }

        self.items.append_item(item)
    }

    fn insert_machine(&mut self, machine: &Machine) {
        self.items.insert_machine(machine);
    }

    fn insert_trait_definition(&mut self, trait_definition: &TraitDefinition) {
        self.items.insert_trait_definition(trait_definition);
    }

    fn push_copied_root_item(&mut self, other: &SyntaxTrees, handle: ItemHandle) -> ItemHandle {
        let item = self.copy_item(other, other.root_item(handle));
        self.push_root_item(item)
    }

    /// Deep-copy a single item from `other` into this tree's tables (the
    /// generic-instance desugar clones attached machines from a snapshot of
    /// the tree being extended). Returns the copied item; the caller pushes
    /// it as a root item after any post-copy rewrites.
    pub fn copy_item_from(&mut self, other: &SyntaxTrees, item: &Item) -> Item {
        self.copy_item(other, item)
    }

    /// Deep-copy one proof fact from another syntax tree. Generic-instance
    /// synthesis uses this to retain field-dependent default-domain facts
    /// while discharging facts that depend only on concrete const arguments.
    pub fn copy_proof_fact_from(&mut self, other: &SyntaxTrees, fact: &ProofFact) -> ProofFact {
        match fact {
            ProofFact::Expression(expression) => {
                ProofFact::Expression(self.copy_expression_handle(other, *expression))
            }
            ProofFact::Membership(membership) => ProofFact::Membership(ProofMembershipFact {
                value: self.copy_expression_handle(other, membership.value),
                domain: self.copy_item_identifier_span(other, membership.domain),
            }),
        }
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
            return_type: copied.return_type,
            service_reach_is_installation_bound: copied.service_reach_is_installation_bound,
            service_reaches: copied.service_reaches,
            invokes: copied.invokes,
            suspends: copied.suspends,
            blocks: copied.blocks,
            contracts: copied.contracts,
            default_body: copied.default_body,
            terminates_guarantee: copied.terminates_guarantee,
        }
    }

    fn copy_item(&mut self, other: &SyntaxTrees, item: &Item) -> Item {
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
                        let mut copied_start = psi_arena::Handle::invalid();
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
                                psi_arena::HandleSpan::empty()
                            } else {
                                psi_arena::HandleSpan::from_parts(copied_start, copied_count)
                            },
                        }
                    }
                };
                Item::Conformance(crate::item::ConformanceItem {
                    lifetime_parameters: conformance.lifetime_parameters.clone(),
                    type_parameters: self
                        .copy_type_parameter_span(other, conformance.type_parameters),
                    subject: conformance.subject.clone(),
                    trait_name: conformance.trait_name.clone(),
                    trait_arguments: self
                        .copy_type_reference_handle_span(other, conformance.trait_arguments),
                    alias: conformance.alias.clone(),
                    body,
                })
            }
            Item::Const(constant) => Item::Const(crate::item::ConstDefinition {
                scope: constant.scope.clone(),
                name: constant.name.clone(),
                type_reference: self.copy_type_reference_handle(other, constant.type_reference),
                value: self.copy_expression_handle(other, constant.value),
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
            Item::Invariant(invariant) => Item::Invariant(crate::item::InvariantDefinition {
                name: invariant.name.clone(),
                constraints: self.copy_constraint_span(other, invariant.constraints),
            }),
            Item::Library(library) => Item::Library(self.copy_library_definition(other, library)),
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
                    type_parameters: self
                        .copy_type_parameter_span(other, proposition.type_parameters),
                    parameters: self
                        .copy_state_parameter_handle_span(other, proposition.parameters),
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
            Item::Export(export_item) => Item::Export(crate::item::ExportItem {
                path: self.copy_item_identifier_span(other, export_item.path),
                alias: export_item.alias.clone(),
            }),
            Item::Use(use_item) => Item::Use(UseItem {
                path: self.copy_item_identifier_span(other, use_item.path),
            }),
            Item::Machine(machine) => Item::Machine(self.copy_machine(other, machine)),
            Item::Trait(trait_definition) => {
                Item::Trait(self.copy_trait_definition(other, trait_definition))
            }
            Item::Target(target) => Item::Target(self.copy_target_definition(other, target)),
            Item::WireData(wire_data) => {
                Item::WireData(self.copy_wire_data_definition(other, wire_data))
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
            supply_mode: data.supply_mode,
            lifetime_parameters: data.lifetime_parameters.clone(),
            type_parameters: self.copy_type_parameter_span(other, data.type_parameters),
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

    fn copy_library_definition(
        &mut self,
        other: &SyntaxTrees,
        library: &LibraryDefinition,
    ) -> LibraryDefinition {
        LibraryDefinition {
            name: library.name.clone(),
            path: library.path.clone(),
            calling_convention: library.calling_convention.clone(),
            functions: self.copy_library_function_span(other, library.functions),
        }
    }

    fn copy_operator_definition(
        &mut self,
        other: &SyntaxTrees,
        operator: &OperatorDefinition,
    ) -> OperatorDefinition {
        OperatorDefinition {
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
            attached_data: machine.attached_data.clone(),
            target: machine.target.clone(),
            boundary: machine.boundary,
            bodyless: machine.bodyless,
            lifetime_parameters: machine.lifetime_parameters.clone(),
            type_parameters: self.copy_type_parameter_span(other, machine.type_parameters),
            satisfies: self.copy_mapped_span(
                other.items.satisfies_clauses(machine.satisfies).to_vec(),
                |this, clause| crate::item::SatisfiesClause {
                    trait_name: clause.trait_name,
                    arguments: this.copy_type_reference_handle_span(other, clause.arguments),
                    requirement: clause.requirement,
                    alias: clause.alias,
                    via: clause.via,
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
                    conformance: bound.conformance.clone(),
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
            service_reaches: self.copy_item_identifier_span(other, machine.service_reaches),
            invokes: self.copy_item_identifier_span(other, machine.invokes),
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
                    conformance: bound.conformance.clone(),
                })
                .collect(),
            parents: self.copy_type_reference_handle_span(other, trait_definition.parents),
            invariants: self.copy_domain_fact_span(other, trait_definition.invariants),
            requires: self.copy_item_identifier_span(other, trait_definition.requires),
            machines: self.copy_state_signature_handle_span(other, trait_definition.machines),
        }
    }

    fn copy_target_definition(
        &mut self,
        other: &SyntaxTrees,
        target: &TargetDefinition,
    ) -> TargetDefinition {
        TargetDefinition {
            name: target.name.clone(),
            host: target.host.as_ref().map(|host| TargetHost {
                provider: self.copy_item_identifier_span(other, host.provider),
                settings: self.copy_target_host_setting_span(other, host.settings),
            }),
            boundary_policies: self.copy_boundary_policy_span(other, target.boundary_policies),
        }
    }

    fn copy_wire_data_definition(
        &mut self,
        other: &SyntaxTrees,
        wire_data: &WireDataDefinition,
    ) -> WireDataDefinition {
        WireDataDefinition {
            name: wire_data.name.clone(),
            encoding: wire_data.encoding.clone(),
            members: self.copy_wire_data_member_span(other, wire_data.members),
        }
    }

    fn copy_type_parameter_span(
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
                    crate::item::TypeParameterKind::Machine { contract } => {
                        crate::item::TypeParameterKind::Machine {
                            contract: contract.as_ref().map(|contract| match contract {
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

    fn copy_boundary_level_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<BoundaryLevel>,
    ) -> HandleSpan<BoundaryLevel> {
        self.copy_span(
            other
                .items
                .boundary_levels(span)
                .iter()
                .map(|level| match level {
                    BoundaryLevel::Host => BoundaryLevel::Host,
                    BoundaryLevel::Named(name) => BoundaryLevel::Named(name.clone()),
                }),
            |this, boundary_level| this.items.append_boundary_level(boundary_level),
        )
    }

    fn copy_library_function_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<LibraryFunction>,
    ) -> HandleSpan<LibraryFunction> {
        self.copy_mapped_span(
            other.items.library_functions(span),
            |this, function| LibraryFunction {
                signature: this.copy_state_signature_value(other, &function.signature),
                symbol: function.symbol.clone(),
                calling_convention: function.calling_convention.clone(),
                boundaries: this.copy_boundary_level_span(other, function.boundaries),
            },
            |this, function| this.items.append_library_function(function),
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

    fn copy_capability_contract_span(
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
                    CapabilityContractKind::Requires => CapabilityContractKind::Requires,
                    CapabilityContractKind::Boundary(BoundaryLevel::Host) => {
                        CapabilityContractKind::Boundary(BoundaryLevel::Host)
                    }
                    CapabilityContractKind::Boundary(BoundaryLevel::Named(name)) => {
                        CapabilityContractKind::Boundary(BoundaryLevel::Named(name.clone()))
                    }
                    CapabilityContractKind::Crashes { cause } => {
                        CapabilityContractKind::Crashes { cause: *cause }
                    }
                },
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

    fn copy_wire_data_member_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<WireDataMember>,
    ) -> HandleSpan<WireDataMember> {
        self.copy_mapped_span(
            other.items.wire_data_members(span),
            |this, member| match member {
                WireDataMember::Field(field) => WireDataMember::Field(WireDataField {
                    number: field.number,
                    name: field.name.clone(),
                    relevance: field.relevance,
                    type_reference: this.copy_type_reference_handle(other, field.type_reference),
                }),
                WireDataMember::Reserved(reserved) => WireDataMember::Reserved(WireDataReserved {
                    number: reserved.number,
                }),
                WireDataMember::Version(version) => WireDataMember::Version(WireDataVersion {
                    name: version.name.clone(),
                    members: this.copy_wire_data_member_span(other, version.members),
                }),
            },
            |this, member| this.items.append_wire_data_member(member),
        )
    }

    fn copy_domain_fact_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<ProofFact>,
    ) -> HandleSpan<ProofFact> {
        self.copy_mapped_span(
            other.items.proof_facts(span),
            |this, fact| this.copy_proof_fact_from(other, fact),
            |this, fact| this.items.append_proof_fact(fact),
        )
    }

    fn copy_target_host_setting_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<TargetHostSetting>,
    ) -> HandleSpan<TargetHostSetting> {
        let settings = other
            .items
            .target_host_settings(span)
            .iter()
            .map(|setting| TargetHostSetting {
                name: setting.name.clone(),
                value: match &setting.value {
                    TargetHostSettingValue::Call {
                        name,
                        argument_tokens,
                    } => TargetHostSettingValue::Call {
                        name: name.clone(),
                        argument_tokens: *argument_tokens,
                    },
                    TargetHostSettingValue::Named(name) => {
                        TargetHostSettingValue::Named(name.clone())
                    }
                },
            });
        self.copy_span(settings, |this, setting| {
            this.items.append_target_host_setting(setting)
        })
    }

    fn copy_boundary_policy_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<BoundaryPolicy>,
    ) -> HandleSpan<BoundaryPolicy> {
        self.copy_mapped_span(
            other.items.boundary_policies(span),
            |this, policy| BoundaryPolicy {
                mode: match policy.mode {
                    BoundaryMode::Checked => BoundaryMode::Checked,
                    BoundaryMode::Unchecked => BoundaryMode::Unchecked,
                },
                path: this.copy_item_identifier_span(other, policy.path),
            },
            |this, policy| this.items.append_boundary_policy(policy),
        )
    }

    fn copy_state_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<StateHandle>,
    ) -> HandleSpan<StateHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for handle in other.items.state_handles(span).iter().copied() {
            let state = other.items.state(handle);
            let parameters = self.copy_state_parameter_handle_span(other, state.parameters);
            let return_type = self.copy_type_reference_handle(other, state.return_type);
            let contracts = self.copy_capability_contract_span(other, state.contracts);
            let statements = self.copy_statement_handle_span(other, state.statements);
            let copied = self.items.insert_state(&State {
                name: state.name.clone(),
                parameters,
                return_type,
                contracts,
                statements,
            });
            let copied = self.items.append_state_handle(copied);
            if count == 0 {
                start = copied;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_state_signature_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<StateSignatureHandle>,
    ) -> HandleSpan<StateSignatureHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for handle in other.items.state_signatures(span).iter().copied() {
            let signature = other.items.state_signature(handle);
            let copied_signature = self.copy_state_signature_node(other, signature);
            let copied = self.items.insert_state_signature(&copied_signature);
            let copied = self.items.append_state_signature_handle(copied);
            if count == 0 {
                start = copied;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_state_signature_value(
        &mut self,
        other: &SyntaxTrees,
        signature: &StateSignature,
    ) -> StateSignature {
        StateSignature {
            name: signature.name.clone(),
            spelling: signature.spelling,
            lifetime_parameters: signature.lifetime_parameters.clone(),
            type_parameters: self.copy_type_parameter_span(other, signature.type_parameters),
            is_default: signature.is_default,
            parameters: self.copy_state_parameter_handle_span(other, signature.parameters),
            return_type: self.copy_type_reference_handle(other, signature.return_type),
            service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
            service_reaches: self.copy_item_identifier_span(other, signature.service_reaches),
            invokes: self.copy_item_identifier_span(other, signature.invokes),
            suspends: signature.suspends,
            blocks: signature.blocks,
            contracts: self.copy_capability_contract_span(other, signature.contracts),
            default_body: self.copy_statement_handle_span(other, signature.default_body),
            terminates_guarantee: signature.terminates_guarantee,
        }
    }

    fn copy_state_signature_node(
        &mut self,
        other: &SyntaxTrees,
        signature: &crate::item::StateSignatureNode,
    ) -> StateSignature {
        StateSignature {
            name: signature.name.clone(),
            spelling: signature.spelling,
            lifetime_parameters: signature.lifetime_parameters.clone(),
            type_parameters: self.copy_type_parameter_span(other, signature.type_parameters),
            is_default: signature.is_default,
            parameters: self.copy_state_parameter_handle_span(other, signature.parameters),
            return_type: self.copy_type_reference_handle(other, signature.return_type),
            service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
            service_reaches: self.copy_item_identifier_span(other, signature.service_reaches),
            invokes: self.copy_item_identifier_span(other, signature.invokes),
            suspends: signature.suspends,
            blocks: signature.blocks,
            contracts: self.copy_capability_contract_span(other, signature.contracts),
            default_body: self.copy_statement_handle_span(other, signature.default_body),
            terminates_guarantee: signature.terminates_guarantee,
        }
    }

    fn copy_state_parameter_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<StateParameterHandle>,
    ) -> HandleSpan<StateParameterHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for handle in other.items.state_parameters(span).iter().copied() {
            let parameter = other.items.state_parameter(handle);
            let type_reference = self.copy_type_reference_handle(other, parameter.type_reference);
            let copied = self.items.insert_state_parameter_node(StateParameterNode {
                name: parameter.name.clone(),
                type_reference,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                is_self: parameter.is_self,
            });
            let copied = self.items.append_state_parameter_handle(copied);
            if count == 0 {
                start = copied;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_statement_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<StatementHandle>,
    ) -> HandleSpan<StatementHandle> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for handle in other.items.statements(span).iter().copied() {
            let statement = self.copy_statement_node(other, other.statements.statement(handle));
            let copied = self.statements.insert(statement);
            let copied = self.items.append_statement_handle(copied);
            if count == 0 {
                start = copied;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_statement_node(
        &mut self,
        other: &SyntaxTrees,
        statement: &StatementNode,
    ) -> StatementNode {
        match statement {
            StatementNode::AssemblyFact(fact) => StatementNode::AssemblyFact(TableAssemblyFact {
                kind: fact.kind,
                expression: self.copy_expression_handle(other, fact.expression),
            }),
            StatementNode::Assignment(assignment) => StatementNode::Assignment(TableAssignment {
                target: self.copy_expression_handle(other, assignment.target),
                value: self.copy_expression_handle(other, assignment.value),
            }),
            StatementNode::Call(call) => StatementNode::Call(TableCall {
                receiver: self.copy_statement_identifier_span(other, call.receiver),
                receiver_starts_at_self: call.receiver_starts_at_self,
                target: call.target.clone(),
                machine_arguments: call.machine_arguments.clone(),
                arguments: self.copy_statement_expression_span(other, call.arguments),
                evidence_arguments: call.evidence_arguments.clone(),
                operational_acknowledgement: call.operational_acknowledgement,
                discards_result: call.discards_result,
            }),
            StatementNode::ProofOutputBindingStatement(binding) => {
                StatementNode::ProofOutputBindingStatement(
                    crate::statement::TableProofOutputBindingStatement {
                        bindings: binding.bindings.clone(),
                        call: self.copy_expression_handle(other, binding.call),
                    },
                )
            }
            StatementNode::Expression(value) => {
                StatementNode::Expression(self.copy_expression_handle(other, *value))
            }
            StatementNode::LocalData(local_data) => StatementNode::LocalData(TableLocalData {
                name: local_data.name.clone(),
                type_reference: self.copy_type_reference_handle(other, local_data.type_reference),
                initial_value: self.copy_expression_handle(other, local_data.initial_value),
                is_mutable: local_data.is_mutable,
            }),
            StatementNode::Transition(transition) => StatementNode::Transition(TableTransition {
                target: self.copy_transition_target(other, transition.target),
                continuation: self.copy_transition_target(other, transition.continuation),
                guard: match transition.guard {
                    TransitionGuardNode::Always => TransitionGuardNode::Always,
                    TransitionGuardNode::When(expression) => {
                        TransitionGuardNode::When(self.copy_expression_handle(other, expression))
                    }
                },
                exit: transition.exit,
                source_span: transition.source_span,
            }),
        }
    }

    fn copy_transition_target(
        &mut self,
        other: &SyntaxTrees,
        handle: TransitionTargetHandle,
    ) -> TransitionTargetHandle {
        if !handle.is_valid() {
            return TransitionTargetHandle::invalid();
        }

        let target = match other.statements.transition_target(handle) {
            TransitionTargetNode::Named {
                path,
                path_starts_at_self,
                arguments,
                evidence_arguments,
            } => TransitionTargetNode::Named {
                path: self.copy_statement_identifier_span(other, *path),
                path_starts_at_self: *path_starts_at_self,
                arguments: self.copy_statement_expression_span(other, *arguments),
                evidence_arguments: evidence_arguments.clone(),
            },
            TransitionTargetNode::Value(value) => {
                TransitionTargetNode::Value(self.copy_expression_handle(other, *value))
            }
            TransitionTargetNode::SelfTarget => TransitionTargetNode::SelfTarget,
            TransitionTargetNode::Terminal => TransitionTargetNode::Terminal,
        };

        self.statements.insert_transition_target(target)
    }

    fn copy_type_reference_handle(
        &mut self,
        other: &SyntaxTrees,
        handle: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        if !handle.is_valid() {
            return TypeReferenceHandle::invalid();
        }

        match other.type_references.type_reference(handle) {
            TypeReferenceNode::Reference {
                referee,
                is_mutable,
                lifetime,
            } => {
                let referee = self.copy_type_reference_handle(other, *referee);
                self.type_references.insert_reference_with_lifetime(
                    referee,
                    *is_mutable,
                    lifetime.clone(),
                )
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                let base_type = self.copy_type_reference_handle(other, *base_type);
                let constraints = self.copy_constraint_span(other, *constraints);
                self.type_references
                    .insert_constrained(base_type, constraints)
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let element_type = self.copy_type_reference_handle(other, *element_type);
                self.type_references
                    .insert_fixed_array(element_type, length.clone())
            }
            TypeReferenceNode::Slice { element_type } => {
                let element_type = self.copy_type_reference_handle(other, *element_type);
                self.type_references.insert_slice(element_type)
            }
            TypeReferenceNode::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                let arguments = self.copy_type_reference_handle_span(other, *arguments);
                self.type_references.insert(TypeReferenceNode::Generic {
                    base_name: base_name.clone(),
                    lifetime_arguments: lifetime_arguments.clone(),
                    arguments,
                })
            }
            TypeReferenceNode::ConstExpression(expression) => {
                let expression = self.copy_expression_handle(other, *expression);
                self.type_references
                    .insert(TypeReferenceNode::ConstExpression(expression))
            }
            TypeReferenceNode::DynamicTrait { name, conformance } => {
                self.type_references
                    .insert(TypeReferenceNode::DynamicTrait {
                        name: name.clone(),
                        conformance: conformance.clone(),
                    })
            }
            TypeReferenceNode::Named(name) => self.type_references.insert_named(name.clone()),
            TypeReferenceNode::SelfType => self.type_references.insert_self_type(),
            TypeReferenceNode::Unit => self.type_references.insert_unit(),
        }
    }

    fn copy_type_reference_handle_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<TypeReferenceHandle>,
    ) -> HandleSpan<TypeReferenceHandle> {
        self.copy_mapped_span(
            other
                .type_references
                .type_reference_handles(span)
                .iter()
                .copied(),
            |this, handle| this.copy_type_reference_handle(other, handle),
            |this, handle| this.type_references.append_type_reference_handle(handle),
        )
    }

    fn copy_constraint_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<TypeConstraintNode>,
    ) -> HandleSpan<TypeConstraintNode> {
        self.copy_mapped_span(
            other.type_references.constraints(span),
            |this, constraint| match constraint {
                TypeConstraintNode::Named(name) => TypeConstraintNode::Named(name.clone()),
                TypeConstraintNode::Domain(domain) => {
                    TypeConstraintNode::Domain(crate::types::DomainConstraint {
                        name: domain.name.clone(),
                        arguments: this.copy_type_reference_handle_span(other, domain.arguments),
                    })
                }
                TypeConstraintNode::Range { minimum, maximum } => TypeConstraintNode::Range {
                    minimum: this.copy_expression_handle(other, *minimum),
                    maximum: this.copy_expression_handle(other, *maximum),
                },
                TypeConstraintNode::ArithmeticDomain(domain) => {
                    TypeConstraintNode::ArithmeticDomain(*domain)
                }
            },
            |this, constraint| this.type_references.append_constraint(constraint),
        )
    }

    fn copy_expression_handle(
        &mut self,
        other: &SyntaxTrees,
        handle: ExpressionHandle,
    ) -> ExpressionHandle {
        if !handle.is_valid() {
            return ExpressionHandle::invalid();
        }

        let expression = match other.expressions.expression(handle) {
            ExpressionNode::ArrayLiteral(values) => {
                ExpressionNode::ArrayLiteral(self.copy_expression_handle_list(other, *values))
            }
            ExpressionNode::Atomic(atomic) => {
                ExpressionNode::Atomic(crate::expression::TableAtomicExpression {
                    value: self.copy_expression_handle(other, atomic.value),
                    result: atomic
                        .result
                        .is_valid()
                        .then(|| self.copy_expression_handle(other, atomic.result))
                        .unwrap_or_else(ExpressionHandle::invalid),
                    ordering: atomic.ordering,
                })
            }
            ExpressionNode::Binary(binary) => ExpressionNode::Binary(TableBinaryExpression {
                left: self.copy_expression_handle(other, binary.left),
                operator: binary.operator,
                right: self.copy_expression_handle(other, binary.right),
            }),
            ExpressionNode::Boolean(value) => ExpressionNode::Boolean(*value),
            ExpressionNode::Cast(cast) => ExpressionNode::Cast(TableCastExpression {
                value: self.copy_expression_handle(other, cast.value),
                target_type: self.copy_type_reference_handle(other, cast.target_type),
                target_label: self.copy_expression_identifier_span(other, cast.target_label),
                domain: cast.domain,
                semantic_domain: self.copy_expression_identifier_span(other, cast.semantic_domain),
                semantic_domain_arguments: self
                    .copy_type_reference_handle_span(other, cast.semantic_domain_arguments),
                form: cast.form,
            }),
            ExpressionNode::Call(call) => ExpressionNode::Call(TableCallExpression {
                receiver: self.copy_expression_handle(other, call.receiver),
                target: call.target.clone(),
                machine_arguments: call.machine_arguments.clone(),
                arguments: self.copy_expression_handle_list(other, call.arguments),
                evidence_arguments: call.evidence_arguments.clone(),
                operational_acknowledgement: call.operational_acknowledgement,
            }),
            ExpressionNode::Float(value) => ExpressionNode::Float(value.clone()),
            ExpressionNode::Indexed(indexed) => ExpressionNode::Indexed(TableIndexedExpression {
                collection: self.copy_expression_handle(other, indexed.collection),
                index: self.copy_expression_handle(other, indexed.index),
            }),
            ExpressionNode::Integer(value) => ExpressionNode::Integer(value.clone()),
            ExpressionNode::Membership(membership) => {
                ExpressionNode::Membership(crate::expression::TableMembershipExpression {
                    value: self.copy_expression_handle(other, membership.value),
                    domain: self.copy_expression_identifier_span(other, membership.domain),
                })
            }
            ExpressionNode::Member(member) => ExpressionNode::Member(TableMemberExpression {
                receiver: self.copy_expression_handle(other, member.receiver),
                member: member.member.clone(),
                case_variant: member.case_variant.clone(),
            }),
            ExpressionNode::Mutable(expression) => {
                ExpressionNode::Mutable(self.copy_expression_handle(other, *expression))
            }
            ExpressionNode::Name(path) => {
                ExpressionNode::Name(self.copy_expression_identifier_span(other, *path))
            }
            ExpressionNode::Range(range) => {
                ExpressionNode::Range(crate::expression::TableRangeExpression {
                    start: self.copy_expression_handle(other, range.start),
                    end: self.copy_expression_handle(other, range.end),
                    end_inclusive: range.end_inclusive,
                })
            }
            ExpressionNode::SelfValue => ExpressionNode::SelfValue,
            ExpressionNode::StructLiteral(struct_literal) => {
                ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    case_name: struct_literal.case_name.clone(),
                    fields: self.copy_struct_field_span(other, struct_literal.fields),
                })
            }
            ExpressionNode::String(value) => ExpressionNode::String(value.clone()),
            ExpressionNode::Unary(unary) => {
                let operand = self.copy_expression_handle(other, unary.operand);
                ExpressionNode::Unary(crate::expression::TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                })
            }
            ExpressionNode::ZeroValue(type_reference) => {
                ExpressionNode::ZeroValue(self.copy_type_reference_handle(other, *type_reference))
            }
        };

        let source_span = other.expressions.source_span(handle);
        let copied = self.expressions.insert(expression);
        self.expressions.set_source_span(copied, source_span);
        copied
    }

    fn copy_expression_handle_list(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        let copied_handles: Vec<_> = other
            .expressions
            .expression_handles(span)
            .iter()
            .copied()
            .map(|handle| self.copy_expression_handle(other, handle))
            .collect();

        self.expressions.insert_expression_handles(copied_handles)
    }

    fn copy_struct_field_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        let copied_fields: Vec<_> = other
            .expressions
            .struct_fields(span)
            .iter()
            .map(|field| TableStructLiteralField {
                name: field.name.clone(),
                value: self.copy_expression_handle(other, field.value),
            })
            .collect();

        self.expressions.insert_struct_fields(copied_fields)
    }

    fn copy_item_identifier_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<Identifier>,
    ) -> HandleSpan<Identifier> {
        self.copy_span(
            other.items.identifier_path_members(span).iter().cloned(),
            |this, member| this.items.append_identifier_path_member(member),
        )
    }

    fn copy_statement_identifier_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<Identifier>,
    ) -> HandleSpan<Identifier> {
        self.copy_span(
            other
                .statements
                .identifier_path_members(span)
                .iter()
                .cloned(),
            |this, member| this.statements.append_identifier_path_member(member),
        )
    }

    fn copy_expression_identifier_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<Identifier>,
    ) -> HandleSpan<Identifier> {
        self.copy_span(
            other
                .expressions
                .identifier_path_members(span)
                .iter()
                .cloned(),
            |this, member| this.expressions.append_identifier_path_member(member),
        )
    }

    fn copy_statement_expression_span(
        &mut self,
        other: &SyntaxTrees,
        span: HandleSpan<ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        let copied_handles: Vec<_> = other
            .statements
            .expression_handles(span)
            .iter()
            .copied()
            .map(|handle| self.copy_expression_handle(other, handle))
            .collect();

        self.statements.insert_expression_handles(copied_handles)
    }

    fn copy_mapped_span<S, T>(
        &mut self,
        values: impl IntoIterator<Item = S>,
        mut map: impl FnMut(&mut Self, S) -> T,
        mut append: impl FnMut(&mut Self, T) -> Handle<T>,
    ) -> HandleSpan<T> {
        let mut start = Handle::invalid();
        let mut count = 0u32;

        for value in values {
            let value = map(self, value);
            let handle = append(self, value);
            if count == 0 {
                start = handle;
            }
            count = count.checked_add(1).expect("copied span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }

    fn copy_span<T>(
        &mut self,
        values: impl IntoIterator<Item = T>,
        mut append: impl FnMut(&mut Self, T) -> Handle<T>,
    ) -> HandleSpan<T> {
        let mut start = Handle::invalid();
        let mut count = 0u32;
        for value in values {
            let handle = append(self, value);
            if count == 0 {
                start = handle;
            }
            count += 1;
        }
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        }
    }
}

impl SyntaxTreeTables {
    pub fn new() -> Self {
        Self {
            items: ItemTable::new(),
            expressions: ExpressionTable::new(),
            statements: StatementTable::new(),
            type_references: TypeReferenceTable::new(),
        }
    }
}

impl Default for SyntaxTreeTables {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for SyntaxTrees {
    type Target = SyntaxTreeTables;

    fn deref(&self) -> &Self::Target {
        &self.tables
    }
}

impl DerefMut for SyntaxTrees {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tables
    }
}

impl Default for SyntaxTrees {
    fn default() -> Self {
        Self::new(SourceId::default())
    }
}
