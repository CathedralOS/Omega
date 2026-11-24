//! Transplant expressions and their referenced declarations while remapping exact symbol identities.
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_resolved::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
use crate::source_resolved::domain::ProofFact;
use crate::source_resolved::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, StaticMachineArgument,
};
use crate::source_resolved::types::{
    FixedArrayLength, GenericApplicationOrigin, OperandLifetimeOwner, OperandLifetimeOwnerHandle,
    TypeConstraint, TypeReference,
};
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;

impl SymbolResolvedTrees {
    pub fn copy_expression_with_symbols(
        &mut self,
        source: ExpressionHandle,
        substitutions: &[(SymbolHandle, SymbolHandle)],
    ) -> ExpressionHandle {
        SymbolCopy {
            program: self,
            substitutions,
            type_arguments: &[],
            types: Vec::new(),
            callables: Vec::new(),
            lifetime_owners: Vec::new(),
            visited_payloads: Vec::new(),
        }
        .expression(source)
    }

    pub fn copy_proof_fact_with_symbols(
        &mut self,
        source: Handle<ProofFact>,
        substitutions: &[(SymbolHandle, SymbolHandle)],
    ) -> Handle<ProofFact> {
        SymbolCopy {
            program: self,
            substitutions,
            type_arguments: &[],
            types: Vec::new(),
            callables: Vec::new(),
            lifetime_owners: Vec::new(),
            visited_payloads: Vec::new(),
        }
        .proof_fact(source)
    }

    /// Copy a selected declaration's type operands into an application.
    /// Exact binder identities select replacements; retained source names
    /// continue to identify the declarations which supplied the type.
    pub fn copy_type_arguments(
        &mut self,
        source: HandleSpan<TypeReference>,
        type_arguments: &[(SymbolHandle, TypeReference)],
    ) -> HandleSpan<TypeReference> {
        let substitutions = type_arguments
            .iter()
            .filter_map(|(formal, actual)| {
                if let TypeReference::Named { symbol, .. } = actual {
                    Some((*formal, *symbol))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        SymbolCopy {
            program: self,
            substitutions: &substitutions,
            type_arguments,
            types: Vec::new(),
            callables: Vec::new(),
            lifetime_owners: Vec::new(),
            visited_payloads: Vec::new(),
        }
        .type_span(source)
    }

    /// Transport the complete callable type to another declaration's
    /// parameter. Local binders receive derived children of that exact
    /// parameter; supplied outer operands retain their own context.
    /// Copying only parameter/result types would lose the contract and
    /// sharing the original locals would capture another use's binders.
    pub fn copy_callable_signature(
        &mut self,
        source: &crate::source_resolved::signature::StateSignature,
        destination: SymbolHandle,
        type_arguments: &[(SymbolHandle, TypeReference)],
        selections: &[(SymbolHandle, SymbolHandle)],
    ) -> crate::source_resolved::signature::StateSignature {
        let mut substitutions = vec![(source.symbol, destination)];
        let mut cursor = 0;
        while cursor < substitutions.len() {
            let (original, copied) = substitutions[cursor];
            cursor += 1;
            let children = self
                .symbols
                .child_handles(original)
                .into_iter()
                .flatten()
                .map(|child| {
                    (
                        child,
                        self.symbols.get(child).kind,
                        self.symbols.name(child).to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            let copies = self.symbols.append_generated_children_from(
                copied,
                children
                    .iter()
                    .map(|(origin, kind, name)| (*origin, *kind, name.as_str())),
            );
            substitutions.extend(
                children
                    .iter()
                    .zip(copies.handles())
                    .map(|((original, _, _), copied)| (*original, copied)),
            );
        }
        substitutions.extend(
            selections
                .iter()
                .copied()
                .filter(|(original, _)| *original != source.symbol),
        );
        substitutions.extend(type_arguments.iter().filter_map(|(formal, actual)| {
            if let TypeReference::Named { symbol, .. } = actual {
                Some((*formal, *symbol))
            } else {
                None
            }
        }));
        SymbolCopy {
            program: self,
            substitutions: &substitutions,
            type_arguments,
            types: Vec::new(),
            callables: Vec::new(),
            lifetime_owners: Vec::new(),
            visited_payloads: Vec::new(),
        }
        .signature(source)
    }
}

struct SymbolCopy<'program, 'substitutions> {
    program: &'program mut SymbolResolvedTrees,
    substitutions: &'substitutions [(SymbolHandle, SymbolHandle)],
    type_arguments: &'substitutions [(SymbolHandle, TypeReference)],
    types: Vec<(Handle<TypeReference>, Handle<TypeReference>)>,
    callables: Vec<(
        crate::source_resolved::callable_values::RuntimeCallableContractHandle,
        crate::source_resolved::callable_values::RuntimeCallableContractHandle,
    )>,
    lifetime_owners: Vec<(OperandLifetimeOwnerHandle, OperandLifetimeOwnerHandle)>,
    visited_payloads: Vec<ExpressionHandle>,
}

impl SymbolCopy<'_, '_> {
    fn signature(
        &mut self,
        source: &crate::source_resolved::signature::StateSignature,
    ) -> crate::source_resolved::signature::StateSignature {
        let mut copied = source.clone();
        copied.symbol = self.symbol(source.symbol);
        let parameters = self
            .program
            .data_type_parameters(source.type_parameters)
            .to_vec();
        let parameters = parameters
            .into_iter()
            .map(|parameter| self.parameter(parameter))
            .collect::<Vec<_>>();
        copied.type_parameters = self
            .program
            .declarations_mut()
            .data_type_parameters
            .insert_many(parameters);
        for bound in &mut copied.conformance_bounds {
            bound.binder = self.symbol(bound.binder);
            bound.trait_symbol = self.symbol(bound.trait_symbol);
            bound.arguments = self.type_span(bound.arguments);
        }
        copied.parameters = self.parameters(source.parameters);
        copied.return_type = source
            .return_type
            .clone()
            .map(|reference| self.type_payload(reference));
        let invokes = self.program.signature_invokes(source.invokes).to_vec();
        copied.invokes = self
            .program
            .declarations_mut()
            .signature_invokes
            .insert_many(invokes);
        let contracts = self.program.signature_contracts(source.contracts).to_vec();
        let contracts = contracts
            .into_iter()
            .map(|mut contract| {
                if let crate::source_resolved::signature::SignatureContractKind::EnsuresForResultCase {
                    result_data,
                    result_case,
                } = &mut contract.kind
                {
                    *result_data = self.symbol(*result_data);
                    *result_case = self.symbol(*result_case);
                }
                contract.facts = self.proof_span(contract.facts);
                contract
            })
            .collect::<Vec<_>>();
        copied.contracts = self
            .program
            .declarations_mut()
            .signature_contracts
            .insert_many(contracts);
        let rows = self
            .program
            .authored_service_reach_rows_for(source.symbol)
            .cloned()
            .collect::<Vec<_>>();
        for mut row in rows {
            row.owner = copied.symbol;
            // Runtime rows can share their formal namespace while their
            // supplying scope changes. Preserve each distinct authored
            // row without duplicating an unchanged declaration record.
            if !self.program.authored_service_reach_rows.contains(&row) {
                self.program.authored_service_reach_rows.push(row);
            }
        }
        copied
    }

    fn parameters(
        &mut self,
        source: HandleSpan<crate::source_resolved::signature::StateParameter>,
    ) -> HandleSpan<crate::source_resolved::signature::StateParameter> {
        let parameters = self.program.state_parameters(source).to_vec();
        let parameters = parameters
            .into_iter()
            .map(|mut parameter| {
                parameter.symbol = self.symbol(parameter.symbol);
                parameter.type_reference = self.type_payload(parameter.type_reference);
                parameter
            })
            .collect::<Vec<_>>();
        self.program
            .declarations_mut()
            .state_parameters
            .insert_many(parameters)
    }

    fn parameter(&mut self, mut parameter: TypeParameter) -> TypeParameter {
        parameter.symbol = self.symbol(parameter.symbol);
        parameter.kind = match parameter.kind {
            TypeParameterKind::Const { type_reference } => TypeParameterKind::Const {
                type_reference: self.type_payload(type_reference),
            },
            TypeParameterKind::Value { type_reference } => TypeParameterKind::Value {
                type_reference: self.type_payload(type_reference),
            },
            TypeParameterKind::Proof { contract } => TypeParameterKind::Proof {
                contract: self.signature(&contract),
            },
            TypeParameterKind::Machine { contract } => TypeParameterKind::Machine {
                contract: match contract {
                    MachineParameterContract::Structural(signature) => {
                        MachineParameterContract::Structural(self.signature(&signature))
                    }
                    MachineParameterContract::AuthoredNominal {
                        requirement,
                        trait_arguments,
                    } => MachineParameterContract::AuthoredNominal {
                        requirement,
                        trait_arguments: self.type_span(trait_arguments),
                    },
                    MachineParameterContract::Nominal {
                        trait_definition,
                        requirement,
                        authored_path,
                        trait_arguments,
                    } => MachineParameterContract::Nominal {
                        trait_definition: self.symbol(trait_definition),
                        requirement: self.symbol(requirement),
                        authored_path,
                        trait_arguments: self.type_span(trait_arguments),
                    },
                    other => other,
                },
            },
            TypeParameterKind::Proposition { mut contract } => {
                contract.parameters = self.parameters(contract.parameters);
                TypeParameterKind::Proposition { contract }
            }
            other => other,
        };
        parameter
    }

    fn proof_fact(&mut self, source: Handle<ProofFact>) -> Handle<ProofFact> {
        if !source.is_valid() {
            return source;
        }
        let (fact, source_span) = self.proof_fact_payload(source);
        let copied = self.program.declarations_mut().proof_facts.insert(fact);
        if let Some(source_span) = source_span {
            self.program.set_proof_fact_source_span(copied, source_span);
        }
        copied
    }

    fn proof_fact_payload(
        &mut self,
        source: Handle<ProofFact>,
    ) -> (ProofFact, Option<source_text::SourceSpan>) {
        let source_span = self.program.proof_fact_source_span(source);
        let fact = self.program.declarations().proof_facts.get(source).clone();
        (self.fact_payload(fact), source_span)
    }

    fn fact_payload(&mut self, fact: ProofFact) -> ProofFact {
        match fact {
            ProofFact::Expression(expression) => ProofFact::Expression(self.expression(expression)),
            ProofFact::Membership(mut membership) => {
                membership.value = self.expression(membership.value);
                let names = self.program.domain_path_members(membership.domain).to_vec();
                membership.domain = self
                    .program
                    .declarations_mut()
                    .domain_path_members
                    .insert_many(names);
                membership.domain_symbol = self.symbol(membership.domain_symbol);
                membership.domain_arguments = self.type_span(membership.domain_arguments);
                membership.receiving_application = self.receiving_application(
                    membership.receiving_application,
                    membership.domain_arguments,
                    Some(membership.domain_symbol),
                );
                ProofFact::Membership(membership)
            }
        }
    }

    fn symbol(&self, source: SymbolHandle) -> SymbolHandle {
        if !source.is_valid() {
            return source;
        }
        self.substitutions
            .iter()
            .find(|(original, _)| *original == source)
            .map_or(source, |(_, replacement)| *replacement)
    }

    fn expression(&mut self, source: ExpressionHandle) -> ExpressionHandle {
        if !source.is_valid() {
            return source;
        }
        let copied = self.program.expression_table_mut().copy_from_self(source);
        self.expression_payloads(copied);
        copied
    }

    fn expression_payloads(&mut self, copied: ExpressionHandle) {
        if !copied.is_valid() {
            return;
        }
        let position = copied.arena_index() as usize;
        if self.visited_payloads.get(position) == Some(&copied) {
            return;
        }
        if self.visited_payloads.len() <= position {
            self.visited_payloads
                .resize(position + 1, Handle::invalid());
        }
        self.visited_payloads[position] = copied;
        let authored = self
            .program
            .expression_table()
            .materialization_source(copied);
        self.expression_payloads(authored);
        let mut node = self.program.expression_table().expression(copied).clone();
        match &mut node {
            ExpressionNode::Match(dispatch) => {
                self.expression_payloads(dispatch.subject);
                let arms = self
                    .program
                    .expression_table()
                    .match_arms(dispatch.arms)
                    .to_vec();
                for (ordinal, mut arm) in arms.into_iter().enumerate() {
                    match arm.pattern {
                        MatchPattern::Value(pattern) | MatchPattern::Guard(pattern) => {
                            self.expression_payloads(pattern)
                        }
                        MatchPattern::Wildcard => {}
                        MatchPattern::Destructure(mut pattern) => {
                            pattern.application_type =
                                self.type_reference(pattern.application_type);
                            arm.pattern = MatchPattern::Destructure(pattern);
                            self.expression_payloads(pattern.head);
                            self.expression_payloads(pattern.guard);
                            let fields = self
                                .program
                                .expression_table()
                                .match_fields(pattern.fields)
                                .to_vec();
                            for (ordinal, mut field) in fields.into_iter().enumerate() {
                                field.field_symbol = self.symbol(field.field_symbol);
                                if let super::MatchFieldAction::Equal(value) = field.action {
                                    self.expression_payloads(value);
                                }
                                *self
                                    .program
                                    .expression_table_mut()
                                    .match_field_mut(pattern.fields.handle_at(ordinal as u32)) =
                                    field;
                            }
                        }
                    }
                    self.expression_payloads(arm.value);
                    self.program
                        .expression_table_mut()
                        .match_arm_mut(dispatch.arms.handle_at(ordinal as u32))
                        .pattern = arm.pattern;
                }
            }
            // A block's statements stay with their body; only executable
            // bodies hold blocks, and typing refuses one anywhere else.
            ExpressionNode::Block(block) => self.expression_payloads(block.tail),
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::ArrayLiteral(elements) => self.expression_span_payloads(*elements),
            ExpressionNode::Atomic(atomic) => {
                self.expression_payloads(atomic.value);
                self.expression_payloads(atomic.result);
            }
            ExpressionNode::Binary(binary) => {
                self.expression_payloads(binary.left);
                self.expression_payloads(binary.right);
            }
            ExpressionNode::Cast(cast) => {
                self.expression_payloads(cast.value);
                cast.target_type = self.type_reference(cast.target_type);
                cast.semantic_domain_arguments = self.type_span(cast.semantic_domain_arguments);
                cast.semantic_domain_symbol = self.symbol(cast.semantic_domain_symbol);
            }
            ExpressionNode::Call(call) => {
                self.expression_payloads(call.callee);
                self.expression_payloads(call.receiver);
                self.expression_span_payloads(call.arguments);
                call.target_symbol = self.symbol(call.target_symbol);
                call.requirement_application = self.type_reference(call.requirement_application);
                if let Some(binding) = &mut call.authored_call_binding {
                    binding.selected = self.symbol(binding.selected);
                }
                for argument in call.machine_arguments.iter_mut() {
                    self.static_argument(argument);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                self.expression_payloads(indexed.collection);
                self.expression_payloads(indexed.index);
            }
            ExpressionNode::Membership(membership) => {
                self.expression_payloads(membership.value);
                membership.domain_symbol = self.symbol(membership.domain_symbol);
                membership.case_type_symbol = self.symbol(membership.case_type_symbol);
                membership.case_symbol = self.symbol(membership.case_symbol);
            }
            ExpressionNode::Member(member) => {
                self.expression_payloads(member.receiver);
                member.member_symbol = self.symbol(member.member_symbol);
            }
            ExpressionNode::Borrow(borrow) => self.expression_payloads(borrow.target),
            ExpressionNode::Name(path) => {
                if let Some(mut application) = self
                    .program
                    .expression_table()
                    .static_member(path.static_application)
                    .cloned()
                {
                    self.static_argument(&mut application);
                    *self
                        .program
                        .expression_table_mut()
                        .static_member_mut(path.static_application)
                        .expect("declaration application remains in its owning table") =
                        application;
                }
                path.head_symbol = self.symbol(path.head_symbol);
                path.symbol = self.symbol(path.symbol);
                path.reserved_result_of = self.symbol(path.reserved_result_of);
                let symbols = self
                    .program
                    .expression_table()
                    .name_path_member_symbols(path.member_symbols)
                    .to_vec();
                for (position, source) in (0..path.member_symbols.count()).zip(symbols) {
                    let replacement = self.symbol(source);
                    self.program
                        .expression_table_mut()
                        .set_name_path_member_symbol_at_offset(
                            path.member_symbols,
                            position,
                            replacement,
                        );
                }
            }
            ExpressionNode::Range(range) => {
                self.expression_payloads(range.start);
                self.expression_payloads(range.end);
            }
            ExpressionNode::StructLiteral(literal) => {
                literal.application_type = self.type_reference(literal.application_type);
                literal.type_symbol = self.symbol(literal.type_symbol);
                literal.case_symbol = literal.case_symbol.map(|symbol| self.symbol(symbol));
                let fields = self
                    .program
                    .expression_table()
                    .struct_fields(literal.fields)
                    .to_vec();
                for (position, mut field) in (0..literal.fields.count()).zip(fields) {
                    self.expression_payloads(field.value);
                    field.field_symbol = self.symbol(field.field_symbol);
                    self.program
                        .expression_table_mut()
                        .set_struct_field_at_offset(literal.fields, position, field);
                }
            }
            ExpressionNode::Unary(unary) => self.expression_payloads(unary.operand),
            ExpressionNode::TypeExpression(reference) | ExpressionNode::ZeroValue(reference) => {
                *reference = self.type_reference(*reference)
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_) => {}
        }
        *self.program.expression_table_mut().expression_mut(copied) = node;
    }

    fn expression_span_payloads(&mut self, expressions: HandleSpan<ExpressionHandle>) {
        let children = self
            .program
            .expression_table()
            .expression_handles(expressions)
            .to_vec();
        for child in children {
            self.expression_payloads(child);
        }
    }

    fn static_argument(&mut self, argument: &mut StaticMachineArgument) {
        argument.type_reference = self.type_reference(argument.type_reference);
        argument.symbol = self.symbol(argument.symbol);
        argument.authored_selection_target = argument
            .authored_selection_target
            .map(|target| self.symbol(target));
        if let Some(Ok(selected)) = &mut argument.signature_free_requirement {
            selected.trait_definition = self.symbol(selected.trait_definition);
            selected.declaring_trait = self.symbol(selected.declaring_trait);
            selected.requirement = self.symbol(selected.requirement);
        }
        if let Some(application) = &mut argument.application {
            for child in application.arguments.iter_mut() {
                self.static_argument(child);
            }
            if application.member.is_valid() {
                let mut member = self
                    .program
                    .expression_table()
                    .static_member(application.member)
                    .expect("copied static member belongs to the receiving expression table")
                    .clone();
                self.static_argument(&mut member);
                *self
                    .program
                    .expression_table_mut()
                    .static_member_mut(application.member)
                    .expect("copied static member remains present") = member;
            }
        }
    }

    fn type_reference(&mut self, source: Handle<TypeReference>) -> Handle<TypeReference> {
        if !source.is_valid() {
            return source;
        }
        if let Some((_, copied)) = self.types.iter().find(|(original, _)| *original == source) {
            return *copied;
        }
        let reference = self
            .program
            .declarations()
            .child_type_references
            .get(source)
            .clone();
        let copied = self
            .program
            .declarations_mut()
            .child_type_references
            .insert(TypeReference::Unit);
        self.types.push((source, copied));
        let reference = self.type_payload(reference);
        *self
            .program
            .declarations_mut()
            .child_type_references
            .get_mut(copied) = reference;
        self.type_origins(source, copied);
        copied
    }

    fn type_origins(&mut self, source: Handle<TypeReference>, copied: Handle<TypeReference>) {
        let origins = self
            .program
            .tables
            .types
            .generic_application_origins
            .iter()
            .filter_map(|(_, origin)| (origin.instance == source).then_some(origin.clone()))
            .collect::<Vec<_>>();
        for origin in origins {
            let application = self.type_reference(origin.application);
            self.program
                .tables
                .types
                .generic_application_origins
                .insert(GenericApplicationOrigin {
                    instance: copied,
                    application,
                });
        }
    }

    fn type_span(&mut self, source: HandleSpan<TypeReference>) -> HandleSpan<TypeReference> {
        let references = self
            .program
            .declarations()
            .child_type_references
            .span_or_empty(source)
            .to_vec();
        let copied = self
            .program
            .declarations_mut()
            .child_type_references
            .insert_many(references.iter().map(|_| TypeReference::Unit));
        for position in 0..source.count() {
            let original = source.handle_at(position);
            if !self
                .types
                .iter()
                .any(|(candidate, _)| *candidate == original)
            {
                self.types.push((original, copied.handle_at(position)));
            }
        }
        for (position, reference) in (0..source.count()).zip(references) {
            let destination = copied.handle_at(position);
            let reference = self.type_payload(reference);
            *self
                .program
                .declarations_mut()
                .child_type_references
                .get_mut(destination) = reference;
            self.type_origins(source.handle_at(position), destination);
        }
        copied
    }

    fn type_payload(&mut self, mut reference: TypeReference) -> TypeReference {
        if let TypeReference::Named { symbol, .. } = &reference
            && let Some((_, actual)) = self
                .type_arguments
                .iter()
                .find(|(formal, _)| formal == symbol)
        {
            return actual.clone();
        }
        let owner = reference.supplied_operand_owner();
        if owner.is_valid() {
            let declaration = self
                .program
                .tables
                .types
                .operand_lifetime_owners
                .get(owner)
                .declaration;
            let copied = self.symbol(declaration);
            if copied != declaration {
                let owner = match self
                    .lifetime_owners
                    .iter()
                    .find(|(original, _)| *original == owner)
                {
                    Some((_, copied)) => *copied,
                    None => {
                        let copied = self.program.tables.types.operand_lifetime_owners.insert(
                            OperandLifetimeOwner {
                                declaration: copied,
                            },
                        );
                        self.lifetime_owners.push((owner, copied));
                        copied
                    }
                };
                reference.retain_supplied_operand_owner(owner);
            }
        }
        match &mut reference {
            TypeReference::Reference(borrow) => {
                borrow.referee = self.type_reference(borrow.referee)
            }
            TypeReference::Constrained(constrained) => {
                constrained.base_type = self.type_reference(constrained.base_type);
                let constraints = self
                    .program
                    .tables
                    .types
                    .constraints
                    .span_or_empty(constrained.constraints)
                    .to_vec();
                let copied = constraints
                    .into_iter()
                    .map(|constraint| self.constraint(constraint))
                    .collect::<Vec<_>>();
                constrained.constraints = self.program.tables.types.constraints.insert_many(copied);
            }
            TypeReference::FixedArray(array) => {
                array.element_type = self.type_reference(array.element_type);
                match &mut array.length {
                    FixedArrayLength::ConstParameter { symbol, .. } => {
                        *symbol = self.symbol(*symbol)
                    }
                    FixedArrayLength::ConstCall { machine, .. } => *machine = self.symbol(*machine),
                    FixedArrayLength::Expression(value) => *value = self.type_reference(*value),
                    FixedArrayLength::Literal(_) => {}
                }
            }
            TypeReference::Slice(slice) => {
                slice.element_type = self.type_reference(slice.element_type)
            }
            TypeReference::Generic(application) => {
                application.base_symbol = self.symbol(application.base_symbol);
                application.arguments = self.type_span(application.arguments);
                application.receiving_application = self.receiving_application(
                    application.receiving_application,
                    application.arguments,
                    Some(application.base_symbol),
                );
            }
            TypeReference::ConstExpression(expression) => {
                expression.expression = self.expression(expression.expression);
                expression.lexical_owner = self.symbol(expression.lexical_owner);
            }
            TypeReference::Callable { contract, .. } => {
                *contract = self.callable_contract(*contract);
            }
            TypeReference::Named { symbol, .. } => *symbol = self.symbol(*symbol),
            TypeReference::AliasUse {
                expansion,
                application,
                ..
            } => {
                **expansion = self.type_payload((**expansion).clone());
                **application = self.type_payload((**application).clone());
            }
            TypeReference::Unit => {}
        }
        reference
    }

    fn callable_contract(
        &mut self,
        source: crate::source_resolved::callable_values::RuntimeCallableContractHandle,
    ) -> crate::source_resolved::callable_values::RuntimeCallableContractHandle {
        if let Some((_, copied)) = self
            .callables
            .iter()
            .find(|(original, _)| *original == source)
        {
            return *copied;
        }
        let mut declaration = self
            .program
            .tables
            .declarations
            .runtime_callable_contracts
            .get(source)
            .clone();
        let copied = self
            .program
            .tables
            .declarations
            .runtime_callable_contracts
            .append(Default::default());
        self.callables.push((source, copied));
        declaration.lexical_owner = self.symbol(declaration.lexical_owner);
        declaration.signature = self.signature(&declaration.signature);
        *self
            .program
            .tables
            .declarations
            .runtime_callable_contracts
            .get_mut(copied) = declaration;
        copied
    }

    fn proof_span(&mut self, source: HandleSpan<ProofFact>) -> HandleSpan<ProofFact> {
        let facts = source
            .handles()
            .map(|handle| self.proof_fact_payload(handle))
            .collect::<Vec<_>>();
        let (facts, spans): (Vec<_>, Vec<_>) = facts.into_iter().unzip();
        let copied = self
            .program
            .tables
            .declarations
            .proof_facts
            .insert_many(facts);
        for (handle, source_span) in copied.handles().zip(spans) {
            if let Some(source_span) = source_span {
                self.program.set_proof_fact_source_span(handle, source_span);
            }
        }
        copied
    }

    fn constraint(&mut self, mut constraint: TypeConstraint) -> TypeConstraint {
        match &mut constraint {
            TypeConstraint::Domain(domain) => {
                domain.arguments = self.type_span(domain.arguments);
                domain.receiving_application = self.receiving_application(
                    domain.receiving_application,
                    domain.arguments,
                    None,
                );
            }
            TypeConstraint::Named(_) | TypeConstraint::ArithmeticDomain(_) => {}
        }
        constraint
    }

    /*
        A copy owns its new argument span but still names the selected
        receiving declaration. Substituting that declaration must not
        retarget the original application or inherit its binders.
    */
    fn receiving_application(
        &mut self,
        source: crate::source_resolved::types::ReceivingApplicationHandle,
        arguments: HandleSpan<TypeReference>,
        selected: Option<SymbolHandle>,
    ) -> crate::source_resolved::types::ReceivingApplicationHandle {
        if !source.is_valid()
            || !self
                .program
                .declarations()
                .child_type_references
                .receiving_applications
                .is_valid(source)
        {
            return source;
        }
        let mut retained = self
            .program
            .declarations()
            .child_type_references
            .receiving_applications
            .get(source)
            .clone();
        retained.arguments = arguments;
        if let Some(selected) = selected
            && retained.selected.is_valid()
            && retained.selected != selected
        {
            retained.conflict = Some(
                crate::source_resolved::types::ReceivingApplicationConflict {
                    attempted_declaration: selected,
                    attempted_arguments: arguments,
                },
            );
        }
        self.program
            .declarations_mut()
            .child_type_references
            .receiving_applications
            .insert(retained)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_resolved::expression::{
        StaticSymbolApplication, TableCallExpression, TableNamePath,
    };
    use crate::source_resolved::name::DiagnosticName;
    use crate::source_resolved::types::{GenericTypeReference, GenericTypeReferenceStorage};
    use source_text::{SourceId, SourceSpan, Span};

    fn generic(base: SymbolHandle, arguments: HandleSpan<TypeReference>) -> TypeReference {
        TypeReference::Generic(GenericTypeReference {
            storage: GenericTypeReferenceStorage {
                supplied_operand_owner: Default::default(),
                base_symbol: base,
                base_name: DiagnosticName::generated("Carrier"),
                lifetime_arguments: Vec::new(),
                arguments,
                ..Default::default()
            },
        })
    }

    #[test]
    fn proof_membership_copies_domain_arguments_and_preserves_provenance() {
        let mut program = SymbolResolvedTrees::default();
        let original = SymbolHandle::from_arena_index(3);
        let replacement = SymbolHandle::from_arena_index(4);
        let domain = program.declarations_mut().domain_path_members.insert_many([
            DiagnosticName::generated("contracts"),
            DiagnosticName::generated("Resident"),
        ]);
        let arguments = program
            .declarations_mut()
            .child_type_references
            .insert_many([TypeReference::Named {
                supplied_operand_owner: Default::default(),
                symbol: original,
                name: DiagnosticName::generated("T"),
            }]);
        let fact = program
            .declarations_mut()
            .proof_facts
            .insert(ProofFact::Membership(
                crate::source_resolved::domain::ProofMembershipFact {
                    domain,
                    domain_symbol: original,
                    domain_arguments: arguments,
                    ..Default::default()
                },
            ));
        let source_span = SourceSpan::new(SourceId(7), Span::new(11, 19));
        program.set_proof_fact_source_span(fact, source_span);
        let copied = program.copy_proof_fact_with_symbols(fact, &[(original, replacement)]);
        assert_eq!(program.proof_fact_source_span(copied), Some(source_span));
        let ProofFact::Membership(membership) = program.declarations().proof_facts.get(copied)
        else {
            panic!("copy remains membership");
        };
        assert_eq!(membership.domain_symbol, replacement);
        assert_ne!(membership.domain, domain);
        assert_eq!(
            program.domain_path_members(membership.domain),
            program.domain_path_members(domain)
        );
        assert_ne!(membership.domain_arguments, arguments);
        assert_eq!(membership.authored_domain_selection, None);
        let TypeReference::Named { symbol, .. } = program
            .declarations()
            .child_type_references
            .get(membership.domain_arguments.handle_at(0))
        else {
            panic!("domain argument remains named");
        };
        assert_eq!(*symbol, replacement);
        let ProofFact::Membership(original_fact) = program.declarations().proof_facts.get(fact)
        else {
            panic!("original remains membership");
        };
        assert_eq!(original_fact.domain_symbol, original);
        assert_eq!(original_fact.domain, domain);
        assert_eq!(original_fact.domain_arguments, arguments);
    }

    #[test]
    fn copies_span_origins_and_expression_payloads_without_mutating_originals() {
        let mut program = SymbolResolvedTrees::default();
        let original = SymbolHandle::from_arena_index(3);
        let replacement = SymbolHandle::from_arena_index(4);
        let member_symbols = program
            .expression_table_mut()
            .reserve_name_path_member_symbols(1);
        program
            .expression_table_mut()
            .set_name_path_member_symbol_at_offset(member_symbols, 0, original);
        let name = program
            .expression_table_mut()
            .insert(ExpressionNode::Name(TableNamePath {
                members: HandleSpan::empty(),
                member_symbols,
                head_symbol: original,
                symbol: original,
                static_application: arena::Handle::invalid(),
                reserved_result_of: SymbolHandle::invalid(),
            }));
        let arguments = program
            .declarations_mut()
            .child_type_references
            .insert_many([TypeReference::ConstExpression(
                crate::source_resolved::types::ConstExpressionTypeReference::new(name),
            )]);
        let application = program
            .declarations_mut()
            .child_type_references
            .insert(generic(original, HandleSpan::empty()));
        program
            .tables
            .types
            .generic_application_origins
            .insert(GenericApplicationOrigin {
                instance: arguments.handle_at(0),
                application,
            });
        let carrier = program
            .declarations_mut()
            .child_type_references
            .insert(generic(original, arguments));
        let source = program
            .expression_table_mut()
            .insert(ExpressionNode::ZeroValue(carrier));
        let source_span = SourceSpan::new(SourceId(7), Span::new(11, 19));
        program
            .expression_table_mut()
            .set_source_span(source, source_span);
        let copied = program.copy_expression_with_symbols(source, &[(original, replacement)]);
        assert_eq!(
            program.tables.bodies.expressions.source_span(copied),
            source_span
        );
        let ExpressionNode::ZeroValue(copied_carrier) =
            *program.tables.bodies.expressions.expression(copied)
        else {
            panic!("copy must retain zero construction");
        };
        assert_ne!(copied_carrier, carrier);
        let TypeReference::Generic(copied_type) = program
            .declarations()
            .child_type_references
            .get(copied_carrier)
        else {
            panic!("copy must retain generic application");
        };
        assert_eq!(copied_type.base_symbol, replacement);
        let copied_argument = copied_type.arguments.handle_at(0);
        let TypeReference::ConstExpression(copied_name) = program
            .declarations()
            .child_type_references
            .get(copied_argument)
        else {
            panic!("copy must retain static expression");
        };
        let ExpressionNode::Name(copied_path) = program
            .expression_table()
            .expression(copied_name.expression)
        else {
            panic!("copy must retain name");
        };
        assert_eq!(copied_path.symbol, replacement);
        assert_eq!(
            program
                .expression_table()
                .name_path_member_symbols(copied_path.member_symbols),
            [replacement]
        );
        let origin = program
            .tables
            .types
            .generic_application_origins
            .iter()
            .find_map(|(_, origin)| (origin.instance == copied_argument).then_some(origin))
            .expect("copied span origin");
        let TypeReference::Generic(origin_application) = program
            .declarations()
            .child_type_references
            .get(origin.application)
        else {
            panic!("copy must retain origin application");
        };
        assert_eq!(origin_application.base_symbol, replacement);
        let ExpressionNode::Name(original_path) =
            program.tables.bodies.expressions.expression(name)
        else {
            panic!("original remains a name");
        };
        assert_eq!(original_path.symbol, original);
        let TypeReference::Generic(original_type) =
            program.declarations().child_type_references.get(carrier)
        else {
            panic!("original remains generic");
        };
        assert_eq!(original_type.base_symbol, original);
        assert_eq!(original_type.arguments, arguments);
    }

    #[test]
    fn rebases_nested_static_arguments_simultaneously_and_preserves_absence() {
        let mut program = SymbolResolvedTrees::default();
        let original = SymbolHandle::from_arena_index(3);
        let replacement = SymbolHandle::from_arena_index(4);
        let argument = StaticMachineArgument {
            authored_selection: None,
            authored_selection_target: None,
            type_reference: Handle::invalid(),
            path: Box::new([]),
            application: None,
            const_literal: None,
            evidence_projection: None,
            symbol: original,
            signature_free_requirement: Some(Ok(
                crate::source_resolved::expression::ResolvedSignatureFreeRequirement {
                    trait_definition: original,
                    declaring_trait: original,
                    requirement: original,
                },
            )),
        };
        let source =
            program
                .expression_table_mut()
                .insert(ExpressionNode::Call(TableCallExpression {
                    callee: ExpressionHandle::invalid(),
                    target_is_static: false,
                    authored_call_binding: None,
                    receiver: ExpressionHandle::invalid(),
                    target_symbol: original,
                    target: DiagnosticName::generated("run"),
                    requirement_application: arena::Handle::invalid(),
                    machine_arguments: vec![StaticMachineArgument {
                        application: Some(Box::new(StaticSymbolApplication {
                            member: arena::Handle::invalid(),
                            lifetime_arguments: Box::new([]),
                            arguments: vec![argument.clone()].into_boxed_slice(),
                        })),
                        ..argument
                    }]
                    .into_boxed_slice(),
                    arguments: HandleSpan::empty(),
                    evidence_arguments: Box::new([]),
                    operational_acknowledgement: Default::default(),
                }));
        let copied = program.copy_expression_with_symbols(
            source,
            &[
                (original, replacement),
                (replacement, SymbolHandle::from_arena_index(5)),
                (SymbolHandle::invalid(), original),
            ],
        );
        let ExpressionNode::Call(call) = program.tables.bodies.expressions.expression(copied)
        else {
            panic!("copy remains a call");
        };
        assert_eq!(call.target_symbol, replacement);
        assert!(!call.callee.is_valid());
        assert!(!call.receiver.is_valid());
        assert!(!call.machine_arguments[0].type_reference.is_valid());
        assert_eq!(call.machine_arguments[0].symbol, replacement);
        assert_eq!(
            call.machine_arguments[0].signature_free_requirement,
            Some(Ok(
                crate::source_resolved::expression::ResolvedSignatureFreeRequirement {
                    trait_definition: replacement,
                    declaring_trait: replacement,
                    requirement: replacement,
                }
            ))
        );
        assert_eq!(
            call.machine_arguments[0]
                .application
                .as_ref()
                .expect("nested application")
                .arguments[0]
                .symbol,
            replacement
        );
        assert!(
            !program
                .copy_expression_with_symbols(ExpressionHandle::invalid(), &[])
                .is_valid()
        );
    }
}
