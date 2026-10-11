//! Closed requirement applications retain their authored owner and telescope.
//!
//! A trait signature is not a machine body. Its instance borrows the same deep
//! callable application mechanism as an applied checking view, but has a fresh
//! executable symbol and an explicit complete source application. Equal ABI
//! carriers never collapse different tuples. Creation derives representation;
//! immutable checking still admits every operand and substituted predicate.
//! Invocation lifetimes remain universal on that signature. The source call's
//! parameter-position lifetime map describes caller loans, not a new public
//! requirement or permission to capture a region in a specialization operand.

use super::{AppliedCallableOwner, AppliedCallableSignature, CallableApplication};
use crate::{TypedTrees, data, signature, static_applications, types};
use arena::HandleSpan;
use symbols::SymbolHandle;

#[derive(Default)]
struct ClosedOperands {
    symbols: Vec<SymbolHandle>,
    types: Vec<types::TypeReferenceHandle>,
    expressions: Vec<crate::expression::ExpressionHandle>,
}

impl Extend<SymbolHandle> for ClosedOperands {
    fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: Symbols) {
        self.symbols.extend(symbols);
    }
}

impl crate::references::TypedReferences for ClosedOperands {
    fn type_reference(&mut self, reference: types::TypeReferenceHandle) -> bool {
        if self.types.contains(&reference) {
            return false;
        }
        self.types.push(reference);
        true
    }
    fn expression(&mut self, expression: crate::expression::ExpressionHandle) -> bool {
        if self.expressions.contains(&expression) {
            return false;
        }
        self.expressions.push(expression);
        true
    }
}

#[derive(Clone, Copy)]
pub struct ClosedRequirementApplication<'program> {
    pub declaring_trait: SymbolHandle,
    pub template: SymbolHandle,
    pub trait_arguments: &'program [types::TypeReferenceHandle],
    pub local_arguments: &'program [types::TypeReferenceHandle],
    pub application: static_applications::StaticMachineApplicationHandle,
    pub signature: &'program signature::StateSignature,
}

impl TypedTrees {
    pub fn closed_requirement_applications(
        &self,
    ) -> impl Iterator<Item = ClosedRequirementApplication<'_>> {
        self.applied_callable_signatures
            .iter()
            .filter_map(|(_, prepared)| {
                let AppliedCallableOwner::RequirementInstance {
                    declaration,
                    template,
                    trait_arguments,
                    local_arguments,
                    application,
                } = prepared.owner
                else {
                    return None;
                };
                Some(ClosedRequirementApplication {
                    declaring_trait: declaration,
                    template,
                    trait_arguments: self
                        .type_reference_table
                        .type_reference_handles(trait_arguments),
                    local_arguments: self
                        .type_reference_table
                        .type_reference_handles(local_arguments),
                    application,
                    signature: &prepared.signature,
                })
            })
    }

    pub fn closed_requirement_application(
        &self,
        instance: SymbolHandle,
    ) -> Option<ClosedRequirementApplication<'_>> {
        self.closed_requirement_applications()
            .find(|application| application.signature.symbol == instance)
    }

    /// Whether a checked satisfaction/map scope selects this closed requirement's
    /// exact declaring application. Inherited paths must agree; an equal local
    /// tuple cannot substitute another trait application or phantom argument.
    /// This rejoins derivation data and does not establish conformance authority.
    pub fn requirement_application_matches_scope<'program>(
        &'program self,
        requirement: SymbolHandle,
        scope: &crate::trait_definition::AppliedTraitScope<'program>,
    ) -> bool {
        let Some(application) = self.closed_requirement_application(requirement) else {
            return false;
        };
        let Some(declaration) = self.trait_definition_by_symbol(application.declaring_trait) else {
            return false;
        };
        let Some(expected) = crate::trait_definition::AppliedTraitScope::new(
            self,
            declaration,
            application.trait_arguments,
        ) else {
            return false;
        };
        scope
            .declaring_scope(self, declaration.symbol)
            .is_some_and(|actual| actual.same_application(self, &expected))
    }

    /// Join a finite requirement coordinate to the exact realization receipt.
    /// These canonical tuple identities are retained by specialization; equal
    /// ABI types and readable const spellings cannot donate another tuple.
    pub fn requirement_realization_specialization(
        &self,
        requirement: SymbolHandle,
        realization: SymbolHandle,
        provider: SymbolHandle,
    ) -> Option<&crate::typed_trees::commitments::MachineSpecialization> {
        let application = self.closed_requirement_application(requirement)?;
        let tuple = self
            .expression_table
            .static_machine_application_arguments(application.application)
            .iter()
            .map(|argument| self.static_const_argument_identity(argument))
            .collect::<Option<Vec<_>>>()?;
        let mut matches = self.machine_specializations.iter().filter(|receipt| {
            receipt.template == realization
                && receipt.instance == provider
                && receipt.is_const_only()
                && receipt.const_argument_identities == tuple
        });
        let receipt = matches.next()?;
        matches.next().is_none().then_some(receipt)
    }

    /// Close an exact declaring trait application and complete local telescope.
    /// The supplied row is derivation data, not an admitted selection receipt.
    pub fn instantiate_requirement_application(
        &mut self,
        declaring_trait: SymbolHandle,
        template: SymbolHandle,
        trait_arguments: &[types::TypeReferenceHandle],
        application: static_applications::StaticMachineApplicationHandle,
    ) -> Result<SymbolHandle, &'static str> {
        if !application.is_valid() {
            return Err("closed requirement has no retained source application");
        }
        let source = self
            .expression_table
            .static_machine_application(application);
        if source.registration_operation != template || source.binding_declaration != template {
            return Err("closed requirement application names a different declaration");
        }
        let bindings = self
            .expression_table
            .static_machine_application_bindings(application)
            .to_vec();
        let local_arguments = bindings
            .iter()
            .map(|binding| match &binding.value {
                static_applications::StaticMachineBindingValue::Type(actual)
                | static_applications::StaticMachineBindingValue::Const(actual) => Ok(*actual),
                static_applications::StaticMachineBindingValue::Machine(actual)
                | static_applications::StaticMachineBindingValue::Conformance(actual) => {
                    self.static_operand_type_reference(actual)
                }
                static_applications::StaticMachineBindingValue::RuntimeValue { .. } => {
                    Err("runtime operand does not close a requirement instance")
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut operands = ClosedOperands::default();
        for argument in trait_arguments.iter().chain(&local_arguments) {
            crate::references::visit_type_reference(self, &mut operands, *argument);
        }
        // A materialized nominal owns an origin application even when its
        // visible node is now a bare name. Closure must follow that origin,
        // just as region-sensitive identity does, including phantom operands.
        let mut next = 0;
        while let Some(reference) = operands.types.get(next).copied() {
            next += 1;
            let symbol = match self.type_reference_table.type_reference(reference) {
                types::TypeReferenceNode::Named { symbol, .. } => *symbol,
                types::TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
                _ => continue,
            };
            if let Some(origin) = self
                .data_definition_by_symbol(symbol)
                .and_then(|definition| definition.generic_instance)
            {
                crate::references::visit_type_reference(self, &mut operands, origin);
            }
        }
        if operands
            .symbols
            .iter()
            .filter(|symbol| symbol.is_valid())
            .any(|symbol| {
                matches!(
                    self.symbols.get(*symbol).kind,
                    symbols::SymbolKind::TypeParameter
                        | symbols::SymbolKind::MachineParameter
                        | symbols::SymbolKind::ConformanceParameter
                        | symbols::SymbolKind::PropositionParameter
                        | symbols::SymbolKind::PropositionMachineParameter
                        | symbols::SymbolKind::Local
                        | symbols::SymbolKind::Parameter
                )
            })
        {
            return Err("open operands do not close a requirement instance");
        }
        for reference in &operands.types {
            if let types::TypeReferenceNode::Named { symbol, .. } =
                self.type_reference_table.type_reference(*reference)
                && let Some(definition) = self.data_definition_by_symbol(*symbol)
                && (!definition.type_parameters.is_empty()
                    || !definition.lifetime_parameters.is_empty())
            {
                return Err("closed requirement operand omits a nominal telescope");
            }
        }
        // The present exact-application representation deliberately refuses
        // region-bearing operands; an erased carrier identity is not custody.
        for argument in trait_arguments.iter().chain(&local_arguments).copied() {
            if self
                .region_free_scoped_type_identity(
                    crate::type_identity::TypeIdentityRequest::ordinary(argument),
                    &[],
                )
                .is_none()
            {
                return Err("closed requirement needs a retained exact region application");
            }
        }
        if let Some(existing) = self.closed_requirement_applications().find(|existing| {
            existing.declaring_trait == declaring_trait
                && existing.template == template
                && existing.trait_arguments.len() == trait_arguments.len()
                && existing
                    .trait_arguments
                    .iter()
                    .zip(trait_arguments)
                    .all(|(left, right)| {
                        self.normalized_type_identity(*left)
                            == self.normalized_type_identity(*right)
                    })
                && self
                    .expression_table
                    .static_machine_application_bindings(existing.application)
                    .len()
                    == bindings.len()
                && self
                    .expression_table
                    .static_machine_application_bindings(existing.application)
                    .iter()
                    .zip(&bindings)
                    .all(|(left, right)| {
                        left.parameter == right.parameter
                            && match (&left.value, &right.value) {
                                (
                                    static_applications::StaticMachineBindingValue::Type(left),
                                    static_applications::StaticMachineBindingValue::Type(right),
                                )
                                | (
                                    static_applications::StaticMachineBindingValue::Const(left),
                                    static_applications::StaticMachineBindingValue::Const(right),
                                ) => {
                                    self.normalized_type_identity(*left)
                                        == self.normalized_type_identity(*right)
                                }
                                (left, right) => left == right,
                            }
                    })
        }) {
            return Ok(existing.signature.symbol);
        }
        let declared = self
            .trait_machine_signature(template)
            .filter(|(owner, _)| owner.symbol == declaring_trait)
            .map(|(_, signature)| signature.clone())
            .ok_or("closed requirement lost its authored signature")?;
        let outer_parameters = self
            .trait_definition_by_symbol(declaring_trait)
            .map(|owner| self.trait_type_parameters(owner).to_vec())
            .ok_or("closed requirement lost its declaring trait")?;
        if outer_parameters.len() != trait_arguments.len() {
            return Err("closed requirement has an incomplete trait application");
        }
        let parameters = self.state_signature_type_parameters(&declared).to_vec();
        if parameters.len() != bindings.len()
            || parameters
                .iter()
                .zip(&bindings)
                .any(|(parameter, binding)| parameter.symbol != binding.parameter)
        {
            return Err(
                "closed requirement needs exactly one binding per authored local parameter",
            );
        }
        let mut substitutions = CallableApplication {
            region_scope: template,
            source_binders: outer_parameters
                .iter()
                .map(|parameter| (parameter.name.clone(), parameter.symbol))
                .collect(),
            types: outer_parameters
                .iter()
                .zip(trait_arguments)
                .map(|(parameter, argument)| (parameter.symbol, *argument))
                .collect(),
            ..CallableApplication::default()
        };
        for (parameter, binding) in parameters.iter().zip(&bindings) {
            match (&parameter.kind, &binding.value) {
                (
                    data::TypeParameterKind::Type,
                    static_applications::StaticMachineBindingValue::Type(actual),
                )
                | (
                    data::TypeParameterKind::Const { .. } | data::TypeParameterKind::Value { .. },
                    static_applications::StaticMachineBindingValue::Const(actual),
                ) if actual.is_valid() => {
                    substitutions.types.push((parameter.symbol, *actual));
                }
                (
                    data::TypeParameterKind::Machine { .. },
                    static_applications::StaticMachineBindingValue::Machine(actual),
                )
                | (
                    data::TypeParameterKind::Implementation,
                    static_applications::StaticMachineBindingValue::Conformance(actual),
                ) => {
                    substitutions
                        .static_arguments
                        .push((parameter.symbol, actual.clone()));
                }
                _ => {
                    return Err(
                        "closed requirement binding has no represented complete static operand",
                    );
                }
            }
        }
        let instance = self.symbols.insert_generated_root_from(
            template,
            self.symbols.get(template).kind,
            declared.name.as_str(),
        );
        substitutions.symbols.push((template, instance));
        let mut closed = substitutions.signature(self, &declared)?;
        closed.type_parameters = HandleSpan::empty();
        let trait_arguments = self
            .type_reference_table
            .insert_type_reference_handles(trait_arguments.iter().copied());
        let local_arguments = self
            .type_reference_table
            .insert_type_reference_handles(local_arguments);
        self.applied_callable_signatures
            .insert(AppliedCallableSignature {
                owner: AppliedCallableOwner::RequirementInstance {
                    declaration: declaring_trait,
                    template,
                    trait_arguments,
                    local_arguments,
                    application,
                },
                signature: closed,
            });
        Ok(instance)
    }
}
