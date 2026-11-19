//! Compiler evaluations can request the value passed to one argument of a
//! particular direct call. Runtime and component execution install no observer.

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::scalar_contents;
use crate::{TerminalBoundaryStorageValue, TerminalExecution};
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, InvocationSite, MachineId, OperationId, ScalarType,
};
use terminal_psi::{BoundaryParameterKind, DirectCallParts, StructuralAccess};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TerminalCallObservation {
    pub caller: MachineId,
    pub site: InvocationSite,
    pub callee: MachineId,
    /// Zero-based source parameter position, including scalar and structural formals.
    pub argument: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ObservedTerminalCall {
    pub call: TerminalCallObservation,
    pub values: Vec<TerminalBoundaryStorageValue>,
}

pub(crate) struct CallObservations {
    selectors: Vec<SelectedArgument>,
    pub(crate) records: Vec<ObservedTerminalCall>,
    // Each recorded value owns storage until it becomes part of a successful
    // result. A failed invocation drops the observations with their leases.
    leases: Vec<EvaluationStorageLease>,
    cells: u64,
    bytes: u64,
    pub(crate) complete: bool,
    _selector_storage: EvaluationStorageLease,
}

pub(crate) struct ObservationAdmission {
    pub(crate) target: Option<String>,
}

#[derive(Clone, Copy)]
struct SelectedArgument {
    call: TerminalCallObservation,
    kind: BoundaryParameterKind,
    carrier_index: usize,
}

fn unsupported() -> TerminalInterpretError {
    TerminalInterpretError::UnsupportedSemanticVariant(
        "compiler call observation requires a direct call with a numeric or owned plain scalar-case argument",
    )
}

impl CallObservations {
    pub(crate) fn finish(
        &mut self,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<(), TerminalInterpretError> {
        if !self.complete {
            account
                .ok_or_else(unsupported)?
                .charge_result_custody(self.cells, self.bytes)
                .map_err(TerminalInterpretError::Storage)?;
            self.leases.clear();
            self.complete = true;
        }
        Ok(())
    }
}

impl TerminalExecution<'_> {
    pub(crate) fn select_call_observations(
        &mut self,
        calls: &[TerminalCallObservation],
    ) -> Result<(), TerminalInterpretError> {
        if self.call_observations.is_some() {
            return Err(unsupported());
        }
        if calls.is_empty() {
            return Ok(());
        }
        let account = self.account.as_ref().ok_or_else(unsupported)?;
        let count = u64::try_from(calls.len()).map_err(|_| malformed())?;
        let bytes = count
            .checked_mul(size_of::<SelectedArgument>() as u64)
            .ok_or_else(malformed)?;
        let storage = account
            .reserve_storage(count, bytes)
            .map_err(TerminalInterpretError::Storage)?;
        let mut selectors = Vec::with_capacity(calls.len());
        for selector in calls {
            let InvocationSite::Operation(id) = selector.site else {
                return Err(unsupported());
            };
            let caller = self.program.verified_machine(selector.caller)?;
            let call = caller
                .operation(id)
                .and_then(|operation| terminal_psi::direct_call_parts(&operation.kind))
                .filter(|call| call.callee == selector.callee)
                .ok_or_else(unsupported)?;
            let callee = self.program.verified_machine(call.callee)?;
            let position = selector.argument as usize;
            let kind = *callee
                .parameter_order
                .get(position)
                .ok_or_else(unsupported)?;
            let carrier_index = callee.parameter_order[..position]
                .iter()
                .filter(|prior| **prior == kind)
                .count();
            selectors.push(SelectedArgument {
                call: *selector,
                kind,
                carrier_index,
            });
        }
        selectors.sort_unstable_by_key(|s| s.call);
        if selectors
            .windows(2)
            .any(|pair| pair[0].call == pair[1].call)
        {
            return Err(unsupported());
        }
        self.call_observations = Some(Box::new(CallObservations {
            selectors,
            records: Vec::new(),
            leases: Vec::new(),
            cells: 0,
            bytes: 0,
            complete: false,
            _selector_storage: storage,
        }));
        Ok(())
    }

    pub(crate) fn observe_direct_call(
        &mut self,
        operation: OperationId,
        call: DirectCallParts<'_>,
    ) -> Result<(), TerminalInterpretError> {
        let site = InvocationSite::Operation(operation);
        let key = (self.current_machine, site, call.callee);
        let observations = self.call_observations.as_ref().ok_or_else(malformed)?;
        let selectors = &observations.selectors;
        let mut index =
            selectors.partition_point(|s| (s.call.caller, s.call.site, s.call.callee) < key);
        while let Some(selector) = self
            .call_observations
            .as_ref()
            .and_then(|o| o.selectors.get(index))
            .copied()
        {
            if (
                selector.call.caller,
                selector.call.site,
                selector.call.callee,
            ) != key
            {
                break;
            }
            self.capture_call_argument(selector, call)?;
            index += 1;
        }
        Ok(())
    }

    fn capture_call_argument(
        &mut self,
        selector: SelectedArgument,
        call: DirectCallParts<'_>,
    ) -> Result<(), TerminalInterpretError> {
        let callee = self.program.verified_machine(call.callee)?;
        let index = selector.carrier_index;
        enum ArgumentValue {
            Integer(crate::TerminalScalarValue),
            Case(crate::TerminalScalarCaseResult),
        }
        let observed = match selector.kind {
            BoundaryParameterKind::Scalar => {
                let scalar =
                    self.scalar_value(*call.arguments.get(index).ok_or_else(unsupported)?)?;
                if !matches!(scalar.scalar_type(), ScalarType::Integer(_)) {
                    return Err(unsupported());
                }
                ArgumentValue::Integer(scalar)
            }
            BoundaryParameterKind::Structural => {
                let argument = call
                    .structural_arguments
                    .get(index)
                    .ok_or_else(unsupported)?;
                let formal = callee
                    .structural_parameters
                    .get(index)
                    .ok_or_else(unsupported)?;
                if argument.access != StructuralAccess::Owned
                    || formal.access != StructuralAccess::Owned
                    || !argument.path.is_empty()
                    || self
                        .program
                        .catalogs
                        .opaque_representations
                        .iter()
                        .any(|r| {
                            r.opaque == formal.structural_type
                                || r.carrier == formal.structural_type
                        })
                {
                    return Err(unsupported());
                }
                let value =
                    self.resolve_live_projection(argument.place, &argument.path, argument.access)?;
                if value.structural_type != formal.structural_type {
                    return Err(unsupported());
                }
                let snapshot = self
                    .snapshot_scalar_case(&value, |cell| match cell {
                        crate::storage::scalar_storage::ScalarCell::Local(value) => Ok(*value),
                        crate::storage::scalar_storage::ScalarCell::Resident { .. } => {
                            Err(unsupported())
                        }
                    })?
                    .ok_or_else(unsupported)?;
                if snapshot.value().fields.iter().any(|field| {
                    !matches!(
                        field.contents,
                        crate::TerminalStorageContents::Scalar {
                            stored_scalar: ScalarType::Integer(_),
                            ..
                        }
                    )
                }) {
                    return Err(unsupported());
                }
                ArgumentValue::Case(snapshot)
            }
        };
        let fields = match &observed {
            ArgumentValue::Integer(_) => 0,
            ArgumentValue::Case(case) => case.value().fields.len(),
        };
        let cells = u64::try_from(fields)
            .ok()
            .and_then(|n| n.checked_add(2))
            .ok_or_else(malformed)?;
        let bytes = (size_of::<ObservedTerminalCall>() + size_of::<EvaluationStorageLease>())
            .checked_add(size_of::<TerminalBoundaryStorageValue>())
            .and_then(|n| {
                fields
                    .checked_mul(
                        size_of::<TerminalBoundaryStorageValue>()
                            + 2 * size_of::<CanonicalStructuralPathSegment>(),
                    )
                    .and_then(|fields| n.checked_add(fields))
            })
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(malformed)?;
        let account = self.account.as_ref().ok_or_else(unsupported)?;
        // The meter already charged the executed call. Retaining its arguments
        // consumes storage and result custody under the same invocation.
        let lease = account
            .reserve_storage(cells, bytes)
            .map_err(TerminalInterpretError::Storage)?;
        let values = match observed {
            ArgumentValue::Case(snapshot) => snapshot.value().canonical_contents(),
            ArgumentValue::Integer(scalar) => vec![TerminalBoundaryStorageValue {
                path: Vec::new(),
                contents: scalar_contents(scalar),
            }],
        };
        let observations = self.call_observations.as_mut().ok_or_else(malformed)?;
        observations.cells = observations
            .cells
            .checked_add(cells)
            .ok_or_else(malformed)?;
        observations.bytes = observations
            .bytes
            .checked_add(bytes)
            .ok_or_else(malformed)?;
        observations.records.push(ObservedTerminalCall {
            call: selector.call,
            values,
        });
        observations.leases.push(lease);
        Ok(())
    }
}

#[cfg(test)]
#[path = "call_observations_tests.rs"]
mod tests;
