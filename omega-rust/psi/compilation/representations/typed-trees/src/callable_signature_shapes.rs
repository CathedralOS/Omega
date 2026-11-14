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

    pub fn shape(self, program: &'source crate::TypedTrees) -> Result<CallableSignatureShape<'source>, &'static str> {
        let (parameters, return_type, generic_arity) = match self {
            Self::Signature(signature) => (
                program.state_signature_parameters(signature), signature.return_type,
                program.state_signature_type_parameters(signature).len(),
            ),
            Self::Machine(symbol) => {
                let machine = program.machine_by_symbol(symbol)
                    .ok_or("callable declaration lost its exact machine")?;
                let [state] = program.machine_states(machine) else {
                    return Err("callable declaration has no unique entry state");
                };
                (program.state_parameters(state), state.return_type, program.machine_type_parameters(machine).len())
            }
        };
        Ok(CallableSignatureShape { generic_arity, parameters, return_type })
    }

    pub fn static_parameters(self, program: &'source crate::TypedTrees) -> Result<&'source [crate::data::TypeParameter], &'static str> {
        match self {
            Self::Signature(signature) => Ok(program.state_signature_type_parameters(signature)),
            Self::Machine(symbol) => program.machine_by_symbol(symbol)
                .map(|machine| program.machine_type_parameters(machine))
                .ok_or("callable declaration lost its exact machine"),
        }
    }

    pub fn has_lifetime_parameters(self, program: &crate::TypedTrees) -> Result<bool, &'static str> {
        match self {
            Self::Signature(signature) => Ok(!signature.lifetime_parameters.is_empty()),
            Self::Machine(symbol) => program.machine_by_symbol(symbol)
                .map(|machine| !machine.lifetime_parameters.is_empty())
                .ok_or("callable declaration lost its exact machine"),
        }
    }
}

impl crate::TypedTrees {
    pub fn source_call_declaration<'source>(
        &'source self,
        owner: SymbolHandle,
        call: crate::references::StaticArgumentCall<'_>,
    ) -> Result<CallableDeclaration<'source>, &'static str> {
        for application in call.applications() {
            if !self.expression_table.static_machine_application_is_valid(application) {
                return Err("callable declaration has a stale source application");
            }
        }
        let (target, dispatch) = match call {
            crate::references::StaticArgumentCall::Expression(_, call) => (call.target_symbol, call.static_requirement_dispatch.is_some()),
            crate::references::StaticArgumentCall::Statement(call) => (call.target_symbol, call.static_requirement_dispatch.is_some()),
        };
        if dispatch || self.trait_machine_signature(target).is_some() {
            return self.resolve_requirement_static_call_signature(owner, call)
                .map(CallableDeclaration::Signature);
        }
        let machine = self.machine_by_symbol(target).or_else(|| self.machine_holding_state(target))
            .ok_or("source call has no retained callable declaration")?;
        let [state] = self.machine_states(machine) else {
            return Err("source call has no unique declared entry state");
        };
        if target != machine.symbol && target != state.symbol {
            return Err("source call does not name its declared entry state");
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
