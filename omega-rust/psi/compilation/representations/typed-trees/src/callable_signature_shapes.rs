//! Compare the calling shape before constructing a callable's body or proof
//! witness. Type correspondence remains the caller's operation: generic
//! applications supply their exact binder substitutions, and closed bindings
//! use resolved identities. Neither path needs to lower a type to compare it.

use crate::signature::StateParameter;
use crate::types::TypeReferenceHandle;
use symbols::{RetainedDeclaration, SymbolHandle};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterOccurrence {
    pub ordinal: usize,
    pub symbol: SymbolHandle,
}

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
                let [state] = program.machine_states(machine) else {
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
    ) -> Result<&'source [crate::data::TypeParameter], &'static str> {
        match self {
            Self::Signature(signature) => Ok(program.state_signature_type_parameters(signature)),
            Self::Machine(symbol) => program
                .machine_by_symbol(symbol)
                .map(|machine| program.machine_type_parameters(machine))
                .ok_or("callable declaration lost its exact machine"),
        }
    }

    pub fn has_lifetime_parameters(
        self,
        program: &crate::TypedTrees,
    ) -> Result<bool, &'static str> {
        match self {
            Self::Signature(signature) => Ok(!signature.lifetime_parameters.is_empty()),
            Self::Machine(symbol) => program
                .machine_by_symbol(symbol)
                .map(|machine| !machine.lifetime_parameters.is_empty())
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
        use crate::static_applications::StaticMachineBindingValue;
        let mut receipts = self
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.instance == instance);
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
        Ok((operand, source.lexical_owner))
    }

    /// Read the public requirement chosen by the existing named-member or
    /// implementation-parameter owner, before asking for an executable body.
    pub fn source_call_requirement_signature<'source>(
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
            self.selected_implementation_operand(owner, dispatch.evidence_parameter)?;
            let mut receipts = self
                .machine_specializations
                .iter()
                .filter(|receipt| receipt.instance == owner);
            let receipt = receipts
                .next()
                .ok_or("public call lost its supplying specialization")?;
            if receipts.next().is_some() {
                return Err("public call has several supplying specializations".into());
            }
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
        if let Some((_, _, signature)) = self.named_conformance_member(namespace, target)? {
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
        self.named_conformance_member(namespace, target)?
            .ok_or("selected call lost its exact named requirement row")?;
        let operand = if let Some(receiver) = receiver {
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
        let [state] = self.machine_states(machine) else {
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
    pub parameters: &'a [StateParameter],
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
