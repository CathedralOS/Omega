//! Checked plans for proof-only float meaning projection.
//!
//! Canonical rows retain unique plan-local source identities, landed formats,
//! and closed projection-catalog operations. Authored occurrences and spans
//! live in a separate provenance table. Neither table contains runtime values
//! or authorizes float evaluation or native lowering.

use numerics::float_projection::FloatProjectionOperation;
use typed_trees::types::PrimitiveType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedProofValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedProofPropositionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedFloatProjectionInputId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedFloatMeaningProjectionOccurrenceId(pub u32);

pub type CheckedFloatProjectionContractIdentity =
    numerics::float_projection::FloatProjectionContractIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedProofOnlyValueType {
    FloatMeaning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedProofValueDeclaration {
    pub id: CheckedProofValueId,
    pub value_type: CheckedProofOnlyValueType,
}

/// One checked float-projection input coordinate. Equal validated source keys
/// share an ID assigned densely by first use. The retained row contains no
/// source symbol, expression handle, or runtime bits; binding it to a landed
/// artifact value remains a later producer step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedFloatProjectionInput {
    pub id: CheckedFloatProjectionInputId,
    pub primitive: PrimitiveType,
}

/// Exact checked provenance for a direct primitive parameter referenced by a
/// top-level machine contract. The source handles remain checked-only custody:
/// Terminal Psi deliberately receives `fallback` until it can represent an
/// IEEE scalar parameter without claiming general float execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedDirectMachineFloatParameter {
    pub owner_machine: symbols::SymbolHandle,
    pub parameter: symbols::SymbolHandle,
    pub fallback: CheckedFloatProjectionInput,
}

/// Exact checked provenance for the scalar result named by an owning
/// top-level machine `ensures` clause. Omega's reserved `result` spelling has
/// no source symbol of its own, so the owning machine plus exact primitive
/// format is the complete checked identity before Terminal binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedDirectMachineFloatResult {
    pub owner_machine: symbols::SymbolHandle,
    pub fallback: CheckedFloatProjectionInput,
}

/// Exact checked provenance for a direct primitive parameter of one nested
/// machine state. A nested state admits arrival `requires` only, so its
/// parameters are Terminal block parameters, never machine parameters or
/// machine results. The owning machine and owning state symbols remain
/// checked-only custody: Terminal lowering rejoins them to the emitted
/// machine/block tables and exact scalar parameter membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedDirectBlockFloatParameter {
    pub owner_machine: symbols::SymbolHandle,
    pub owner_state: symbols::SymbolHandle,
    pub parameter: symbols::SymbolHandle,
    pub fallback: CheckedFloatProjectionInput,
}

/// Exact checked provenance for an IEEE leaf below one top-level structural
/// machine parameter. Source handles remain checked-only; Terminal lowering
/// rejoins the owner, parameter position, and complete retained member path to
/// the emitted structural parameter/type tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDirectStructuralFloatLeaf {
    pub owner_machine: symbols::SymbolHandle,
    pub field: crate::CheckedStructuralParameterField,
    pub fallback: CheckedFloatProjectionInput,
}

/// Exact use-site coordinate where one transported `ensures` clause
/// instantiates: one scalar call or selected boundary operation inside the
/// owning machine's authored statement. The coordinate joins the checked use
/// to the emitted Terminal operation through the lowered occurrence tables;
/// it names a use of a contract, never a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedFloatUseSite {
    pub owner_machine: symbols::SymbolHandle,
    pub owner_state: symbols::SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
}

/// Exact checked provenance for the scalar result produced by one call use
/// inside the owning machine. A transported `ensures` clause names the
/// importing use's result, not one shared value per declaration, so `result`
/// carries the use-site coordinate. Source handles remain checked-only;
/// Terminal lowering rejoins the coordinate to the emitted call operation's
/// declared scalar result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedDirectCallFloatResult {
    pub use_site: CheckedFloatUseSite,
    pub fallback: CheckedFloatProjectionInput,
}

/// Exact checked provenance for the scalar result produced by one non-call
/// operation use inside the owning machine. `use_expression` retains the
/// authored operator-use occurrence: today's one non-call float producer
/// (selected IEEE fused multiply-add) occupies call ordinal zero of its
/// statement, so the authored expression keeps two uses sharing a statement
/// distinct before the lowered occurrence join runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedDirectOperationFloatResult {
    pub use_site: CheckedFloatUseSite,
    pub use_expression: typed_trees::expression::ExpressionHandle,
    pub fallback: CheckedFloatProjectionInput,
}

/// The semantic source retained for one checked float projection.
///
/// Exact literals carry their landed raw bits directly. The transitional
/// coordinate remains only for source forms whose artifact-relative carrier
/// has not landed; it must never be treated as an exact-literal identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedFloatProjectionSource {
    TransitionalInput(CheckedFloatProjectionInput),
    DirectMachineParameter(CheckedDirectMachineFloatParameter),
    DirectMachineResult(CheckedDirectMachineFloatResult),
    DirectBlockParameter(CheckedDirectBlockFloatParameter),
    DirectStructuralLeaf(CheckedDirectStructuralFloatLeaf),
    DirectCallResult(CheckedDirectCallFloatResult),
    DirectOperationResult(CheckedDirectOperationFloatResult),
    ExactBinary32Literal(u32),
    ExactBinary64Literal(u64),
}

impl CheckedFloatProjectionSource {
    pub const fn primitive(&self) -> PrimitiveType {
        match self {
            Self::TransitionalInput(input) => input.primitive,
            Self::DirectMachineParameter(parameter) => parameter.fallback.primitive,
            Self::DirectMachineResult(result) => result.fallback.primitive,
            Self::DirectBlockParameter(parameter) => parameter.fallback.primitive,
            Self::DirectStructuralLeaf(leaf) => leaf.fallback.primitive,
            Self::DirectCallResult(result) => result.fallback.primitive,
            Self::DirectOperationResult(result) => result.fallback.primitive,
            Self::ExactBinary32Literal(_) => PrimitiveType::F32,
            Self::ExactBinary64Literal(_) => PrimitiveType::F64,
        }
    }
}

/// Exact proof-only projection selected from the shared closed catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedFloatMeaningProjection {
    pub result: CheckedProofValueDeclaration,
    pub source: CheckedFloatProjectionSource,
    pub operation: FloatProjectionOperation,
    pub contract: CheckedFloatProjectionContractIdentity,
}

/// Diagnostic provenance for one authored projection call. Multiple
/// occurrences may name the same canonical proof value; the span is never
/// part of that value's semantic identity and is erased before Terminal Psi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedFloatMeaningProjectionOccurrence {
    pub id: CheckedFloatMeaningProjectionOccurrenceId,
    pub value: CheckedProofValueId,
    pub source_span: source::SourceSpan,
}

/// One proof-only equality whose operands are exact results in the retained
/// float-projection table. It is not a runtime Boolean or a machine operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedFloatMeaningEqualityProposition {
    pub id: CheckedProofPropositionId,
    pub left: CheckedProofValueId,
    pub right: CheckedProofValueId,
    /// Exact typed contract expression that authored this checked equality.
    /// This coordinate is used only to join exit checking back to the
    /// validated occurrence and is erased before Terminal Psi.
    pub source_expression: typed_trees::expression::ExpressionHandle,
    /// The exact use site where a transported `ensures` equality instantiated,
    /// or `None` for the authored declaration row. Imported instances share
    /// the declaration's `source_expression`, so the reflexivity rejoin keys
    /// (owner, expression, use site) to keep declaration rows disjoint from
    /// every instantiation and each instantiation distinct per use.
    pub use_site: Option<CheckedFloatUseSite>,
}

impl CheckedFloatMeaningProjection {
    /// Independently replay the checked source/result shapes before this row
    /// crosses into Terminal Psi.
    pub fn validate(&self) -> Result<(), CheckedFloatMeaningProjectionError> {
        if self.result.value_type != CheckedProofOnlyValueType::FloatMeaning {
            return Err(CheckedFloatMeaningProjectionError::ResultTypeMismatch);
        }
        let expected = match self.operation {
            FloatProjectionOperation::Meaning32 => PrimitiveType::F32,
            FloatProjectionOperation::Meaning64 => PrimitiveType::F64,
        };
        if self.source.primitive() != expected {
            return Err(CheckedFloatMeaningProjectionError::SourceFormatMismatch);
        }
        match &self.source {
            CheckedFloatProjectionSource::DirectMachineParameter(parameter)
                if !parameter.owner_machine.is_valid() || !parameter.parameter.is_valid() =>
            {
                return Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance);
            }
            CheckedFloatProjectionSource::DirectMachineResult(result)
                if !result.owner_machine.is_valid() =>
            {
                return Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance);
            }
            CheckedFloatProjectionSource::DirectBlockParameter(parameter)
                if !parameter.owner_machine.is_valid()
                    || !parameter.owner_state.is_valid()
                    || !parameter.parameter.is_valid() =>
            {
                return Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance);
            }
            CheckedFloatProjectionSource::DirectStructuralLeaf(leaf)
                if !leaf.owner_machine.is_valid() || leaf.field.path.is_empty() =>
            {
                return Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance);
            }
            CheckedFloatProjectionSource::DirectCallResult(result)
                if !result.use_site.owner_machine.is_valid()
                    || !result.use_site.owner_state.is_valid() =>
            {
                return Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance);
            }
            CheckedFloatProjectionSource::DirectOperationResult(result)
                if !result.use_site.owner_machine.is_valid()
                    || !result.use_site.owner_state.is_valid()
                    || !result.use_expression.is_valid() =>
            {
                return Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance);
            }
            _ => {}
        }
        if self.contract != self.operation.contract_identity() {
            return Err(CheckedFloatMeaningProjectionError::ContractIdentityMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedFloatMeaningProjectionError {
    ResultTypeMismatch,
    SourceFormatMismatch,
    InvalidSourceProvenance,
    ContractIdentityMismatch,
}

#[cfg(test)]
mod tests {
    use super::{
        CheckedDirectCallFloatResult, CheckedDirectMachineFloatParameter,
        CheckedDirectMachineFloatResult, CheckedDirectOperationFloatResult,
        CheckedFloatMeaningProjection, CheckedFloatMeaningProjectionError,
        CheckedFloatProjectionInput, CheckedFloatProjectionInputId, CheckedFloatProjectionSource,
        CheckedFloatUseSite, CheckedProofOnlyValueType, CheckedProofValueDeclaration,
        CheckedProofValueId, FloatProjectionOperation, PrimitiveType,
    };

    fn projection() -> CheckedFloatMeaningProjection {
        CheckedFloatMeaningProjection {
            result: CheckedProofValueDeclaration {
                id: CheckedProofValueId(3),
                value_type: CheckedProofOnlyValueType::FloatMeaning,
            },
            source: CheckedFloatProjectionSource::TransitionalInput(CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(7),
                primitive: PrimitiveType::F32,
            }),
            operation: FloatProjectionOperation::Meaning32,
            contract: FloatProjectionOperation::Meaning32.contract_identity(),
        }
    }

    #[test]
    fn checked_projection_replays_exact_format_without_source_handles() {
        let plan = projection();
        assert_eq!(plan.validate(), Ok(()));
        assert_eq!(plan.result.id, CheckedProofValueId(3));
        assert_eq!(
            plan.source,
            CheckedFloatProjectionSource::TransitionalInput(CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(7),
                primitive: PrimitiveType::F32,
            })
        );
    }

    #[test]
    fn checked_projection_rejects_cross_format_substitution() {
        let mut plan = projection();
        plan.operation = FloatProjectionOperation::Meaning64;
        assert_eq!(
            plan.validate(),
            Err(CheckedFloatMeaningProjectionError::SourceFormatMismatch)
        );
    }

    #[test]
    fn checked_projection_rejects_catalog_or_contract_substitution() {
        let mut plan = projection();
        plan.contract.catalog_version += 1;
        assert_eq!(
            plan.validate(),
            Err(CheckedFloatMeaningProjectionError::ContractIdentityMismatch)
        );
        plan = projection();
        plan.contract.commitment[0] ^= 1;
        assert_eq!(
            plan.validate(),
            Err(CheckedFloatMeaningProjectionError::ContractIdentityMismatch)
        );
    }

    #[test]
    fn checked_direct_parameter_requires_nonempty_owner_and_parameter_handles() {
        let mut plan = projection();
        plan.source = CheckedFloatProjectionSource::DirectMachineParameter(
            CheckedDirectMachineFloatParameter {
                owner_machine: symbols::SymbolHandle::from_arena_index(3),
                parameter: symbols::SymbolHandle::from_arena_index(5),
                fallback: CheckedFloatProjectionInput {
                    id: CheckedFloatProjectionInputId(7),
                    primitive: PrimitiveType::F32,
                },
            },
        );
        assert_eq!(plan.validate(), Ok(()));
        if let CheckedFloatProjectionSource::DirectMachineParameter(parameter) = &mut plan.source {
            parameter.owner_machine = symbols::SymbolHandle::invalid();
        }
        assert_eq!(
            plan.validate(),
            Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance)
        );
        if let CheckedFloatProjectionSource::DirectMachineParameter(parameter) = &mut plan.source {
            parameter.owner_machine = symbols::SymbolHandle::from_arena_index(3);
            parameter.parameter = symbols::SymbolHandle::invalid();
        }
        assert_eq!(
            plan.validate(),
            Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance)
        );
        if let CheckedFloatProjectionSource::DirectMachineParameter(parameter) = &mut plan.source {
            parameter.parameter = symbols::SymbolHandle::from_arena_index(5);
            parameter.fallback.primitive = PrimitiveType::I32;
        }
        assert_eq!(
            plan.validate(),
            Err(CheckedFloatMeaningProjectionError::SourceFormatMismatch)
        );
    }

    #[test]
    fn checked_direct_result_requires_nonempty_owner_and_exact_format() {
        let mut plan = projection();
        plan.source =
            CheckedFloatProjectionSource::DirectMachineResult(CheckedDirectMachineFloatResult {
                owner_machine: symbols::SymbolHandle::from_arena_index(3),
                fallback: CheckedFloatProjectionInput {
                    id: CheckedFloatProjectionInputId(7),
                    primitive: PrimitiveType::F32,
                },
            });
        assert_eq!(plan.validate(), Ok(()));
        if let CheckedFloatProjectionSource::DirectMachineResult(result) = &mut plan.source {
            result.owner_machine = symbols::SymbolHandle::invalid();
        }
        assert_eq!(
            plan.validate(),
            Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance)
        );
        if let CheckedFloatProjectionSource::DirectMachineResult(result) = &mut plan.source {
            result.owner_machine = symbols::SymbolHandle::from_arena_index(3);
            result.fallback.primitive = PrimitiveType::F64;
        }
        assert_eq!(
            plan.validate(),
            Err(CheckedFloatMeaningProjectionError::SourceFormatMismatch)
        );
    }

    #[test]
    fn checked_use_site_results_require_a_whole_use_coordinate() {
        let use_site = || CheckedFloatUseSite {
            owner_machine: symbols::SymbolHandle::from_arena_index(3),
            owner_state: symbols::SymbolHandle::from_arena_index(5),
            statement_index: 0,
            call_ordinal: 0,
        };
        let fallback = || CheckedFloatProjectionInput {
            id: CheckedFloatProjectionInputId(7),
            primitive: PrimitiveType::F32,
        };
        let mut plan = projection();
        plan.source =
            CheckedFloatProjectionSource::DirectCallResult(CheckedDirectCallFloatResult {
                use_site: use_site(),
                fallback: fallback(),
            });
        assert_eq!(plan.validate(), Ok(()));
        if let CheckedFloatProjectionSource::DirectCallResult(result) = &mut plan.source {
            result.use_site.owner_state = symbols::SymbolHandle::invalid();
        }
        assert_eq!(
            plan.validate(),
            Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance)
        );

        let mut plan = projection();
        plan.source = CheckedFloatProjectionSource::DirectOperationResult(
            CheckedDirectOperationFloatResult {
                use_site: use_site(),
                use_expression: typed_trees::expression::ExpressionHandle::from_arena_index(2),
                fallback: fallback(),
            },
        );
        assert_eq!(plan.validate(), Ok(()));
        if let CheckedFloatProjectionSource::DirectOperationResult(result) = &mut plan.source {
            result.use_expression = typed_trees::expression::ExpressionHandle::invalid();
        }
        assert_eq!(
            plan.validate(),
            Err(CheckedFloatMeaningProjectionError::InvalidSourceProvenance)
        );
    }
}
