//! Compare callable signatures before building bodies or proof witnesses.
//! Declaration matching checks exact static binders and types; bound callers
//! supply their own substitutions. Neither lowers a type to compare it.

use crate::signature::StateParameter;
use crate::types::TypeReferenceHandle;
use symbols::{RetainedDeclaration, SymbolHandle};

/// Declaration matching ignores whether expression syntax also names this
/// callable. Exact static binders, parameter modes and full result types choose
/// an overload; this comparison establishes no contract refinement.
pub(crate) fn realizes_signature(
    program: &crate::TypedTrees,
    machine: &crate::machine::Machine,
    required_statics: arena::RootRows<'_, crate::data::TypeParameter>,
    required_parameters: arena::RootRows<'_, StateParameter>,
    result: TypeReferenceHandle,
) -> bool {
    let Some(state) = program.machine_states(machine).first() else {
        return false;
    };
    let Some((machine_binders, required_binders)) =
        realization_static_binders(program, machine, required_statics)
    else {
        return false;
    };
    let actual_parameters = program.state_parameters(state);
    actual_parameters.len() == required_parameters.len()
        && actual_parameters
            .iter()
            .zip(required_parameters)
            .all(|(actual, required)| {
                actual.is_const == required.is_const
                    && actual.is_mutable == required.is_mutable
                    && program.type_identity(crate::type_identity::TypeIdentityRequest {
                        binders: &machine_binders,
                        ..crate::type_identity::TypeIdentityRequest::ordinary(actual.type_reference)
                    }) == program.type_identity(crate::type_identity::TypeIdentityRequest {
                        binders: &required_binders,
                        ..crate::type_identity::TypeIdentityRequest::ordinary(
                            required.type_reference,
                        )
                    })
            })
        && state.return_type.is_valid() == result.is_valid()
        && (!result.is_valid()
            || program.type_identity(crate::type_identity::TypeIdentityRequest {
                binders: &machine_binders,
                ..crate::type_identity::TypeIdentityRequest::ordinary(state.return_type)
            }) == program.type_identity(crate::type_identity::TypeIdentityRequest {
                binders: &required_binders,
                ..crate::type_identity::TypeIdentityRequest::ordinary(result)
            }))
}
fn realization_static_binders(
    program: &crate::TypedTrees,
    machine: &crate::machine::Machine,
    required_parameters: arena::RootRows<'_, crate::data::TypeParameter>,
) -> Option<(Vec<(SymbolHandle, String)>, Vec<(SymbolHandle, String)>)> {
    let machine_parameters = program.machine_type_parameters(machine);
    if machine_parameters.len() != required_parameters.len() {
        return None;
    }
    let machine_binders = machine_parameters
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| (parameter.symbol, format!("${ordinal}")))
        .collect::<Vec<_>>();
    let required_binders = required_parameters
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| (parameter.symbol, format!("${ordinal}")))
        .collect::<Vec<_>>();
    for (machine_parameter, required_parameter) in
        machine_parameters.iter().zip(required_parameters)
    {
        match (&machine_parameter.kind, &required_parameter.kind) {
            (crate::data::TypeParameterKind::Type, crate::data::TypeParameterKind::Type)
                if !crate::data::type_parameter_demands_stronger_properties(
                    required_parameter,
                    machine_parameter,
                ) => {}
            (
                crate::data::TypeParameterKind::Const {
                    type_reference: machine_carrier,
                },
                crate::data::TypeParameterKind::Const {
                    type_reference: required_carrier,
                },
            ) if machine_parameter.bounds == required_parameter.bounds
                && program.type_identity(crate::type_identity::TypeIdentityRequest {
                    binders: &machine_binders,
                    ..crate::type_identity::TypeIdentityRequest::ordinary(*machine_carrier)
                }) == program.type_identity(crate::type_identity::TypeIdentityRequest {
                    binders: &required_binders,
                    ..crate::type_identity::TypeIdentityRequest::ordinary(*required_carrier)
                }) => {}
            _ => return None,
        }
    }
    Some((machine_binders, required_binders))
}

/// One source binding, before any body is read. The coordinator resolves the
/// slot and its source owner; Psi consumes declaration occurrences rather than
/// interpreting a target name. This records a shape obligation, not installation
/// authority, a provider choice, or an effect ceiling.
#[derive(Debug, Clone)]
pub struct BoundCallableShape {
    /// Diagnostic text only; selection has already resolved the declarations.
    pub slot: String,
    /// Diagnostic description of the resolved visible arrival contract.
    pub arrival_description: &'static str,
    pub source_span: Option<source::SourceSpan>,
    pub implementation: SymbolHandle,
    /// Coordinates in the producing typed context, used to retain the source
    /// closure before projection. The receipts below authenticate rejoining.
    pub semantic_schema_symbol: SymbolHandle,
    pub requirement_symbol: SymbolHandle,
    pub semantic_schema: RetainedDeclaration,
    pub requirement: RetainedDeclaration,
    pub visible_parameters: Vec<ParameterOccurrence>,
    /// The selected binding's exact mathematical address interpretation.
    pub address_model: Option<CallableAddressModel>,
}

pub use semantic_vocabulary::ScalarAddressModel as CallableAddressModel;
pub use symbols::ParameterOccurrence;

/// Caller construction reads the public declaration before selection has an
/// executable body. The view borrows the existing signature or machine; it
/// conveys no permission or refinement result.
#[derive(Clone, Copy)]
pub enum CallableDeclaration<'source> {
    Signature(&'source crate::signature::StateSignature),
    Machine(SymbolHandle),
}

impl<'source> CallableDeclaration<'source> {
    pub fn symbol(self) -> SymbolHandle {
        match self {
            Self::Signature(signature) => signature.symbol,
            Self::Machine(symbol) => symbol,
        }
    }

    pub fn shape(
        self,
        program: &'source crate::TypedTrees,
    ) -> Result<CallableSignatureShape<'source>, &'static str> {
        let (parameters, return_type, generic_arity) = match self {
            Self::Signature(signature) => (
                program.state_signature_parameters(signature),
                signature.return_type,
                program.state_signature_type_parameters(signature).len(),
            ),
            Self::Machine(symbol) => {
                let machine = program
                    .machine_by_symbol(symbol)
                    .ok_or("callable declaration lost its exact machine")?;
                let Some([state]) = program.machine_states(machine).as_array() else {
                    return Err("callable declaration has no unique entry state");
                };
                (
                    program.state_parameters(state),
                    state.return_type,
                    program.machine_type_parameters(machine).len(),
                )
            }
        };
        Ok(CallableSignatureShape {
            generic_arity,
            parameters,
            return_type,
        })
    }

    pub fn static_parameters(
        self,
        program: &'source crate::TypedTrees,
    ) -> Result<arena::RootRows<'source, crate::data::TypeParameter>, &'static str> {
        match self {
            Self::Signature(signature) => Ok(program.state_signature_type_parameters(signature)),
            Self::Machine(symbol) => program
                .machine_by_symbol(symbol)
                .map(|machine| program.machine_type_parameters(machine))
                .ok_or("callable declaration lost its exact machine"),
        }
    }

    pub fn lifetime_parameters(
        self,
        program: &'source crate::TypedTrees,
    ) -> Result<&'source [crate::name::Identifier], &'static str> {
        match self {
            Self::Signature(signature) => Ok(&signature.lifetime_parameters),
            Self::Machine(symbol) => program
                .machine_by_symbol(symbol)
                .map(|machine| machine.lifetime_parameters.as_slice())
                .ok_or("callable declaration lost its exact machine"),
        }
    }
}

impl crate::TypedTrees {
    /// Borrow the operand actually bound to this parameter, with the owner
    /// that supplied it. The specialization and source application must agree;
    /// this query describes the input and does not prove its conformance.
    pub fn selected_implementation_operand(
        &self,
        instance: SymbolHandle,
        parameter: SymbolHandle,
    ) -> Result<(&crate::expression::StaticMachineArgument, SymbolHandle), &'static str> {
        self.selected_implementation_receipt(instance, parameter)
            .map(|(_, operand, owner)| (operand, owner))
    }

    /// [`Self::selected_implementation_operand`], also returning the one
    /// specialization receipt that supplied it, so a caller that needs the
    /// receipt does not search the specialization rows a second time.
    fn selected_implementation_receipt(
        &self,
        instance: SymbolHandle,
        parameter: SymbolHandle,
    ) -> Result<
        (
            &crate::MachineSpecialization,
            &crate::expression::StaticMachineArgument,
            SymbolHandle,
        ),
        &'static str,
    > {
        use crate::static_applications::StaticMachineBindingValue;
        let mut receipts = self.machine_specializations.for_instance(instance);
        let receipt = receipts
            .next()
            .ok_or("selected parameter has no supplying specialization")?;
        if receipts.next().is_some()
            || !self
                .expression_table
                .static_machine_application_is_valid(receipt.binding_application)
        {
            return Err("selected parameter has an ambiguous or stale supplying application");
        }
        let source = self.static_machine_application(receipt.binding_application);
        if source.binding_declaration != receipt.template
            || !self
                .symbols
                .symbols()
                .nodes()
                .is_valid(source.lexical_owner)
        {
            return Err("selected parameter lost its exact source declaration");
        }
        let StaticMachineBindingValue::Conformance(operand) = self
            .specialization_binding(receipt, parameter)
            .ok_or("selected parameter has no unique retained binding")?
        else {
            return Err("selected parameter was not bound to an implementation operand");
        };
        Ok((receipt, operand, source.lexical_owner))
    }

    /// Read the public requirement chosen by the existing named-member or
    /// implementation-parameter owner, before asking for an executable body.
    pub fn source_call_requirement_signature<'source>(
        &'source self,
        owner: SymbolHandle,
        call: crate::references::StaticArgumentCall<'_>,
    ) -> Result<&'source crate::signature::StateSignature, String> {
        let source = self.source_call_requirement_declaration(owner, call)?;
        Ok(self
            .selected_requirement_invocation_signature(owner, call.application(), source)?
            .unwrap_or(source))
    }

    fn source_call_requirement_declaration<'source>(
        &'source self,
        owner: SymbolHandle,
        call: crate::references::StaticArgumentCall<'_>,
    ) -> Result<&'source crate::signature::StateSignature, String> {
        for application in call.applications() {
            if !self
                .expression_table
                .static_machine_application_is_valid(application)
            {
                return Err("public call has a stale source application".into());
            }
        }
        let (target, namespace, dispatch, authored_binding) = match call {
            crate::references::StaticArgumentCall::Expression(_, call) => (
                call.target_symbol,
                self.expression_table
                    .name_path_node(call.receiver)
                    .map(|path| path.symbol)
                    .unwrap_or_default(),
                call.static_requirement_dispatch.as_ref(),
                call.authored_call_binding,
            ),
            crate::references::StaticArgumentCall::Statement(call) => (
                call.target_symbol,
                call.receiver_symbol,
                call.static_requirement_dispatch.as_ref(),
                None,
            ),
        };
        if let Some(dispatch) = dispatch {
            // The receipt is the unique one the operand query authenticated.
            let (receipt, _, _) =
                self.selected_implementation_receipt(owner, dispatch.evidence_parameter)?;
            let original = authored_binding
                .map(|binding| binding.original)
                .unwrap_or_else(|| call.registration_operation(self));
            if !self.symbols.symbols().nodes().is_valid(original)
                || self.symbols.get(original).parent != dispatch.evidence_parameter
            {
                return Err("public call lost its exact authored parameter member".into());
            }
            let bound = self
                .evidence_bound_requirement(
                    receipt.template,
                    dispatch.evidence_parameter,
                    original,
                )?
                .ok_or("public call lost its exact authored requirement bound")?;
            if bound.signature.symbol != dispatch.requirement
                || bound.application.declaration.symbol != dispatch.declaring_trait
            {
                return Err("public call changed its declaring trait or requirement".into());
            }
            return self
                .applied_callable_signature(&bound.application, bound.signature.symbol)
                .or_else(|| {
                    bound
                        .application
                        .keeps_declared_signatures(self)
                        .then_some(bound.signature)
                })
                .ok_or_else(|| "public call has no complete applied requirement signature".into());
        }
        let named = match call {
            crate::references::StaticArgumentCall::Expression(_, call) => {
                self.named_conformance_call_member(call)?
            }
            crate::references::StaticArgumentCall::Statement(_) => {
                self.named_conformance_member(namespace, target)?
            }
        };
        if let Some((_, _, signature)) = named {
            return Ok(signature);
        }
        if let Some(application) = self.closed_requirement_application(target) {
            return Ok(application.signature);
        }
        let (declaration, signature) = self
            .trait_machine_signature(target)
            .ok_or("public call has no exact retained requirement declaration")?;
        if !declaration.type_parameters.is_empty() || !declaration.lifetime_parameters.is_empty() {
            return Err("public call needs its complete applied declaring trait".into());
        }
        Ok(signature)
    }

    /// Borrow the implementation operand that supplied this retained call.
    /// A parameter keeps its original binding scope; a named call keeps its
    /// receiver application. This describes a selection without accepting it.
    pub fn source_call_selected_operand<'source>(
        &'source self,
        owner: SymbolHandle,
        call: crate::references::StaticArgumentCall<'source>,
    ) -> Result<
        (
            std::borrow::Cow<'source, crate::expression::StaticMachineArgument>,
            SymbolHandle,
        ),
        String,
    > {
        use crate::expression::StaticMachineArgument;
        use crate::references::StaticArgumentCall;
        let (namespace, dispatch, receiver) = match call {
            StaticArgumentCall::Expression(_, call) => (
                self.expression_table
                    .name_path_node(call.receiver)
                    .map_or_else(SymbolHandle::invalid, |path| path.symbol),
                call.static_requirement_dispatch.as_ref(),
                Some(call.receiver),
            ),
            StaticArgumentCall::Statement(call) => (
                call.receiver_symbol,
                call.static_requirement_dispatch.as_ref(),
                None,
            ),
        };
        let lexical_owner = call.lexical_owner(self);
        let owner = if lexical_owner.is_valid() {
            lexical_owner
        } else {
            owner
        };
        if !self.symbols.symbols().nodes().is_valid(owner) {
            return Err("selected call lost its supplying declaration".into());
        }
        if let Some(dispatch) = dispatch {
            let (operand, supplying) =
                self.selected_implementation_operand(owner, dispatch.evidence_parameter)?;
            self.static_operand_region_supplier(operand, supplying)?;
            return Ok((std::borrow::Cow::Borrowed(operand), supplying));
        }
        let target = match call {
            StaticArgumentCall::Expression(_, call) => call.target_symbol,
            StaticArgumentCall::Statement(call) => call.target_symbol,
        };
        let member = match call {
            StaticArgumentCall::Expression(_, call) => self.named_conformance_call_member(call)?,
            StaticArgumentCall::Statement(_) => self.named_conformance_member(namespace, target)?,
        };
        member.ok_or("selected call lost its exact named requirement row")?;
        let operand = if let StaticArgumentCall::Expression(_, call) = call
            && call.requirement_application.is_valid()
        {
            let mut operand =
                self.static_operand_from_type_reference(call.requirement_application)?;
            operand.type_reference = call.requirement_application;
            std::borrow::Cow::Owned(operand)
        } else if let Some(receiver) = receiver {
            let path = self
                .expression_table
                .name_path_node(receiver)
                .ok_or("selected call lost its named receiver")?;
            if path.static_application.is_valid() {
                std::borrow::Cow::Borrowed(
                    self.expression_table
                        .static_member(path.static_application)
                        .ok_or("selected call has a stale receiver application")?,
                )
            } else {
                std::borrow::Cow::Owned(self.declaration_operand_from_expression(receiver)?)
            }
        } else {
            // The named-row query above serves only nongeneric maps here;
            // the statement form therefore has no unrepresented outer tuple.
            std::borrow::Cow::Owned(StaticMachineArgument {
                symbol: namespace,
                ..Default::default()
            })
        };
        if operand.symbol != namespace {
            return Err("selected call receiver changed its retained implementation".into());
        }
        let conformance = self
            .conformance_by_symbol(namespace)
            .ok_or("selected call receiver lost its implementation declaration")?;
        if operand.const_literal.is_some()
            || operand.evidence_projection.is_some()
            || operand.application.as_ref().is_some_and(|application| {
                application.member.is_valid()
                    || application.arguments.len()
                        != self.conformance_type_parameters(conformance).len()
                    || application.lifetime_arguments.len() != conformance.lifetime_parameters.len()
            })
        {
            return Err("selected call receiver lost its complete implementation tuple".into());
        }
        self.static_operand_region_supplier(&operand, owner)?;
        Ok((operand, owner))
    }

    pub fn source_call_declaration<'source>(
        &'source self,
        owner: SymbolHandle,
        call: crate::references::StaticArgumentCall<'_>,
    ) -> Result<CallableDeclaration<'source>, String> {
        for application in call.applications() {
            if !self
                .expression_table
                .static_machine_application_is_valid(application)
            {
                return Err("callable declaration has a stale source application".into());
            }
        }
        let (target, dispatch) = match call {
            crate::references::StaticArgumentCall::Expression(_, call) => (
                call.target_symbol,
                call.static_requirement_dispatch.is_some(),
            ),
            crate::references::StaticArgumentCall::Statement(call) => (
                call.target_symbol,
                call.static_requirement_dispatch.is_some(),
            ),
        };
        if !self.symbols.symbols().nodes().is_valid(target) {
            return Err("source call has a stale callable declaration".into());
        }
        if dispatch || self.trait_machine_signature(target).is_some() {
            return self
                .source_call_requirement_signature(owner, call)
                .map(CallableDeclaration::Signature);
        }
        let machine = self
            .machine_by_symbol(target)
            .or_else(|| self.machine_holding_state(target))
            .ok_or("source call has no retained callable declaration")?;
        let Some([state]) = self.machine_states(machine).as_array() else {
            return Err("source call has no unique declared entry state".into());
        };
        if target != machine.symbol && target != state.symbol {
            return Err("source call does not name its declared entry state".into());
        }
        Ok(CallableDeclaration::Machine(machine.symbol))
    }
}

#[derive(Clone, Copy)]
pub struct CallableSignatureShape<'a> {
    pub generic_arity: usize,
    pub parameters: arena::RootRows<'a, StateParameter>,
    pub return_type: TypeReferenceHandle,
}

pub use semantic_vocabulary::CallableShapeMismatch;

/// Preserve declaration order and distinguish a known mismatch from a failed
/// type-identity query. A missing identity is not evidence of a source error.
pub fn compare_callable_signature_shapes<Error>(
    required: CallableSignatureShape<'_>,
    actual: CallableSignatureShape<'_>,
    mut types_match: impl FnMut(TypeReferenceHandle, TypeReferenceHandle) -> Result<bool, Error>,
) -> Result<Vec<CallableShapeMismatch>, Error> {
    if required.generic_arity != actual.generic_arity {
        return Ok(vec![CallableShapeMismatch::GenericArity {
            required: required.generic_arity,
            actual: actual.generic_arity,
        }]);
    }
    if required.parameters.len() != actual.parameters.len() {
        return Ok(vec![CallableShapeMismatch::ParameterArity {
            required: required.parameters.len(),
            actual: actual.parameters.len(),
        }]);
    }
    let mut mismatches = Vec::new();
    for (position, (required, actual)) in required
        .parameters
        .iter()
        .zip(actual.parameters)
        .enumerate()
    {
        if required.is_mutable != actual.is_mutable
            || required.is_const != actual.is_const
            || required.relevance != actual.relevance
        {
            mismatches.push(CallableShapeMismatch::ParameterMode { position });
            continue;
        }
        if !types_match(actual.type_reference, required.type_reference)? {
            mismatches.push(CallableShapeMismatch::ParameterType { position });
        }
    }
    if !types_match(actual.return_type, required.return_type)? {
        mismatches.push(CallableShapeMismatch::ReturnType);
    }
    Ok(mismatches)
}

#[cfg(test)]
mod invocation_lifetime_tests {
    use super::CallableDeclaration;
    use crate::type_identity::LifetimeParameterIdentity as Region;
    use crate::{TypedTrees, machine::Machine, name::Identifier, types::TypeReferenceNode};
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

    #[test]
    fn invocation_inputs_rebind_exact_nested_regions_and_preserve_foreign_owners() {
        let mut symbols = SymbolTableBuilder::default();
        let source = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("source"));
        let caller = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("caller"));
        let foreign = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("foreign"));
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let telescope = vec![Identifier::from("items"), Identifier::from("bytes")];
        program.push_machine(Machine {
            symbol: source,
            lifetime_parameters: telescope.clone(),
            ..Default::default()
        });
        program.push_machine(Machine {
            symbol: foreign,
            lifetime_parameters: vec![Identifier::from("bytes")],
            ..Default::default()
        });
        let caller = Machine {
            symbol: caller,
            lifetime_parameters: telescope,
            ..Default::default()
        };
        program.push_machine(caller.clone());
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let inner = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: unit,
                access: language_core::ReferenceAccess::Shared,
                lifetime: Some(Identifier::from("bytes")),
            });
        let sequence = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice {
                element_type: inner,
            });
        let outer = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: sequence,
                access: language_core::ReferenceAccess::Shared,
                lifetime: Some(Identifier::from("items")),
            });
        for (reference, ordinal) in [(outer, 0), (inner, 1)] {
            program
                .type_reference_table
                .retain_operand_lifetime_owner(reference, source)
                .unwrap();
            program
                .type_reference_table
                .retain_operand_lifetimes(
                    reference,
                    [Region {
                        declaration: source,
                        ordinal,
                    }],
                )
                .unwrap();
        }
        for foreign_inner in [false, true] {
            if foreign_inner {
                program
                    .type_reference_table
                    .retain_operand_lifetimes(
                        inner,
                        [Region {
                            declaration: foreign,
                            ordinal: 0,
                        }],
                    )
                    .unwrap();
            }
            let mut actual_caller = caller.clone();
            let (copied, _) = program
                .instantiate_invocation_signature(
                    &[outer],
                    unit,
                    &[],
                    CallableDeclaration::Machine(source),
                    &mut actual_caller,
                )
                .unwrap();
            let copied = copied[0];
            assert_eq!(
                program.type_reference_table.operand_lifetimes(copied),
                &[Region {
                    declaration: caller.symbol,
                    ordinal: 0
                }]
            );
            let TypeReferenceNode::Reference { referee, .. } =
                program.type_reference_table.type_reference(copied)
            else {
                panic!("outer reference")
            };
            let TypeReferenceNode::Slice { element_type } =
                program.type_reference_table.type_reference(*referee)
            else {
                panic!("nested slice")
            };
            assert_eq!(
                program
                    .type_reference_table
                    .operand_lifetimes(*element_type),
                &[Region {
                    declaration: if foreign_inner {
                        foreign
                    } else {
                        caller.symbol
                    },
                    ordinal: usize::from(!foreign_inner),
                }]
            );
            assert_eq!(
                program.type_reference_table.operand_lifetimes(outer),
                &[Region {
                    declaration: source,
                    ordinal: 0
                }]
            );
        }
        let mut incomplete = caller.clone();
        incomplete.lifetime_parameters.pop();
        assert_eq!(
            program
                .instantiate_invocation_signature(
                    &[outer],
                    unit,
                    &[],
                    CallableDeclaration::Machine(source),
                    &mut incomplete,
                )
                .unwrap_err(),
            "generated caller has a different lifetime telescope"
        );
    }

    #[test]
    fn invocation_data_members_use_the_enclosing_application_region_order() {
        let mut symbols = SymbolTableBuilder::default();
        let data = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Envelope"));
        let caller = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("caller"));
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        program.push_data_definition(crate::data::DataDefinition {
            symbol: data,
            name: Identifier::from("Envelope"),
            lifetime_parameters: vec![Identifier::from("stored"), Identifier::from("backing")],
            ..Default::default()
        });
        program.push_machine(Machine {
            symbol: caller,
            lifetime_parameters: vec![Identifier::from("bytes"), Identifier::from("items")],
            ..Default::default()
        });
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let inner = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: unit,
                access: language_core::ReferenceAccess::Shared,
                lifetime: Some(Identifier::from("backing")),
            });
        let slice = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice {
                element_type: inner,
            });
        let outer = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: slice,
                access: language_core::ReferenceAccess::Shared,
                lifetime: Some(Identifier::from("stored")),
            });
        for (reference, ordinal) in [(outer, 0), (inner, 1)] {
            program
                .type_reference_table
                .retain_operand_lifetime_owner(reference, data)
                .unwrap();
            program
                .type_reference_table
                .retain_operand_lifetimes(
                    reference,
                    [Region {
                        declaration: data,
                        ordinal,
                    }],
                )
                .unwrap();
        }
        let application = program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: data,
                base_name: Identifier::from("Envelope"),
                lifetime_arguments: vec![Identifier::from("items"), Identifier::from("bytes")],
                arguments: Default::default(),
            });
        let actual = [
            Region {
                declaration: caller,
                ordinal: 1,
            },
            Region {
                declaration: caller,
                ordinal: 0,
            },
        ];
        program
            .type_reference_table
            .retain_operand_lifetime_owner(application, caller)
            .unwrap();
        program
            .type_reference_table
            .retain_operand_lifetimes(application, actual)
            .unwrap();
        let applied = program
            .instantiate_data_member_type(outer, application)
            .unwrap();
        assert_eq!(
            program.type_reference_table.operand_lifetimes(applied),
            &actual[..1]
        );
        let TypeReferenceNode::Reference { referee, .. } = program.type_reference_node(applied)
        else {
            panic!("outer view")
        };
        let TypeReferenceNode::Slice { element_type } = program.type_reference_node(*referee)
        else {
            panic!("typed sequence")
        };
        assert_eq!(
            program
                .type_reference_table
                .operand_lifetimes(*element_type),
            &actual[1..]
        );
        assert_eq!(
            program.type_reference_table.operand_lifetimes(outer),
            &[Region {
                declaration: data,
                ordinal: 0,
            }]
        );
    }

    #[test]
    fn invocation_elision_closes_the_input_tuple_before_relating_its_result() {
        let mut symbols = SymbolTableBuilder::default();
        let data = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("Holder"));
        let source = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("source"));
        let caller = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("caller"));
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        program.push_data_definition(crate::data::DataDefinition {
            symbol: data,
            name: Identifier::from("Holder"),
            lifetime_parameters: vec![Identifier::from("storage")],
            ..Default::default()
        });
        program.push_machine(Machine {
            symbol: source,
            ..Default::default()
        });
        let mut caller = Machine {
            symbol: caller,
            ..Default::default()
        };
        program.push_machine(caller.clone());
        let nominal = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: data,
                name: Identifier::from("Holder"),
            });
        let (inputs, result) = program
            .instantiate_invocation_signature(
                &[nominal],
                nominal,
                &[],
                CallableDeclaration::Machine(source),
                &mut caller,
            )
            .unwrap();
        assert_eq!(caller.lifetime_parameters.len(), 1);
        let expected = [Region {
            declaration: caller.symbol,
            ordinal: 0,
        }];
        for reference in [inputs[0], result] {
            assert_eq!(
                program.type_reference_table.operand_lifetimes(reference),
                &expected
            );
            let TypeReferenceNode::Generic {
                lifetime_arguments, ..
            } = program.type_reference_node(reference)
            else {
                panic!("closed nominal input")
            };
            assert_eq!(lifetime_arguments, &caller.lifetime_parameters);
        }
        program
            .type_reference_table
            .retain_operand_lifetimes(inputs[0], [Region::default()])
            .unwrap();
        assert!(
            program
                .instantiate_data_member_type(nominal, inputs[0])
                .is_err(),
            "a retained incomplete elision cannot revive a data binder by spelling"
        );
    }
}
