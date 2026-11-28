use super::{CallableApplication, CallableLifetimeBinding, CallableLifetimeOrigin};
use crate::source_state::{TypedTrees, data, domain, signature};
use arena::{Handle, HandleSpan};

impl TypedTrees {}

impl CallableApplication {
    pub fn service_reach(
        &mut self,
        program: &mut TypedTrees,
        id: semantic_vocabulary::ServiceReachId,
    ) -> Result<semantic_vocabulary::ServiceReachId, &'static str> {
        let row = program
            .service_reaches
            .definition(id)
            .ok_or("missing declared service in callable substitution")?;
        let Some(application) = &row.application else {
            return Ok(id);
        };
        let mut environment = application
            .arguments
            .scope(program)
            .ok_or("service application lost its declaring scope")?
            .environment();
        environment.substitute_service(program, self)?;
        program.intern_service_reach_environment(&environment)
    }

    pub(super) fn applied_reach_row(
        &mut self,
        program: &mut TypedTrees,
        row: semantic_vocabulary::ServiceReachRowId,
    ) -> Result<semantic_vocabulary::ServiceReachRowId, &'static str> {
        if row.0 == 0 {
            return Ok(row);
        }
        let services = program.service_reach_rows.services(row).to_vec();
        let applied = services
            .into_iter()
            .map(|service| self.service_reach(program, service))
            .collect::<Result<Vec<_>, _>>()?;
        let concrete = program.service_reach_rows.intern(applied);
        Ok(if program.service_reach_rows.is_abstract(row) {
            program.service_reach_rows.intern_abstract(concrete)
        } else {
            concrete
        })
    }

    /// A copied declaration owns the written reach clause, including a
    /// written empty ceiling. Substitute each target in the binder scope
    /// that also produces the copied signature's normalized reach.
    pub(super) fn copy_authored_reach(
        &mut self,
        program: &mut TypedTrees,
        declared: symbols::SymbolHandle,
        applied: symbols::SymbolHandle,
    ) -> Result<(), &'static str> {
        let mut rows = program
            .authored_service_reach_rows_for(declared)
            .cloned()
            .collect::<Vec<_>>();
        for row in &mut rows {
            row.owner = applied;
            for target in &mut row.targets {
                target.service = match target.service {
                    signature::ReachSubject::Service(id) => {
                        signature::ReachSubject::Service(self.service_reach(program, id)?)
                    }
                    signature::ReachSubject::Static(symbol) => {
                        signature::ReachSubject::Static(self.symbol(symbol))
                    }
                };
            }
        }
        program.authored_service_reach_rows.extend(rows);
        Ok(())
    }

    pub(super) fn invocations(
        &mut self,
        program: &mut TypedTrees,
        declared: HandleSpan<signature::AuthoredInvocation>,
    ) -> Result<HandleSpan<signature::AuthoredInvocation>, &'static str> {
        let mut invocations = program
            .signature_invokes
            .span(declared)
            .ok_or("callable invocation span is stale")?
            .to_vec();
        for invocation in &mut invocations {
            invocation.target = match invocation.target {
                signature::AuthoredInvocationTarget::Parameter { ordinal, symbol } => {
                    signature::AuthoredInvocationTarget::Parameter {
                        ordinal,
                        symbol: self.symbol(symbol),
                    }
                }
                signature::AuthoredInvocationTarget::Service(symbol) => {
                    signature::AuthoredInvocationTarget::Service(self.symbol(symbol))
                }
                signature::AuthoredInvocationTarget::AppliedService(service) => {
                    signature::AuthoredInvocationTarget::AppliedService(
                        self.service_reach(program, service)?,
                    )
                }
                unresolved @ (signature::AuthoredInvocationTarget::Unresolved
                | signature::AuthoredInvocationTarget::Undeclared) => unresolved,
            };
        }
        Ok(program.signature_invokes.insert_many(invocations))
    }

    pub(crate) fn signature(
        &mut self,
        program: &mut TypedTrees,
        declared: &signature::StateSignature,
    ) -> Result<signature::StateSignature, &'static str> {
        let type_parameters = program.state_signature_type_parameters(declared).to_vec();
        let parameters = program.state_signature_parameters(declared).to_vec();
        let previous_symbols = self.symbols.len();
        let previous_lifetimes = self.lifetimes.len();
        let previous_binders = self.source_binders.len();
        let previous_region_scope = self.region_scope;
        self.region_scope = declared.symbol;
        let result = (|| {
            self.source_binders.extend(
                type_parameters
                    .iter()
                    .map(|parameter| (parameter.name.clone(), parameter.symbol))
                    .chain(
                        parameters
                            .iter()
                            .map(|parameter| (parameter.name.clone(), parameter.symbol)),
                    ),
            );
            let selected_lifetime_telescope = declared.symbol == self.lifetime_declaration;
            if !selected_lifetime_telescope {
                self.lifetimes
                    .extend(declared.lifetime_parameters.iter().enumerate().map(
                        |(ordinal, parameter)| CallableLifetimeBinding {
                            parameter: parameter.clone(),
                            argument: parameter.clone(),
                            region: crate::source_state::type_identity::LifetimeParameterIdentity {
                                declaration: declared.symbol,
                                ordinal,
                            },
                            origin: CallableLifetimeOrigin::Rigid,
                        },
                    ));
            }
            let pending = type_parameters
                .iter()
                .map(|parameter| (parameter.symbol, parameter.name.as_str()))
                .chain(
                    parameters
                        .iter()
                        .map(|parameter| (parameter.symbol, parameter.name.as_str())),
                )
                .filter(|(symbol, _)| {
                    symbol.is_valid()
                        && !self.types.iter().any(|(parameter, _)| parameter == symbol)
                        && !self
                            .static_arguments
                            .iter()
                            .any(|(parameter, _)| parameter == symbol)
                })
                .map(|(symbol, name)| {
                    let kind = if self.runtime_values.contains(&symbol) {
                        symbols::SymbolKind::Parameter
                    } else {
                        program.symbols.get(symbol).kind
                    };
                    (symbol, kind, name.to_owned())
                })
                .collect::<Vec<_>>();
            if !pending.is_empty() {
                let owner = self.symbol(declared.symbol);
                if !owner.is_valid() || !program.symbols.symbols().nodes().is_valid(owner) {
                    return Err("applied callable signature has no retained declaration owner");
                }
                let binders = pending
                    .iter()
                    .map(|(symbol, kind, name)| (*symbol, *kind, name.as_str()));
                let children = if owner == self.owned_signature {
                    program
                        .symbols
                        .insert_generated_children_from(owner, binders)
                } else {
                    // An application that retains its signature declaration
                    // still owns its fresh binders. Append them without
                    // replacing the declaration's authored child range.
                    program
                        .symbols
                        .append_generated_children_from(owner, binders)
                };
                for (offset, (symbol, _, _)) in (0u32..).zip(pending.iter()) {
                    self.symbols.push((*symbol, children.handle_at(offset)));
                }
            }
            let mut applied = declared.clone();
            if selected_lifetime_telescope {
                applied.lifetime_parameters.retain(|parameter| {
                    !self
                        .lifetimes
                        .iter()
                        .any(|binding| binding.parameter == *parameter)
                });
            }
            applied.symbol = self.symbol(declared.symbol);
            let mut applied_parameters = Vec::with_capacity(type_parameters.len());
            for mut parameter in type_parameters {
                let original = parameter.symbol;
                parameter.symbol = self.symbol(original);
                // A fresh structural callback owns its complete applied signature,
                // including the authored distinction between omitted reach and none.
                let nested_owner = matches!(
                    &parameter.kind,
                    data::TypeParameterKind::Machine {
                        contract: data::MachineParameterContract::Structural(header)
                    } if header.symbol == original
                ) && self.symbols[previous_symbols..]
                    .contains(&(original, parameter.symbol));
                let previous_owner = self.owned_signature;
                if nested_owner {
                    self.owned_signature = parameter.symbol;
                }
                let kind = self.parameter_kind(program, parameter.kind);
                self.owned_signature = previous_owner;
                parameter.kind = kind?;
                applied_parameters.push(parameter);
            }
            applied.type_parameters = program.data_type_parameters.insert_many(applied_parameters);
            for bound in &mut applied.conformance_bounds {
                bound.binder = self.symbol(bound.binder);
                bound.arguments = bound
                    .arguments
                    .iter()
                    .map(|argument| self.type_reference(program, *argument))
                    .collect::<Result<Vec<_>, _>>()?;
            }
            applied.parameters = self.parameters(program, (&parameters).into())?;
            for (declared, applied_parameter) in program
                .state_signature_type_parameters(declared)
                .iter()
                .cloned()
                .zip(
                    program
                        .state_signature_type_parameters(&applied)
                        .iter()
                        .cloned(),
                )
                .collect::<Vec<_>>()
            {
                if !self.runtime_values.contains(&declared.symbol) {
                    continue;
                }
                let data::TypeParameterKind::Value { type_reference } = applied_parameter.kind
                else {
                    return Err("runtime requirement projection is not a Value binder");
                };
                program.state_parameters.append_to_span(
                    &mut applied.parameters,
                    signature::StateParameter {
                        symbol: applied_parameter.symbol,
                        name: applied_parameter.name,
                        type_reference,
                        is_const: false,
                        is_mutable: false,
                        relevance: language_core::BindingRelevance::Relevant,
                    },
                );
            }
            applied.return_type = self.type_reference(program, declared.return_type)?;
            applied.contracts = HandleSpan::empty();
            for mut contract in program.state_signature_contracts(declared).to_vec() {
                if let signature::SignatureContractKind::EnsuresForResultCase {
                    result_data,
                    result_case,
                } = &mut contract.kind
                {
                    *result_data = self.symbol(*result_data);
                    *result_case = self.symbol(*result_case);
                }
                contract.facts = self.facts(program, contract.facts)?;
                program
                    .signature_contracts
                    .append_to_span(&mut applied.contracts, contract);
            }
            applied.service_reach_row =
                self.applied_reach_row(program, declared.service_reach_row)?;
            if applied.symbol != declared.symbol && applied.symbol == self.owned_signature {
                self.copy_authored_reach(program, declared.symbol, applied.symbol)?;
            }
            applied.invokes = self.invocations(program, declared.invokes)?;
            if let source_declarations::TerminationGuarantee::Terminates { premises } =
                &mut applied.termination_guarantee
            {
                for premise in premises {
                    premise.subject.root = self.symbol(premise.subject.root);
                    for projection in &mut premise.subject.projections {
                        *projection = self.symbol(*projection);
                    }
                }
            }
            Ok(applied)
        })();
        self.symbols.truncate(previous_symbols);
        self.lifetimes.truncate(previous_lifetimes);
        self.source_binders.truncate(previous_binders);
        self.region_scope = previous_region_scope;
        result
    }

    pub(crate) fn parameter_kind(
        &mut self,
        program: &mut TypedTrees,
        kind: data::TypeParameterKind,
    ) -> Result<data::TypeParameterKind, &'static str> {
        Ok(match kind {
            data::TypeParameterKind::Const { type_reference } => data::TypeParameterKind::Const {
                type_reference: self.type_reference(program, type_reference)?,
            },
            data::TypeParameterKind::Value { type_reference } => data::TypeParameterKind::Value {
                type_reference: self.type_reference(program, type_reference)?,
            },
            data::TypeParameterKind::Proof { contract } => data::TypeParameterKind::Proof {
                contract: self.signature(program, &contract)?,
            },
            data::TypeParameterKind::Machine { contract } => {
                let contract = match contract {
                    data::MachineParameterContract::Structural(declared) => {
                        data::MachineParameterContract::Structural(
                            self.signature(program, &declared)?,
                        )
                    }
                    data::MachineParameterContract::Nominal {
                        receiving_trait,
                        receiving_arguments,
                        trait_definition,
                        requirement,
                        trait_arguments,
                        instantiated,
                    } => {
                        let receiving_arguments = receiving_arguments
                            .into_iter()
                            .map(|argument| self.type_reference(program, argument))
                            .collect::<Result<Vec<_>, _>>()?;
                        let arguments = trait_arguments
                            .into_iter()
                            .map(|argument| self.type_reference(program, argument))
                            .collect::<Result<Vec<_>, _>>()?;
                        if let Some(signature) = instantiated {
                            data::MachineParameterContract::Nominal {
                                receiving_trait,
                                receiving_arguments,
                                trait_definition,
                                requirement,
                                trait_arguments: arguments,
                                instantiated: Some(Box::new(self.signature(program, &signature)?)),
                            }
                        } else {
                            self.nominal_contract(
                                program,
                                receiving_trait,
                                requirement,
                                receiving_arguments,
                            )?
                        }
                    }
                    data::MachineParameterContract::RequirementIdentity => {
                        data::MachineParameterContract::RequirementIdentity
                    }
                };
                data::TypeParameterKind::Machine { contract }
            }
            data::TypeParameterKind::Proposition { mut contract } => {
                let parameters = program
                    .state_parameters
                    .span_or_empty(contract.parameters)
                    .to_vec();
                contract.parameters = self.parameters(program, (&parameters).into())?;
                data::TypeParameterKind::Proposition { contract }
            }
            data::TypeParameterKind::Type => data::TypeParameterKind::Type,
            data::TypeParameterKind::Implementation => data::TypeParameterKind::Implementation,
        })
    }

    fn parameters(
        &mut self,
        program: &mut TypedTrees,
        declared: arena::RootRows<'_, signature::StateParameter>,
    ) -> Result<HandleSpan<signature::StateParameter>, &'static str> {
        let mut parameters = HandleSpan::empty();
        let previous_symbols = self.symbols.len();
        for parameter in declared {
            if parameter.symbol.is_valid() && self.symbol(parameter.symbol) == parameter.symbol {
                let kind = program.symbols.get(parameter.symbol).kind;
                let fresh = program.symbols.insert_generated_root_from(
                    parameter.symbol,
                    kind,
                    parameter.name.as_str(),
                );
                self.symbols.push((parameter.symbol, fresh));
            }
        }
        for mut parameter in declared.iter().cloned() {
            parameter.symbol = self.symbol(parameter.symbol);
            parameter.type_reference = self.type_reference(program, parameter.type_reference)?;
            program
                .state_parameters
                .append_to_span(&mut parameters, parameter);
        }
        self.symbols.truncate(previous_symbols);
        Ok(parameters)
    }

    pub fn facts(
        &mut self,
        program: &mut TypedTrees,
        declared: HandleSpan<domain::ProofFact>,
    ) -> Result<HandleSpan<domain::ProofFact>, &'static str> {
        let original = program.proof_facts.span_or_empty(declared).to_vec();
        if original.len() != declared.len() {
            return Err("applied callable has a stale proof-fact span");
        }
        let mut copied = Vec::with_capacity(original.len());
        for (offset, fact) in original.into_iter().enumerate() {
            let source_index = declared
                .start()
                .arena_index()
                .checked_add(
                    u32::try_from(offset).map_err(|_| "applied proof-fact offset overflow")?,
                )
                .ok_or("applied proof-fact handle overflow")?;
            let source_fact = Handle::from_parts(source_index, declared.start().generation());
            let span = program.proof_fact_source_span(source_fact);
            let fact = match fact {
                domain::ProofFact::Expression(expression) => {
                    domain::ProofFact::Expression(self.expression(program, expression)?)
                }
                domain::ProofFact::Membership(mut membership) => {
                    membership.value = self.expression(program, membership.value)?;
                    membership.domain_arguments =
                        self.type_arguments(program, membership.domain_arguments)?;
                    membership.domain_symbol = self.symbol(membership.domain_symbol);
                    let arguments = program
                        .type_reference_table
                        .type_reference_handles(membership.domain_arguments)
                        .to_vec();
                    membership.semantic_domain = self.domain_identity(
                        program,
                        membership.domain_symbol,
                        &arguments,
                        membership.semantic_domain,
                    )?;
                    domain::ProofFact::Membership(membership)
                }
                domain::ProofFact::Proposition(mut application) => {
                    let arguments = program
                        .expression_table
                        .expression_handles(application.arguments)
                        .to_vec();
                    let arguments = arguments
                        .into_iter()
                        .map(|argument| self.expression(program, argument))
                        .collect::<Result<Vec<_>, _>>()?;
                    application.arguments = program
                        .expression_table
                        .insert_expression_handles(arguments);
                    if let Some((_, replacement)) = self
                        .types
                        .iter()
                        .rev()
                        .find(|(symbol, _)| *symbol == application.proposition)
                    {
                        match program.type_reference_table.type_reference(*replacement) {
                            crate::source_state::types::TypeReferenceNode::Named {
                                symbol,
                                name,
                            } => {
                                application.proposition = *symbol;
                                application.name = name.clone();
                            }
                            // A proposition generic over types, applied
                            // at the operand's arguments (`same_count<T>`).
                            crate::source_state::types::TypeReferenceNode::Generic {
                                base_symbol,
                                base_name,
                                lifetime_arguments,
                                arguments,
                            } if lifetime_arguments.is_empty()
                                && application.binder_arguments.is_empty()
                                && program.symbols.symbols().nodes().is_valid(*base_symbol)
                                && program.symbols.get(*base_symbol).kind
                                    == symbols::SymbolKind::Proposition =>
                            {
                                application.proposition = *base_symbol;
                                application.name = base_name.clone();
                                let mut binder_arguments = Vec::new();
                                for &argument in program
                                    .type_reference_table
                                    .type_reference_handles(*arguments)
                                {
                                    let crate::source_state::types::TypeReferenceNode::Named {
                                        symbol,
                                        name,
                                    } = program.type_reference_table.type_reference(argument)
                                    else {
                                        return Err(
                                            "applied callable proposition operand applies a type that is not named",
                                        );
                                    };
                                    binder_arguments.push(
                                            crate::source_state::proposition::PropositionBinderArgument {
                                                kind: crate::source_state::proposition::PropositionBinderArgumentKind::Type,
                                                path: vec![name.clone()].into_boxed_slice(),
                                                const_literal: None,
                                                evidence_projection: None,
                                                symbol: *symbol,
                                            },
                                        );
                                }
                                application.binder_arguments = binder_arguments.into_boxed_slice();
                            }
                            _ => {
                                return Err(
                                    "applied callable proposition head needs a named proposition operand",
                                );
                            }
                        }
                    } else {
                        application.proposition = self.symbol(application.proposition);
                    }
                    for argument in &mut application.binder_arguments {
                        if let Some((_, replacement)) = self
                            .types
                            .iter()
                            .rev()
                            .find(|(symbol, _)| *symbol == argument.symbol)
                        {
                            match program.type_reference_table.type_reference(*replacement) {
                                crate::source_state::types::TypeReferenceNode::Named {
                                    symbol,
                                    name,
                                } => {
                                    argument.symbol = *symbol;
                                    argument.path = vec![name.clone()].into_boxed_slice();
                                }
                                crate::source_state::types::TypeReferenceNode::ConstExpression(
                                    expression,
                                ) => {
                                    let crate::source_state::expression::ExpressionNode::Integer(
                                        value,
                                    ) = program.expression_table.expression(*expression)
                                    else {
                                        return Err(
                                            "nominal callable proposition operand needs a normalized const literal",
                                        );
                                    };
                                    argument.const_literal = Some(value.clone());
                                    argument.symbol = symbols::SymbolHandle::invalid();
                                    argument.path = Box::new([]);
                                }
                                _ => {
                                    return Err(
                                        "compound nominal callable proposition operands are not yet represented",
                                    );
                                }
                            }
                        } else {
                            argument.symbol = self.symbol(argument.symbol);
                        }
                    }
                    domain::ProofFact::Proposition(application)
                }
            };
            copied.push((fact, span));
        }
        let facts = program
            .proof_facts
            .insert_many(copied.iter().map(|(fact, _)| fact.clone()));
        for (offset, (_, source_span)) in copied.into_iter().enumerate() {
            if let Some(source_span) = source_span {
                let index = facts
                    .start()
                    .arena_index()
                    .checked_add(
                        u32::try_from(offset).map_err(|_| "applied proof-fact offset overflow")?,
                    )
                    .ok_or("applied proof-fact handle overflow")?;
                program.set_proof_fact_source_span(
                    Handle::from_parts(index, facts.start().generation()),
                    source_span,
                );
            }
        }
        Ok(facts)
    }
}

#[cfg(test)]
#[path = "../tests/signatures.rs"]
mod tests;
