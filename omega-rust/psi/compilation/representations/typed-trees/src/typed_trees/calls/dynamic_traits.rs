use crate::TypedTrees;
use crate::domain::ProofFact;
use crate::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
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

/// One requirement as a `dyn Trait<Args>` caller reaches it: the
/// requirement's signature with the declaring trait's parameters replaced by
/// the arguments that selector application implies.
///
/// Every implementation selected for this interface realizes exactly this
/// shape (conformance checking compares them), so lowering plans a dynamic
/// call's operands and result against it rather than against one
/// representative implementation.
///
/// A selector call `Trait::member(selector, data...)` targets `symbol`, the
/// slot, not the generic requirement. `parameters` mirrors the authored
/// operand list: position 0 is the selector, which dispatch consumes and no
/// implementation receives, and the requirement's instantiated formals
/// follow. Every analysis that pairs a call's operands with its target's
/// formals therefore sees a concrete roster of the right arity, exactly as it
/// does for a bodyless declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicSlotSignature {
    /// The slot a selector call targets.
    pub symbol: SymbolHandle,
    /// Normalized identity of the selector type (`dyn Shape<Item>`).
    pub interface_identity: crate::type_identity::TypeIdentityId,
    pub declaring_trait: SymbolHandle,
    /// The exact requirement this slot instantiates.
    pub requirement: SymbolHandle,
    /// Closed call contract when a static family tuple was selected. The
    /// declaring requirement above remains the roster and table identity.
    pub call_requirement: SymbolHandle,
    pub parameters: Vec<crate::signature::StateParameter>,
    pub return_type: TypeReferenceHandle,
}

impl DynamicSlotSignature {
    /// The requirement's instantiated formals, after the selector operand.
    pub fn data_parameters(&self) -> &[crate::signature::StateParameter] {
        self.parameters.get(1..).unwrap_or_default()
    }
}

impl TypedTrees {
    /// The instantiated signature of `requirement` under the selector type
    /// whose normalized identity is `interface_identity`.
    pub fn dynamic_slot(
        &self,
        interface_identity: crate::type_identity::TypeIdentityId,
        requirement: SymbolHandle,
    ) -> Option<&DynamicSlotSignature> {
        self.dynamic_slots.iter().find(|slot| {
            slot.call_requirement == requirement && slot.interface_identity == interface_identity
        })
    }

    /// The machine signature a dynamic slot's `requirement` resolves to in its
    /// declaring trait's machine list; `None` when either lookup misses.
    pub fn dynamic_slot_requirement(
        &self,
        slot: &DynamicSlotSignature,
    ) -> Option<&crate::signature::StateSignature> {
        let definition = self.trait_definition_by_symbol(slot.declaring_trait)?;
        self.trait_machine_signature_by_symbol(definition, slot.requirement)
    }

    /// The slot a selector call targets, by the slot's own symbol.
    pub fn dynamic_slot_by_symbol(&self, symbol: SymbolHandle) -> Option<&DynamicSlotSignature> {
        if !symbol.is_valid() {
            return None;
        }
        self.dynamic_slots.iter().find(|slot| slot.symbol == symbol)
    }

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

        Ok(())
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
                    for pattern in self
                        .expression_table
                        .match_pattern_dependencies(arm.pattern)
                    {
                        self.collect_expression_symbols(pattern, symbols);
                    }
                    self.collect_expression_symbols(arm.value, symbols);
                }
            }
            ExpressionNode::SavedSubject(saved) => {
                self.collect_expression_symbols(
                    self.expression_table.value_capture(saved.capture).subject,
                    symbols,
                );
                self.collect_expression_symbols(saved.pattern_head, symbols);
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
            | ExpressionNode::UndeterminedNaN(_)
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
        .find_map(|definition| (definition.symbol == symbol).then_some(definition.is_public))
        .or_else(|| program.machine(symbol).map(|machine| machine.is_public))
        .or_else(|| {
            program.traits().iter().find_map(|definition| {
                (definition.symbol == symbol).then_some(definition.is_public)
            })
        })
        .or_else(|| {
            program.domain_definitions().iter().find_map(|definition| {
                (definition.symbol == symbol).then_some(definition.is_public)
            })
        })
        .or_else(|| {
            program.propositions().iter().find_map(|definition| {
                (definition.symbol == symbol).then_some(definition.is_public)
            })
        })
        .or_else(|| {
            program.conformances().iter().find_map(|conformance| {
                (conformance.symbol == symbol).then_some(conformance.is_public)
            })
        })
        .or_else(|| {
            program.const_declarations().iter().find_map(|declaration| {
                (declaration.symbol == symbol).then_some(declaration.is_public)
            })
        })
        .or_else(|| {
            program
                .mathematical_definitions()
                .iter()
                .find_map(|definition| {
                    (definition.symbol == symbol).then_some(definition.is_public)
                })
        })
        .or_else(|| {
            program.operators().iter().find_map(|definition| {
                (definition.symbol == symbol).then_some(definition.is_public)
            })
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
            symbol, arguments, ..
        } => {
            symbols.push(*symbol);
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                collect_type_reference_symbols(program, *argument, symbols);
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

fn push_lifetime_name(lifetimes: &mut Vec<String>, name: &Identifier) {
    let name = name.as_str();
    if !name.is_empty() && !lifetimes.iter().any(|seen| seen == name) {
        lifetimes.push(name.to_owned());
    }
}
