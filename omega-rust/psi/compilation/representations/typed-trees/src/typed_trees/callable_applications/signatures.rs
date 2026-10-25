//! Lexical telescopes and proof clauses of a callable application.

use super::{CallableApplication, CallableLifetimeBinding, CallableLifetimeOrigin};
use crate::{TypedTrees, data, domain, signature};
use arena::{Handle, HandleSpan};

impl TypedTrees {
    /// Callable application preserves telescope positions while freshening
    /// unsupplied local binders. Rejoin by those positions, never by spelling:
    /// the caller's actual can have the same name as an unrelated local.
    pub fn applied_signature_parameter(
        &self,
        declared: &signature::StateSignature,
        applied: &signature::StateSignature,
        parameter: symbols::SymbolHandle,
    ) -> Option<symbols::SymbolHandle> {
        let declared_types = self.state_signature_type_parameters(declared);
        let applied_types = self.state_signature_type_parameters(applied);
        if declared_types.len() == applied_types.len()
            && let Some(position) = declared_types
                .iter()
                .position(|candidate| candidate.symbol == parameter)
        {
            return Some(applied_types[position].symbol);
        }
        let declared_values = self.state_signature_parameters(declared);
        let applied_values = self.state_signature_parameters(applied);
        if declared_values.len() != applied_values.len() {
            return None;
        }
        declared_values
            .iter()
            .position(|candidate| candidate.symbol == parameter)
            .map(|position| applied_values[position].symbol)
    }

    /// Rejoin one declaration clause to the corresponding applied clause.
    /// Callable application preserves contract and fact order while replacing
    /// their payloads. The original fact remains the authority/subject identity;
    /// this query supplies only its selected payload, never a global fact row.
    pub fn applied_signature_contract_fact(
        &self,
        declared: &signature::StateSignature,
        applied: &signature::StateSignature,
        fact: Handle<domain::ProofFact>,
    ) -> Option<&domain::ProofFact> {
        let declared_contracts = self.state_signature_contracts(declared);
        let applied_contracts = self.state_signature_contracts(applied);
        if declared_contracts.len() != applied_contracts.len() {
            return None;
        }
        for (source, selected) in declared_contracts.iter().zip(applied_contracts) {
            if source.kind != selected.kind || source.facts.len() != selected.facts.len() {
                return None;
            }
            for offset in 0..source.facts.len() {
                let offset = u32::try_from(offset).ok()?;
                if source.facts.handle_at(offset) == fact {
                    return self
                        .proof_facts
                        .span_or_empty(selected.facts)
                        .get(offset as usize);
                }
            }
        }
        None
    }
}

impl CallableApplication {
    pub(super) fn signature(
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
                // A nested callable introduces its own fresh binders. The outer
                // captured application must not rewrite a same-spelled local.
                self.lifetimes
                    .extend(declared.lifetime_parameters.iter().enumerate().map(
                        |(ordinal, parameter)| CallableLifetimeBinding {
                            parameter: parameter.clone(),
                            argument: parameter.clone(),
                            region: crate::type_identity::LifetimeParameterIdentity {
                                declaration: declared.symbol,
                                ordinal,
                            },
                            origin: CallableLifetimeOrigin::Rigid,
                        },
                    ));
            }
            // Mint the complete local scope before copying any payload: later
            // binders and value subjects may occur in earlier bounds or predicates.
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
                .map(|(symbol, name)| (symbol, program.symbols.get(symbol).kind, name.to_owned()))
                .collect::<Vec<_>>();
            let owner = self.symbol(declared.symbol);
            if owner.is_valid() && owner == self.owned_signature {
                // Only the producer of a fresh executable signature owns this
                // hierarchy range. Logical checking views deliberately reuse
                // declaration identities and cannot replace authored children.
                // Install the complete telescope once, preserving simultaneous
                // substitution and each binder's independent provenance.
                let children = program.symbols.insert_generated_children_from(
                    owner,
                    pending
                        .iter()
                        .map(|(symbol, kind, name)| (*symbol, *kind, name.as_str())),
                );
                for (offset, (symbol, _, _)) in (0u32..).zip(pending.iter()) {
                    self.symbols.push((*symbol, children.handle_at(offset)));
                }
            } else {
                for (symbol, kind, name) in pending {
                    let fresh = program
                        .symbols
                        .insert_generated_root_from(symbol, kind, &name);
                    self.symbols.push((symbol, fresh));
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
                parameter.symbol = self.symbol(parameter.symbol);
                parameter.kind = self.parameter_kind(program, parameter.kind)?;
                applied_parameters.push(parameter);
            }
            applied.type_parameters = program.data_type_parameters.insert_many(applied_parameters);
            applied.parameters = self.parameters(program, &parameters)?;
            applied.return_type = self.type_reference(program, declared.return_type)?;
            applied.contracts = HandleSpan::empty();
            for mut contract in program.state_signature_contracts(declared).to_vec() {
                contract.facts = self.facts(program, contract.facts)?;
                program
                    .signature_contracts
                    .append_to_span(&mut applied.contracts, contract);
            }
            applied.where_facts = self.facts(program, declared.where_facts)?;
            applied.invokes = HandleSpan::empty();
            for mut invocation in program.state_signature_invokes(declared).to_vec() {
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
                    signature::AuthoredInvocationTarget::Unresolved => {
                        signature::AuthoredInvocationTarget::Unresolved
                    }
                };
                program
                    .signature_invokes
                    .append_to_span(&mut applied.invokes, invocation);
            }
            if let language_semantics::TerminationGuarantee::Terminates { premises } =
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

    pub(super) fn parameter_kind(
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
            data::TypeParameterKind::Machine { contract } => {
                let contract = match contract {
                    data::MachineParameterContract::Structural(declared) => {
                        data::MachineParameterContract::Structural(
                            self.signature(program, &declared)?,
                        )
                    }
                    data::MachineParameterContract::Nominal {
                        trait_definition,
                        requirement,
                        trait_arguments,
                        ..
                    } => {
                        let arguments = trait_arguments
                            .into_iter()
                            .map(|argument| self.type_reference(program, argument))
                            .collect::<Result<Vec<_>, _>>()?;
                        let instantiated = self
                            .nominal(program, trait_definition, requirement, &arguments)?
                            .map(Box::new);
                        data::MachineParameterContract::Nominal {
                            trait_definition,
                            requirement,
                            trait_arguments: arguments,
                            instantiated,
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
                contract.parameters = self.parameters(program, &parameters)?;
                data::TypeParameterKind::Proposition { contract }
            }
            data::TypeParameterKind::Type => data::TypeParameterKind::Type,
            data::TypeParameterKind::Implementation => data::TypeParameterKind::Implementation,
        })
    }

    fn parameters(
        &mut self,
        program: &mut TypedTrees,
        declared: &[signature::StateParameter],
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

    fn facts(
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
                        // The selected proposition belongs to the caller's
                        // telescope, just like its supplied type operands.
                        // Freshening alone leaves the receiving proposition
                        // binder attached to already-substituted arguments.
                        let crate::types::TypeReferenceNode::Named { symbol, name } =
                            program.type_reference_table.type_reference(*replacement)
                        else {
                            return Err(
                                "applied callable proposition head needs a named proposition operand",
                            );
                        };
                        application.proposition = *symbol;
                        application.name = name.clone();
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
                                crate::types::TypeReferenceNode::Named { symbol, name } => {
                                    argument.symbol = *symbol;
                                    argument.path = vec![name.clone()].into_boxed_slice();
                                }
                                crate::types::TypeReferenceNode::ConstExpression(expression) => {
                                    let crate::expression::ExpressionNode::Integer(value) =
                                        program.expression_table.expression(*expression)
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
        // Publish the span in one batch, after recursive copies. Appending
        // before descent would interleave unrelated facts in this telescope.
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
