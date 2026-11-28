//! Reconstruct the selected case of an ordinary, local sum from previously
//! observed scalars. Only whole-value copies and moves preserve this fact.

use super::{Attempt, CellUpdate, Facts, Trace, Word, bind};
use semantic_vocabulary::{PlaceId, StructuralCaseId, StructuralFieldId, ValueId};
use terminal_psi::{
    AtomicAccessEvent, Operation, OperationKind, RecordFieldValue, StructuralOperationResult,
    TerminalModule,
};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct CaseValue {
    case: StructuralCaseId,
    fields: Vec<(StructuralFieldId, Word)>,
}

impl CaseValue {
    fn from_operation(
        operation: &OperationKind,
        cases: &Facts<PlaceId, Self>,
        word: impl Fn(ValueId) -> Option<Word>,
    ) -> Option<Self> {
        match operation {
            OperationKind::EstablishScalarCase {
                result_case,
                fields,
            } => Some(Self {
                case: *result_case,
                fields: fields
                    .iter()
                    .map(|field| Some((field.field, word(field.value)?)))
                    .collect::<Option<_>>()?,
            }),
            OperationKind::EstablishStructuralCase {
                result_case,
                fields,
            } => Some(Self {
                case: *result_case,
                fields: fields
                    .iter()
                    .map(|field| {
                        let RecordFieldValue::Scalar { value, .. } = field.value else {
                            return None;
                        };
                        Some((field.field, word(value)?))
                    })
                    .collect::<Option<_>>()?,
            }),
            OperationKind::StructuralLeafCopy { source, path } if path.is_empty() => {
                cases.get(source).cloned()
            }
            OperationKind::MoveStructuralValue { source } if source.path.is_empty() => {
                cases.get(&source.place).cloned()
            }
            _ => None,
        }
    }

    pub(super) fn is(&self, case: StructuralCaseId) -> bool {
        self.case == case
    }

    pub(super) fn payload(&self, selected: StructuralFieldId) -> Option<Word> {
        self.fields
            .iter()
            .find(|(field, _)| *field == selected)
            .map(|(_, word)| *word)
    }

    fn atomic_attempt(
        declaration: &terminal_psi::StructuralTypeDeclaration,
        result: &terminal_psi::StructuralOperationResult,
        custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
        outcome: language_core::atomic::AtomicCompareExchangeOutcomeCase,
        expected: super::Operand,
        proposal: super::Operand,
    ) -> Option<Self> {
        use language_core::atomic::{
            AtomicCompareExchangeOutcomeCase as Outcome,
            AtomicCompareExchangeOutcomePayload as Payload,
        };
        use terminal_psi::types::{atomic_once_outcome_cases, atomic_once_result_is_plain};
        if !atomic_once_result_is_plain(result, custody) {
            return None;
        }
        let cases = atomic_once_outcome_cases(declaration, expected.scalar_type(), custody)?;
        let schema = custody
            .outcome_identity
            .cases()
            .iter()
            .find(|schema| schema.case == outcome)?;
        let case = &cases[usize::from(schema.tag)];
        let fields = match schema.payload {
            None => Vec::new(),
            Some(payload) => {
                let [field] = case.fields.as_slice() else {
                    return None;
                };
                let value = match payload {
                    Payload::Proposed => Word::from(proposal),
                    Payload::Observed if outcome == Outcome::Mismatched => Word::Observed,
                    Payload::Observed | Payload::Displaced => Word::from(expected),
                };
                vec![(field.id, value)]
            }
        };
        Some(Self {
            case: case.id,
            fields,
        })
    }
}

impl Trace {
    pub(super) fn case_result(
        &mut self,
        module: &TerminalModule,
        machine: &terminal_psi::TerminalMachine,
        operation: &Operation,
        result: &StructuralOperationResult,
        outcome: Attempt,
    ) -> Option<bool> {
        let declared = module.structural_type(result.structural_type)?;
        if declared.nominal_cleanup.is_some() || declared.owns_linear_claim {
            return None;
        }
        if matches!(operation.kind, OperationKind::EstablishStructuralLocal {})
            && result.is_affine_plain()
        {
            bind(&mut self.vacant, result.place, ())?;
            return Some(false);
        }
        if let OperationKind::AtomicAccess {
            place,
            path,
            resident_obligation: None,
            event:
                AtomicAccessEvent::CompareExchangeOnce {
                    expected,
                    replacement,
                    custody,
                    ..
                },
        } = &operation.kind
        {
            let expected = self.scalar(*expected)?.operand()?;
            let replacement = self.scalar(*replacement)?.operand()?;
            let case = CaseValue::atomic_attempt(
                declared,
                result,
                *custody,
                outcome,
                expected,
                replacement,
            )?;
            self.observe(
                operation.id,
                self.subject(module, machine, *place, path)?,
                super::AtomicWordUpdate::Stable(CellUpdate::CompareExchange {
                    expected,
                    replacement,
                }),
                true,
            )?;
            self.test(super::Comparison {
                pivot: expected,
                order: std::cmp::Ordering::Equal,
                mask: None,
            })?;
            bind(&mut self.cases, result.place, case)?;
            return Some(true);
        }
        if !(result.is_unrestricted_plain()
            || result.is_affine_plain()
                && matches!(operation.kind, OperationKind::MoveStructuralValue { .. }))
        {
            return None;
        }
        let case = CaseValue::from_operation(&operation.kind, &self.cases, |id| self.scalar(id))?;
        if let OperationKind::MoveStructuralValue { source } = &operation.kind {
            self.cases.remove(&source.place)?;
        }
        bind(&mut self.cases, result.place, case)?;
        Some(false)
    }
}
