use super::CallableApplication;
use crate::source_state::{
    TypedTrees, expression::TableCallExpression, signature, trait_definition::AppliedTraitScope,
};
use symbols::{SymbolHandle, SymbolKind};

impl CallableApplication {
    pub(crate) fn namespace_symbols(
        &self,
        program: &mut TypedTrees,
        binder: SymbolHandle,
        expressions: &[crate::source_state::expression::ExpressionHandle],
    ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, &'static str> {
        let selected = if let Some((_, argument)) = self
            .static_arguments
            .iter()
            .rev()
            .find(|(source, _)| *source == binder)
        {
            argument.clone()
        } else if let Some((_, argument)) = self
            .types
            .iter()
            .rev()
            .find(|(source, _)| *source == binder)
        {
            program.static_operand_from_type_reference(*argument)?
        } else {
            return Ok(Vec::new());
        };
        if !selected.names_implementation(&program.symbols) {
            return Err("applied requirement namespace needs an implementation operand");
        }
        if selected.application.is_none() {
            return program.selected_implementation_namespace_symbols(binder, &selected);
        }
        let owner = program.symbols.get(binder).parent;
        let mut calls = Vec::new();
        for handle in expressions {
            let Some(call) = program.expression_table.call_node(*handle) else {
                continue;
            };
            if !program
                .expression_table
                .name_path_node(call.receiver)
                .is_some_and(|path| path.head_symbol == binder)
            {
                continue;
            }
            program
                .evidence_bound_requirement(owner, binder, call.target_symbol)
                .map_err(|_| "applied namespace call lost its source bound")?
                .ok_or("applied namespace call lost its exact requirement")?;
            calls.push((*handle, call.receiver));
        }
        calls.sort_unstable_by_key(|(_, receiver)| (receiver.arena_index(), receiver.generation()));
        if calls.is_empty() || expressions.iter().any(|handle| {
                matches!(program.expression_table.expression(*handle), crate::source_state::expression::ExpressionNode::Name(path) if path.head_symbol == binder)
                    && calls.binary_search_by_key(&(handle.arena_index(), handle.generation()), |(_, receiver)| (receiver.arena_index(), receiver.generation())).is_err()
            }) {
                return Err("applied namespace needs a complete qualified call");
            }
        let qualifier = if selected.type_reference.is_valid() {
            selected.type_reference
        } else {
            let owner = self
                .trait_region_application
                .as_ref()
                .map_or(self.static_argument_owner, |(scope, _)| {
                    scope.argument_owner()
                });
            program.static_operand_type_reference_in_scope(&selected, owner)?
        };
        // Keep the complete selected operand on each call. Psi04's ordinary
        // qualified-selection owner checks its application and public row.
        for (handle, _) in calls {
            let crate::source_state::expression::ExpressionNode::Call(call) =
                program.expression_table.expression_mut(handle)
            else {
                unreachable!();
            };
            call.requirement_application = qualifier;
        }
        program.implementation_namespace_symbols(binder, selected.symbol)
    }
}

impl TypedTrees {
    pub fn selected_implementation_namespace_symbols(
        &self,
        binder: SymbolHandle,
        selected: &crate::source_state::expression::StaticMachineArgument,
    ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, &'static str> {
        let program = self;
        if !selected.names_implementation(&program.symbols) {
            return Err("applied requirement namespace needs an implementation operand");
        }
        if selected.application.is_some() {
            return Err("applied requirement namespace needs its complete selected application");
        }
        self.implementation_namespace_symbols(binder, selected.symbol)
    }

    fn implementation_namespace_symbols(
        &self,
        binder: SymbolHandle,
        selected: SymbolHandle,
    ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, &'static str> {
        let program = self;
        let mut symbols = vec![(binder, selected)];
        let owner = program.symbols.get(binder).parent;
        for child in program.symbols.child_handles(binder).into_iter().flatten() {
            if program.symbols.get(child).kind != SymbolKind::State {
                continue;
            }
            let requirement = program
                .evidence_bound_requirement(owner, binder, child)
                .map_err(|_| "applied requirement namespace lost its authored bound")?
                .ok_or("applied requirement namespace lost its requirement")?;
            let mut candidates = Vec::new();
            if let Some(conformance) = program.conformance_by_symbol(selected) {
                candidates.extend(
                    conformance
                        .rows
                        .iter()
                        .filter(|row| {
                            row.declaring_trait == requirement.application.declaration.symbol
                                && row.requirement == requirement.signature.symbol
                        })
                        .map(|row| row.requirement),
                );
            } else {
                let selected_owner = program.symbols.get(selected).parent;
                for target in program
                    .symbols
                    .child_handles(selected)
                    .into_iter()
                    .flatten()
                {
                    if let Some(bound) = program
                        .evidence_bound_requirement(selected_owner, selected, target)
                        .map_err(|_| "selected namespace lost its authored bound")?
                        && bound.signature.symbol == requirement.signature.symbol
                        && bound.application.declaration.symbol
                            == requirement.application.declaration.symbol
                    {
                        candidates.push(target);
                    }
                }
            }
            let [target] = candidates.as_slice() else {
                return Err("selected namespace has no unique exact requirement row");
            };
            symbols.push((child, *target));
        }
        Ok(symbols)
    }
}

impl TypedTrees {
    /// The call retains a named map's independently supplied telescope in
    /// its requirement application, and the source application owns the
    /// lexical scope. Neither the receiver's name nor the expected result
    /// supplies an omitted argument.
    pub fn named_conformance_call_member(
        &self,
        call: &TableCallExpression,
    ) -> Result<
        Option<(
            &crate::source_state::trait_definition::Conformance,
            &crate::source_state::trait_definition::ConformanceRow,
            &signature::StateSignature,
        )>,
        &'static str,
    > {
        // Parameter dispatch already owns its requirement and realization.
        // A nominal receiver cannot reinterpret that independent selection.
        if call.static_requirement_dispatch.is_some() {
            return Ok(None);
        }
        let Some(receiver) = self.expression_table.name_path_node(call.receiver) else {
            return Ok(None);
        };
        if !call.requirement_application.is_valid() {
            return self.named_conformance_member(receiver.symbol, call.target_symbol);
        }
        if !receiver.symbol.is_valid() || !self.symbols.symbols().nodes().is_valid(receiver.symbol)
        {
            return Err("named application has no exact receiver");
        }
        if self.symbols.get(receiver.symbol).kind != SymbolKind::Conformance {
            return Ok(None);
        }
        let selected = self
            .conformance_by_symbol(receiver.symbol)
            .ok_or("named application lost its implementation")?;
        let parameters = self.conformance_type_parameters(selected);
        if parameters.is_empty() != selected.type_parameters.is_empty()
            || !selected.lifetime_parameters.is_empty()
            || parameters.iter().any(|parameter| {
                parameter.kind != crate::source_state::data::TypeParameterKind::Type
                    || parameter.bounds != crate::source_state::data::DataProperties::default()
            })
        {
            return Err("named application needs its complete eligible type telescope");
        }
        if !self
            .type_reference_table
            .contains_type_reference(call.requirement_application)
            || !self
                .expression_table
                .static_machine_application_is_valid(call.authored_machine_application)
        {
            return Err("named application changed its retained map or source application");
        }
        let arguments = match self.type_reference_node(call.requirement_application) {
            crate::source_state::types::TypeReferenceNode::Named { symbol, .. }
                if *symbol == receiver.symbol && parameters.is_empty() =>
            {
                &[][..]
            }
            crate::source_state::types::TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } if *base_symbol == receiver.symbol && lifetime_arguments.is_empty() => {
                self.type_reference_handles(*arguments)
            }
            _ => return Err("named application changed its complete map constructor"),
        };
        let source = self.static_machine_application(call.authored_machine_application);
        let closed = self.closed_requirement_application(call.target_symbol);
        let template = closed
            .as_ref()
            .map_or(call.target_symbol, |closed| closed.template);
        // A bodyless trait requirement is no machine declaration, so
        // source retention leaves its local binding declaration absent
        // unless stage 04 closed a generic requirement application.
        // Its registered operation and supplying owner remain exact.
        if source.registration_operation != template
            || (source.binding_declaration.is_valid() && source.binding_declaration != template)
            || (closed.is_none() && !source.bindings.is_empty())
            || !source.lifetime_bindings.is_empty()
            || !source.lexical_owner.is_valid()
            || !self
                .symbols
                .symbols()
                .nodes()
                .is_valid(source.lexical_owner)
        {
            return Err("named application changed its requirement or supplying owner");
        }
        if arguments.len() != parameters.len()
            || arguments.iter().any(|argument| {
                !self.type_reference_table.contains_type_reference(*argument)
                    || self
                        .scoped_type_identity_in_region_scope(
                            crate::source_state::type_identity::TypeIdentityRequest::ordinary(
                                *argument,
                            ),
                            &[],
                            source.lexical_owner,
                        )
                        .is_none()
            })
        {
            return Err("named application lost a complete map argument in its supplying scope");
        }
        let scope = self
            .applied_requirement_scope(source.lexical_owner, call.requirement_application)
            .ok_or("named application lost its complete trait application")?;
        self.named_conformance_member_at(receiver.symbol, call.target_symbol, Some(scope))
    }

    /// Include every map argument, even one unused by the trait scope,
    /// and its actual supplying owner when reusing a captured body.
    /// This key identifies a selection; it grants no body proof.
    pub fn named_conformance_capture_key(
        &self,
        call: &TableCallExpression,
    ) -> Result<
        Option<(
            SymbolHandle,
            crate::source_state::type_identity::NormalizedTypeIdentity,
        )>,
        &'static str,
    > {
        if self.named_conformance_call_member(call)?.is_none()
            || !call.requirement_application.is_valid()
        {
            return Ok(None);
        }
        let source = self.static_machine_application(call.authored_machine_application);
        let identity = crate::source_state::type_identity::applied_index_identity(
            self,
            crate::source_state::type_identity::IndexIdentityArgument::TypeReference(
                call.requirement_application,
            ),
            source.lexical_owner,
            &[],
        )
        .ok_or("named application lost its complete capture identity")?;
        Ok(Some((source.lexical_owner, identity)))
    }

    /// Retain the actual map constructor bindings and source scope before
    /// its body is copied. These are source data, not checked facts or an
    /// assertion that a conformance body was accepted.
    pub fn prepare_named_conformance_application(
        &mut self,
        call: &TableCallExpression,
    ) -> Result<Option<crate::source_state::ClosedConformanceApplication>, &'static str> {
        use crate::source_state::static_applications::{
            StaticMachineBinding, StaticMachineBindingValue,
        };
        let Some((owner, identity)) = self.named_conformance_capture_key(call)? else {
            return Ok(None);
        };
        let encoding = self
            .type_identity_text(identity)
            .ok_or("named application lost its canonical encoding")?;
        let crate::source_state::types::TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = self.type_reference_node(call.requirement_application)
        else {
            return Err("named application lost its complete constructor");
        };
        let declaration = *base_symbol;
        let arguments = self.type_reference_handles(*arguments).to_vec();
        let selected = self
            .conformance_by_symbol(declaration)
            .ok_or("named application lost its source declaration")?;
        let parameters = self
            .conformance_type_parameters(selected)
            .iter()
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        let scope = self
            .applied_requirement_scope(owner, call.requirement_application)
            .ok_or("named application lost its source scope")?;
        let environment = scope.environment();
        let type_arguments = arguments
            .iter()
            .map(|argument| {
                self.scoped_type_identity_in_region_scope(
                    crate::source_state::type_identity::TypeIdentityRequest::ordinary(*argument),
                    &[],
                    owner,
                )
                .and_then(|identity| self.type_identity_text(identity))
                .ok_or("named application lost the identity of a supplied map argument")
            })
            .collect::<Result<_, _>>()?;
        let operands = arguments
            .iter()
            .map(|reference| {
                let mut argument = self.static_operand_from_type_reference(*reference)?;
                argument.type_reference = *reference;
                Ok(argument)
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        let bindings = parameters
            .into_iter()
            .zip(&arguments)
            .map(|(parameter, argument)| StaticMachineBinding {
                parameter,
                value: StaticMachineBindingValue::Type(*argument),
            })
            .collect::<Vec<_>>();
        let scope = environment.retain(self)?;
        let name = crate::source_state::name::Identifier::generated(self.symbols.name(declaration));
        let application =
            self.expression_table
                .retain_static_machine_application(declaration, name, &operands);
        self.expression_table
            .set_static_machine_application_bindings(application, &bindings);
        let source = self
            .expression_table
            .static_machine_application_mut(application);
        source.lexical_owner = owner;
        source.binding_declaration = declaration;
        let closed = crate::source_state::ClosedConformanceApplication {
            declaration,
            trait_scope: scope,
            callable_application: application,
            arguments: operands.into_boxed_slice(),
            lifetime_arguments: Vec::new(),
            type_arguments,
            const_arguments: Vec::new(),
            machine_arguments: Vec::new(),
            ..Default::default()
        };
        self.finish_conformance_capture(closed, encoding).map(Some)
    }

    /// A named call and a Build choice bind the same implementation
    /// constructor. Its declaration supplies every row and its receiver.
    pub(crate) fn finish_conformance_capture(
        &self,
        mut record: crate::source_state::ClosedConformanceApplication,
        encoding: String,
    ) -> Result<crate::source_state::ClosedConformanceApplication, &'static str> {
        let declaration = self
            .conformance_by_symbol(record.declaration)
            .ok_or("implementation application lost its source declaration")?;
        let scope = record
            .trait_scope
            .scope(self)
            .ok_or("implementation application lost its exact trait receiver")?;
        record.trait_definition = scope.declaration.symbol;
        record.trait_arguments = self
            .trait_type_parameters(scope.declaration)
            .iter()
            .map(|parameter| {
                self.normalized_type_identity_text(scope.parameter_argument(self, parameter.symbol))
            })
            .collect();
        record.rows = declaration
            .rows
            .iter()
            .map(|row| crate::source_state::ClosedConformanceRowIdentity {
                declaring_trait: row.declaring_trait,
                requirement: row.requirement,
                realization_machine: row.realization_machine,
                realization_state: row.realization_state,
            })
            .collect();
        record.seal_encoding(encoding);
        Ok(record)
    }

    /// The named operation record, its unique requirement row and that
    /// row's public signature at the retained application. This identifies
    /// a choice; body validity and invocation authority remain separate.
    pub fn named_conformance_member(
        &self,
        namespace: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Result<
        Option<(
            &crate::source_state::trait_definition::Conformance,
            &crate::source_state::trait_definition::ConformanceRow,
            &signature::StateSignature,
        )>,
        &'static str,
    > {
        self.named_conformance_member_at(namespace, requirement, None)
    }

    pub(crate) fn named_conformance_member_at(
        &self,
        namespace: SymbolHandle,
        requirement: SymbolHandle,
        application: Option<AppliedTraitScope<'_>>,
    ) -> Result<
        Option<(
            &crate::source_state::trait_definition::Conformance,
            &crate::source_state::trait_definition::ConformanceRow,
            &signature::StateSignature,
        )>,
        &'static str,
    > {
        if !namespace.is_valid() {
            return Ok(None);
        }
        if !self.symbols.symbols().nodes().is_valid(namespace) {
            return Err("named public view has a foreign namespace");
        }
        if self.symbols.get(namespace).kind != SymbolKind::Conformance {
            return Ok(None);
        }
        let selected = self
            .conformance_by_symbol(namespace)
            .ok_or("named public view lost its selected implementation")?;
        // Applied generic maps retain their independently supplied tuple in
        // the ordinary dispatch path above; a bare declaration cannot supply it.
        if application.is_none()
            && (!selected.type_parameters.is_empty() || !selected.lifetime_parameters.is_empty())
        {
            return Err("named public view has no complete selected application");
        }
        let closed = self.closed_requirement_application(requirement);
        let template = closed
            .as_ref()
            .map_or(requirement, |selected| selected.template);
        let mut rows = selected
            .rows
            .iter()
            .filter(|row| row.requirement == template);
        let row = rows
            .next()
            .ok_or("named public view has no exact requirement row")?;
        if rows.next().is_some() {
            return Err("named public view has ambiguous requirement rows");
        }
        let machine = self
            .machine_by_symbol(row.realization_machine)
            .ok_or("named public view lost its selected machine")?;
        let Some([entry]) = self.machine_states(machine).as_array() else {
            return Err("named public view needs one exact selected entry");
        };
        if entry.symbol != row.realization_state {
            return Err("named public view changed its selected entry");
        }
        // An inline member belongs to this exact row. A reference row
        // instead retains an already declared machine and its own contract;
        // identifying either choice does not prove its refinement.
        if row.source.owns_realization() {
            let (owner, actual) = self
                .owned_conformance_realization(row.realization_machine)
                .ok_or("named public view lost its owned realization")?;
            if owner.symbol != selected.symbol || actual != row {
                return Err("named public view changed its owned realization");
            }
        }
        let scope = application
            .or_else(|| AppliedTraitScope::from_conformance(self, selected))
            .ok_or("named public view lost its complete trait application")?
            .declaring_scope(self, row.declaring_trait)
            .ok_or("named public view lost its declaring trait scope")?;
        if let Some(closed) = closed {
            if !self.requirement_application_matches_contract_scope(requirement, &scope) {
                return Err("named public view disagrees with its selected trait application");
            }
            // The call names this closed signature, including its clause
            // expressions and any arguments supplied to the requirement.
            return Ok(Some((selected, row, closed.signature)));
        }
        self.applied_callable_signature(&scope, template)
            .or_else(|| {
                (scope.substitutions.is_empty() && scope.declaration.lifetime_parameters.is_empty())
                    .then(|| self.trait_machine_signature_by_symbol(scope.declaration, template))
                    .flatten()
            })
            .map(|signature| Some((selected, row, signature)))
            .ok_or("named public view has no prepared applied signature")
    }
}
