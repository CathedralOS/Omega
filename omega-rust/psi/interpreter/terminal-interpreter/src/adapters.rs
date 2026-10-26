//! Checked host adapters: the typed binding surface embedders implement.
//!
//! The embedding contract requires the host to supply typed adapters and
//! descriptions rather than a private dispatch system. An adapter declares
//! the exact boundary signature it implements; `TerminalInstance::bind_import`
//! compares that signature to the demanded slot's declared contract before
//! the binding is established, so a wrong, missing, or incompatible host
//! object rejects before any guest effect runs. Once bound, the interpreter
//! routes `BoundaryCall` effects for that exact slot to that adapter — two
//! slots carrying the same requirement trait keep independent hosts because
//! dispatch keys on the slot, never on a type or name.
//!
//! `TerminalScalarImport` and `TerminalUnitImport` cover the common leaf
//! contracts; hosts with structural imports implement
//! [`TerminalImportAdapter`] directly. The raw [`TerminalEffectHandler`]
//! trait remains the low-level hook; adapters are the checked surface built
//! over it.

use crate::effects::results::TerminalEffectResult;
use crate::effects::{TerminalEffect, TerminalEffectHandler, TerminalEffectRejection};
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{ScalarType, StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult, ExternalCompletionKind,
    StructuralAccess, StructuralMultiplicity, StructuralPathQualification,
};

/// The exact bindable signature of one import slot, projected from its
/// declared boundary contract into the form a host adapter matches.
///
/// Equality is contract equality: scalar lane order and types, structural
/// lane order with type, multiplicity, access and qualifications, and the
/// completion contract and closed result. A host implementation differing in
/// any of these is a different requirement and must not bind — the same rule
/// the verifier applies to provider conformance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalImportSignature {
    /// Completion is independent of the transitional result shape. An external
    /// completion with a Unit result cannot bind a normal-return host adapter.
    pub completion: BoundaryCompletion,
    /// Ordered scalar formals. Boundary calls bind scalar arguments
    /// positionally in exactly this order.
    pub scalar_parameters: Vec<ScalarType>,
    /// Ordered structural formals, positional independently of the scalar
    /// lane.
    pub structural_parameters: Vec<TerminalImportStructuralParameter>,
    /// The closed result the host must produce.
    pub result: BoundaryMachineResult,
    /// Exact normal-return membership preservation supplied by this import.
    pub normal_qualifications: Vec<terminal_psi::StructuralDomainRequirement>,
}

/// One structural formal of an import signature, without the proof-local
/// place identity that never reaches the runtime boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalImportStructuralParameter {
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub access: StructuralAccess,
    /// Signature preconditions the caller establishes, checked at admission
    /// before the host runs.
    pub qualifications: Vec<StructuralDomainId>,
    /// Qualification preconditions rooted beneath this parameter.
    pub projected_qualifications: Vec<StructuralPathQualification>,
}

impl From<&BoundaryMachineDeclaration> for TerminalImportSignature {
    fn from(declaration: &BoundaryMachineDeclaration) -> Self {
        Self {
            completion: declaration.completion,
            scalar_parameters: declaration.scalar_parameters.clone(),
            structural_parameters: declaration
                .structural_parameters
                .iter()
                .map(|parameter| TerminalImportStructuralParameter {
                    structural_type: parameter.structural_type,
                    multiplicity: parameter.multiplicity,
                    access: parameter.access,
                    qualifications: parameter.qualifications.clone(),
                    projected_qualifications: parameter.projected_qualifications.clone(),
                })
                .collect(),
            result: declaration.result.clone(),
            normal_qualifications: declaration.normal_qualifications.clone(),
        }
    }
}

/// Why an adapter cannot be constructed for an import description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalAdapterError {
    /// The normal-return adapter cannot supply this external completion.
    ExternalCompletionUnsupported { kind: ExternalCompletionKind },
    /// The adapter covers scalar formals only; this import declares
    /// structural parameters.
    StructuralParametersUnsupported,
    /// The adapter covers a different closed result kind than this import
    /// declares.
    ResultKindUnsupported,
}

impl std::fmt::Display for TerminalAdapterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExternalCompletionUnsupported { kind } => write!(
                formatter,
                "external completion {kind:?} is outside this normal-return adapter's contract"
            ),
            Self::StructuralParametersUnsupported => write!(
                formatter,
                "import declares structural parameters outside this adapter's contract"
            ),
            Self::ResultKindUnsupported => write!(
                formatter,
                "import result kind is outside this adapter's contract"
            ),
        }
    }
}

impl std::error::Error for TerminalAdapterError {}

/// A host object that declares the exact boundary signature it implements.
///
/// `bind_import` compares `implemented_signature` to the slot's declared
/// contract before establishing the binding; the adapter's
/// [`TerminalEffectHandler`] implementation then receives this slot's
/// boundary calls through `handle_effect_result`. Adapters stay borrowed or
/// owned by the instance roster for the instance's life — they are the
/// checked counterpart of the interpreter's raw effect hook, not a
/// replacement for it.
pub trait TerminalImportAdapter: TerminalEffectHandler {
    /// The exact contract this host object implements, used for the bind-time
    /// conformance check.
    fn implemented_signature(&self) -> TerminalImportSignature;
}

/// Checked adapter for a scalar-result import slot: wraps a host function in
/// signature validation on both directions.
///
/// Each call first checks the operation's scalar argument list against the
/// bound signature — an argument list that does not match rejects before the
/// host function runs — then checks the returned value against the declared
/// result scalar. The interpreter repeats the result check against its own
/// catalog before committing, so a mismatched host result can never commit
/// even if a custom adapter skips its own check.
pub struct TerminalScalarImport<F> {
    signature: TerminalImportSignature,
    result_scalar_type: ScalarType,
    function: F,
}

impl<F> TerminalScalarImport<F>
where
    F: FnMut(&[TerminalScalarValue]) -> Result<TerminalScalarValue, TerminalEffectRejection>,
{
    /// Build the adapter for one exact import description. Rejects a slot
    /// whose contract carries structural parameters or a non-scalar result —
    /// those imports need a direct [`TerminalImportAdapter`] implementation.
    pub fn for_import(
        import: &BoundaryMachineDeclaration,
        function: F,
    ) -> Result<Self, TerminalAdapterError> {
        let signature = TerminalImportSignature::from(import);
        require_normal_completion(&signature)?;
        if !signature.structural_parameters.is_empty() {
            return Err(TerminalAdapterError::StructuralParametersUnsupported);
        }
        let result_scalar_type = signature
            .result
            .scalar()
            .ok_or(TerminalAdapterError::ResultKindUnsupported)?;
        Ok(Self {
            signature,
            result_scalar_type,
            function,
        })
    }
}

impl<F> TerminalImportAdapter for TerminalScalarImport<F>
where
    F: FnMut(&[TerminalScalarValue]) -> Result<TerminalScalarValue, TerminalEffectRejection>,
{
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl<F> TerminalEffectHandler for TerminalScalarImport<F>
where
    F: FnMut(&[TerminalScalarValue]) -> Result<TerminalScalarValue, TerminalEffectRejection>,
{
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "scalar-result import requires the result-aware dispatch",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            return Err(TerminalEffectRejection::new(
                "scalar import received a non-boundary effect",
            ));
        };
        check_scalar_arguments(&self.signature, arguments)?;
        let value = (self.function)(arguments)?;
        if value.scalar_type() != self.result_scalar_type {
            return Err(TerminalEffectRejection::new(
                "host result scalar does not match the bound signature",
            ));
        }
        Ok(TerminalEffectResult::Scalar(value))
    }
}

/// Checked adapter for a unit-result import slot: wraps a host function that
/// consumes the call's scalar arguments and produces no result value.
///
/// Structural-argument imports are out of scope here — `for_import` rejects
/// them — and a host function that rejects is surfaced as an unreconciled
/// effect before any guest state commits it.
pub struct TerminalUnitImport<F> {
    signature: TerminalImportSignature,
    function: F,
}

impl<F> TerminalUnitImport<F>
where
    F: FnMut(&[TerminalScalarValue]) -> Result<(), TerminalEffectRejection>,
{
    /// Build the adapter for one exact import description, rejecting slots
    /// with structural parameters or a non-unit result.
    pub fn for_import(
        import: &BoundaryMachineDeclaration,
        function: F,
    ) -> Result<Self, TerminalAdapterError> {
        let signature = TerminalImportSignature::from(import);
        require_normal_completion(&signature)?;
        if !signature.structural_parameters.is_empty() {
            return Err(TerminalAdapterError::StructuralParametersUnsupported);
        }
        if !signature.result.is_unit() {
            return Err(TerminalAdapterError::ResultKindUnsupported);
        }
        Ok(Self {
            signature,
            function,
        })
    }
}

impl<F> TerminalImportAdapter for TerminalUnitImport<F>
where
    F: FnMut(&[TerminalScalarValue]) -> Result<(), TerminalEffectRejection>,
{
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl<F> TerminalEffectHandler for TerminalUnitImport<F>
where
    F: FnMut(&[TerminalScalarValue]) -> Result<(), TerminalEffectRejection>,
{
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            return Err(TerminalEffectRejection::new(
                "unit import received a non-boundary effect",
            ));
        };
        check_scalar_arguments(&self.signature, arguments)?;
        (self.function)(arguments)
    }
}

fn require_normal_completion(
    signature: &TerminalImportSignature,
) -> Result<(), TerminalAdapterError> {
    match signature.completion {
        BoundaryCompletion::NormalReturn => Ok(()),
        BoundaryCompletion::External(kind) => {
            Err(TerminalAdapterError::ExternalCompletionUnsupported { kind })
        }
    }
}

/// The adapter-side argument check: the call's scalar argument list must
/// equal the bound signature's scalar lane in count and type. The
/// interpreter's own call validation already enforced this against the
/// declared boundary; repeating it here keeps a hand-built adapter honest
/// even if it is attached to an unexpected effect.
fn check_scalar_arguments(
    signature: &TerminalImportSignature,
    arguments: &[TerminalScalarValue],
) -> Result<(), TerminalEffectRejection> {
    if arguments.len() == signature.scalar_parameters.len()
        && arguments
            .iter()
            .zip(&signature.scalar_parameters)
            .all(|(argument, expected)| argument.scalar_type() == *expected)
    {
        Ok(())
    } else {
        Err(TerminalEffectRejection::new(
            "boundary call arguments do not match the bound signature",
        ))
    }
}
