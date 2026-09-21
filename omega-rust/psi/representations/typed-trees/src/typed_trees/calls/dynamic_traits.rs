use crate::TypedTrees;
use crate::domain::ProofFact;
use crate::expression::{ExpressionHandle, ExpressionNode, MatchPattern, StaticMachineArgument};
use crate::name::Identifier;
use crate::signature::StateSignature;
use crate::trait_definition::{DynamicSignatureIneligibility, TraitDefinition};
use crate::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

/// Why a requirement is absent from the non-signature portion of a local
/// dynamic trait surface. Signature shape stays with
/// [`TypedTrees::dynamic_signature_eligibility`]; these are the separate later
/// judgments that query deliberately defers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicNonSignatureIneligibility {
    /// The requirement's public contract names a declaration whose visibility
    /// does not reach the erased caller: a public surface cannot leak a
    /// satisfier-private or otherwise unexported identity.
    PrivateContractIdentity,
    /// The result names a borrow lifetime no input borrow can express; the
    /// erased caller shape retains no binder to project it from.
    UnprojectableResultLifetime,
    /// The normalized operational contract cannot fit the per-requirement
    /// envelope a local borrowed descriptor retains. Installation-bound
    /// service reach is a component property a local descriptor never
    /// carries, and native-only callback entries admit no ordinary erased
    /// call arity or value flow.
    UnfittableEnvelope,
}

impl TypedTrees {
    /// Derive the signature-only portion of one requirement's local dynamic
    /// eligibility. Contract privacy, lifetime projection, and operational
    /// envelope fitting are separate later judgments performed by
    /// [`Self::dynamic_non_signature_eligibility`]; this query never claims
    /// those checks have already happened.
    pub fn dynamic_signature_eligibility(
        &self,
        trait_definition: &TraitDefinition,
        requirement: &StateSignature,
    ) -> Result<(), DynamicSignatureIneligibility> {
        if trait_definition.is_boundary {
            return Err(DynamicSignatureIneligibility::BoundaryRequirement);
        }
        // Requirement-local const/value binders are admissible only as an
        // explicit finite family: the signature `where` clause must declare
        // the complete tuple roster so dispatch can retain one exact row per
        // tuple. Every other local-generic shape stays ineligible.
        if !self.state_signature_type_parameters(requirement).is_empty()
            && !matches!(
                self.finite_signature_family(requirement),
                crate::finite_family::FamilyProbe::Finite { .. }
            )
        {
            return Err(DynamicSignatureIneligibility::RequirementLocalGenerics);
        }

        let parameters = self.state_signature_parameters(requirement);
        let receivers = parameters
            .iter()
            .filter(|parameter| parameter.is_self)
            .collect::<Vec<_>>();
        let receiver = match receivers.as_slice() {
            [receiver] => *receiver,
            [] => return Err(DynamicSignatureIneligibility::MissingBorrowedReceiver),
            _ => return Err(DynamicSignatureIneligibility::MultipleReceivers),
        };
        if !is_reference_to_self(self, receiver.type_reference) {
            return Err(DynamicSignatureIneligibility::ByValueReceiver);
        }
        if parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .any(|parameter| type_reference_contains_self(self, parameter.type_reference))
        {
            return Err(DynamicSignatureIneligibility::SelfOutsideReceiver);
        }
        if requirement.return_type.is_valid()
            && type_reference_contains_self(self, requirement.return_type)
        {
            return Err(DynamicSignatureIneligibility::SelfResult);
        }
        Ok(())
    }

    /// Requirements that survive the currently implemented signature-only
    /// dynamic-surface projection, in trait declaration order.
    pub fn dynamic_signature_surface<'program>(
        &'program self,
        trait_definition: &'program TraitDefinition,
    ) -> impl Iterator<Item = &'program StateSignature> + 'program {
        self.trait_machine_signatures(trait_definition)
            .iter()
            .filter(|requirement| {
                self.dynamic_signature_eligibility(trait_definition, requirement)
                    .is_ok()
            })
    }

    /// Derive the non-signature portion of one requirement's local dynamic
    /// eligibility: contract privacy, lifetime projection, and operational
    /// envelope fitting. The signature judgment stays with
    /// [`Self::dynamic_signature_eligibility`]; the two compose independently
    /// and neither claims the other has already run.
    pub fn dynamic_non_signature_eligibility(
        &self,
        requirement: &StateSignature,
    ) -> Result<(), DynamicNonSignatureIneligibility> {
        if self.requirement_contract_names_private_identity(requirement) {
            return Err(DynamicNonSignatureIneligibility::PrivateContractIdentity);
        }
        if !self.result_lifetimes_expressible_from_inputs(requirement) {
            return Err(DynamicNonSignatureIneligibility::UnprojectableResultLifetime);
        }
        if requirement.service_reach_is_installation_bound
            || !requirement.native_callback_parameters.is_empty()
        {
            return Err(DynamicNonSignatureIneligibility::UnfittableEnvelope);
        }
        Ok(())
    }

    /// Requirements surviving BOTH the signature projection and the later
    /// non-signature judgments, in trait declaration order. This is the
    /// complete local dynamic surface.
    pub fn dynamic_surface<'program>(
        &'program self,
        trait_definition: &'program TraitDefinition,
    ) -> impl Iterator<Item = &'program StateSignature> + 'program {
        self.dynamic_signature_surface(trait_definition)
            .filter(|requirement| self.dynamic_non_signature_eligibility(requirement).is_ok())
    }

    /// Contract privacy: the requirement's public contract — its requires,
    /// ensures, crash routes, and `where` facts — must name no identity a
    /// dynamic caller cannot see. Contract-local binders carry no independent
    /// visibility and are not declarations, so only symbols that resolve to
    /// an unexported declaration count against the surface.
    fn requirement_contract_names_private_identity(&self, requirement: &StateSignature) -> bool {
        let mut symbols = Vec::new();
        for contract in self.state_signature_contracts(requirement) {
            for fact in self.proof_facts.span_or_empty(contract.facts) {
                self.collect_proof_fact_symbols(fact, &mut symbols);
            }
        }
        for fact in self.proof_facts.span_or_empty(requirement.where_facts) {
            self.collect_proof_fact_symbols(fact, &mut symbols);
        }
        symbols
            .iter()
            .any(|symbol| contract_identity_is_public(self, *symbol) == Some(false))
    }

    /// Lifetime projection: every borrow lifetime the result names must be
    /// expressible from an input. The erased caller shape retains only the
    /// authored parameter borrows, so a result lifetime no parameter binds
    /// cannot be projected through a dynamic call.
    fn result_lifetimes_expressible_from_inputs(&self, requirement: &StateSignature) -> bool {
        let mut input_lifetimes = Vec::new();
        for parameter in self.state_signature_parameters(requirement) {
            collect_type_reference_lifetimes(self, parameter.type_reference, &mut input_lifetimes);
        }
        if !requirement.return_type.is_valid() {
            return true;
        }
        let mut result_lifetimes = Vec::new();
        collect_type_reference_lifetimes(self, requirement.return_type, &mut result_lifetimes);
        result_lifetimes
            .iter()
            .all(|lifetime| input_lifetimes.contains(lifetime))
    }

    fn collect_proof_fact_symbols(&self, fact: &ProofFact, symbols: &mut Vec<SymbolHandle>) {
        match fact {
            ProofFact::Expression(expression) => {
                self.collect_expression_symbols(*expression, symbols);
            }
            ProofFact::Membership(membership) => {
                symbols.push(membership.domain_symbol);
                self.collect_expression_symbols(membership.value, symbols);
                for argument in self
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments)
                {
                    collect_type_reference_symbols(self, *argument, symbols);
                }
            }
            ProofFact::Proposition(application) => {
                symbols.push(application.proposition);
                for binder in application.binder_arguments.iter() {
                    symbols.push(binder.symbol);
                }
                for argument in self
                    .expression_table
                    .expression_handles(application.arguments)
                {
                    self.collect_expression_symbols(*argument, symbols);
                }
            }
        }
    }

    /// Symbols an expression names on its public face: declaration targets
    /// and carriers, not the dispatch internals of an already-selected static
    /// call (`static_requirement_dispatch`) or a binder occurrence
    /// (`static_machine_parameter`).
    fn collect_expression_symbols(
        &self,
        expression: ExpressionHandle,
        symbols: &mut Vec<SymbolHandle>,
    ) {
        if !expression.is_valid() {
            return;
        }
        match self.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                self.collect_expression_symbols(dispatch.subject, symbols);
                for arm in self.expression_table.match_arms(dispatch.arms) {
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        self.collect_expression_symbols(pattern, symbols);
                    }
                    self.collect_expression_symbols(arm.value, symbols);
                }
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in self.expression_table.expression_handles(*values) {
                    self.collect_expression_symbols(*value, symbols);
                }
            }
            ExpressionNode::Atomic(atomic) => {
                self.collect_expression_symbols(atomic.value, symbols);
                self.collect_expression_symbols(atomic.result, symbols);
            }
            ExpressionNode::Binary(binary) => {
                self.collect_expression_symbols(binary.left, symbols);
                self.collect_expression_symbols(binary.right, symbols);
            }
            ExpressionNode::Borrow(borrow) => {
                self.collect_expression_symbols(borrow.target, symbols);
            }
            ExpressionNode::Call(call) => {
                symbols.push(call.target_symbol);
                self.collect_expression_symbols(call.receiver, symbols);
                for argument in call.machine_arguments.iter() {
                    self.collect_static_argument_symbols(argument, symbols);
                }
                if let Some(operation) = &call.private_layout_operation {
                    self.collect_static_argument_symbols(&operation.selected_slot, symbols);
                }
                if let Some(operation) = &call.quotient_operation {
                    self.collect_static_argument_symbols(
                        &operation.representative_operation,
                        symbols,
                    );
                    for theorem in operation.theorem_evidence.iter() {
                        self.collect_static_argument_symbols(&theorem.application, symbols);
                    }
                }
                for argument in self.expression_table.expression_handles(call.arguments) {
                    self.collect_expression_symbols(*argument, symbols);
                }
            }
            ExpressionNode::Cast(cast) => {
                self.collect_expression_symbols(cast.value, symbols);
                collect_type_reference_symbols(self, cast.target_type, symbols);
                collect_type_reference_symbols(self, cast.result_type, symbols);
                symbols.push(cast.semantic_domain_symbol);
            }
            ExpressionNode::Indexed(indexed) => {
                self.collect_expression_symbols(indexed.collection, symbols);
                self.collect_expression_symbols(indexed.index, symbols);
            }
            ExpressionNode::Member(member) => {
                self.collect_expression_symbols(member.receiver, symbols);
                symbols.push(member.member_symbol);
            }
            ExpressionNode::Name(path) => {
                symbols.push(path.head_symbol);
                symbols.push(path.symbol);
                symbols.extend(
                    self.expression_table
                        .name_path_member_symbols(path.member_symbols)
                        .iter()
                        .copied(),
                );
            }
            ExpressionNode::Range(range) => {
                self.collect_expression_symbols(range.start, symbols);
                self.collect_expression_symbols(range.end, symbols);
            }
            ExpressionNode::StructLiteral(literal) => {
                symbols.push(literal.type_symbol);
                if let Some(case) = literal.case_symbol {
                    symbols.push(case);
                }
                for field in self.expression_table.struct_fields(literal.fields) {
                    symbols.push(field.field_symbol);
                    self.collect_expression_symbols(field.value, symbols);
                }
            }
            ExpressionNode::Unary(unary) => {
                self.collect_expression_symbols(unary.operand, symbols);
            }
            ExpressionNode::ZeroValue(type_reference) => {
                collect_type_reference_symbols(self, *type_reference, symbols);
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_) => {}
        }
    }

    fn collect_static_argument_symbols(
        &self,
        argument: &StaticMachineArgument,
        symbols: &mut Vec<SymbolHandle>,
    ) {
        symbols.push(argument.symbol);
        if argument.type_reference.is_valid() {
            collect_type_reference_symbols(self, argument.type_reference, symbols);
        }
        if let Some(application) = &argument.application {
            for nested in application.arguments.iter() {
                self.collect_static_argument_symbols(nested, symbols);
            }
        }
    }
}

/// `Some(visibility)` when the symbol resolves to a visibility-bearing
/// declaration; `None` for contract-local binders, parameters, fields, and
/// every other symbol that owns no independent exported identity.
fn contract_identity_is_public(program: &TypedTrees, symbol: SymbolHandle) -> Option<bool> {
    if !symbol.is_valid() {
        return None;
    }
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)
        .map(|definition| definition.is_public)
        .or_else(|| {
            program
                .machines()
                .iter()
                .find(|machine| machine.symbol == symbol)
                .map(|machine| machine.is_public)
        })
        .or_else(|| {
            program
                .traits()
                .iter()
                .find(|definition| definition.symbol == symbol)
                .map(|definition| definition.is_public)
        })
        .or_else(|| {
            program
                .domain_definitions()
                .iter()
                .find(|definition| definition.symbol == symbol)
                .map(|definition| definition.is_public)
        })
        .or_else(|| {
            program
                .propositions()
                .iter()
                .find(|definition| definition.symbol == symbol)
                .map(|definition| definition.is_public)
        })
        .or_else(|| {
            program
                .conformances()
                .iter()
                .find(|conformance| conformance.symbol == symbol)
                .map(|conformance| conformance.is_public)
        })
        .or_else(|| {
            program
                .const_declarations()
                .iter()
                .find(|declaration| declaration.symbol == symbol)
                .map(|declaration| declaration.is_public)
        })
        .or_else(|| {
            program
                .mathematical_definitions()
                .iter()
                .find(|definition| definition.symbol == symbol)
                .map(|definition| definition.is_public)
        })
        .or_else(|| {
            program
                .operators()
                .iter()
                .find(|definition| definition.symbol == symbol)
                .map(|definition| definition.is_public)
        })
}

/// Declaration symbols a type reference names, recursing through reference,
/// constraint, generic-argument, and container interiors.
fn collect_type_reference_symbols(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &mut Vec<SymbolHandle>,
) {
    if !type_reference.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, .. } => symbols.push(*symbol),
        TypeReferenceNode::Reference { referee, .. } => {
            collect_type_reference_symbols(program, *referee, symbols)
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_type_reference_symbols(program, *base_type, symbols);
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::Domain(domain) => {
                        symbols.push(domain.symbol);
                        for argument in &domain.arguments {
                            collect_type_reference_symbols(program, *argument, symbols);
                        }
                    }
                    TypeConstraintNode::Range {
                        minimum, maximum, ..
                    } => {
                        program.collect_expression_symbols(*minimum, symbols);
                        program.collect_expression_symbols(*maximum, symbols);
                    }
                    TypeConstraintNode::Named(_) | TypeConstraintNode::ArithmeticDomain(_) => {}
                }
            }
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            symbols.push(*base_symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_type_reference_symbols(program, *argument, symbols);
            }
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            collect_type_reference_symbols(program, *element_type, symbols);
            if let crate::types::FixedArrayLength::ConstParameter { symbol, .. } = length {
                symbols.push(*symbol);
            }
        }
        TypeReferenceNode::Slice { element_type } => {
            collect_type_reference_symbols(program, *element_type, symbols)
        }
        TypeReferenceNode::ConstExpression(expression) => {
            program.collect_expression_symbols(*expression, symbols)
        }
        TypeReferenceNode::DynamicTrait {
            symbol,
            conformance,
            ..
        } => {
            symbols.push(*symbol);
            if let Some(conformance) = conformance {
                symbols.push(*conformance);
            }
        }
        TypeReferenceNode::Unit => {}
    }
}

/// Borrow-region names a type reference binds or projects, recursing through
/// reference, constraint, generic-argument, and container interiors.
fn collect_type_reference_lifetimes(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    lifetimes: &mut Vec<String>,
) {
    if !type_reference.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee, lifetime, ..
        } => {
            if let Some(lifetime) = lifetime {
                push_lifetime_name(lifetimes, lifetime);
            }
            collect_type_reference_lifetimes(program, *referee, lifetimes);
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            collect_type_reference_lifetimes(program, *base_type, lifetimes)
        }
        TypeReferenceNode::Generic {
            lifetime_arguments,
            arguments,
            ..
        } => {
            for lifetime in lifetime_arguments {
                push_lifetime_name(lifetimes, lifetime);
            }
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_type_reference_lifetimes(program, *argument, lifetimes);
            }
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            collect_type_reference_lifetimes(program, *element_type, lifetimes)
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => {}
    }
}

fn is_reference_to_self(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    matches!(
        program.type_reference_table.type_reference(*referee),
        TypeReferenceNode::Named { name, .. } if name.as_str() == "Self"
    )
}

fn type_reference_contains_self(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => name.as_str() == "Self",
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_contains_self(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_contains_self(program, *base_type)
        }
        TypeReferenceNode::Generic { arguments, .. } => program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .any(|argument| type_reference_contains_self(program, *argument)),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            type_reference_contains_self(program, *element_type)
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

fn push_lifetime_name(lifetimes: &mut Vec<String>, name: &Identifier) {
    let name = name.as_str();
    if !name.is_empty() && !lifetimes.iter().any(|seen| seen == name) {
        lifetimes.push(name.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ProofMembershipFact;
    use crate::signature::{NativeCallbackParameter, SignatureContract, StateParameter};

    fn self_reference(program: &mut TypedTrees) -> TypeReferenceHandle {
        let self_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("Self"),
            });
        program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: self_type,
                access: language_core::ReferenceAccess::Shared,
                lifetime: None,
            })
    }

    fn borrowed_u8(program: &mut TypedTrees, lifetime: Option<&str>) -> TypeReferenceHandle {
        let u8_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("u8"),
            });
        program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: u8_type,
                access: language_core::ReferenceAccess::Shared,
                lifetime: lifetime.map(Identifier::generated),
            })
    }

    /// A signature with one borrowed `&self` receiver plus whatever shape the
    /// test configures before the requirement is stored on the trait.
    fn requirement(
        build: impl Fn(&mut TypedTrees, &mut StateSignature),
    ) -> (TypedTrees, TraitDefinition) {
        let mut program = TypedTrees::default();
        let mut trait_definition = TraitDefinition {
            name: Identifier::generated("Iface"),
            ..TraitDefinition::default()
        };
        let mut signature = StateSignature {
            name: Identifier::generated("requirement"),
            ..StateSignature::default()
        };
        let receiver = StateParameter {
            name: Identifier::generated("self"),
            type_reference: self_reference(&mut program),
            is_self: true,
            ..StateParameter::default()
        };
        program.push_state_signature_parameter(&mut signature, receiver);
        build(&mut program, &mut signature);
        program.push_trait_machine_signature(&mut trait_definition, signature);
        program.push_trait_definition(trait_definition.clone());
        (program, trait_definition)
    }

    fn sole_requirement<'program>(
        program: &'program TypedTrees,
        trait_definition: &'program TraitDefinition,
    ) -> &'program StateSignature {
        &program.trait_machine_signatures(trait_definition)[0]
    }

    #[test]
    fn result_lifetime_bound_by_an_input_projects() {
        let (program, trait_definition) = requirement(|program, signature| {
            let input = borrowed_u8(program, Some("a"));
            program.push_state_signature_parameter(
                signature,
                StateParameter {
                    name: Identifier::generated("source"),
                    type_reference: input,
                    ..StateParameter::default()
                },
            );
            signature.return_type = borrowed_u8(program, Some("a"));
        });

        let requirement = sole_requirement(&program, &trait_definition);
        assert_eq!(
            program.dynamic_non_signature_eligibility(requirement),
            Ok(())
        );
        let surface = program
            .dynamic_surface(&trait_definition)
            .map(|signature| signature.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(surface, vec!["requirement"]);
    }

    #[test]
    fn result_lifetime_no_input_binds_is_unprojectable() {
        let (program, trait_definition) = requirement(|program, signature| {
            signature.return_type = borrowed_u8(program, Some("a"));
        });

        let requirement = sole_requirement(&program, &trait_definition);
        assert_eq!(
            program.dynamic_non_signature_eligibility(requirement),
            Err(DynamicNonSignatureIneligibility::UnprojectableResultLifetime)
        );
        assert_eq!(program.dynamic_surface(&trait_definition).count(), 0);
    }

    #[test]
    fn contract_naming_a_private_domain_is_not_surface() {
        let (program, trait_definition) = requirement(|program, signature| {
            let domain = crate::domain::DomainDefinition {
                symbol: SymbolHandle::from_arena_index(40),
                is_public: false,
                ..crate::domain::DomainDefinition::default()
            };
            program.push_domain_definition(domain);
            let mut contract = SignatureContract {
                kind: crate::signature::SignatureContractKind::Requires,
                ..SignatureContract::default()
            };
            program.proof_facts.append_to_span(
                &mut contract.facts,
                ProofFact::Membership(ProofMembershipFact {
                    domain_symbol: SymbolHandle::from_arena_index(40),
                    ..ProofMembershipFact::default()
                }),
            );
            program.push_state_signature_contract(signature, contract);
        });

        let requirement = sole_requirement(&program, &trait_definition);
        assert_eq!(
            program.dynamic_non_signature_eligibility(requirement),
            Err(DynamicNonSignatureIneligibility::PrivateContractIdentity)
        );
        assert_eq!(program.dynamic_surface(&trait_definition).count(), 0);
    }

    #[test]
    fn contract_naming_a_public_domain_remains_surface() {
        let (program, trait_definition) = requirement(|program, signature| {
            let domain = crate::domain::DomainDefinition {
                symbol: SymbolHandle::from_arena_index(40),
                is_public: true,
                ..crate::domain::DomainDefinition::default()
            };
            program.push_domain_definition(domain);
            let mut contract = SignatureContract {
                kind: crate::signature::SignatureContractKind::Requires,
                ..SignatureContract::default()
            };
            program.proof_facts.append_to_span(
                &mut contract.facts,
                ProofFact::Membership(ProofMembershipFact {
                    domain_symbol: SymbolHandle::from_arena_index(40),
                    ..ProofMembershipFact::default()
                }),
            );
            program.push_state_signature_contract(signature, contract);
        });

        let requirement = sole_requirement(&program, &trait_definition);
        assert_eq!(
            program.dynamic_non_signature_eligibility(requirement),
            Ok(())
        );
    }

    #[test]
    fn installation_bound_reach_cannot_fit_a_local_envelope() {
        let (program, trait_definition) = requirement(|_program, signature| {
            signature.service_reach_is_installation_bound = true;
        });

        let requirement = sole_requirement(&program, &trait_definition);
        assert_eq!(
            program.dynamic_non_signature_eligibility(requirement),
            Err(DynamicNonSignatureIneligibility::UnfittableEnvelope)
        );
        assert_eq!(program.dynamic_surface(&trait_definition).count(), 0);
    }

    #[test]
    fn native_callback_parameter_cannot_fit_an_erased_envelope() {
        let (program, trait_definition) = requirement(|_program, signature| {
            signature
                .native_callback_parameters
                .push(NativeCallbackParameter::default());
        });

        let requirement = sole_requirement(&program, &trait_definition);
        assert_eq!(
            program.dynamic_non_signature_eligibility(requirement),
            Err(DynamicNonSignatureIneligibility::UnfittableEnvelope)
        );
        assert_eq!(program.dynamic_surface(&trait_definition).count(), 0);
    }
}
