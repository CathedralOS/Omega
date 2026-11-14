/*
We represent typed call signatures and dispatch families (`calls`), executable
machines, states, and statement control graphs (`control_flow`), and typed
symbol/identity names (`names`) in `TypedTrees`.
*/

pub mod calls {
    pub mod application_graph {
        use crate::TypedTrees;
        use crate::references;
        use semantic_vocabulary::proof_applications::{
            ProofApplicationMachine, ProofApplicationPlan, ProofApplicationTargetMember,
            ProofDeclarationDependencies,
        };
        use symbols::SymbolHandle;

        pub fn source_application_plan(typed: &TypedTrees) -> ProofApplicationPlan<SymbolHandle> {
            let mut plan = ProofApplicationPlan::default();
            let mut declarations = references::collect_program_declarations(typed);
            declarations.retain(|owner| typed.machine_by_symbol(*owner).is_none());
            for owner in typed.traits() {
                declarations.extend(
                    typed
                        .trait_machine_signatures(owner)
                        .iter()
                        .map(|signature| signature.symbol),
                );
                declarations.extend(
                    owner
                        .refinement_clauses
                        .iter()
                        .map(|row| row.signature.symbol),
                );
            }
            declarations.sort_unstable_by_key(|owner| (owner.arena_index(), owner.generation()));
            declarations.dedup();
            plan.declarations = plan.symbols.insert_many(declarations.iter().copied());
            for (declaration, dependencies) in
                references::collect_declaration_symbol_dependencies(typed)
            {
                let dependencies = dependencies
                    .into_iter()
                    .map(|symbol| {
                        typed
                            .trait_machine_signature(symbol)
                            .map_or(symbol, |(owner, _)| owner.symbol)
                    })
                    .filter(|dependency| {
                        typed.machine_by_symbol(*dependency).is_none()
                            && declarations.contains(dependency)
                    });
                let dependencies = plan.symbols.insert_many(dependencies);
                let signatures = match typed.trait_definition_by_symbol(declaration) {
                    Some(owner) => plan.symbols.insert_many(
                        typed
                            .trait_machine_signatures(owner)
                            .iter()
                            .map(|signature| signature.symbol)
                            .chain(
                                owner
                                    .refinement_clauses
                                    .iter()
                                    .map(|row| row.signature.symbol),
                            ),
                    ),
                    None => Default::default(),
                };
                plan.declaration_dependencies
                    .insert(ProofDeclarationDependencies {
                        declaration,
                        dependencies,
                        signatures,
                    });
            }
            for machine in typed.machines() {
                let states = plan.symbols.insert_many(
                    typed
                        .machine_states(machine)
                        .iter()
                        .map(|state| state.symbol),
                );
                let binders = plan.symbols.insert_many(
                    typed
                        .machine_kind_type_parameters(machine)
                        .map(|binder| binder.symbol),
                );
                let family_members = typed
                    .target_machine_families
                    .iter()
                    .find(|family| family.canonical == machine.symbol)
                    .map(|family| {
                        plan.target_members
                            .insert_many(family.members.iter().map(|member| {
                                ProofApplicationTargetMember {
                                    target: member.target.as_str().to_owned(),
                                    machine: member.machine,
                                }
                            }))
                    })
                    .unwrap_or_default();
                plan.machines.insert(ProofApplicationMachine {
                    machine: machine.symbol,
                    states,
                    binders,
                    authored_target: typed.machine_authored_target(machine).map(str::to_owned),
                    source_target: typed.source_target_context(machine).map(str::to_owned),
                    family_interface: machine.target_family_interface,
                    family_members,
                    dynamic_realizations: Default::default(),
                    product_selected: true,
                });
            }
            plan.provider_candidates
                .insert_many(typed.conformances().iter().flat_map(|conformance| {
                    conformance
                        .rows
                        .iter()
                        .map(|row| (row.requirement, row.realization_machine))
                }));
            for machine in typed.machines() {
                plan.provider_candidates.insert_many(
                    typed
                        .machine_trait_conformances(machine)
                        .iter()
                        .filter(|clause| clause.requirement_symbol.is_valid())
                        .map(|clause| (clause.requirement_symbol, machine.symbol)),
                );
            }
            plan
        }
    }
    pub mod boundary {
        use crate::TypedTrees;
        use crate::signature::StateSignature;
        use crate::statement::TableCall;

        pub fn called_boundary_signature<'program>(
            program: &'program TypedTrees,
            call: &TableCall,
        ) -> Option<&'program StateSignature> {
            let (declaration, signature) = if let Some(dispatch) = &call.static_requirement_dispatch
            {
                let declaration = program.trait_definition_by_symbol(dispatch.declaring_trait)?;
                let signature =
                    program.trait_machine_signature_by_symbol(declaration, dispatch.requirement)?;
                (declaration, signature)
            } else {
                program.trait_machine_signature(call.target_symbol)?
            };
            declaration.is_boundary.then_some(signature)
        }
    }
    pub mod dynamic_traits {
        use crate::TypedTrees;
        use crate::domain::ProofFact;
        use crate::expression::ExpressionHandle;
        use crate::name::Identifier;
        use crate::signature::StateSignature;
        use crate::trait_definition::{DynamicSignatureIneligibility, TraitDefinition};
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum DynamicNonSignatureIneligibility {
            PrivateContractIdentity,
            UnprojectableResultLifetime,
            UnfittableEnvelope,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct DynamicSlotSignature {
            pub symbol: SymbolHandle,
            pub interface_identity: crate::type_identity::TypeIdentityId,
            pub declaring_trait: SymbolHandle,
            pub requirement: SymbolHandle,
            pub call_requirement: SymbolHandle,
            pub parameters: Vec<crate::signature::StateParameter>,
            pub return_type: TypeReferenceHandle,
        }

        impl DynamicSlotSignature {
            pub fn data_parameters(&self) -> &[crate::signature::StateParameter] {
                self.parameters.get(1..).unwrap_or_default()
            }
        }

        impl TypedTrees {
            pub fn dynamic_slot(
                &self,
                interface_identity: crate::type_identity::TypeIdentityId,
                requirement: SymbolHandle,
            ) -> Option<&DynamicSlotSignature> {
                self.dynamic_slots.iter().find(|slot| {
                    slot.call_requirement == requirement
                        && slot.interface_identity == interface_identity
                })
            }

            pub fn dynamic_slot_requirement(
                &self,
                slot: &DynamicSlotSignature,
            ) -> Option<&crate::signature::StateSignature> {
                let definition = self.trait_definition_by_symbol(slot.declaring_trait)?;
                self.trait_machine_signature_by_symbol(definition, slot.requirement)
            }

            pub fn dynamic_slot_by_symbol(
                &self,
                symbol: SymbolHandle,
            ) -> Option<&DynamicSlotSignature> {
                if !symbol.is_valid() {
                    return None;
                }
                self.dynamic_slots.iter().find(|slot| slot.symbol == symbol)
            }

            pub fn dynamic_signature_eligibility(
                &self,
                trait_definition: &TraitDefinition,
                requirement: &StateSignature,
            ) -> Result<(), DynamicSignatureIneligibility> {
                if trait_definition.is_boundary {
                    return Err(DynamicSignatureIneligibility::BoundaryRequirement);
                }
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

            fn requirement_contract_names_private_identity(
                &self,
                requirement: &StateSignature,
            ) -> bool {
                let mut symbols = Vec::new();
                for contract in self.state_signature_contracts(requirement) {
                    for fact in self.proof_facts.span_or_empty(contract.facts) {
                        self.collect_proof_fact_symbols(fact, &mut symbols);
                    }
                }
                symbols
                    .iter()
                    .any(|symbol| contract_identity_is_public(self, *symbol) == Some(false))
            }

            fn result_lifetimes_expressible_from_inputs(
                &self,
                requirement: &StateSignature,
            ) -> bool {
                let mut input_lifetimes = Vec::new();
                for parameter in self.state_signature_parameters(requirement) {
                    collect_type_reference_lifetimes(
                        self,
                        parameter.type_reference,
                        &mut input_lifetimes,
                    );
                }
                if !requirement.return_type.is_valid() {
                    return true;
                }
                let mut result_lifetimes = Vec::new();
                collect_type_reference_lifetimes(
                    self,
                    requirement.return_type,
                    &mut result_lifetimes,
                );
                result_lifetimes
                    .iter()
                    .all(|lifetime| input_lifetimes.contains(lifetime))
            }

            fn collect_proof_fact_symbols(
                &self,
                fact: &ProofFact,
                symbols: &mut Vec<SymbolHandle>,
            ) {
                match fact {
                    ProofFact::Expression(expression) => {
                        self.collect_expression_symbols(*expression, symbols);
                    }
                    ProofFact::Membership(membership) => {
                        symbols.push(membership.domain_symbol);
                        self.collect_expression_symbols(membership.value, symbols);
                        for argument in self.type_reference_handles(membership.domain_arguments) {
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

            fn collect_expression_symbols(
                &self,
                expression: ExpressionHandle,
                symbols: &mut Vec<SymbolHandle>,
            ) {
                if expression.is_valid() {
                    symbols.extend(crate::references::collect_expression_symbol_dependencies(
                        self, expression,
                    ));
                }
            }
        }

        fn contract_identity_is_public(program: &TypedTrees, symbol: SymbolHandle) -> Option<bool> {
            if !symbol.is_valid() {
                return None;
            }
            program
                .data_definitions()
                .iter()
                .find_map(|definition| {
                    (definition.symbol == symbol).then_some(definition.is_public)
                })
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

        fn collect_type_reference_symbols(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
            symbols: &mut Vec<SymbolHandle>,
        ) {
            if type_reference.is_valid() {
                symbols.extend(crate::references::collect_type_symbol_dependencies(
                    program,
                    type_reference,
                ));
            }
        }

        fn collect_type_reference_lifetimes(
            program: &TypedTrees,
            type_reference: TypeReferenceHandle,
            lifetimes: &mut Vec<String>,
        ) {
            if !type_reference.is_valid() {
                return;
            }
            let mut inputs = LifetimeInputs {
                program,
                lifetimes,
                scopes: Vec::new(),
                pending_lifetimes: Vec::new(),
                types: std::collections::HashSet::new(),
                expressions: std::collections::HashSet::new(),
            };
            crate::references::visit_type_reference(program, &mut inputs, type_reference);
        }

        struct LifetimeInputs<'program, 'output> {
            program: &'program TypedTrees,
            lifetimes: &'output mut Vec<String>,
            scopes: Vec<Vec<String>>,
            pending_lifetimes: Vec<String>,
            types: std::collections::HashSet<(TypeReferenceHandle, Vec<String>)>,
            expressions: std::collections::HashSet<(ExpressionHandle, Vec<String>)>,
        }

        impl LifetimeInputs<'_, '_> {
            fn context(&self) -> Vec<String> {
                let mut names = self.scopes.iter().flatten().cloned().collect::<Vec<_>>();
                names.sort_unstable();
                names.dedup();
                names
            }
            fn lifetime(&mut self, name: &Identifier) {
                if !self
                    .scopes
                    .iter()
                    .flatten()
                    .any(|bound| bound == name.as_str())
                {
                    push_lifetime_name(self.lifetimes, name);
                }
            }
            fn argument(&mut self, argument: &crate::expression::StaticMachineArgument) {
                if let Some(application) = &argument.application {
                    for lifetime in application.lifetime_arguments.iter() {
                        self.lifetime(lifetime);
                    }
                    for nested in application.arguments.iter() {
                        self.argument(nested);
                    }
                    let program = self.program;
                    if let Some(member) = program.expression_table.static_member(application.member)
                    {
                        self.argument(member);
                    }
                }
            }
        }

        impl Extend<SymbolHandle> for LifetimeInputs<'_, '_> {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
        }

        impl crate::references::TypedReferences for LifetimeInputs<'_, '_> {
            fn callable_signature(&mut self, signature: &StateSignature) {
                self.pending_lifetimes = signature
                    .lifetime_parameters
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect();
            }
            fn enter_lexical_scope(&mut self, _: SymbolHandle) {
                self.scopes
                    .push(std::mem::take(&mut self.pending_lifetimes));
            }
            fn leave_lexical_scope(&mut self) {
                self.scopes.pop();
            }
            fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
                let context = self.context();
                if !self.types.insert((reference, context)) {
                    return false;
                }
                let program = self.program;
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Reference {
                        lifetime: Some(name),
                        ..
                    } => self.lifetime(name),
                    TypeReferenceNode::Generic {
                        lifetime_arguments, ..
                    } => {
                        for lifetime in lifetime_arguments {
                            self.lifetime(lifetime);
                        }
                    }
                    _ => {}
                }
                true
            }
            fn expression(&mut self, expression: ExpressionHandle) -> bool {
                let context = self.context();
                if !self.expressions.insert((expression, context)) {
                    return false;
                }
                let program = self.program;
                match program.expression_table.expression(expression) {
                    crate::expression::ExpressionNode::MachineValue(value) => {
                        self.argument(&value.selected)
                    }
                    crate::expression::ExpressionNode::Call(call) => {
                        for argument in call.machine_arguments.iter() {
                            self.argument(argument);
                        }
                        if let Some(operation) = &call.installed_interrupt_operation {
                            self.argument(&operation.registrar);
                            self.argument(&operation.timer_slot);
                        }
                        if let Some(operation) = &call.private_layout_operation {
                            self.argument(&operation.selected_slot);
                        }
                        if let Some(operation) = &call.quotient_operation {
                            self.argument(&operation.representative_operation);
                            for theorem in operation.theorem_evidence.iter() {
                                self.argument(&theorem.application);
                            }
                        }
                    }
                    _ => {}
                }
                true
            }
            fn static_application(
                &mut self,
                application: crate::static_applications::StaticMachineApplicationHandle,
            ) {
                let program = self.program;
                for binding in program.static_machine_application_lifetimes(application) {
                    self.lifetime(&binding.argument);
                }
            }
        }

        fn push_lifetime_name(lifetimes: &mut Vec<String>, name: &Identifier) {
            let name = name.as_str();
            if !name.is_empty() && !lifetimes.iter().any(|seen| seen == name) {
                lifetimes.push(name.to_owned());
            }
        }
    }
    pub mod finite_family {
        use crate::TypedTrees;
        use crate::expression::BinaryOperator;
        use crate::expression::ExpressionNode::Binary;
        use crate::expression::ExpressionNode::Name;
        use crate::signature::StateSignature;
        use crate::types;
        use language_semantics::const_value::FiniteScalarValue;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct FamilyTuple {
            pub values: Box<[FiniteScalarValue]>,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum FamilyProbe {
            NotFinite(String),
            Finite {
                arity: usize,
                tuples: Vec<FamilyTuple>,
            },
        }

        impl TypedTrees {
            pub fn finite_signature_family(&self, signature: &StateSignature) -> FamilyProbe {
                self.finite_declaration_family(
                    signature.symbol,
                    !signature.lifetime_parameters.is_empty(),
                    self.state_signature_type_parameters(signature),
                    self.state_signature_contracts(signature),
                )
            }

            pub fn finite_machine_family(&self, machine: &crate::machine::Machine) -> FamilyProbe {
                self.finite_declaration_family(
                    machine.symbol,
                    !machine.lifetime_parameters.is_empty(),
                    self.machine_type_parameters(machine),
                    self.machine_contracts(machine),
                )
            }

            pub fn finite_value_binder_in_scope(
                &self,
                mut owner: symbols::SymbolHandle,
                binder: symbols::SymbolHandle,
            ) -> bool {
                let mut visited = Vec::new();
                while owner.is_valid() {
                    owner = self.lifetime_declaration_owner(owner);
                    if !owner.is_valid() || visited.contains(&owner) {
                        return false;
                    }
                    if let Some(machine) = self.machine_declaration_for_lexical_scope(owner) {
                        if let Some(parameter) = self
                            .machine_type_parameters(machine)
                            .iter()
                            .find(|parameter| parameter.symbol == binder)
                        {
                            return matches!(
                                parameter.kind,
                                crate::data::TypeParameterKind::Value { .. }
                            ) && matches!(
                                self.finite_machine_family(machine),
                                FamilyProbe::Finite { .. }
                            );
                        }
                    } else if self.symbols.get(owner).kind == symbols::SymbolKind::Machine {
                        return false;
                    }
                    visited.push(owner);
                    owner = self.declaration_scope_parent(owner);
                }
                false
            }

            fn finite_declaration_family(
                &self,
                owner: symbols::SymbolHandle,
                has_lifetimes: bool,
                type_parameters: &[crate::data::TypeParameter],
                contracts: &[crate::signature::SignatureContract],
            ) -> FamilyProbe {
                if has_lifetimes {
                    return FamilyProbe::NotFinite(
                "invocation lifetimes are arbitrary regions, not an enumerable value roster"
                    .to_owned(),
            );
                }
                let binders = type_parameters
                    .iter()
                    .filter(|parameter| {
                        matches!(
                            parameter.kind,
                            crate::data::TypeParameterKind::Const { .. }
                                | crate::data::TypeParameterKind::Value { .. }
                        )
                    })
                    .collect::<Vec<_>>();
                if binders.len() != type_parameters.len() {
                    return FamilyProbe::NotFinite(
                "the requirement carries non-value generic binders, which value equality cannot enumerate"
                    .to_owned(),
            );
                }
                let mut family = FamilyProbe::NotFinite(
                    "declares no finite-family `requires` clause".to_owned(),
                );
                for fact in contracts
                    .iter()
                    .filter(|contract| {
                        contract.kind == crate::signature::SignatureContractKind::Requires
                    })
                    .flat_map(|contract| self.proof_facts.span_or_empty(contract.facts))
                {
                    let Some(root) = fact.as_expression() else {
                        continue;
                    };
                    let candidate = self.explicit_signature_family(owner, &binders, *root);
                    if matches!(&candidate, FamilyProbe::Finite { .. }) {
                        if matches!(&family, FamilyProbe::Finite { .. }) {
                            return FamilyProbe::NotFinite(
                        "the finite family must be one explicit disjunction of complete binder equalities".to_owned(),
                    );
                        }
                        family = candidate;
                    } else if !matches!(&family, FamilyProbe::Finite { .. }) {
                        family = candidate;
                    }
                }
                family
            }

            fn explicit_signature_family(
                &self,
                owner: symbols::SymbolHandle,
                binders: &[&crate::data::TypeParameter],
                root: crate::expression::ExpressionHandle,
            ) -> FamilyProbe {
                let mut tuples = Vec::new();
                for alternative in self.binary_chain_leaves(root, BinaryOperator::Or) {
                    let mut assignments: Vec<Option<FiniteScalarValue>> = vec![None; binders.len()];
                    for conjunct in self.binary_chain_leaves(alternative, BinaryOperator::And) {
                        let Some((binder_index, value)) =
                            self.equality_assignment(owner, binders, conjunct)
                        else {
                            return FamilyProbe::NotFinite(
                        "the `requires` clause is not explicit `Binder == literal` equalities; opaque \
                         predicates and inequalities do not enumerate a finite family"
                            .to_owned(),
                    );
                        };
                        if assignments[binder_index].is_some() {
                            return FamilyProbe::NotFinite(
                                "an alternative binds the same value binder more than once"
                                    .to_owned(),
                            );
                        }
                        assignments[binder_index] = Some(value);
                    }
                    if assignments.iter().any(Option::is_none) {
                        return FamilyProbe::NotFinite(
                    "an alternative does not bind every value binder; a finite family requires \
                     complete tuples so authored correlations are preserved"
                        .to_owned(),
                );
                    }
                    let tuple = assignments
                        .into_iter()
                        .map(|value| value.expect("complete alternative tuple"))
                        .collect::<Vec<_>>();
                    tuples.push(FamilyTuple {
                        values: tuple.into_boxed_slice(),
                    });
                }
                // Preserve the published roster ordering while keeping the
                // scalar values, rather than reparsing keys for later checks.
                tuples.sort_by_cached_key(|tuple| {
                    tuple
                        .values
                        .iter()
                        .map(|value| value.canonical_identity())
                        .collect::<Vec<_>>()
                });
                tuples.dedup_by(|left, right| left.values == right.values);
                FamilyProbe::Finite {
                    arity: binders.len(),
                    tuples,
                }
            }

            fn binary_chain_leaves(
                &self,
                expression: crate::expression::ExpressionHandle,
                operator: BinaryOperator,
            ) -> Vec<crate::expression::ExpressionHandle> {
                let mut leaves = Vec::new();
                let mut stack = vec![expression];
                while let Some(current) = stack.pop() {
                    match self.expression_table.expression(current) {
                        Binary(binary) if binary.operator == operator => {
                            stack.push(binary.right);
                            stack.push(binary.left);
                        }
                        _ => leaves.push(current),
                    }
                }
                leaves
            }

            fn equality_assignment(
                &self,
                owner: symbols::SymbolHandle,
                binders: &[&crate::data::TypeParameter],
                expression: crate::expression::ExpressionHandle,
            ) -> Option<(usize, FiniteScalarValue)> {
                let Binary(binary) = self.expression_table.expression(expression) else {
                    return None;
                };
                if binary.operator != BinaryOperator::Equal {
                    return None;
                }
                let (binder, value) = match (
                    self.binder_operand(binders, binary.left),
                    self.binder_operand(binders, binary.right),
                    self.literal_operand(owner, binary.left),
                    self.literal_operand(owner, binary.right),
                ) {
                    (Some(binder), None, None, Some(value))
                    | (None, Some(binder), Some(value), None) => (binder, value),
                    _ => return None,
                };
                let carrier = match &binders[binder].kind {
                    crate::data::TypeParameterKind::Const { type_reference }
                    | crate::data::TypeParameterKind::Value { type_reference } => *type_reference,
                    _ => return None,
                };
                self.value_fits_carrier(carrier, value)
                    .then_some((binder, value))
            }

            fn binder_operand(
                &self,
                binders: &[&crate::data::TypeParameter],
                expression: crate::expression::ExpressionHandle,
            ) -> Option<usize> {
                let Name(path) = self.expression_table.expression(expression) else {
                    return None;
                };
                let members = self.expression_table.name_path_members(path.members);
                let [member] = members else { return None };
                binders.iter().position(|parameter| {
                    if path.symbol.is_valid() {
                        parameter.symbol == path.symbol
                    } else {
                        parameter.name.as_str() == member.as_str()
                    }
                })
            }

            fn literal_operand(
                &self,
                owner: symbols::SymbolHandle,
                expression: crate::expression::ExpressionHandle,
            ) -> Option<FiniteScalarValue> {
                match self.expression_table.expression(expression) {
                    crate::expression::ExpressionNode::Integer(literal) => {
                        let value = literal
                            .value_i64()
                            .map(i128::from)
                            .or_else(|| literal.value_u64().map(i128::from))?;
                        Some(FiniteScalarValue::Integer(value))
                    }
                    crate::expression::ExpressionNode::Boolean(value) => {
                        Some(FiniteScalarValue::Boolean(*value))
                    }
                    Name(path) => {
                        let members = self.expression_table.name_path_members(path.members);
                        let [member] = members else { return None };
                        let selected = if path.symbol.is_valid() {
                            path.symbol
                        } else {
                            self.symbols.find_top_level_by_name_and_kinds_from_source(
                                member.as_str(),
                                &[symbols::SymbolKind::Const],
                                self.symbols.symbol_provenance_source_span(owner)?,
                            )?
                        };
                        let declaration = self
                            .const_declarations()
                            .iter()
                            .find(|declaration| declaration.symbol == selected)?;
                        let value = language_semantics::const_value::CanonicalConstValue::new(
                            self.display_type_reference(declaration.declared_type),
                            declaration.evaluated_value.as_ref()?.encoding()?.to_owned(),
                            member.as_str(),
                        );
                        FiniteScalarValue::from_decoded(&value.decode_encoding()?)
                    }
                    _ => None,
                }
            }

            fn value_fits_carrier(
                &self,
                carrier: types::TypeReferenceHandle,
                value: FiniteScalarValue,
            ) -> bool {
                let Some(primitive) = self.primitive_type_reference(carrier) else {
                    return false;
                };
                match value {
                    FiniteScalarValue::Integer(value) => primitive
                        .integer_bounds()
                        .is_some_and(|(low, high)| low <= value && value <= high),
                    FiniteScalarValue::Boolean(_) => primitive == types::PrimitiveType::Bool,
                }
            }
        }
    }
    pub(crate) mod operand_order {
        use crate::static_applications::StaticMachineApplicationHandle;
        use crate::{
            TypedTrees, expression::ExpressionNode, static_applications::StaticMachineBindingValue,
        };

        pub struct CallArgumentEvaluationOrder {
            positions: Vec<usize>,
            position: usize,
            count: usize,
        }

        impl CallArgumentEvaluationOrder {
            pub fn is_natural(&self) -> bool {
                self.positions.is_empty()
            }
        }

        impl Iterator for CallArgumentEvaluationOrder {
            type Item = usize;

            fn next(&mut self) -> Option<usize> {
                if self.position == self.count {
                    return None;
                }
                let position = self.position;
                self.position += 1;
                Some(self.positions.get(position).copied().unwrap_or(position))
            }
        }

        impl TypedTrees {
            pub fn call_argument_evaluation_order(
                &self,
                application: StaticMachineApplicationHandle,
                target: symbols::SymbolHandle,
                count: usize,
            ) -> Result<CallArgumentEvaluationOrder, &'static str> {
                let natural = || CallArgumentEvaluationOrder {
                    positions: Vec::new(),
                    position: 0,
                    count,
                };
                if !application.is_valid() {
                    return Ok(natural());
                }
                let table = &self.expression_table;
                let values = table
                    .static_machine_application_bindings(application)
                    .iter()
                    .filter_map(|binding| match binding.value {
                        StaticMachineBindingValue::RuntimeValue { expression, .. } => {
                            Some((binding.parameter, expression))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if values.is_empty() {
                    return Ok(natural());
                }
                let closed = self.closed_requirement_application(target);
                let parameters = if let Some(closed) = &closed {
                    Some(self.state_signature_parameters(closed.signature))
                } else if let Some(state) = self.state_by_symbol(target) {
                    Some(self.state_parameters(state))
                } else if let Some(machine) = self.machine_by_symbol(target) {
                    self.machine_states(machine)
                        .first()
                        .map(|state| self.state_parameters(state))
                } else if let Some(slot) = self.dynamic_slot_by_symbol(target) {
                    Some(slot.parameters.as_slice())
                } else {
                    self.trait_machine_signature(target)
                        .map(|(_, signature)| self.state_signature_parameters(signature))
                        .or_else(|| {
                            self.machine_parameter_signature(target)
                                .map(|(_, signature)| self.state_signature_parameters(signature))
                        })
                        .or_else(|| {
                            if !target.is_valid() {
                                return None;
                            }
                            let binder = self.symbols.get(target).parent;
                            if !binder.is_valid()
                                || self.symbols.get(binder).kind
                                    != symbols::SymbolKind::ConformanceParameter
                            {
                                return None;
                            }
                            let owner = self.symbols.get(binder).parent;
                            self.evidence_bound_requirement(owner, binder, target)
                                .ok()
                                .flatten()
                                .map(|bound| self.state_signature_parameters(bound.signature))
                        })
                }
                .ok_or("runtime operand order lost its exact receiving declaration")?;
                if parameters.len() != count {
                    return Err(
                        "runtime operand order differs from its complete executable roster",
                    );
                }
                let mut runtime = Vec::with_capacity(values.len());
                for (binder, expression) in values {
                    let mut matches = parameters
                        .iter()
                        .enumerate()
                        .filter(|(_, parameter)| derives_from(self, parameter.symbol, binder));
                    let position = matches.next().map(|(position, _)| position);
                    if matches.next().is_some() {
                        return Err("runtime operand order repeats a receiving Value formal");
                    }
                    runtime.push((position, expression));
                }
                if runtime.iter().all(|(position, _)| position.is_none())
                    && closed.is_none_or(|closed| {
                        closed
                            .local_arguments
                            .iter()
                            .all(|argument| argument.static_operand().is_some())
                    })
                {
                    return Ok(natural());
                }
                let runtime_positions = runtime
                    .iter()
                    .map(|(position, _)| {
                        position.ok_or("runtime operand order lost a receiving Value formal")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                for (ordinal, position) in runtime_positions.iter().enumerate() {
                    if runtime_positions[..ordinal].contains(position) {
                        return Err("runtime operand order maps independent binders to one formal");
                    }
                }
                let mut explicit = Vec::new();
                let mut inferred = Vec::new();
                for ((_, mut expression), position) in runtime.into_iter().zip(&runtime_positions) {
                    while expression.is_valid() {
                        match table.expression(expression) {
                            ExpressionNode::Member(member) => expression = member.receiver,
                            _ => break,
                        }
                    }
                    let positions = if expression.is_valid()
                        && self.saved_argument_source(expression).is_some()
                    {
                        &mut inferred
                    } else {
                        &mut explicit
                    };
                    positions.push(*position);
                }
                explicit
                    .extend((0..count).filter(|position| !runtime_positions.contains(position)));
                explicit.extend(inferred);
                if explicit.iter().copied().eq(0..count) {
                    return Ok(natural());
                }
                Ok(CallArgumentEvaluationOrder {
                    positions: explicit,
                    position: 0,
                    count,
                })
            }
        }

        fn derives_from(
            program: &TypedTrees,
            mut symbol: symbols::SymbolHandle,
            binder: symbols::SymbolHandle,
        ) -> bool {
            while symbol.is_valid() {
                if symbol == binder {
                    return true;
                }
                let preceding = program.symbols.get(symbol).generated_from;
                if !preceding.is_valid() || preceding.arena_index() >= symbol.arena_index() {
                    return false;
                }
                symbol = preceding;
            }
            false
        }
    }
    pub mod signature {
        use crate::name::Identifier;
        use crate::types::TypeReferenceHandle;
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct AuthoredServiceReachTarget {
            pub service: SymbolHandle,
            pub source_span: source::SourceSpan,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct AuthoredServiceReachRow {
            pub owner: SymbolHandle,
            pub keyword_source_spans: Vec<source::SourceSpan>,
            pub targets: Vec<AuthoredServiceReachTarget>,
            pub installation_bound: bool,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum AuthoredInvocationTarget {
            Unresolved,
            Parameter { ordinal: u32, symbol: SymbolHandle },
            Service(SymbolHandle),
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct AuthoredInvocation {
            pub name: Identifier,
            pub source_span: source::SourceSpan,
            pub target: AuthoredInvocationTarget,
        }

        impl Default for AuthoredInvocation {
            fn default() -> Self {
                Self {
                    name: Identifier::default(),
                    source_span: source::SourceSpan::default(),
                    target: AuthoredInvocationTarget::Unresolved,
                }
            }
        }

        impl AuthoredInvocation {
            pub fn as_str(&self) -> &str {
                self.name.as_str()
            }
        }

        impl std::fmt::Display for AuthoredInvocation {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.name.fmt(formatter)
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct StateSignature {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub spelling: Option<language_core::OperatorSpelling>,
            pub lifetime_parameters: Vec<Identifier>,
            pub type_parameters: HandleSpan<crate::data::TypeParameter>,
            pub conformance_bounds: Vec<crate::machine::GenericConformanceBound>,
            pub is_default: bool,
            pub parameters: HandleSpan<StateParameter>,
            pub native_callback_parameters: Vec<NativeCallbackParameter>,
            pub return_type: TypeReferenceHandle,
            pub invokes: HandleSpan<AuthoredInvocation>,
            pub service_reach_row: language_semantics::ServiceReachRowId,
            pub service_reach_is_installation_bound: bool,
            pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
            pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
            pub suspends: bool,
            pub blocks: bool,
            pub contracts: HandleSpan<SignatureContract>,
            pub termination_guarantee: language_semantics::TerminationGuarantee,
            /// The requirement wrote `crashes none`. It publishes no contract
            /// row, so this is what tells it from an omitted clause, which
            /// defers to the selected member's routes (generics.md).
            pub declares_no_crash_route: bool,
        }

        impl Default for StateSignature {
            fn default() -> Self {
                Self {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::default(),
                    spelling: None,
                    lifetime_parameters: Vec::new(),
                    type_parameters: HandleSpan::empty(),
                    conformance_bounds: Vec::new(),
                    is_default: false,
                    parameters: HandleSpan::empty(),
                    native_callback_parameters: Vec::new(),
                    return_type: TypeReferenceHandle::invalid(),
                    invokes: HandleSpan::empty(),
                    service_reach_row: language_semantics::ServiceReachRowId::NULL,
                    service_reach_is_installation_bound: false,
                    suspends_keyword_source_spans: Vec::new(),
                    blocks_keyword_source_spans: Vec::new(),
                    suspends: false,
                    blocks: false,
                    contracts: HandleSpan::empty(),
                    termination_guarantee: language_semantics::TerminationGuarantee::NoGuarantee,
                    declares_no_crash_route: false,
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct NativeCallbackParameter {
            pub name: Identifier,
            pub binder: Identifier,
            pub native_ordinal: u32,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct StateParameter {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub type_reference: TypeReferenceHandle,
            pub is_const: bool,
            pub is_mutable: bool,
            pub relevance: language_core::BindingRelevance,
        }

        pub fn state_parameter_position(
            parameters: &[StateParameter],
            symbol: SymbolHandle,
        ) -> Option<usize> {
            parameters
                .iter()
                .position(|parameter| parameter.symbol == symbol)
        }

        impl Default for StateParameter {
            fn default() -> Self {
                Self {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::default(),
                    type_reference: TypeReferenceHandle::invalid(),
                    is_const: false,
                    is_mutable: false,
                    relevance: language_core::BindingRelevance::Relevant,
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct SignatureContract {
            pub kind: SignatureContractKind,
            pub keyword_source_span: Option<source::SourceSpan>,
            pub binding: Option<Identifier>,
            pub facts: arena::HandleSpan<crate::domain::ProofFact>,
            pub token_count: usize,
        }

        impl SignatureContract {
            pub fn is_crashes(&self) -> bool {
                matches!(self.kind, SignatureContractKind::Crashes { .. })
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum SignatureContractKind {
            #[default]
            Requires,
            Ensures,
            EnsuresForResultCase {
                result_data: SymbolHandle,
                result_case: SymbolHandle,
            },
            Crashes {
                cause: CrashCause,
            },
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum CrashCause {
            Trap,
            Abort,
        }
    }
    pub mod static_applications {
        use crate::expression::{ExpressionHandle, StaticMachineArgument};
        use crate::name::Identifier;
        use crate::type_identity::declaration_lifetime_parameters;
        use crate::type_identity::lifetime_parameter_identity;
        use crate::types::TypeReferenceHandle;
        use arena::{Handle, HandleSpan};
        use symbols::SymbolHandle;

        pub type StaticMachineApplicationHandle = Handle<StaticMachineApplication>;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct StaticConformanceSelection {
            pub operand_ordinal: u32,
            pub application: Option<Box<crate::ClosedConformanceApplication>>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct StaticMachineApplication {
            pub callable_contract_source: StaticMachineApplicationHandle,
            pub registration_operation: SymbolHandle,
            pub lexical_owner: SymbolHandle,
            pub target: Identifier,
            pub arguments: HandleSpan<StaticMachineArgument>,
            pub input_selections: HandleSpan<crate::product_operand::ProductInputSelection>,
            pub selected_machine_arguments: HandleSpan<StaticMachineArgument>,
            pub bindings: HandleSpan<StaticMachineBinding>,
            pub lifetime_bindings: HandleSpan<StaticMachineLifetimeBinding>,
            pub lifetime_binding_declaration: SymbolHandle,
            pub binding_declaration: SymbolHandle,
            pub conformance_selection: Option<Box<StaticConformanceSelection>>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct StaticMachineLifetimeBinding {
            pub parameter: Identifier,
            pub argument: Identifier,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct StaticMachineBinding {
            pub parameter: SymbolHandle,
            pub value: StaticMachineBindingValue,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum StaticMachineBindingValue {
            Type(TypeReferenceHandle),
            Const(TypeReferenceHandle),
            RuntimeValue {
                type_reference: TypeReferenceHandle,
                subject: SymbolHandle,
                expression: ExpressionHandle,
            },
            Machine(StaticMachineArgument),
            Conformance(StaticMachineArgument),
        }

        impl Default for StaticMachineBindingValue {
            fn default() -> Self {
                Self::Type(TypeReferenceHandle::invalid())
            }
        }

        pub fn project_type_operand(
            program: &crate::TypedTrees,
            reference: TypeReferenceHandle,
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> TypeReferenceHandle {
            if let crate::types::TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
                && let Some((_, actual)) = substitutions.iter().find(|(formal, _)| formal == symbol)
            {
                return *actual;
            }
            reference
        }

        impl crate::TypedTrees {
            pub fn static_application_lifetimes_for_declaration(
                &self,
                application: StaticMachineApplicationHandle,
                receiving: SymbolHandle,
            ) -> Result<std::borrow::Cow<'_, [StaticMachineLifetimeBinding]>, &'static str>
            {
                let source = self
                    .expression_table
                    .static_machine_application(application);
                let bindings = self
                    .expression_table
                    .static_machine_application_lifetimes(application);
                if bindings.is_empty() {
                    return Ok(std::borrow::Cow::Borrowed(bindings));
                }
                let original = self.lifetime_declaration_owner(source.lifetime_binding_declaration);
                let receiving = self.lifetime_declaration_owner(receiving);
                let registered = self.lifetime_declaration_owner(source.registration_operation);
                let registered = if declaration_lifetime_parameters(self, registered).is_some() {
                    registered
                } else if let Some((_, signature)) =
                    self.machine_parameter_signature(source.registration_operation)
                {
                    self.lifetime_declaration_owner(signature.symbol)
                } else if let Some(machine) =
                    self.machine_holding_state(source.registration_operation)
                {
                    self.lifetime_declaration_owner(machine.symbol)
                } else {
                    return Err(
                        "static region group lost its original registered callable telescope",
                    );
                };
                if original != registered {
                    return Err(
                        "static region group does not belong to its exact registered callable telescope",
                    );
                }
                let original_parameters = declaration_lifetime_parameters(self, original)
                    .ok_or("static region group lost its original receiving declaration")?;
                let receiving_parameters = declaration_lifetime_parameters(self, receiving)
                    .ok_or("static region group lost its requested receiving declaration")?;
                let same_declaration = original == receiving;
                if !same_declaration {
                    let corresponds = |implementation, requirement| {
                        self.trait_machine_signature(requirement).is_some_and(
                            |(owner, signature)| {
                                self.owned_conformance_requirement_scope(
                                    implementation,
                                    owner.symbol,
                                    signature.symbol,
                                )
                                .is_some()
                            },
                        )
                    };
                    if !corresponds(receiving, original) && !corresponds(original, receiving) {
                        return Err(
                            "static region group has no exact requirement/owned-realizer correspondence",
                        );
                    }
                    if original_parameters.len() != receiving_parameters.len() {
                        return Err(
                            "requirement and owned realizer have different own region telescopes",
                        );
                    }
                }
                let mut projected = Vec::new();
                for (ordinal, binding) in bindings.iter().enumerate() {
                    let parameter = lifetime_parameter_identity(
                        self,
                        original,
                        binding.parameter.as_str(),
                    )
                    .filter(|parameter| parameter.declaration == original)
                    .ok_or(
                        "static region binding does not name its original declaration's own formal",
                    )?;
                    if bindings[..ordinal]
                        .iter()
                        .any(|previous| previous.parameter == binding.parameter)
                    {
                        return Err("static region group repeats an original receiving formal");
                    }
                    lifetime_parameter_identity(
                        self,
                        source.lexical_owner,
                        binding.argument.as_str(),
                    )
                    .ok_or(
                        "static region argument has no identity in its original supplying scope",
                    )?;
                    if !same_declaration {
                        projected.push(StaticMachineLifetimeBinding {
                    parameter: receiving_parameters
                        .get(parameter.ordinal)
                        .ok_or("static region formal ordinal exceeds its exact realizer telescope")?
                        .clone(),
                    argument: binding.argument.clone(),
                });
                    }
                }
                if same_declaration {
                    Ok(std::borrow::Cow::Borrowed(bindings))
                } else {
                    Ok(std::borrow::Cow::Owned(projected))
                }
            }
        }
    }
    use crate::expression::ExpressionNode::Name;
    use crate::expression::TableCallExpression;
    pub use operand_order::CallArgumentEvaluationOrder;

    impl crate::TypedTrees {
        pub fn call_has_no_runtime_receiver_for_target(&self, call: &TableCallExpression) -> bool {
            if let Some(owner) = self.machine_holding_state(call.target_symbol)
                && let Some(target) = self
                    .machine_states(owner)
                    .iter()
                    .find(|target| target.symbol == call.target_symbol)
            {
                return self.call_has_no_runtime_receiver(call, owner, target);
            }
            if crate::operator::selected_named_expression_call_has_no_runtime_receiver(self, call) {
                return true;
            }
            if let Some(receiver) = self.expression_table.name_path_node(call.receiver)
                && receiver.symbol.is_valid()
                && self.symbols.get(receiver.symbol).kind
                    == symbols::SymbolKind::ConformanceParameter
                && let Ok(Some(requirement)) = self.evidence_bound_requirement(
                    self.symbols.get(receiver.symbol).parent,
                    receiver.symbol,
                    call.target_symbol,
                )
            {
                return call.target == requirement.signature.name
                    && call.arguments.len()
                        == self.state_signature_parameters(requirement.signature).len();
            }
            let binder = self.symbols.get(call.target_symbol).parent;
            if binder.is_valid()
                && self.symbols.get(binder).kind == symbols::SymbolKind::ConformanceParameter
                && let Ok(Some(requirement)) = self.evidence_bound_requirement(
                    self.symbols.get(binder).parent,
                    binder,
                    call.target_symbol,
                )
            {
                return self
                    .expression_table
                    .name_path_node(call.receiver)
                    .is_some_and(|path| path.symbol == binder)
                    && call.target == requirement.signature.name
                    && call.arguments.len()
                        == self.state_signature_parameters(requirement.signature).len();
            }
            let slot = self.dynamic_slot_by_symbol(call.target_symbol);
            let requirement = slot.map_or(call.target_symbol, |slot| slot.requirement);
            let mut requirements = self
                .traits()
                .iter()
                .filter(|owner| slot.is_none_or(|slot| slot.declaring_trait == owner.symbol))
                .filter_map(|owner| {
                    self.trait_machine_signature_by_symbol(owner, requirement)
                        .map(|signature| (owner, signature))
                });
            let Some((owner, signature)) = requirements.next() else {
                return false;
            };
            let parameters = self.state_signature_parameters(signature);
            let operand_count = match slot {
                Some(slot) if slot.data_parameters().len() == parameters.len() => {
                    slot.parameters.len()
                }
                Some(_) => return false,
                None => parameters.len(),
            };
            let arguments = self.expression_table.expression_handles(call.arguments);
            if requirements.next().is_some()
                || !call.target_symbol.is_valid()
                || arguments.len() != call.arguments.count() as usize
                || arguments.len() != operand_count
                || (slot.is_none() && call.target.as_str() != signature.name.as_str())
            {
                return false;
            }
            if !call.receiver.is_valid() {
                return true;
            }
            if !self.expression_table.expression_is_valid(call.receiver) {
                return false;
            }
            matches!(self.expression_table.expression(call.receiver),
            Name(path) if path.symbol == owner.symbol)
        }

        pub fn call_has_no_runtime_receiver(
            &self,
            call: &TableCallExpression,
            owner: &crate::machine::Machine,
            target: &crate::state::State,
        ) -> bool {
            if call.target_symbol != target.symbol {
                return false;
            }
            if !call.receiver.is_valid() {
                return true;
            }
            if !self.expression_table.expression_is_valid(call.receiver) {
                return false;
            }
            let Name(path) = self.expression_table.expression(call.receiver) else {
                return false;
            };
            if path.symbol.is_valid()
                && matches!(
                    self.symbols.get(path.symbol).kind,
                    symbols::SymbolKind::Conformance | symbols::SymbolKind::ConformanceParameter
                )
            {
                return true;
            }
            owner.attached_data_symbol.is_valid() && path.symbol == owner.attached_data_symbol
        }
    }
    /// Exact declared callable queries used while constructing typed source.
    /// These expose signatures; they perform no body or proof admission.
    pub mod declaration_queries {
        use crate::machine::Machine;
        use crate::state::State;
        use crate::statement::StatementNode;
        use symbols::SymbolHandle;
        pub fn find_state(
            program: &crate::TypedTrees,
            state_symbol: SymbolHandle,
        ) -> Option<(&Machine, &State)> {
            let machine = program.machine_holding_state(state_symbol)?;
            let state = program.machine_state(machine.symbol, state_symbol)?.1;
            Some((machine, state))
        }
        pub fn find_machine_head(
            program: &crate::TypedTrees,
            symbol: SymbolHandle,
        ) -> Option<(&Machine, &State)> {
            if let Some(machine) = program.machine_by_symbol(symbol) {
                return program
                    .machine_states(machine)
                    .first()
                    .map(|entry| (machine, entry));
            }
            let (machine, state) = find_state(program, symbol)?;
            program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == state.symbol)
                .then_some((machine, state))
        }
        pub fn call_target_state(
            program: &crate::TypedTrees,
            symbol: SymbolHandle,
        ) -> Option<&State> {
            find_machine_head(program, symbol)
                .map(|(_, state)| state)
                .or_else(|| program.state_by_symbol(symbol))
        }
        pub fn call_target_entry_symbol(
            program: &crate::TypedTrees,
            symbol: SymbolHandle,
        ) -> SymbolHandle {
            call_target_state(program, symbol).map_or(symbol, |state| state.symbol)
        }
        pub fn call_target_return_type(
            program: &crate::TypedTrees,
            target_state_symbol: SymbolHandle,
        ) -> Option<crate::types::TypeReferenceHandle> {
            if let Some(state) = call_target_state(program, target_state_symbol) {
                return Some(state.return_type);
            }
            if let Some(slot) = program.dynamic_slot_by_symbol(target_state_symbol) {
                return Some(slot.return_type);
            }
            if let Some(application) = program.closed_requirement_application(target_state_symbol) {
                return Some(application.signature.return_type);
            }
            if let Some((_, signature)) = program.machine_parameter_signature(target_state_symbol) {
                return Some(signature.return_type);
            }
            if let Some(operator) =
                crate::operator::declaration_by_symbol(program, target_state_symbol)
            {
                return Some(operator.return_type);
            }
            if let Some(reference) = asm_intrinsic_result_type(program, target_state_symbol) {
                return Some(reference);
            }
            program
                .trait_machine_signature(target_state_symbol)
                .map(|(_, signature)| signature.return_type)
        }
        pub fn asm_intrinsic_result_type(
            program: &crate::TypedTrees,
            target: SymbolHandle,
        ) -> Option<crate::types::TypeReferenceHandle> {
            let atom = match program.symbols.builtin_function_for_symbol(target)? {
                symbols::BuiltinFunction::AsmPortIn => symbols::BuiltinTypeAtom::U8,
                symbols::BuiltinFunction::AsmSnapshotFlags
                | symbols::BuiltinFunction::AsmReadMsr
                | symbols::BuiltinFunction::AsmReadCr0
                | symbols::BuiltinFunction::AsmReadCr2
                | symbols::BuiltinFunction::AsmReadCr3
                | symbols::BuiltinFunction::AsmReadCr4
                | symbols::BuiltinFunction::AsmReadSctlrEl1
                | symbols::BuiltinFunction::AsmReadTcrEl1
                | symbols::BuiltinFunction::AsmReadTtbr0El1
                | symbols::BuiltinFunction::AsmReadTtbr1El1
                | symbols::BuiltinFunction::AsmReadMairEl1
                | symbols::BuiltinFunction::AsmReadVbarEl1
                | symbols::BuiltinFunction::AsmReadTpidrEl1
                | symbols::BuiltinFunction::AsmReadEsrEl1
                | symbols::BuiltinFunction::AsmReadFarEl1 => symbols::BuiltinTypeAtom::U64,
                _ => return None,
            };
            let symbol = program
                .symbols
                .child_handles(program.symbols.root())?
                .find(|candidate| program.symbols.builtin_type_atom(*candidate) == Some(atom))?;
            program
                .type_reference_table
                .find_named_type_reference(symbol)
        }
        pub fn terminal_callable_target(
            program: &crate::TypedTrees,
            machine: &Machine,
            target: SymbolHandle,
        ) -> Option<(SymbolHandle, crate::types::TypeReferenceHandle)> {
            if let Some(owner) = program.machine_by_symbol(target) {
                let entry = program.machine_states(owner).first()?;
                return (owner.symbol != machine.symbol)
                    .then_some((entry.symbol, entry.return_type));
            }
            if let Some((owner, state)) = find_state(program, target) {
                return (owner.symbol != machine.symbol)
                    .then_some((state.symbol, state.return_type));
            }
            call_target_return_type(program, target).map(|return_type| (target, return_type))
        }
        pub fn statement_source_span(
            program: &crate::TypedTrees,
            statement: &StatementNode,
        ) -> Option<source::SourceSpan> {
            let authored = |source_span: source::SourceSpan| -> Option<source::SourceSpan> {
                (source_span != source::SourceSpan::default()).then_some(source_span)
            };
            let expression_span = |handle: crate::expression::ExpressionHandle| {
                handle
                    .is_valid()
                    .then(|| program.expression_table.source_span(handle))
                    .and_then(authored)
            };
            match statement {
                StatementNode::Assembly(_) => None,
                StatementNode::AssemblyFact(fact) => {
                    program.proof_fact_source_span(fact.fact).and_then(authored)
                }
                StatementNode::Assignment(assignment) => {
                    expression_span(assignment.value).or_else(|| expression_span(assignment.target))
                }
                StatementNode::Call(call) => authored(call.source_span),
                StatementNode::Expression(expression) => expression_span(*expression),
                StatementNode::LocalData(local) => expression_span(local.initial_value),
                StatementNode::Transition(transition) => authored(transition.source_span),
            }
        }
    }

    /// Expected result types supplied by ordinary source contexts to calls.
    pub mod expected_source_calls {
        use crate::expression::{ExpressionHandle, ExpressionNode};
        use crate::types::{TypeReferenceHandle, TypeReferenceNode};
        pub fn push_expected_call(
            program: &crate::TypedTrees,
            value: ExpressionHandle,
            expected_type: TypeReferenceHandle,
            expected: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
        ) {
            match program.expression_table.expression(value) {
                ExpressionNode::Match(dispatch) => {
                    for arm in program.expression_table.match_arms(dispatch.arms) {
                        push_expected_call(program, arm.value, expected_type, expected);
                    }
                }
                ExpressionNode::Borrow(inner) => {
                    let reference = program
                        .type_reference_table
                        .unconstrained_type_reference(expected_type);
                    if let TypeReferenceNode::Reference { referee, .. } =
                        program.type_reference_table.type_reference(reference)
                    {
                        push_expected_call(program, inner.target, *referee, expected);
                    }
                }
                ExpressionNode::Atomic(atomic) => {
                    if program.atomic_once_operands(atomic).is_none() {
                        push_expected_call(program, atomic.value, expected_type, expected);
                    }
                }
                ExpressionNode::Call(_) => {
                    if let Some((_, existing)) = expected
                        .iter_mut()
                        .find(|(candidate, _)| *candidate == value)
                    {
                        *existing = expected_type;
                    } else {
                        expected.push((value, expected_type));
                    }
                }
                ExpressionNode::ArrayLiteral(values) => {
                    let Some(element_type) = array_element_type(program, expected_type) else {
                        return;
                    };
                    for element in program
                        .expression_table
                        .expression_handles(*values)
                        .iter()
                        .copied()
                    {
                        push_expected_call(program, element, element_type, expected);
                    }
                }
                ExpressionNode::MachineValue(_)
                | ExpressionNode::Binary(_)
                | ExpressionNode::Boolean(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Name(_)
                | ExpressionNode::SavedSubject(_)
                | ExpressionNode::Range(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::String(_)
                | ExpressionNode::Unary(_)
                | ExpressionNode::TypeExpression(_)
                | ExpressionNode::ZeroValue(_) => {}
            }
        }

        fn array_element_type(
            program: &crate::TypedTrees,
            type_reference: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            match program.type_reference_table.type_reference(type_reference) {
                crate::types::TypeReferenceNode::Reference { referee, .. }
                | crate::types::TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => array_element_type(program, *referee),
                crate::types::TypeReferenceNode::FixedArray { element_type, .. }
                | crate::types::TypeReferenceNode::Slice { element_type } => Some(*element_type),
                crate::types::TypeReferenceNode::Callable { .. }
                | crate::types::TypeReferenceNode::Generic { .. }
                | crate::types::TypeReferenceNode::DynamicTrait { .. }
                | crate::types::TypeReferenceNode::Named { .. }
                | crate::types::TypeReferenceNode::ConstExpression(_)
                | crate::types::TypeReferenceNode::Unit => None,
            }
        }
    }
}

pub mod control_flow {
    pub mod machine {
        use crate::expression::ExpressionHandle;
        use crate::name::Identifier;
        use crate::signature::{AuthoredInvocation, SignatureContract};
        use crate::state::State;
        use crate::types::TypeReferenceHandle;
        use arena::HandleSpan;
        use language_core::operator_spelling::OperatorSpelling;
        use symbols::SymbolHandle;

        impl crate::TypedTrees {
            pub fn machine_authored_target<'program>(
                &'program self,
                machine: &'program Machine,
            ) -> Option<&'program str> {
                machine.authored_target.as_ref().map(Identifier::as_str)
            }

            pub fn source_target_context<'program>(
                &'program self,
                machine: &Machine,
            ) -> Option<&'program str> {
                let mut owner = machine.symbol;
                let mut visited = Vec::new();
                while owner.is_valid() {
                    owner = self.lifetime_declaration_owner(owner);
                    if !owner.is_valid() || visited.contains(&owner) {
                        return None;
                    }
                    if let Some(declaration) = self.machine_declaration_for_lexical_scope(owner) {
                        if let Some(target) = self.machine_authored_target(declaration) {
                            return Some(target);
                        }
                    } else if self.symbols.get(owner).kind == symbols::SymbolKind::Machine {
                        return None;
                    }
                    visited.push(owner);
                    owner = self.declaration_scope_parent(owner);
                }
                None
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum MachineBodyStatus {
            Deferred,
            #[default]
            Complete,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum MachinePurpose {
            #[default]
            Runtime,
            ProviderDefaults,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct LexicalRegionOrigin {
            pub machine: SymbolHandle,
            pub state: SymbolHandle,
            pub arguments: HandleSpan<ExpressionHandle>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct Machine {
            pub lexical_region_origin: Option<LexicalRegionOrigin>,
            pub assembly_target_requirement: language_core::inline_assembly::AsmTargetApplicability,
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub attached_data: Option<Identifier>,
            pub attached_data_symbol: SymbolHandle,
            pub attached_data_application: TypeReferenceHandle,
            pub generic_data_template: SymbolHandle,
            pub spelling: Option<OperatorSpelling>,
            pub is_public: bool,
            pub interface_exposure:
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure,
            pub supply_mode: language_semantics::MachineSupplyMode,
            pub purpose: MachinePurpose,
            pub target: Option<Identifier>,
            pub authored_target: Option<Identifier>,
            pub target_family_interface: bool,
            pub body_is_present: bool,
            pub body_status: MachineBodyStatus,
            pub structural_type_equations_pending: bool,
            pub termination_plan: language_semantics::MachineTerminationPlan,
            pub service_reach_row: language_semantics::ServiceReachRowId,
            pub service_reach_is_installation_bound: bool,
            pub admitted_same_stack_bytes: u64,
            /// The header wrote `crashes none`, which leaves no row in
            /// `contracts`. `wiki/spec/language/effects.md`: "An explicit
            /// empty value, `reaches none`, `crashes none`, `suspends false`
            /// or `blocks false`, promises the empty value and rejects any
            /// body or selection that violates it." False for an omitted
            /// clause, which on a private body requests inference.
            pub declares_no_crash_route: bool,
            pub suspends_keyword_source_spans: Vec<source::SourceSpan>,
            pub blocks_keyword_source_spans: Vec<source::SourceSpan>,
            pub lifetime_parameters: Vec<Identifier>,
            pub type_parameters: HandleSpan<crate::data::TypeParameter>,
            pub declared_type_equations: HandleSpan<DeclaredTypeEquation>,
            pub owned_data: HandleSpan<OwnedData>,
            pub satisfies: HandleSpan<TraitConformance>,
            pub conformance_bounds: Vec<GenericConformanceBound>,
            pub invokes: HandleSpan<AuthoredInvocation>,
            pub suspends: bool,
            pub blocks: bool,
            pub contracts: HandleSpan<SignatureContract>,
            pub states: HandleSpan<State>,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct DeclaredTypeEquation {
            pub parameter: TypeReferenceHandle,
            pub definition: TypeReferenceHandle,
        }

        impl Machine {
            pub fn has_specialization_parameters(&self) -> bool {
                !self.type_parameters.is_empty() || !self.conformance_bounds.is_empty()
            }

            pub fn supplies_checked_body(&self) -> bool {
                match self.supply_mode {
                    language_semantics::MachineSupplyMode::CheckedBody => true,
                    language_semantics::MachineSupplyMode::Boundary => self.body_is_present,
                    _ => false,
                }
            }

            pub fn has_bodyless_signature(&self) -> bool {
                !self.supplies_checked_body() && !self.body_is_present
            }

            pub fn symbol_spelling(&self) -> std::borrow::Cow<'_, str> {
                match &self.target {
                    Some(target) => std::borrow::Cow::Owned(format!(
                        "{}::{}",
                        self.name.as_str(),
                        target.as_str()
                    )),
                    None => std::borrow::Cow::Borrowed(self.name.as_str()),
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct TargetMachineFamily {
            pub name: Identifier,
            pub canonical: SymbolHandle,
            pub purpose: MachinePurpose,
            pub members: Vec<TargetMachineFamilyMember>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct TargetMachineFamilyMember {
            pub target: Identifier,
            pub machine: SymbolHandle,
        }

        impl Default for Machine {
            fn default() -> Self {
                Self {
                    lexical_region_origin: None,
                    assembly_target_requirement:
                        language_core::inline_assembly::AsmTargetApplicability::Any,
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::default(),
                    attached_data: None,
                    attached_data_symbol: SymbolHandle::invalid(),
                    attached_data_application: TypeReferenceHandle::invalid(),
                    generic_data_template: SymbolHandle::invalid(),
                    spelling: None,
                    is_public: false,
                    interface_exposure: Default::default(),
                    supply_mode: language_semantics::MachineSupplyMode::CheckedBody,
                    purpose: MachinePurpose::Runtime,
                    target: None,
                    authored_target: None,
                    target_family_interface: false,
                    body_is_present: true,
                    body_status: MachineBodyStatus::Complete,
                    structural_type_equations_pending: false,
                    termination_plan: language_semantics::MachineTerminationPlan::default(),
                    service_reach_row: language_semantics::ServiceReachRowId::NULL,
                    service_reach_is_installation_bound: false,
                    admitted_same_stack_bytes: 0,
                    declares_no_crash_route: false,
                    suspends_keyword_source_spans: Vec::new(),
                    blocks_keyword_source_spans: Vec::new(),
                    lifetime_parameters: Vec::new(),
                    type_parameters: HandleSpan::empty(),
                    declared_type_equations: HandleSpan::empty(),
                    owned_data: HandleSpan::empty(),
                    satisfies: HandleSpan::empty(),
                    conformance_bounds: Vec::new(),
                    invokes: HandleSpan::empty(),
                    suspends: false,
                    blocks: false,
                    contracts: HandleSpan::empty(),
                    states: HandleSpan::empty(),
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct GenericConformanceBound {
            pub binder: SymbolHandle,
            pub binder_name: Identifier,
            pub trait_symbol: SymbolHandle,
            pub trait_name: Identifier,
            pub lifetime_arguments: Vec<Identifier>,
            pub arguments: Vec<TypeReferenceHandle>,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct OwnedData {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub type_reference: TypeReferenceHandle,
            pub initial_value: ExpressionHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TraitConformance {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub trait_lifetime_arguments: Vec<u32>,
            pub arguments: HandleSpan<crate::types::TypeReferenceHandle>,
            pub requirement: Option<Identifier>,
            pub requirement_symbol: SymbolHandle,
            pub requirement_source_span: Option<source::SourceSpan>,
            pub alias: Option<Identifier>,
            pub external_binding: Option<language_semantics::ExternalBindingId>,
            pub via_expression: crate::expression::ExpressionHandle,
            pub external_binding_source_span: Option<source::SourceSpan>,
        }

        #[derive(Debug, Clone, Copy)]
        pub enum SatisfiedDeclaration<'program> {
            Trait {
                definition: &'program crate::trait_definition::TraitDefinition,
                requirement: &'program crate::signature::StateSignature,
            },
            TopLevelRequirement(&'program Machine),
            Operator(&'program crate::operator::OperatorDefinition),
        }

        impl SatisfiedDeclaration<'_> {
            pub fn symbol(self) -> SymbolHandle {
                match self {
                    Self::Trait { requirement, .. } => requirement.symbol,
                    Self::TopLevelRequirement(requirement) => requirement.symbol,
                    Self::Operator(operator) => operator.symbol,
                }
            }
        }

        pub fn resolve_satisfied_declaration<'program>(
            program: &'program crate::TypedTrees,
            machine: &'program Machine,
            conformance: &'program TraitConformance,
        ) -> Option<SatisfiedDeclaration<'program>> {
            let requirement_name = conformance.requirement.as_ref()?;
            if let Some(definition) = program.trait_definition_by_symbol(conformance.symbol) {
                let entry = program.machine_states(machine).first()?;
                let implementation_dispatch =
                    program.normalized_result_dispatch_set(entry.return_type);
                let named = program
                    .trait_machine_signatures(definition)
                    .iter()
                    .filter(|requirement| requirement.name == *requirement_name)
                    .collect::<Vec<_>>();
                let matching = if named.len() == 1 {
                    named
                } else {
                    named
                        .into_iter()
                        .filter(|requirement| {
                            program.normalized_result_dispatch_set(requirement.return_type)
                                == implementation_dispatch
                        })
                        .collect()
                };
                let [requirement] = matching.as_slice() else {
                    return None;
                };
                return Some(SatisfiedDeclaration::Trait {
                    definition,
                    requirement,
                });
            }

            if !program
                .type_reference_handles(conformance.arguments)
                .is_empty()
            {
                return None;
            }
            let operator = crate::operator::resolve_satisfied_checked_operator_for_conformance(
                program,
                machine,
                conformance,
            )?;
            if let Some(requirement) =
                program
                    .machines_with_symbol(operator.symbol)
                    .find(|requirement| {
                        requirement.supply_mode
                            == language_semantics::MachineSupplyMode::TopLevelRequirement
                    })
            {
                return Some(SatisfiedDeclaration::TopLevelRequirement(requirement));
            }
            Some(SatisfiedDeclaration::Operator(operator))
        }

        pub fn machine_by_symbol(machines: &[Machine], symbol: SymbolHandle) -> Option<&Machine> {
            machines.iter().find(|machine| machine.symbol == symbol)
        }
    }
    pub mod state {
        use crate::name::Identifier;
        use crate::signature::SignatureContract;
        use crate::signature::StateParameter;
        use crate::statement::StatementNode;
        use arena::HandleSpan;
        use symbols::SymbolHandle;

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct State {
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub parameters: HandleSpan<StateParameter>,
            pub return_type: crate::types::TypeReferenceHandle,
            pub contracts: HandleSpan<SignatureContract>,
            pub statement_nodes: HandleSpan<StatementNode>,
        }

        pub fn state_by_symbol(states: &[State], symbol: SymbolHandle) -> Option<&State> {
            states.iter().find(|state| state.symbol == symbol)
        }

        pub fn state_position(states: &[State], symbol: SymbolHandle) -> Option<usize> {
            states.iter().position(|state| state.symbol == symbol)
        }

        impl crate::TypedTrees {
            pub fn parameter_domain_membership_contract(
                &mut self,
                parameter_symbol: symbols::SymbolHandle,
                parameter_name: crate::name::Identifier,
                domain_symbol: symbols::SymbolHandle,
                domain_full_name: &str,
                domain_arguments: Vec<crate::types::TypeReferenceHandle>,
                semantic_domain: language_semantics::SemanticDomainId,
            ) -> crate::signature::SignatureContract {
                let domain_arguments = self
                    .type_reference_table
                    .insert_type_reference_handles(domain_arguments);
                let mut members = arena::HandleSpan::empty();
                self.expression_table
                    .push_name_path_member(&mut members, parameter_name);
                let mut member_symbols = arena::HandleSpan::empty();
                self.expression_table
                    .push_name_path_member_symbol(&mut member_symbols, parameter_symbol);
                let value = self
                    .expression_table
                    .insert(crate::expression::ExpressionNode::Name(
                        crate::expression::TableNamePath::new(
                            members,
                            member_symbols,
                            parameter_symbol,
                            parameter_symbol,
                        ),
                    ));

                let mut domain = arena::HandleSpan::empty();
                for part in domain_full_name.split("::") {
                    self.domain_path_members
                        .append_to_span(&mut domain, crate::name::Identifier::generated(part));
                }

                let mut facts = arena::HandleSpan::empty();
                self.proof_facts.append_to_span(
                    &mut facts,
                    crate::domain::ProofFact::Membership(crate::domain::ProofMembershipFact {
                        value,
                        domain,
                        domain_symbol,
                        domain_arguments,
                        semantic_domain,
                        authored_domain_selection: None,
                    }),
                );

                crate::signature::SignatureContract {
                    kind: crate::signature::SignatureContractKind::Requires,
                    keyword_source_span: None,
                    binding: None,
                    facts,
                    token_count: 0,
                }
            }
        }
    }
    pub mod statement {
        use crate::name::Identifier;
        use crate::remapped;
        use arena::{Arena, Handle, HandleSpan};
        use source::SourceSpan;
        use symbols::SymbolHandle;
        pub(crate) mod assembly {
            use super::StatementTable;
            use crate::name::Identifier;
            use arena::{Handle, HandleSpan};

            #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
            pub struct AssemblyControlFlow {
                pub target_requirement: language_core::inline_assembly::AsmTargetApplicability,
                pub primitives: HandleSpan<crate::expression::ExpressionHandle>,
                pub scratch: [crate::expression::ExpressionHandle;
                    language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
                pub blocks: HandleSpan<AssemblyBlock>,
                pub statement_count: u32,
            }
            impl Default for AssemblyControlFlow {
                fn default() -> Self {
                    Self {
                        target_requirement: Default::default(),
                        primitives: Default::default(),
                        scratch: [crate::expression::ExpressionHandle::invalid();
                            language_core::inline_assembly::ASM_SCRATCH_SLOT_COUNT],
                        blocks: Default::default(),
                        statement_count: 0,
                    }
                }
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
            pub enum AssemblyControlPoint {
                Entry(Handle<AssemblyControlFlow>),
                Label(Handle<AssemblyBlock>),
            }

            #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
            pub struct AssemblyBlock {
                pub label: Identifier,
                pub start: u32,
                pub count: u32,
                pub exit: AssemblyExit,
            }

            #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
            pub enum AssemblyExit {
                #[default]
                Fallthrough,
                Jump(Handle<AssemblyBlock>),
                ZeroFlag(Handle<AssemblyBlock>, bool),
                EnclosingTransition,
            }

            impl StatementTable {
                pub fn insert_assembly(
                    &mut self,
                    graph: AssemblyControlFlow,
                ) -> Handle<AssemblyControlFlow> {
                    self.assembly_graphs.insert(graph)
                }
                pub fn assembly_mut(
                    &mut self,
                    handle: Handle<AssemblyControlFlow>,
                ) -> &mut AssemblyControlFlow {
                    self.assembly_graphs.get_mut(handle)
                }
                pub fn assembly(
                    &self,
                    handle: Handle<AssemblyControlFlow>,
                ) -> &AssemblyControlFlow {
                    self.assembly_graphs.get(handle)
                }
                pub fn insert_assembly_blocks(
                    &mut self,
                    blocks: impl IntoIterator<Item = AssemblyBlock>,
                ) -> HandleSpan<AssemblyBlock> {
                    self.assembly_blocks.insert_many(blocks)
                }
                pub fn assembly_blocks(&self, span: HandleSpan<AssemblyBlock>) -> &[AssemblyBlock] {
                    self.assembly_blocks.span_or_empty(span)
                }
                pub fn assembly_block_mut(
                    &mut self,
                    handle: Handle<AssemblyBlock>,
                ) -> &mut AssemblyBlock {
                    self.assembly_blocks.get_mut(handle)
                }
                pub fn assembly_block(&self, handle: Handle<AssemblyBlock>) -> &AssemblyBlock {
                    self.assembly_blocks.get(handle)
                }
                pub(crate) fn copy_assembly_from(
                    &mut self,
                    source: &Self,
                    handle: Handle<AssemblyControlFlow>,
                ) -> Handle<AssemblyControlFlow> {
                    let graph = source.assembly(handle);
                    let blocks = self.insert_assembly_blocks(
                        source.assembly_blocks(graph.blocks).iter().cloned(),
                    );
                    for handle in blocks.handles() {
                        if let AssemblyExit::Jump(target) | AssemblyExit::ZeroFlag(target, _) =
                            self.assembly_blocks.get(handle).exit
                        {
                            let index = graph
                                .blocks
                                .handles()
                                .position(|handle| handle == target)
                                .expect("assembly copy retains only graph-local edges");
                            self.assembly_blocks.get_mut(handle).exit =
                                if let AssemblyExit::ZeroFlag(_, when_set) =
                                    self.assembly_blocks.get(handle).exit
                                {
                                    AssemblyExit::ZeroFlag(blocks.handle_at(index as u32), when_set)
                                } else {
                                    AssemblyExit::Jump(blocks.handle_at(index as u32))
                                };
                        }
                    }
                    self.insert_assembly(AssemblyControlFlow {
                        target_requirement: graph.target_requirement,
                        blocks,
                        statement_count: graph.statement_count,
                        scratch: graph.scratch,
                        primitives: graph.primitives,
                    })
                }
            }
        }
        pub(crate) mod control_graph {
            use super::{AssemblyControlPoint, AssemblyExit, StatementNode, StatementTable};
            use arena::HandleSpan;
            use std::ops::Bound;

            pub struct StatementControlGraph {
                pub blocks: Vec<StatementControlBlock>,
            }

            pub struct StatementExecutionIter<'source> {
                statements: &'source [StatementNode],
                order: std::vec::IntoIter<usize>,
            }

            pub struct StatementExecutionOrder {
                source_count: usize,
                indices: Vec<usize>,
                positions: Vec<usize>,
            }

            impl StatementExecutionOrder {
                pub fn indices(&self) -> &[usize] {
                    &self.indices
                }
                pub fn position(&self, source: usize) -> Option<usize> {
                    if source == self.source_count {
                        return Some(self.indices.len());
                    }
                    self.positions
                        .get(source)
                        .copied()
                        .and_then(|position| position.checked_sub(1))
                }
                pub fn prefix_end(&self, endpoint: Bound<usize>) -> Option<usize> {
                    match endpoint {
                        Bound::Unbounded => Some(self.indices.len()),
                        Bound::Excluded(source) => self.position(source),
                        Bound::Included(source) => self
                            .position(source)
                            .filter(|position| *position < self.indices.len())?
                            .checked_add(1),
                    }
                }

                pub fn precedes(&self, left: usize, right: usize) -> bool {
                    self.position(left)
                        .zip(self.position(right))
                        .is_some_and(|(left, right)| left < right)
                }
                pub fn at_or_before(&self, left: usize, right: usize) -> bool {
                    self.position(left)
                        .zip(self.position(right))
                        .is_some_and(|(left, right)| left <= right)
                }
                pub fn latest(&self, left: usize, right: usize) -> Option<usize> {
                    Some(if self.position(left)? < self.position(right)? {
                        right
                    } else {
                        left
                    })
                }
                pub fn successor(&self, source: usize) -> Option<usize> {
                    let position = self.position(source)?;
                    (position < self.indices.len()).then(|| {
                        self.indices
                            .get(position + 1)
                            .copied()
                            .unwrap_or(self.source_count)
                    })
                }
            }

            impl<'source> Iterator for StatementExecutionIter<'source> {
                type Item = (usize, &'source StatementNode);
                fn next(&mut self) -> Option<Self::Item> {
                    self.order
                        .next()
                        .map(|index| (index, &self.statements[index]))
                }
            }

            pub struct StatementControlBlock {
                pub start: usize,
                pub count: usize,
                pub exit: StatementControlExit,
            }

            pub enum StatementControlExit {
                Continue(usize),
                SourceExit,
                End,
            }

            impl StatementControlGraph {
                pub fn finite_execution_order(
                    &self,
                    statements: &[StatementNode],
                ) -> Option<Vec<usize>> {
                    let mut visited = vec![false; self.blocks.len()];
                    let mut order = Vec::new();
                    let mut next = 0;
                    loop {
                        let block = self.blocks.get(next)?;
                        if visited[next] {
                            return None;
                        }
                        visited[next] = true;
                        let window = &statements[block.start..block.start + block.count];
                        let reachable = super::reachable_state_statements(window);
                        order.extend(block.start..block.start + reachable.len());
                        if reachable.len() != window.len()
                            || reachable.last().is_some_and(|node| {
                                node.as_transition().is_some_and(|transition| {
                                    matches!(transition.guard, super::TransitionGuardNode::Always)
                                })
                            })
                        {
                            return Some(order);
                        }
                        match block.exit {
                            StatementControlExit::Continue(destination) => next = destination,
                            StatementControlExit::SourceExit | StatementControlExit::End => {
                                return Some(order);
                            }
                        }
                    }
                }
            }

            impl StatementTable {
                pub fn admitted_execution_order(
                    &self,
                    statements: HandleSpan<StatementNode>,
                ) -> StatementExecutionOrder {
                    let indices = self
                        .finite_execution_order(statements)
                        .expect("typed statement control shape was admitted")
                        .expect("finite assembly control was admitted");
                    let mut positions = vec![0; statements.len()];
                    for (position, source) in indices.iter().copied().enumerate() {
                        positions[source] = position + 1;
                    }
                    StatementExecutionOrder {
                        source_count: statements.len(),
                        indices,
                        positions,
                    }
                }

                pub fn admitted_execution_statements(
                    &self,
                    statements: HandleSpan<StatementNode>,
                ) -> StatementExecutionIter<'_> {
                    let order = self
                        .finite_execution_order(statements)
                        .expect("typed statement control shape was admitted")
                        .expect("finite assembly control was admitted");
                    StatementExecutionIter {
                        statements: self.statements(statements),
                        order: order.into_iter(),
                    }
                }

                pub fn finite_execution_order(
                    &self,
                    statements: HandleSpan<StatementNode>,
                ) -> Result<Option<Vec<usize>>, String> {
                    let graph = self.control_graph(statements)?;
                    Ok(graph.finite_execution_order(self.statements(statements)))
                }

                pub fn control_graph(
                    &self,
                    statements: HandleSpan<StatementNode>,
                ) -> Result<StatementControlGraph, String> {
                    let source = self.statements(statements);
                    if source.len() != statements.len() {
                        return Err("lexical control frame has stale statement handles".into());
                    }
                    let mut blocks = Vec::new();
                    let mut start = 0;
                    let mut cursor = 0;
                    while cursor < source.len() {
                        let StatementNode::Assembly(point) = &source[cursor] else {
                            cursor += 1;
                            continue;
                        };
                        let AssemblyControlPoint::Entry(handle) = point else {
                            return Err(
                                "assembly label has no owning lexical control window".into()
                            );
                        };
                        let graph = self.assembly(*handle);
                        let assembly = self.assembly_blocks(graph.blocks);
                        if assembly.len() != graph.blocks.len() || assembly.is_empty() {
                            return Err("assembly control frame has stale or absent blocks".into());
                        }
                        let body_start = cursor + 1;
                        let body_end = body_start
                            .checked_add(graph.statement_count as usize)
                            .filter(|end| *end <= source.len())
                            .ok_or("assembly instruction window escapes its lexical frame")?;
                        let first_block = blocks.len() + 1;
                        blocks.push(StatementControlBlock {
                            start,
                            count: body_start - start,
                            exit: StatementControlExit::Continue(first_block),
                        });
                        let continuation = first_block + assembly.len();
                        let mut expected = 0;
                        for (index, block) in assembly.iter().enumerate() {
                            if block.start as usize != expected || block.count == 0 {
                                return Err(
                                    "assembly blocks do not partition their instruction window"
                                        .into(),
                                );
                            }
                            expected = expected
                                .checked_add(block.count as usize)
                                .filter(|end| *end <= graph.statement_count as usize)
                                .ok_or("assembly block escapes its instruction window")?;
                            let block_start = body_start + block.start as usize;
                            let expected_label = graph.blocks.handle_at(index as u32);
                            if !matches!(&source[block_start], StatementNode::Assembly(AssemblyControlPoint::Label(label)) if *label == expected_label)
                            {
                                return Err(
                                    "assembly block has no matching lexical label point".into()
                                );
                            }
                            if source[block_start + 1..body_start + expected]
                                .iter()
                                .any(|node| matches!(node, StatementNode::Assembly(_)))
                            {
                                return Err(
                                    "assembly block contains an independent control point".into()
                                );
                            }
                            let exit = match block.exit {
                    AssemblyExit::ZeroFlag(_, _) => return Err(
                        "conditional assembly control must be normalized before finite execution"
                            .into(),
                    ),
                    AssemblyExit::Fallthrough => {
                        StatementControlExit::Continue(if index + 1 == assembly.len() {
                            continuation
                        } else {
                            first_block + index + 1
                        })
                    }
                    AssemblyExit::Jump(target) => {
                        let index = graph
                            .blocks
                            .handles()
                            .position(|handle| handle == target)
                            .ok_or("assembly edge escapes its declaring graph")?;
                        StatementControlExit::Continue(first_block + index)
                    }
                    AssemblyExit::EnclosingTransition => StatementControlExit::SourceExit,
                };
                            blocks.push(StatementControlBlock {
                                start: block_start,
                                count: block.count as usize,
                                exit,
                            });
                        }
                        if expected != graph.statement_count as usize {
                            return Err(
                                "assembly control leaves unowned instruction statements".into()
                            );
                        }
                        cursor = body_end;
                        start = cursor;
                    }
                    blocks.push(StatementControlBlock {
                        start,
                        count: source.len() - start,
                        exit: StatementControlExit::End,
                    });
                    Ok(StatementControlGraph { blocks })
                }
            }

            #[cfg(test)]
            mod tests {
                use super::StatementExecutionOrder;
                use std::ops::Bound::{Excluded, Included, Unbounded};

                #[test]
                fn execution_prefix_endpoints_keep_source_anchors() {
                    let order = StatementExecutionOrder {
                        source_count: 6,
                        indices: vec![0, 4, 5, 1, 2],
                        positions: vec![1, 4, 5, 0, 2, 3],
                    };
                    assert_eq!(order.prefix_end(Excluded(2)), Some(4));
                    assert_eq!(order.prefix_end(Included(5)), Some(3));
                    assert_eq!(order.prefix_end(Included(2)), Some(5));
                    assert_eq!(order.prefix_end(Excluded(3)), None);
                    assert_eq!(order.prefix_end(Included(3)), None);
                    assert_eq!(order.prefix_end(Excluded(6)), Some(5));
                    assert_eq!(order.prefix_end(Included(6)), None);
                    assert_eq!(order.prefix_end(Unbounded), Some(5));
                }
            }
        }
        pub(crate) mod payloads {
            use crate::domain::ProofFact;
            use crate::expression::ExpressionHandle;
            use crate::statement::*;
            use crate::{TypedTreeTables, remapped};
            use arena::{Handle, HandleSpan};
            impl TypedTreeTables {
                pub fn copy_statement_nodes_deep_from(
                    &mut self,
                    source_tables: &crate::TypedTreeTables,
                    statements: HandleSpan<StatementNode>,
                ) -> HandleSpan<StatementNode> {
                    self.copy_statement_nodes_with_type_expressions(
                        source_tables,
                        statements,
                        &mut Vec::new(),
                    )
                }

                pub fn copy_statement_nodes_with_type_expressions(
                    &mut self,
                    source_tables: &crate::TypedTreeTables,
                    statements: HandleSpan<StatementNode>,
                    expressions: &mut Vec<crate::types::TypeExpressionCopy>,
                ) -> HandleSpan<StatementNode> {
                    let source = &source_tables.statement_table;
                    let mut copied = Vec::new();
                    let mut assembly_blocks = Vec::new();
                    let mut operand_captures = Vec::new();

                    for (source_handle, statement) in
                        statements.handles().zip(source.statements(statements))
                    {
                        let statement = match statement {
                            StatementNode::Assembly(AssemblyControlPoint::Entry(handle)) => {
                                let target =
                                    self.statement_table.copy_assembly_from(source, *handle);
                                let primitives: Vec<_> = source
                                    .expression_handles(source.assembly(*handle).primitives)
                                    .iter()
                                    .map(|expression| {
                                        self.expression_table
                                            .copy_from(&source_tables.expression_table, *expression)
                                    })
                                    .collect();
                                let primitives =
                                    self.statement_table.insert_expression_handles(primitives);
                                self.statement_table.assembly_mut(target).primitives = primitives;
                                let mut scratch = self.statement_table.assembly(target).scratch;
                                for expression in &mut scratch {
                                    if expression.is_valid() {
                                        *expression = self.expression_table.copy_from(
                                            &source_tables.expression_table,
                                            *expression,
                                        );
                                    }
                                }
                                self.statement_table.assembly_mut(target).scratch = scratch;
                                assembly_blocks.extend(
                                    source.assembly(*handle).blocks.handles().zip(
                                        self.statement_table.assembly(target).blocks.handles(),
                                    ),
                                );
                                StatementNode::Assembly(AssemblyControlPoint::Entry(target))
                            }
                            StatementNode::Assembly(AssemblyControlPoint::Label(handle)) => {
                                let target = assembly_blocks
                                    .iter()
                                    .find(|(source, _)| source == handle)
                                    .map(|(_, target)| *target)
                                    .expect("assembly window copying includes its owning header");
                                StatementNode::Assembly(AssemblyControlPoint::Label(target))
                            }
                            StatementNode::AssemblyFact(fact) => {
                                StatementNode::AssemblyFact(TableAssemblyFact {
                                    kind: fact.kind,
                                    fact: self.copy_proof_fact_from(source_tables, fact.fact),
                                })
                            }
                            StatementNode::Assignment(assignment) => {
                                StatementNode::Assignment(TableAssignment {
                                    target: self.expression_table.copy_from(
                                        &source_tables.expression_table,
                                        assignment.target,
                                    ),
                                    value: self.expression_table.copy_from(
                                        &source_tables.expression_table,
                                        assignment.value,
                                    ),
                                })
                            }
                            StatementNode::Call(call) => {
                                let receiver = self.statement_table.name_path_members.insert_many(
                                    source.name_path_members(call.receiver).iter().cloned(),
                                );
                                let (arguments, captures, authored_machine_application) =
                                    self.expression_table.copy_call_operands_from(
                                        &source_tables.expression_table,
                                        source.expression_handles(call.arguments),
                                        call.authored_machine_application,
                                    );
                                let arguments = self.statement_table.insert_expression_handles(
                                    self.expression_table.expression_handles(arguments).to_vec(),
                                );
                                operand_captures.push((copied.len(), source_handle, captures));
                                let reflected_field_origin =
                                    call.reflected_field_origin.as_ref().map(|origin| {
                                        let query = self
                                            .copy_statement_nodes_with_type_expressions(
                                                source_tables,
                                                HandleSpan::from_parts(origin.query, 1),
                                                expressions,
                                            )
                                            .start();
                                        let callback =
                                            self.expression_table.copy_static_argument_from(
                                                &source_tables.expression_table,
                                                &origin.callback,
                                            );
                                        ReflectedFieldCallOrigin { query, callback }
                                    });
                                let mut installed_interrupt_operation =
                                    call.installed_interrupt_operation.clone();
                                if let Some(request) = &mut installed_interrupt_operation {
                                    request.invalidate_preparation();
                                    request.registrar =
                                        self.expression_table.copy_static_argument_from(
                                            &source_tables.expression_table,
                                            &request.registrar,
                                        );
                                    request.timer_slot =
                                        self.expression_table.copy_static_argument_from(
                                            &source_tables.expression_table,
                                            &request.timer_slot,
                                        );
                                }
                                let runtime_call = call.runtime_call.as_ref().map(|binding| {
                                    self.copy_runtime_callable_binding_from(
                                        source_tables,
                                        binding,
                                        expressions,
                                    )
                                });
                                StatementNode::Call(TableCall {
                                    runtime_call,
                                    installed_interrupt_operation,
                                    authored_machine_application,
                                    static_machine_parameter: call.static_machine_parameter,
                                    receiver_root_symbol: call.receiver_root_symbol,
                                    receiver_symbol: call.receiver_symbol,
                                    target_symbol: call.target_symbol,
                                    receiver,
                                    target: call.target.clone(),
                                    static_requirement_dispatch: call
                                        .static_requirement_dispatch
                                        .clone(),
                                    machine_arguments: call
                                        .machine_arguments
                                        .iter()
                                        .map(|argument| {
                                            self.expression_table.copy_static_argument_from(
                                                &source_tables.expression_table,
                                                argument,
                                            )
                                        })
                                        .collect(),
                                    arguments,
                                    evidence_arguments: call.evidence_arguments.clone(),
                                    operational_acknowledgement: call.operational_acknowledgement,
                                    discards_result: call.discards_result,
                                    source_span: call.source_span,
                                    authored_call_selection: call.authored_call_selection,
                                    reflected_field_origin,
                                })
                            }
                            StatementNode::Expression(expression) => StatementNode::Expression(
                                self.expression_table
                                    .copy_from(&source_tables.expression_table, *expression),
                            ),
                            StatementNode::LocalData(local) => {
                                let type_reference = local.type_reference.is_valid().then(|| {
                                    self.copy_type_reference_with_expressions(
                                        source_tables,
                                        local.type_reference,
                                        expressions,
                                    )
                                });
                                let initial_value = local.initial_value.is_valid().then(|| {
                                    self.expression_table.copy_from(
                                        &source_tables.expression_table,
                                        local.initial_value,
                                    )
                                });
                                let reflected_query_origin =
                                    local.reflected_query_origin.map(|query| {
                                        self.copy_statement_nodes_with_type_expressions(
                                            source_tables,
                                            HandleSpan::from_parts(query, 1),
                                            expressions,
                                        )
                                        .start()
                                    });
                                StatementNode::LocalData(TableLocalData {
                                    role: local.role,
                                    symbol: local.symbol,
                                    name: local.name.clone(),
                                    type_reference: type_reference
                                        .unwrap_or_else(crate::types::TypeReferenceHandle::invalid),
                                    initial_value: initial_value
                                        .unwrap_or_else(ExpressionHandle::invalid),
                                    is_mutable: local.is_mutable,
                                    type_is_inferred: local.type_is_inferred,
                                    is_operand_capture: local.is_operand_capture,
                                    relevance: local.relevance,
                                    reflected_query_origin,
                                    pattern_fields: self.expression_table.copy_pattern_fields_from(
                                        &source_tables.expression_table,
                                        local.pattern_fields,
                                    ),
                                    pattern_subject: local.pattern_subject,
                                })
                            }
                            StatementNode::Transition(transition) => {
                                let target = self.statement_table.copy_transition_target_deep_from(
                                    source,
                                    &source_tables.expression_table,
                                    &mut self.expression_table,
                                    transition.target,
                                );
                                let continuation =
                                    self.statement_table.copy_transition_target_deep_from(
                                        source,
                                        &source_tables.expression_table,
                                        &mut self.expression_table,
                                        transition.continuation,
                                    );
                                let guard = match transition.guard {
                                    TransitionGuardNode::Always => TransitionGuardNode::Always,
                                    TransitionGuardNode::When(guard) => TransitionGuardNode::When(
                                        self.expression_table
                                            .copy_from(&source_tables.expression_table, guard),
                                    ),
                                };
                                let patterns = source
                                    .transition_patterns(transition.patterns)
                                    .iter()
                                    .map(|pattern| {
                                        self.expression_table.copy_transition_pattern_from(
                                            &source_tables.expression_table,
                                            *pattern,
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                let patterns =
                                    self.statement_table.insert_transition_patterns(patterns);
                                StatementNode::Transition(TableTransition {
                                    patterns,
                                    target,
                                    continuation,
                                    guard,
                                    proof_selectors: self
                                        .statement_table
                                        .outcome_proof_selectors
                                        .insert_many(
                                            source
                                                .outcome_proof_selectors(transition.proof_selectors)
                                                .iter()
                                                .cloned(),
                                        ),
                                    exit: transition.exit,
                                    source_span: transition.source_span,
                                })
                            }
                        };
                        copied.push(statement);
                    }

                    let copied = self.statement_table.insert_statements(copied);
                    let handles = copied.handles().collect::<Vec<_>>();
                    for (position, source, captures) in operand_captures {
                        self.expression_table.rebind_statement_operand_captures(
                            &captures,
                            source,
                            handles[position],
                        );
                    }
                    copied
                }
                pub fn remap_statement_symbols<'scope>(
                    &mut self,
                    statements: HandleSpan<StatementNode>,
                    symbols: impl Into<crate::SymbolRebinding<'scope>>,
                ) {
                    let symbols = symbols.into();
                    let statement_handles: Vec<_> = statements.handles().collect();
                    for handle in statement_handles {
                        let statement = self.statement_table.statement(handle).clone();
                        match statement {
                            StatementNode::Assembly(AssemblyControlPoint::Entry(graph)) => {
                                let primitives = self
                                    .statement_table
                                    .expression_handles(
                                        self.statement_table.assembly(graph).primitives,
                                    )
                                    .to_vec();
                                for expression in primitives {
                                    self.expression_table.remap_symbols_in(expression, symbols);
                                }
                                for expression in self.statement_table.assembly(graph).scratch {
                                    if expression.is_valid() {
                                        self.expression_table.remap_symbols_in(expression, symbols);
                                    }
                                }
                            }
                            StatementNode::Assembly(AssemblyControlPoint::Label(_)) => {}
                            StatementNode::AssemblyFact(fact) => {
                                self.remap_proof_fact_symbols(fact.fact, symbols);
                            }
                            StatementNode::Assignment(assignment) => {
                                self.expression_table
                                    .remap_symbols_in(assignment.target, symbols);
                                self.expression_table
                                    .remap_symbols_in(assignment.value, symbols);
                            }
                            StatementNode::Call(call) => {
                                if let Some(binding) = &call.runtime_call {
                                    self.remap_runtime_callable_binding_symbols_in(
                                        binding, symbols,
                                    );
                                }
                                let reflected_field_origin =
                                    call.reflected_field_origin.map(|mut origin| {
                                        self.remap_statement_symbols(
                                            HandleSpan::from_parts(origin.query, 1),
                                            symbols,
                                        );
                                        self.expression_table.remap_static_argument_symbols(
                                            &mut origin.callback,
                                            symbols.authored(),
                                        );
                                        origin
                                    });
                                self.expression_table.remap_static_application(
                                    call.authored_machine_application,
                                    symbols,
                                );
                                for argument in self
                                    .statement_table
                                    .expression_handles(call.arguments)
                                    .to_vec()
                                {
                                    self.expression_table.remap_symbols_in(argument, symbols);
                                }
                                let StatementNode::Call(current) =
                                    self.statement_table.statements.get_mut(handle)
                                else {
                                    unreachable!();
                                };
                                current.receiver_symbol =
                                    remapped(current.receiver_symbol, symbols);
                                current.receiver_root_symbol =
                                    remapped(current.receiver_root_symbol, symbols);
                                current.target_symbol = remapped(current.target_symbol, symbols);
                                let mut request = current.installed_interrupt_operation.take();
                                if let Some(request) = &mut request {
                                    request.invalidate_preparation();
                                    self.expression_table.remap_static_argument_symbols(
                                        &mut request.registrar,
                                        symbols.execution(),
                                    );
                                    self.expression_table.remap_static_argument_symbols(
                                        &mut request.timer_slot,
                                        symbols.execution(),
                                    );
                                }
                                let mut arguments = std::mem::take(&mut current.machine_arguments);
                                for argument in &mut arguments {
                                    self.expression_table.remap_static_argument_symbols(
                                        argument,
                                        symbols.execution(),
                                    );
                                }
                                let StatementNode::Call(current) =
                                    self.statement_table.statements.get_mut(handle)
                                else {
                                    unreachable!();
                                };
                                current.machine_arguments = arguments;
                                current.installed_interrupt_operation = request;
                                current.reflected_field_origin = reflected_field_origin;
                            }
                            StatementNode::Expression(expression) => {
                                self.expression_table.remap_symbols_in(expression, symbols);
                            }
                            StatementNode::LocalData(local) => {
                                if let Some(query) = local.reflected_query_origin {
                                    self.remap_statement_symbols(
                                        HandleSpan::from_parts(query, 1),
                                        symbols,
                                    );
                                }
                                self.remap_type_reference_symbols_in(local.type_reference, symbols);
                                self.expression_table
                                    .remap_symbols_in(local.initial_value, symbols);
                                // A resolved record pattern names its record
                                // and fields by symbol, as an arm's pattern does.
                                for row in local.pattern_fields.handles() {
                                    let row = self.expression_table.match_field_mut(row);
                                    if row.field_symbol.is_valid() {
                                        row.field_symbol = remapped(row.field_symbol, symbols);
                                    }
                                }
                                let StatementNode::LocalData(current) =
                                    self.statement_table.statements.get_mut(handle)
                                else {
                                    unreachable!();
                                };
                                current.symbol = remapped(current.symbol, symbols);
                                if current.pattern_subject.is_valid() {
                                    current.pattern_subject =
                                        remapped(current.pattern_subject, symbols);
                                }
                            }
                            StatementNode::Transition(transition) => {
                                for retained in self
                                    .statement_table
                                    .transition_patterns(transition.patterns)
                                    .to_vec()
                                {
                                    self.expression_table
                                        .remap_symbols_in(retained.subject, symbols);
                                    for expression in [
                                        retained.pattern.head,
                                        retained.pattern.guard,
                                        retained.pattern.condition,
                                    ] {
                                        self.expression_table.remap_symbols_in(expression, symbols);
                                    }
                                    for handle in retained.pattern.fields.handles() {
                                        let field = self
                                            .expression_table
                                            .match_fields(retained.pattern.fields)
                                            [(handle.arena_index()
                                                - retained.pattern.fields.start().arena_index())
                                                as usize]
                                            .clone();
                                        if let crate::expression::MatchFieldAction::Equal(value) =
                                            field.action
                                        {
                                            self.expression_table.remap_symbols_in(value, symbols);
                                        }
                                        self.expression_table
                                            .match_field_mut(handle)
                                            .field_symbol = remapped(field.field_symbol, symbols);
                                    }
                                }

                                self.statement_table.remap_transition_target_symbols(
                                    transition.target,
                                    &mut self.expression_table,
                                    symbols,
                                );
                                self.statement_table.remap_transition_target_symbols(
                                    transition.continuation,
                                    &mut self.expression_table,
                                    symbols,
                                );
                                if let TransitionGuardNode::When(guard) = transition.guard {
                                    self.expression_table.remap_symbols_in(guard, symbols);
                                }
                            }
                        }
                    }
                }
                pub fn expression_roots(
                    &self,
                    statement: &StatementNode,
                ) -> Vec<(StatementRootRole, ExpressionHandle)> {
                    use StatementRootRole as Role;
                    let mut roots = Vec::new();
                    match statement {
                        StatementNode::Assembly(_) => {}
                        StatementNode::AssemblyFact(fact) => roots.extend(
                            self.proof_fact_expression_roots(fact.fact)
                                .into_iter()
                                .map(|expression| (Role::AssemblyFact, expression)),
                        ),
                        StatementNode::Assignment(assignment) => {
                            roots.push((Role::AssignmentTarget, assignment.target));
                            roots.push((Role::AssignmentValue, assignment.value));
                        }
                        StatementNode::Call(call) => {
                            if let Some(binding) = call.runtime_call {
                                roots.push((Role::CallCallee, binding.callee));
                            }
                            roots.extend(
                                self.statement_table
                                    .expression_handles(call.arguments)
                                    .iter()
                                    .map(|argument| (Role::CallArgument, *argument)),
                            );
                        }
                        StatementNode::Expression(expression) => {
                            roots.push((Role::Expression, *expression))
                        }
                        StatementNode::LocalData(local) => {
                            if local.role != language_core::LocalBindingRole::AtomicObservedResult {
                                roots.push((Role::LocalInitializer, local.initial_value));
                            }
                        }
                        StatementNode::Transition(transition) => {
                            if let TransitionGuardNode::When(guard) = transition.guard {
                                roots.push((Role::TransitionGuard, guard));
                            }
                            for target in [transition.target, transition.continuation] {
                                if !self.statement_table.transition_target_is_valid(target) {
                                    continue;
                                }
                                match self.statement_table.transition_target(target) {
                                    TransitionTargetNode::Named { arguments, .. } => roots.extend(
                                        self.statement_table
                                            .expression_handles(*arguments)
                                            .iter()
                                            .map(|argument| (Role::TransitionArgument, *argument)),
                                    ),
                                    TransitionTargetNode::Value(value) => {
                                        roots.push((Role::TransitionValue, *value));
                                    }
                                    TransitionTargetNode::Terminal => {}
                                }
                            }
                        }
                    }
                    roots
                }
                pub fn proof_fact_expression_roots(
                    &self,
                    handle: Handle<ProofFact>,
                ) -> Vec<ExpressionHandle> {
                    match self.proof_facts.get(handle) {
                        ProofFact::Expression(expression) => vec![*expression],
                        ProofFact::Membership(membership) => vec![membership.value],
                        ProofFact::Proposition(application) => self
                            .expression_table
                            .expression_handles(application.arguments)
                            .to_vec(),
                    }
                }
                fn copy_proof_fact_from(
                    &mut self,
                    source: &TypedTreeTables,
                    handle: Handle<ProofFact>,
                ) -> Handle<ProofFact> {
                    let mut fact = source.proof_facts.get(handle).clone();
                    match &mut fact {
                        ProofFact::Expression(expression) => {
                            *expression = self
                                .expression_table
                                .copy_from(&source.expression_table, *expression)
                        }
                        ProofFact::Membership(membership) => {
                            membership.value = self
                                .expression_table
                                .copy_from(&source.expression_table, membership.value);
                            membership.domain = self.domain_path_members.insert_many(
                                source
                                    .domain_path_members
                                    .span_or_empty(membership.domain)
                                    .iter()
                                    .cloned(),
                            );
                            let arguments = source
                                .type_reference_table
                                .type_reference_handles(membership.domain_arguments)
                                .iter()
                                .map(|argument| self.copy_type_reference_from(source, *argument))
                                .collect::<Vec<_>>();
                            membership.domain_arguments = self
                                .type_reference_table
                                .insert_type_reference_handles(arguments);
                        }
                        ProofFact::Proposition(application) => {
                            let arguments = source
                                .expression_table
                                .expression_handles(application.arguments)
                                .iter()
                                .map(|argument| {
                                    self.expression_table
                                        .copy_from(&source.expression_table, *argument)
                                })
                                .collect::<Vec<_>>();
                            application.arguments =
                                self.expression_table.insert_expression_handles(arguments);
                        }
                    }
                    let copied = self.proof_facts.insert(fact);
                    let source_index = crate::proof_fact_source_span_index(handle);
                    if let Some(Some(span)) = source.proof_fact_source_spans.get(source_index) {
                        let index = crate::proof_fact_source_span_index(copied);
                        self.proof_fact_source_spans
                            .resize(self.proof_fact_source_spans.len().max(index + 1), None);
                        self.proof_fact_source_spans[index] = Some(*span);
                    }
                    copied
                }
                fn remap_proof_fact_symbols(
                    &mut self,
                    handle: Handle<ProofFact>,
                    symbols: crate::SymbolRebinding<'_>,
                ) {
                    for expression in self.proof_fact_expression_roots(handle) {
                        self.expression_table.remap_symbols_in(expression, symbols);
                    }
                    match self.proof_facts.get_mut(handle) {
                        ProofFact::Expression(_) => {}
                        ProofFact::Membership(membership) => {
                            membership.domain_symbol = remapped(membership.domain_symbol, symbols);
                            for argument in self
                                .type_reference_table
                                .type_reference_handles(membership.domain_arguments)
                                .to_vec()
                            {
                                self.remap_type_reference_symbols_in(argument, symbols);
                            }
                        }
                        ProofFact::Proposition(application) => {
                            application.proposition = remapped(application.proposition, symbols);
                            for argument in &mut application.binder_arguments {
                                argument.symbol = remapped(argument.symbol, symbols.authored());
                            }
                        }
                    }
                }
            }
        }
        pub(crate) mod returns {
            use super::{
                StatementNode, StatementTable, TransitionExit, TransitionGuardNode,
                TransitionTargetNode,
            };
            use crate::expression::ExpressionHandle;

            pub fn direct_state_return_values(
                table: &StatementTable,
                statements: &[StatementNode],
            ) -> Option<Vec<ExpressionHandle>> {
                let mut values = Vec::new();
                for (position, statement) in statements.iter().enumerate() {
                    match statement {
                        StatementNode::Transition(transition) => {
                            if transition.exit != TransitionExit::Ordinary
                                || !transition.target.is_valid()
                            {
                                return None;
                            }
                            for target in transition.targets() {
                                if !table.transition_target_is_valid(target) {
                                    return None;
                                }
                                let TransitionTargetNode::Value(value) =
                                    table.transition_target(target)
                                else {
                                    return None;
                                };
                                values.push(*value);
                            }
                            if transition.guard == TransitionGuardNode::Always
                                || transition.continuation.is_valid()
                            {
                                return Some(values);
                            }
                        }
                        StatementNode::Expression(value) if position + 1 == statements.len() => {
                            values.push(*value);
                            return Some(values);
                        }
                        StatementNode::Assembly(_) => return None,
                        _ => {}
                    }
                }
                None
            }
        }
        use crate::expression::ExpressionHandle;
        pub use assembly::{
            AssemblyBlock, AssemblyControlFlow, AssemblyControlPoint, AssemblyExit,
        };
        pub use control_graph::{
            StatementControlBlock, StatementControlExit, StatementControlGraph,
            StatementExecutionIter, StatementExecutionOrder,
        };
        pub use returns::direct_state_return_values;

        pub type StatementHandle = Handle<StatementNode>;

        pub fn reachable_transition_prefix(statements: &[StatementNode]) -> &[StatementNode] {
            match statements.iter().position(|statement| {
                matches!(statement, StatementNode::Transition(transition)
            if transition.guard == TransitionGuardNode::Always)
            }) {
                Some(ordinal) => &statements[..=ordinal],
                None => statements,
            }
        }

        pub fn reachable_state_statements(statements: &[StatementNode]) -> &[StatementNode] {
            let Some(start) = statements
                .iter()
                .position(|statement| matches!(statement, StatementNode::Transition(_)))
            else {
                return statements;
            };
            let tail = reachable_transition_prefix(&statements[start..]);
            &statements[..start + tail.len()]
        }

        pub type TransitionTargetHandle = Handle<TransitionTargetNode>;

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct StatementTable {
            assembly_graphs: Arena<AssemblyControlFlow>,
            assembly_blocks: Arena<AssemblyBlock>,
            statements: Arena<StatementNode>,
            expression_handles: Arena<ExpressionHandle>,
            name_path_members: Arena<Identifier>,
            transition_targets: Arena<TransitionTargetNode>,
            transition_patterns: Arena<TransitionPattern>,
            outcome_proof_selectors: Arena<OutcomeProofSelector>,
        }

        impl StatementTable {
            pub fn insert_transition_patterns(
                &mut self,
                patterns: impl IntoIterator<Item = TransitionPattern>,
            ) -> HandleSpan<TransitionPattern> {
                self.transition_patterns.insert_many(patterns)
            }

            pub fn transition_patterns(
                &self,
                patterns: HandleSpan<TransitionPattern>,
            ) -> &[TransitionPattern] {
                self.transition_patterns.span_or_empty(patterns)
            }

            pub fn new() -> Self {
                Self {
                    assembly_graphs: Arena::new(),
                    assembly_blocks: Arena::new(),
                    statements: Arena::new(),
                    expression_handles: Arena::new(),
                    name_path_members: Arena::new(),
                    transition_targets: Arena::new(),
                    transition_patterns: Arena::new(),
                    outcome_proof_selectors: Arena::new(),
                }
            }

            pub fn insert(&mut self, statement: StatementNode) -> StatementHandle {
                self.statements.append(statement)
            }

            pub fn insert_statements(
                &mut self,
                statements: impl IntoIterator<Item = StatementNode>,
            ) -> HandleSpan<StatementNode> {
                self.statements.insert_many(statements)
            }

            pub fn push_statement(
                &mut self,
                span: &mut HandleSpan<StatementNode>,
                statement: StatementNode,
            ) -> StatementHandle {
                self.statements.append_to_span(span, statement)
            }

            crate::span_columns! {
                expression_handles(ExpressionHandle)
                    reserve reserve_expression_handles("expression handle span count overflow")
                    set set_expression_handle_at_offset
                    push push_expression_handle;
            }

            pub fn insert_expression_handles(
                &mut self,
                expressions: impl IntoIterator<Item = ExpressionHandle>,
            ) -> HandleSpan<ExpressionHandle> {
                self.expression_handles.insert_many(expressions)
            }

            pub fn push_name_path_member(
                &mut self,
                span: &mut HandleSpan<Identifier>,
                member: Identifier,
            ) {
                self.name_path_members.append_to_span(span, member);
            }

            pub fn insert_transition_target(
                &mut self,
                target: TransitionTargetNode,
            ) -> TransitionTargetHandle {
                self.transition_targets.insert(target)
            }

            pub fn insert_outcome_proof_selectors(
                &mut self,
                selectors: impl IntoIterator<Item = OutcomeProofSelector>,
            ) -> HandleSpan<OutcomeProofSelector> {
                self.outcome_proof_selectors.insert_many(selectors)
            }

            pub fn outcome_proof_selectors(
                &self,
                span: HandleSpan<OutcomeProofSelector>,
            ) -> &[OutcomeProofSelector] {
                self.outcome_proof_selectors.span_or_empty(span)
            }

            pub(crate) fn copy_transition_target_deep_from(
                &mut self,
                source: &StatementTable,
                source_expressions: &crate::expression::ExpressionTable,
                target_expressions: &mut crate::expression::ExpressionTable,
                target: TransitionTargetHandle,
            ) -> TransitionTargetHandle {
                if !target.is_valid() {
                    return TransitionTargetHandle::invalid();
                }
                let target = match source.transition_target(target) {
                    TransitionTargetNode::Named {
                        path,
                        static_machine_parameter,
                        arguments,
                        evidence_arguments,
                        source_span,
                        authored_call_selection,
                        operational_acknowledgement,
                    } => {
                        let members = self
                            .name_path_members
                            .insert_many(source.name_path_members(path.members).iter().cloned());
                        let arguments = self.expression_handles.insert_many(
                            source
                                .expression_handles(*arguments)
                                .iter()
                                .map(|argument| {
                                    target_expressions.copy_from(source_expressions, *argument)
                                }),
                        );
                        TransitionTargetNode::Named {
                            static_machine_parameter: *static_machine_parameter,
                            path: TableNamePath {
                                members,
                                receiver_symbol: path.receiver_symbol,
                                head_symbol: path.head_symbol,
                                symbol: path.symbol,
                            },
                            arguments,
                            evidence_arguments: evidence_arguments.clone(),
                            source_span: *source_span,
                            authored_call_selection: *authored_call_selection,
                            operational_acknowledgement: *operational_acknowledgement,
                        }
                    }
                    TransitionTargetNode::Value(value) => TransitionTargetNode::Value(
                        target_expressions.copy_from(source_expressions, *value),
                    ),
                    TransitionTargetNode::Terminal => TransitionTargetNode::Terminal,
                };
                self.insert_transition_target(target)
            }

            pub(crate) fn remap_transition_target_symbols<'scope>(
                &mut self,
                target: TransitionTargetHandle,
                expressions: &mut crate::expression::ExpressionTable,
                symbols: impl Into<crate::SymbolRebinding<'scope>>,
            ) {
                let symbols = symbols.into();
                if !target.is_valid() {
                    return;
                }
                let snapshot = self.transition_target(target).clone();
                match snapshot {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => {
                        for argument in self.expression_handles(arguments).to_vec() {
                            expressions.remap_symbols_in(argument, symbols);
                        }
                        let TransitionTargetNode::Named { path: current, .. } =
                            self.transition_targets.get_mut(target)
                        else {
                            unreachable!();
                        };
                        current.receiver_symbol = remapped(path.receiver_symbol, symbols);
                        current.head_symbol = remapped(path.head_symbol, symbols);
                        current.symbol = remapped(path.symbol, symbols);
                    }
                    TransitionTargetNode::Value(value) => {
                        expressions.remap_symbols_in(value, symbols);
                    }
                    TransitionTargetNode::Terminal => {}
                }
            }

            pub fn statement_is_valid(&self, handle: StatementHandle) -> bool {
                self.statements.is_valid(handle)
            }

            pub fn statement(&self, handle: StatementHandle) -> &StatementNode {
                self.statements.get(handle)
            }

            pub fn statement_mut(&mut self, handle: StatementHandle) -> &mut StatementNode {
                self.statements.get_mut(handle)
            }

            #[inline]
            pub fn statements(&self, span: HandleSpan<StatementNode>) -> &[StatementNode] {
                self.statements.span_or_empty(span)
            }

            pub fn indexed_statements(
                &self,
                span: HandleSpan<StatementNode>,
            ) -> impl Iterator<Item = (usize, &StatementNode)> {
                self.statements(span).iter().enumerate()
            }

            pub fn statement_at(
                &self,
                state: &crate::state::State,
                index: usize,
            ) -> Option<&StatementNode> {
                self.statements(state.statement_nodes).get(index)
            }

            pub fn iter_statements(
                &self,
                span: HandleSpan<StatementNode>,
            ) -> impl Iterator<Item = (StatementHandle, &StatementNode)> {
                span.handles()
                    .map(|handle| (handle, self.statements.get(handle)))
            }

            pub fn statements_mut(
                &mut self,
                span: HandleSpan<StatementNode>,
            ) -> &mut [StatementNode] {
                self.statements.span_mut_or_empty(span)
            }

            pub fn expression_handles(
                &self,
                span: HandleSpan<ExpressionHandle>,
            ) -> &[ExpressionHandle] {
                self.expression_handles.span_or_empty(span)
            }

            pub fn name_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
                self.name_path_members.span_or_empty(span)
            }

            pub fn transition_target(
                &self,
                handle: TransitionTargetHandle,
            ) -> &TransitionTargetNode {
                self.transition_targets.get(handle)
            }

            pub fn named_transition_target(
                &self,
                handle: TransitionTargetHandle,
            ) -> Option<(&TableNamePath, HandleSpan<ExpressionHandle>)> {
                match self.transition_target(handle) {
                    TransitionTargetNode::Named {
                        path, arguments, ..
                    } => Some((path, *arguments)),
                    _ => None,
                }
            }

            pub fn transition_target_mut(
                &mut self,
                handle: TransitionTargetHandle,
            ) -> &mut TransitionTargetNode {
                self.transition_targets.get_mut(handle)
            }

            pub fn transition_target_is_valid(&self, handle: TransitionTargetHandle) -> bool {
                self.transition_targets.is_valid(handle)
            }

            pub fn statement_count(&self) -> usize {
                self.statements.len()
            }

            pub fn revision(&self) -> u64 {
                self.statements
                    .revision()
                    .wrapping_add(self.expression_handles.revision())
                    .wrapping_add(self.name_path_members.revision())
                    .wrapping_add(self.transition_targets.revision())
                    .wrapping_add(self.transition_patterns.revision())
                    .wrapping_add(self.outcome_proof_selectors.revision())
            }

            pub fn transition_targets(
                &self,
                span: HandleSpan<StatementNode>,
            ) -> impl Iterator<Item = TransitionTargetHandle> + '_ {
                self.statements(span)
                    .iter()
                    .filter_map(StatementNode::as_transition)
                    .flat_map(TableTransition::targets)
            }

            pub fn transition_target_count(&self) -> usize {
                self.transition_targets.len()
            }
        }

        impl Default for StatementTable {
            fn default() -> Self {
                Self::new()
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum StatementRootRole {
            CallCallee,
            AssemblyFact,
            AssignmentTarget,
            AssignmentValue,
            CallArgument,
            Expression,
            LocalInitializer,
            TransitionGuard,
            TransitionArgument,
            TransitionValue,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub enum StatementNode {
            Assembly(AssemblyControlPoint),
            AssemblyFact(TableAssemblyFact),
            Assignment(TableAssignment),
            Call(TableCall),
            Expression(ExpressionHandle),
            LocalData(TableLocalData),
            Transition(TableTransition),
        }

        impl StatementNode {
            pub fn as_assembly_fact(&self) -> Option<&TableAssemblyFact> {
                let StatementNode::AssemblyFact(fact) = self else {
                    return None;
                };
                Some(fact)
            }

            pub fn as_assignment(&self) -> Option<&TableAssignment> {
                let StatementNode::Assignment(assignment) = self else {
                    return None;
                };
                Some(assignment)
            }

            pub fn as_call(&self) -> Option<&TableCall> {
                let StatementNode::Call(call) = self else {
                    return None;
                };
                Some(call)
            }

            pub fn as_expression(&self) -> Option<&ExpressionHandle> {
                let StatementNode::Expression(expression) = self else {
                    return None;
                };
                Some(expression)
            }

            pub fn as_local_data(&self) -> Option<&TableLocalData> {
                let StatementNode::LocalData(local) = self else {
                    return None;
                };
                Some(local)
            }

            pub fn as_transition(&self) -> Option<&TableTransition> {
                let StatementNode::Transition(transition) = self else {
                    return None;
                };
                Some(transition)
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct TableAssemblyFact {
            pub kind: AssemblyFactKind,
            pub fact: Handle<crate::domain::ProofFact>,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum AssemblyFactKind {
            Requires,
            Ensures,
        }

        impl Default for StatementNode {
            fn default() -> Self {
                Self::Expression(ExpressionHandle::invalid())
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TableAssignment {
            pub target: ExpressionHandle,
            pub value: ExpressionHandle,
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TableCall {
            pub runtime_call: Option<crate::callable_values::RuntimeCallBinding>,
            pub installed_interrupt_operation:
                Option<crate::expression::InstalledInterruptOperationRequest>,
            pub authored_machine_application:
                crate::static_applications::StaticMachineApplicationHandle,
            pub receiver_root_symbol: SymbolHandle,
            pub receiver_symbol: SymbolHandle,
            pub target_symbol: SymbolHandle,
            pub static_machine_parameter: SymbolHandle,
            pub receiver: HandleSpan<Identifier>,
            pub target: Identifier,
            pub static_requirement_dispatch: Option<crate::StaticRequirementDispatch>,
            pub machine_arguments: Box<[crate::expression::StaticMachineArgument]>,
            pub arguments: HandleSpan<ExpressionHandle>,
            pub evidence_arguments: Box<[Identifier]>,
            pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
            pub discards_result: bool,
            pub source_span: SourceSpan,
            pub authored_call_selection: Option<
                language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
            >,
            pub reflected_field_origin: Option<ReflectedFieldCallOrigin>,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct ReflectedFieldCallOrigin {
            pub query: StatementHandle,
            pub callback: crate::expression::StaticMachineArgument,
        }

        #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
        pub struct TableLocalData {
            pub role: language_core::LocalBindingRole,
            pub symbol: SymbolHandle,
            pub name: Identifier,
            pub type_reference: crate::types::TypeReferenceHandle,
            pub initial_value: ExpressionHandle,
            pub is_mutable: bool,
            pub type_is_inferred: bool,
            pub is_operand_capture: bool,
            pub relevance: language_core::BindingRelevance,
            pub reflected_query_origin: Option<StatementHandle>,
            /// A record pattern's roster (`let { a, b as _ } = subject;`):
            /// one row per field the author named, in authored order, bound
            /// or waived. Typed preparation gives each row the symbol of the
            /// subject's field it names; a row that names none stays without
            /// one. Empty unless `role` is `RecordPattern`.
            pub pattern_fields: HandleSpan<crate::expression::TableMatchField>,
            /// The data declaration the roster was resolved against: the
            /// subject's declared record type. Invalid until typed
            /// preparation resolves the subject, and when it cannot.
            pub pattern_subject: SymbolHandle,
        }

        impl TableLocalData {
            pub fn ordinary_value_initializer(&self) -> Option<ExpressionHandle> {
                (self.role.has_ordinary_initializer() && self.initial_value.is_valid())
                    .then_some(self.initial_value)
            }
        }

        impl Default for TableLocalData {
            fn default() -> Self {
                Self {
                    role: language_core::LocalBindingRole::Value,
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::default(),
                    type_reference: crate::types::TypeReferenceHandle::invalid(),
                    initial_value: ExpressionHandle::invalid(),
                    is_mutable: false,
                    type_is_inferred: false,
                    is_operand_capture: false,
                    relevance: language_core::BindingRelevance::Relevant,
                    reflected_query_origin: None,
                    pattern_fields: HandleSpan::empty(),
                    pattern_subject: SymbolHandle::invalid(),
                }
            }
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TransitionPattern {
            pub is_structural: bool,
            pub subject: ExpressionHandle,
            pub pattern: crate::expression::TableDestructurePattern,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub struct TableTransition {
            pub patterns: HandleSpan<TransitionPattern>,
            pub target: TransitionTargetHandle,
            pub continuation: TransitionTargetHandle,
            pub guard: TransitionGuardNode,
            pub proof_selectors: HandleSpan<OutcomeProofSelector>,
            pub exit: TransitionExit,
            pub source_span: SourceSpan,
        }

        impl TableTransition {
            pub fn targets(&self) -> impl Iterator<Item = TransitionTargetHandle> + use<> {
                [self.target, self.continuation]
                    .into_iter()
                    .filter(|handle| handle.is_valid())
            }
        }

        impl Default for TableTransition {
            fn default() -> Self {
                Self {
                    patterns: HandleSpan::empty(),
                    target: TransitionTargetHandle::invalid(),
                    continuation: TransitionTargetHandle::invalid(),
                    guard: TransitionGuardNode::Always,
                    proof_selectors: HandleSpan::empty(),
                    exit: TransitionExit::Ordinary,
                    source_span: Default::default(),
                }
            }
        }

        #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct OutcomeProofSelector {
            pub output_field: Identifier,
            pub binding: Identifier,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub enum TransitionExit {
            #[default]
            Ordinary,
            Crash(crate::signature::CrashCause),
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
        pub enum TransitionGuardNode {
            Always,
            When(ExpressionHandle),
        }

        #[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
        pub enum TransitionTargetNode {
            Named {
        operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
        path: TableNamePath,
        static_machine_parameter: SymbolHandle,
        arguments: HandleSpan<ExpressionHandle>,
        evidence_arguments: Box<[Identifier]>,
        source_span: SourceSpan,
        authored_call_selection: Option<
            language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
        >,
    },
    Value(ExpressionHandle),
    #[default]
    Terminal,
}

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
        pub struct TableNamePath {
            pub receiver_symbol: SymbolHandle,
            pub members: HandleSpan<Identifier>,
            pub head_symbol: SymbolHandle,
            pub symbol: SymbolHandle,
        }
    }
}

pub mod names {
    pub mod identity {
        use crate::TypedTrees;
        use crate::data::DataMember;

        pub fn normalized_schema_report_fingerprint(
            typed: &TypedTrees,
            data: &crate::data::DataDefinition,
        ) -> u64 {
            fn member_name(
                hash: &mut semantic_vocabulary::Fnv1a,
                identity: Option<u64>,
                name: &str,
                position: usize,
            ) {
                match identity {
                    Some(identity) => {
                        hash.byte(1);
                        hash.u64(identity);
                    }
                    None => {
                        hash.byte(0);
                        hash.u64(position as u64);
                        hash.string_framed(name);
                    }
                }
            }

            let mut hash = semantic_vocabulary::Fnv1a::new();
            hash.bytes(b"omega.schema.v2");
            let members = typed.data_members(data);
            let mut fields = members
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field),
                    DataMember::Variant(_) => None,
                })
                .collect::<Vec<_>>();
            let mut cases = members
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) => Some(variant),
                    DataMember::Field(_) => None,
                })
                .collect::<Vec<_>>();
            if fields.iter().all(|field| field.identity.is_some()) {
                fields.sort_by_key(|field| field.identity);
            }
            if cases.iter().all(|case| case.identity.is_some()) {
                cases.sort_by_key(|case| case.identity);
            }
            hash.u64(fields.len() as u64);
            for (position, field) in fields.iter().enumerate() {
                member_name(&mut hash, field.identity, field.name.as_str(), position);
                hash.byte(match field.relevance {
                    language_core::BindingRelevance::Relevant => 0,
                    language_core::BindingRelevance::Erased => 1,
                });
                hash.string_framed(typed.display_type_reference(field.type_reference).as_str());
            }
            hash.u64(cases.len() as u64);
            for (position, case) in cases.iter().enumerate() {
                member_name(&mut hash, case.identity, case.name.as_str(), position);
                let mut payload = typed.data_payload_fields(case).iter().collect::<Vec<_>>();
                if payload.iter().all(|field| field.identity.is_some()) {
                    payload.sort_by_key(|field| field.identity);
                }
                hash.u64(payload.len() as u64);
                for (payload_position, field) in payload.iter().enumerate() {
                    member_name(
                        &mut hash,
                        field.identity,
                        field.name.as_str(),
                        payload_position,
                    );
                    hash.byte(match field.relevance {
                        language_core::BindingRelevance::Relevant => 0,
                        language_core::BindingRelevance::Erased => 1,
                    });
                    hash.string_framed(typed.display_type_reference(field.type_reference).as_str());
                }
                let mut retired = case.retired_payload_identities.clone();
                retired.sort_unstable();
                hash.u64(retired.len() as u64);
                for identity in retired {
                    hash.u64(identity);
                }
            }
            let mut retired = data.retired_identities.clone();
            retired.sort_unstable();
            hash.u64(retired.len() as u64);
            for identity in retired {
                hash.u64(identity);
            }
            hash.finish().max(1)
        }
    }
    pub mod name {
        use std::borrow::Cow;
        use std::fmt;
        use std::ops::Deref;
        use std::sync::Arc;

        #[derive(Clone, Default, Eq, compiler_storage::Record)]
        pub struct Identifier {
            text: IdentifierText,
        }

        #[derive(Clone, Default, PartialEq, Eq, compiler_storage::Record)]
        enum IdentifierText {
            #[default]
            Missing,
            Static(Cow<'static, str>),
            Generated(Arc<str>),
        }

        impl Identifier {
            pub fn generated(text: impl Into<String>) -> Self {
                Self {
                    text: IdentifierText::Generated(Arc::from(text.into().into_boxed_str())),
                }
            }

            pub fn generated_static(text: &'static str) -> Self {
                Self {
                    text: IdentifierText::Static(Cow::Borrowed(text)),
                }
            }

            #[inline]
            pub fn as_str(&self) -> &str {
                match &self.text {
                    IdentifierText::Missing => "",
                    IdentifierText::Static(text) => text,
                    IdentifierText::Generated(text) => text.as_ref(),
                }
            }

            pub fn is_self_receiver(&self) -> bool {
                language_core::is_self_receiver(self.as_str())
            }

            pub fn is_reserved_result(&self) -> bool {
                language_core::is_reserved_result(self.as_str())
            }
        }

        impl From<&str> for Identifier {
            fn from(text: &str) -> Self {
                Self::generated(text)
            }
        }

        impl Deref for Identifier {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl fmt::Display for Identifier {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl fmt::Debug for Identifier {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(self.as_str(), formatter)
            }
        }

        impl PartialEq for Identifier {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                self.as_str() == other.as_str()
            }
        }
    }
}
