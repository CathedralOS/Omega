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
    pub semantic_schema: RetainedDeclaration,
    pub requirement: RetainedDeclaration,
    pub visible_parameters: Vec<ParameterOccurrence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParameterOccurrence {
    pub ordinal: usize,
    pub symbol: SymbolHandle,
}

#[derive(Clone, Copy)]
pub struct CallableSignatureShape<'a> {
    pub generic_arity: usize,
    pub parameters: &'a [StateParameter],
    pub return_type: TypeReferenceHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallableShapeMismatch {
    GenericArity { required: usize, actual: usize },
    ParameterArity { required: usize, actual: usize },
    ParameterMode { position: usize },
    ParameterType { position: usize },
    ReturnType,
}

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
