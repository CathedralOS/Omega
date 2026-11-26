use crate::SyntaxTrees;
use crate::expression::{ExpressionHandle, StaticMachineArgument};
use crate::identifier::Identifier;
use crate::item::{
    CapabilityContract, CapabilityContractKind, CapabilityDefinition, CapabilityField,
    CapabilityMember, CapabilityState, ConformanceItem, ConformanceMember, ConstDefinition,
    ConstDestinationObligation, ConstInitializerNormalization, ConstQualification, DataDefinition,
    DataField, DataMember, DataVariant, DomainAliasDefinition, DomainDefinition,
    GenericConformanceBound, Item, Machine, MachineParameterContract, MathematicalDefinition,
    MathematicalDefinitionBody, MathematicalParameterNode, MathematicalTypeHandle,
    MathematicalTypeNode, MeasureDefinition, OperatorDefinition, PackageDeclaration, ProofFact,
    ProofMembershipFact, PropositionBody, PropositionDefinition, PropositionParameterSignature,
    QuotientDefinition, QuotientEquivalenceSelection, SatisfiesClause, StateParameterHandle,
    StateParameterNode, StateSignature, TraitDefinition, TraitRefinementClause, TypeParameter,
    TypeParameterKind, UseItem,
};
use crate::types::TypeReferenceHandle;
use arena::{Handle, HandleSpan};
use std::ops::{Deref, DerefMut};

/// The supplied occurrence belongs to the destination tree and retains its declaring scope.
#[derive(Clone)]
pub struct SuppliedSyntaxArgument {
    pub binder: String,
    pub ty: TypeReferenceHandle,
}

pub(crate) struct SyntaxCopy<'destination> {
    destination: &'destination mut SyntaxTrees,
    pub(crate) expression_copies: Vec<(ExpressionHandle, ExpressionHandle)>,
    pub(crate) type_substitutions: Vec<(
        crate::types::TypeReferenceHandle,
        crate::types::TypeReferenceHandle,
    )>,
    pub(crate) expression_substitutions: Vec<(ExpressionHandle, ExpressionHandle)>,
    pub(crate) supplied_type_copies: Vec<crate::types::TypeReferenceHandle>,
    pub(crate) retain_alias_header_role: bool,
    lifetime_substitutions: Vec<(
        source_text::SourceSpan,
        String,
        crate::identifier::Identifier,
    )>,
    pub(crate) scoped_arguments: Vec<SuppliedSyntaxArgument>,
    pub(crate) scoped_lifetimes: Vec<(String, crate::identifier::Identifier)>,
    pub(crate) array_length_substitutions: Vec<(
        source_text::SourceSpan,
        String,
        crate::types::TypeReferenceHandle,
    )>,
    pub(crate) match_subject_copies: Vec<(
        Handle<crate::expression::MatchSubjectCapture>,
        Handle<crate::expression::MatchSubjectCapture>,
    )>,
}

impl<'destination> SyntaxCopy<'destination> {
    fn new(destination: &'destination mut SyntaxTrees, other: &SyntaxTrees) -> Self {
        destination.text.retain_from(&other.text);
        Self {
            destination,
            expression_copies: Vec::new(),
            type_substitutions: Vec::new(),
            expression_substitutions: Vec::new(),
            supplied_type_copies: Vec::new(),
            retain_alias_header_role: true,
            lifetime_substitutions: Vec::new(),
            scoped_arguments: Vec::new(),
            scoped_lifetimes: Vec::new(),
            array_length_substitutions: Vec::new(),
            match_subject_copies: Vec::new(),
        }
    }
}

impl Deref for SyntaxCopy<'_> {
    type Target = SyntaxTrees;

    fn deref(&self) -> &Self::Target {
        self.destination
    }
}

impl DerefMut for SyntaxCopy<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.destination
    }
}

impl SyntaxTrees {
    pub fn copy_item_from(&mut self, other: &SyntaxTrees, item: &Item) -> Item {
        SyntaxCopy::new(self, other).copy_item(other, item)
    }

    pub fn copy_expression_from(
        &mut self,
        other: &SyntaxTrees,
        handle: ExpressionHandle,
    ) -> ExpressionHandle {
        SyntaxCopy::new(self, other).copy_expression_handle(other, handle)
    }

    pub fn copy_assembly_operands_from(
        &mut self,
        other: &SyntaxTrees,
        operands: &mut crate::statement::AssemblyOperands,
    ) {
        SyntaxCopy::new(self, other).copy_assembly_operands(other, operands);
    }

    /// Copy a prepared body as a unit so shared operands keep their identity.
    pub fn copy_statements_from(
        &mut self,
        other: &SyntaxTrees,
        statements: &[crate::statement::StatementHandle],
    ) -> Vec<crate::statement::StatementHandle> {
        let mut copy = SyntaxCopy::new(self, other);
        statements
            .iter()
            .map(|&handle| {
                let node = copy.copy_statement_node(other, other.statements.statement(handle));
                copy.statements.insert(node)
            })
            .collect()
    }

    pub fn copy_proof_fact_from(
        &mut self,
        other: &SyntaxTrees,
        source: Handle<ProofFact>,
    ) -> Handle<ProofFact> {
        SyntaxCopy::new(self, other).copy_proof_fact(other, source)
    }

    pub fn copy_state_signature_from(
        &mut self,
        other: &SyntaxTrees,
        signature: &StateSignature,
    ) -> StateSignature {
        SyntaxCopy::new(self, other).copy_state_signature(other, signature)
    }

    pub fn copy_type_arguments_with_bindings(
        &mut self,
        other: &SyntaxTrees,
        types: &[TypeReferenceHandle],
        arguments: Vec<SuppliedSyntaxArgument>,
    ) -> Vec<TypeReferenceHandle> {
        let mut copy = SyntaxCopy::new(self, other);
        copy.scoped_arguments = arguments;
        types
            .iter()
            .map(|ty| copy.copy_type_reference_handle(other, *ty))
            .collect()
    }

    pub fn copy_state_signature_with_arguments(
        &mut self,
        other: &SyntaxTrees,
        signature: &StateSignature,
        arguments: Vec<SuppliedSyntaxArgument>,
        lifetimes: &[(String, Identifier)],
    ) -> StateSignature {
        let mut copy = SyntaxCopy::new(self, other);
        copy.scoped_arguments = arguments;
        copy.scoped_lifetimes = lifetimes.to_vec();
        copy.copy_state_signature(other, signature)
    }

    pub fn copy_static_machine_argument(
        &mut self,
        other: &SyntaxTrees,
        argument: &StaticMachineArgument,
    ) -> StaticMachineArgument {
        SyntaxCopy::new(self, other).copy_static_machine_argument(other, argument)
    }

    pub(crate) fn copy_mathematical_definition(
        &mut self,
        other: &SyntaxTrees,
        definition: &MathematicalDefinition,
    ) -> MathematicalDefinition {
        SyntaxCopy::new(self, other).copy_mathematical_definition(other, definition)
    }

    pub fn instantiate_type_reference_from(
        &mut self,
        other: &SyntaxTrees,
        reference: crate::types::TypeReferenceHandle,
        types: &[(
            crate::types::TypeReferenceHandle,
            crate::types::TypeReferenceHandle,
        )],
        expressions: &[(ExpressionHandle, ExpressionHandle)],
        lifetimes: &[(
            source_text::SourceSpan,
            String,
            crate::identifier::Identifier,
        )],
        array_lengths: &[(
            source_text::SourceSpan,
            String,
            crate::types::TypeReferenceHandle,
        )],
    ) -> (
        crate::types::TypeReferenceHandle,
        Vec<crate::types::TypeReferenceHandle>,
    ) {
        let mut copy = SyntaxCopy::new(self, other);
        copy.retain_alias_header_role = false;
        copy.type_substitutions.extend_from_slice(types);
        copy.expression_substitutions.extend_from_slice(expressions);
        copy.lifetime_substitutions.extend_from_slice(lifetimes);
        copy.array_length_substitutions
            .extend_from_slice(array_lengths);
        let reference = copy.copy_type_reference_handle(other, reference);
        (reference, copy.supplied_type_copies)
    }

    pub fn copy_type_reference_handle(
        &mut self,
        other: &SyntaxTrees,
        handle: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        SyntaxCopy::new(self, other).copy_type_reference_handle(other, handle)
    }
}

impl SyntaxCopy<'_> {
    pub(crate) fn copied_lifetime(
        &self,
        other: &SyntaxTrees,
        name: &crate::identifier::Identifier,
    ) -> crate::identifier::Identifier {
        self.lifetime_substitutions
            .iter()
            .find_map(|(source, spelling, actual)| {
                (*source == name.source_span() && spelling == name.as_str(other))
                    .then(|| actual.clone())
            })
            .or_else(|| {
                self.scoped_lifetimes.iter().find_map(|(spelling, actual)| {
                    (spelling == name.as_str(other)).then(|| actual.clone())
                })
            })
            .unwrap_or_else(|| name.clone())
    }

    fn copy_proof_fact(
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
                MathematicalDefinitionBody::DependentArrow { binders, body } => {
                    MathematicalDefinitionBody::DependentArrow {
                        binders: self.copy_mapped_span(
                            other.items.mathematical_parameters(binders).to_vec(),
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
                        body: self.copy_expression_handle(other, body),
                    }
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
                let copied = other
                    .items
                    .conformance_members(conformance.members)
                    .iter()
                    .map(|member| match member {
                        ConformanceMember::Machine(machine) => {
                            ConformanceMember::Machine(self.copy_machine(other, machine))
                        }
                        ConformanceMember::Synthesized(machine) => {
                            ConformanceMember::Synthesized(self.copy_machine(other, machine))
                        }
                        ConformanceMember::TraitDefault {
                            declaring_trait,
                            requirement_ordinal,
                            machine,
                        } => ConformanceMember::TraitDefault {
                            declaring_trait: declaring_trait.clone(),
                            requirement_ordinal: *requirement_ordinal,
                            machine: self.copy_machine(other, machine),
                        },
                        ConformanceMember::Selection {
                            declaring_trait,
                            requirement,
                            target,
                        } => ConformanceMember::Selection {
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
                let members = if copied_count == 0 {
                    arena::HandleSpan::empty()
                } else {
                    arena::HandleSpan::from_parts(copied_start, copied_count)
                };
                Item::Conformance(ConformanceItem {
                    is_public: conformance.is_public,
                    lifetime_parameters: conformance.lifetime_parameters.clone(),
                    type_parameters: self
                        .copy_type_parameter_span(other, conformance.type_parameters),
                    trait_name: conformance.trait_name.clone(),
                    trait_lifetime_arguments: conformance.trait_lifetime_arguments.clone(),
                    trait_arguments: self
                        .copy_type_reference_handle_span(other, conformance.trait_arguments),
                    alias: conformance.alias.clone(),
                    members,
                })
            }
            Item::Const(constant) => {
                let type_reference =
                    self.copy_type_reference_handle(other, constant.type_reference);
                let initializer = self.copy_expression_handle(other, constant.initializer);
                let declaration = crate::item::ConstSyntax {
                    scope: constant.scope.clone(),
                    name: constant.name.clone(),
                    is_public: constant.is_public,
                    type_reference,
                    initializer,
                };
                let preparation = constant.preparation.as_deref().map(|prepared| {
                    let materialized_initializer = prepared
                        .materialized_initializer
                        .map(|handle| self.copy_expression_handle(other, handle));
                    let value = materialized_initializer.unwrap_or(initializer);
                    Box::new(crate::item::ConstPreparation {
                        materialized_initializer,
                        pending_target_observation: prepared.pending_target_observation,
                        normalization: prepared.normalization.as_ref().map(|normalization| {
                            ConstInitializerNormalization {
                                authored_custody: normalization.authored_custody.clone(),
                                declaration_custody: normalization.declaration_custody.clone(),
                                authored_expression: self.copy_expression_handle(
                                    other,
                                    normalization.authored_expression,
                                ),
                                result_value: normalization.result_value.clone(),
                                selections: normalization.selections.clone(),
                                builtin_operators: normalization.builtin_operators.clone(),
                                call_selections: normalization.call_selections.clone(),
                            }
                        }),
                        pending_qualification: prepared.pending_qualification.as_ref().map(
                            |obligation| {
                                if obligation.declaration == constant.name.source_span()
                                    && obligation.destination == constant.type_reference
                                    && obligation.initializer == constant.value()
                                {
                                    ConstDestinationObligation {
                                        declaration: constant.name.source_span(),
                                        destination: type_reference,
                                        initializer: value,
                                    }
                                } else {
                                    obligation.clone()
                                }
                            },
                        ),
                        qualification: prepared.qualification.as_ref().and_then(|qualification| {
                            (qualification.destination == constant.type_reference
                                && qualification.initializer == constant.value())
                            .then_some(ConstQualification {
                                destination: type_reference,
                                initializer: value,
                            })
                        }),
                    })
                });
                Item::Const(ConstDefinition {
                    declaration,
                    preparation,
                })
            }
            Item::Data(data) => Item::Data(self.copy_data_definition(other, data)),
            Item::TypeAlias(alias) => Item::TypeAlias(crate::item::TypeAliasDefinition {
                name: alias.name.clone(),
                is_public: alias.is_public,
                lifetime_parameters: alias.lifetime_parameters.clone(),
                type_parameters: self.copy_type_parameter_span(other, alias.type_parameters),
                conformance_bounds: alias
                    .conformance_bounds
                    .iter()
                    .map(|bound| crate::item::GenericConformanceBound {
                        binder: bound.binder.clone(),
                        trait_name: bound.trait_name.clone(),
                        lifetime_arguments: bound.lifetime_arguments.clone(),
                        arguments: self.copy_type_reference_handle_span(other, bound.arguments),
                    })
                    .collect(),
                authored_expansion: self
                    .copy_type_reference_handle(other, alias.authored_expansion),
                expansion: self.copy_type_reference_handle(other, alias.expansion),
                asserted_multiplicity: alias.asserted_multiplicity,
            }),
            Item::Domain(domain) => Item::Domain(DomainDefinition {
                name: domain.name.clone(),
                type_parameters: self.copy_type_parameter_span(other, domain.type_parameters),
                target_type: self.copy_type_reference_handle(other, domain.target_type),
                index_arguments: self
                    .copy_type_reference_handle_span(other, domain.index_arguments),
                is_public: domain.is_public,
                alias: domain.alias.as_ref().map(|alias| DomainAliasDefinition {
                    constituents: alias
                        .constituents
                        .iter()
                        .map(|constituent| self.copy_item_identifier_span(other, *constituent))
                        .collect(),
                }),
                cleanup_selection: domain
                    .cleanup_selection
                    .as_ref()
                    .map(|selection| self.copy_static_machine_argument(other, selection)),
                authored_routes: domain.authored_routes.clone(),
                access_policy: domain
                    .access_policy
                    .as_ref()
                    .map(|policy| self.copy_static_machine_argument(other, policy)),
                classification: domain.classification,
                predicate_presence: domain.predicate_presence,
                facts: self.copy_domain_fact_span(other, domain.facts),
                operators: self.copy_operator_definition_span(other, domain.operators),
            }),
            Item::Measure(measure) => Item::Measure(self.copy_measure_definition(other, measure)),
            Item::Operator(operator) => {
                Item::Operator(self.copy_operator_definition(other, operator))
            }
            Item::Package(package) => Item::Package(PackageDeclaration {
                path: self.copy_item_identifier_span(other, package.path),
            }),
            Item::Proposition(proposition) => Item::Proposition(PropositionDefinition {
                name: proposition.name.clone(),
                is_public: proposition.is_public,
                type_parameters: self.copy_type_parameter_span(other, proposition.type_parameters),
                parameters: self.copy_state_parameter_handle_span(other, proposition.parameters),
                transparent_formula_source_span: proposition.transparent_formula_source_span,
                body: match proposition.body {
                    PropositionBody::Primitive => PropositionBody::Primitive,
                    PropositionBody::Witness { evidence } => PropositionBody::Witness {
                        evidence: self.copy_type_reference_handle(other, evidence),
                    },
                    PropositionBody::Transparent { proposition } => PropositionBody::Transparent {
                        proposition: self.copy_expression_handle(other, proposition),
                    },
                },
            }),
            Item::Use(use_item) => Item::Use(match use_item {
                UseItem::Namespace { path } => UseItem::Namespace {
                    path: self.copy_item_identifier_span(other, *path),
                },
                UseItem::File {
                    root,
                    components,
                    alias,
                    span,
                    qualifiers,
                } => UseItem::File {
                    qualifiers: qualifiers.clone(),
                    root: root.clone(),
                    components: self.copy_item_identifier_span(other, *components),
                    alias: alias.clone(),
                    span: *span,
                },
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
            cleanup_selection: data
                .cleanup_selection
                .as_ref()
                .map(|selection| self.copy_static_machine_argument(other, selection)),
            supply_mode: data.supply_mode,
            lifetime_parameters: data.lifetime_parameters.clone(),
            type_parameters: self.copy_type_parameter_span(other, data.type_parameters),
            conformance_bounds: data
                .conformance_bounds
                .iter()
                .map(|bound| GenericConformanceBound {
                    binder: bound.binder.clone(),
                    trait_name: bound.trait_name.clone(),
                    lifetime_arguments: bound.lifetime_arguments.clone(),
                    arguments: self.copy_type_reference_handle_span(other, bound.arguments),
                })
                .collect(),
            generic_instance: data
                .generic_instance
                .map(|origin| self.copy_type_reference_handle(other, origin)),
            properties: data.properties,
            quotient: data.quotient.as_ref().map(|quotient| QuotientDefinition {
                carrier: self.copy_type_reference_handle(other, quotient.carrier),
                relation: self.copy_item_identifier_span(other, quotient.relation),
                equivalence: quotient.equivalence.as_ref().map(|selection| {
                    QuotientEquivalenceSelection {
                        relation: self.copy_item_identifier_span(other, selection.relation),
                        trait_name: selection.trait_name.clone(),
                        trait_arguments: self
                            .copy_type_reference_handle_span(other, selection.trait_arguments),
                        conformance_name: selection.conformance_name.clone(),
                    }
                }),
            }),
            requires_facts: self.copy_domain_fact_span(other, data.requires_facts),
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

    /// Both a machine declaration and a callable signature introduce a lexical telescope.
    pub(crate) fn with_binders<T>(
        &mut self,
        other: &SyntaxTrees,
        parameters: HandleSpan<TypeParameter>,
        lifetimes: &[Identifier],
        copy: impl FnOnce(&mut Self) -> T,
    ) -> T {
        if self.scoped_arguments.is_empty() && self.scoped_lifetimes.is_empty() {
            return copy(self);
        }
        let arguments = std::mem::take(&mut self.scoped_arguments);
        let regions = std::mem::take(&mut self.scoped_lifetimes);
        self.scoped_arguments.extend(
            arguments
                .iter()
                .filter(|argument| {
                    !other
                        .items
                        .type_parameters(parameters)
                        .iter()
                        .any(|p| p.name.as_str(other) == argument.binder)
                })
                .cloned(),
        );
        self.scoped_lifetimes.extend(
            regions
                .iter()
                .filter(|(name, _)| !lifetimes.iter().any(|region| region.as_str(other) == name))
                .cloned(),
        );
        let result = copy(self);
        self.scoped_arguments = arguments;
        self.scoped_lifetimes = regions;
        result
    }

    pub(crate) fn copy_nested_machines(
        &mut self,
        other: &SyntaxTrees,
        children: HandleSpan<Item>,
    ) -> HandleSpan<Item> {
        self.copy_mapped_span(
            children.handles(),
            |this, handle| this.copy_item(other, other.items.item(handle)),
            |this, child| this.items.append_item(child),
        )
    }

    fn copy_machine(&mut self, other: &SyntaxTrees, machine: &Machine) -> Machine {
        self.with_binders(
            other,
            machine.type_parameters,
            &machine.lifetime_parameters,
            |scope| scope.copy_machine_fields(other, machine),
        )
    }

    fn copy_machine_fields(&mut self, other: &SyntaxTrees, machine: &Machine) -> Machine {
        Machine {
            target_family_interface: machine.target_family_interface,
            name: machine.name.clone(),
            generic_data_template: machine.generic_data_template.clone(),
            default_body_trait: machine.default_body_trait.clone(),
            attached_data: machine.attached_data.clone(),
            spelling: machine.spelling,
            is_public: machine.is_public,
            target: machine.target.clone(),
            authored_target: machine.authored_target.clone(),
            boundary: machine.boundary,
            bodyless: machine.bodyless,
            lifetime_parameters: machine.lifetime_parameters.clone(),
            type_parameters: self.copy_type_parameter_span(other, machine.type_parameters),
            satisfies: self.copy_mapped_span(
                other.items.satisfies_clauses(machine.satisfies).to_vec(),
                |scope, clause| SatisfiesClause {
                    trait_name: clause.trait_name,
                    lifetime_arguments: clause
                        .lifetime_arguments
                        .iter()
                        .map(|name| scope.copied_lifetime(other, name))
                        .collect(),
                    arguments: scope.copy_type_reference_handle_span(other, clause.arguments),
                    requirement: clause.requirement,
                    alias: clause.alias,
                    via: clause.via,
                    via_expression: if clause.via_expression.is_valid() {
                        scope.copy_expression_handle(other, clause.via_expression)
                    } else {
                        ExpressionHandle::invalid()
                    },
                    via_keyword_source_span: clause.via_keyword_source_span,
                },
                |this, clause| this.items.append_satisfies_clause(clause),
            ),
            conformance_bounds: machine
                .conformance_bounds
                .iter()
                .map(|bound| GenericConformanceBound {
                    binder: bound.binder.clone(),
                    trait_name: bound.trait_name.clone(),
                    lifetime_arguments: bound
                        .lifetime_arguments
                        .iter()
                        .map(|name| self.copied_lifetime(other, name))
                        .collect(),
                    arguments: self.copy_type_reference_handle_span(other, bound.arguments),
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
                ExpressionHandle::invalid()
            },
            service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
            service_reach_keyword_source_spans: machine.service_reach_keyword_source_spans.clone(),
            service_reaches: self.copy_type_reference_handle_span(other, machine.service_reaches),
            invokes: self.copy_type_reference_handle_span(other, machine.invokes),
            suspends_keyword_source_spans: machine.suspends_keyword_source_spans.clone(),
            blocks_keyword_source_spans: machine.blocks_keyword_source_spans.clone(),
            suspends: machine.suspends,
            blocks: machine.blocks,
            admitted_same_stack_bytes: machine.admitted_same_stack_bytes,
            declares_no_crash_route: machine.declares_no_crash_route,
            contracts: self.copy_capability_contract_span(other, machine.contracts),
            states: self.copy_state_handle_span(other, machine.states),
            nested_machines: self.copy_nested_machines(other, machine.nested_machines),
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
                .map(|bound| GenericConformanceBound {
                    binder: bound.binder.clone(),
                    trait_name: bound.trait_name.clone(),
                    lifetime_arguments: bound.lifetime_arguments.clone(),
                    arguments: self.copy_type_reference_handle_span(other, bound.arguments),
                })
                .collect(),
            parents: self.copy_type_reference_handle_span(other, trait_definition.parents),
            requires: self.copy_item_identifier_span(other, trait_definition.requires),
            machines: self.copy_state_signature_handle_span(other, trait_definition.machines),
            refines: trait_definition
                .refines
                .map(|base| self.copy_type_reference_handle(other, base)),
            refinement_clauses: trait_definition
                .refinement_clauses
                .iter()
                .map(|clause| TraitRefinementClause {
                    requirement: clause.requirement.clone(),
                    signature: self.copy_state_signature(other, &clause.signature),
                })
                .collect(),
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
                    TypeParameterKind::Type => TypeParameterKind::Type,
                    TypeParameterKind::Implementation => TypeParameterKind::Implementation,
                    TypeParameterKind::Const { type_reference } => TypeParameterKind::Const {
                        type_reference: this.copy_type_reference_handle(other, *type_reference),
                    },
                    TypeParameterKind::Value { type_reference } => TypeParameterKind::Value {
                        type_reference: this.copy_type_reference_handle(other, *type_reference),
                    },
                    TypeParameterKind::Proof { contract } => TypeParameterKind::Proof {
                        contract: this.copy_state_signature(other, contract),
                    },
                    TypeParameterKind::Machine { contract } => TypeParameterKind::Machine {
                        contract: contract.as_ref().map(|contract| match contract {
                            MachineParameterContract::Inferred => {
                                MachineParameterContract::Inferred
                            }
                            MachineParameterContract::RequirementIdentity => {
                                MachineParameterContract::RequirementIdentity
                            }
                            MachineParameterContract::Structural(signature) => {
                                MachineParameterContract::Structural(
                                    this.copy_state_signature(other, signature),
                                )
                            }
                            MachineParameterContract::Nominal {
                                requirement,
                                trait_arguments,
                            } => MachineParameterContract::Nominal {
                                requirement: this.copy_item_identifier_span(other, *requirement),
                                trait_arguments: this
                                    .copy_type_reference_handle_span(other, *trait_arguments),
                            },
                        }),
                    },
                    TypeParameterKind::Proposition { contract } => TypeParameterKind::Proposition {
                        contract: contract
                            .as_ref()
                            .map(|contract| PropositionParameterSignature {
                                name: contract.name.clone(),
                                parameters: this
                                    .copy_state_parameter_handle_span(other, contract.parameters),
                                result: this.copy_type_reference_handle(other, contract.result),
                            }),
                    },
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
                    signature: this.copy_state_signature(other, &state.signature),
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
                    requires_facts: this.copy_domain_fact_span(other, variant.requires_facts),
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
        for source in span.handles() {
            copied.push_contiguous(self.copy_proof_fact(other, source));
        }
        copied
    }
}
