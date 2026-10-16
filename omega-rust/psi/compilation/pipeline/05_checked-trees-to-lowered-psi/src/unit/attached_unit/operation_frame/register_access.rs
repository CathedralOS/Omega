//! Observable register accesses over the ordinary scalar operand/result lanes.

use super::OperationFrame;
use crate::unit::{
    CheckedScalarExpressionRole, LoweringError, Operation, OperationKind, OperationResult,
    ServiceReachSummary, ValueDeclaration, allocate_dense, lookup_service_id, terminal_scalar_type,
    unsupported,
};
use checked_trees::{CheckedCallScalarArgument, CheckedUnitCallCoordinate};
use language_core::inline_assembly::AsmRegisterOperation;
use typed_trees::types::PrimitiveType;

impl OperationFrame<'_, '_> {
    pub(super) fn register_access(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        coordinate: CheckedUnitCallCoordinate,
        operation: AsmRegisterOperation,
        result: Option<&checked_trees::CheckedUnitScalarResultBindingPlan>,
        arguments: &[CheckedCallScalarArgument],
        service_reach: &ServiceReachSummary,
    ) -> Result<(), LoweringError> {
        // The resolved builtin minted this plan with its canonical singleton
        // builtin service reach. Keep that exact symbol rather than resolving
        // a service spelling again during emission.
        let rows = &self.checked.facts.service_reaches.rows;
        let [service] = rows.services(service_reach.direct) else {
            return unsupported("register access lost its exact checked builtin service reach");
        };
        if !rows.services(service_reach.transitive).contains(service) {
            return unsupported("register access lost its transitive builtin service reach");
        }
        let service = lookup_service_id(self.service_ids, *service)?;
        let mut inputs = Vec::with_capacity(arguments.len());
        for (ordinal, argument) in arguments.iter().enumerate() {
            let argument_ordinal = u32::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("register access operand ordinal overflow")
            })?;
            let value = self.evaluation.source_value(
                identities,
                self.checked,
                self.machine,
                self.state,
                coordinate.statement_index,
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: coordinate.call_ordinal,
                    argument_ordinal,
                },
                argument,
                self.source_value_count,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
                self.calls,
            )?;
            inputs.push(value.id);
        }
        let event = match (operation, inputs.as_slice()) {
            (AsmRegisterOperation::ProcessorHalt, []) => {
                terminal_psi::RegisterAccessEvent::ProcessorHalt
            }
            (AsmRegisterOperation::SystemRead(register), []) => {
                terminal_psi::RegisterAccessEvent::Read { register }
            }
            (AsmRegisterOperation::SystemWrite(register), [value]) => {
                terminal_psi::RegisterAccessEvent::Write {
                    register,
                    value: *value,
                }
            }
            (AsmRegisterOperation::PortRead, [port]) => {
                terminal_psi::RegisterAccessEvent::PortRead { port: *port }
            }
            (AsmRegisterOperation::PortWrite, [port, value]) => {
                terminal_psi::RegisterAccessEvent::PortWrite {
                    port: *port,
                    value: *value,
                }
            }
            (AsmRegisterOperation::ModelSpecificRead, [index]) => {
                terminal_psi::RegisterAccessEvent::ModelSpecificRead { index: *index }
            }
            (AsmRegisterOperation::ModelSpecificWrite, [index, value]) => {
                terminal_psi::RegisterAccessEvent::ModelSpecificWrite {
                    index: *index,
                    value: *value,
                }
            }
            _ => return unsupported("register access disagrees with its evaluated operands"),
        };
        if operation.produces_value() != result.is_some()
            || result.is_some_and(|result| {
                operation.result_carrier().is_none_or(|carrier| {
                    PrimitiveType::from_name(carrier.name()) != Some(result.primitive_type)
                })
            })
        {
            return unsupported("register result binding disagrees with its scalar event");
        }
        let kind = OperationKind::RegisterAccess { service, event };
        if !operation.produces_value() {
            self.push_unit(kind);
            return Ok(());
        }
        let value = ValueDeclaration {
            qualifications: Default::default(),
            id: crate::unit::value_id(allocate_dense(self.next_value)?),
            scalar_type: terminal_scalar_type(
                PrimitiveType::from_name(
                    operation
                        .result_carrier()
                        .ok_or(LoweringError::Unsupported(
                            "register result has no scalar carrier",
                        ))?
                        .name(),
                )
                .ok_or(LoweringError::Unsupported(
                    "unknown register scalar carrier",
                ))?,
            )?,
        };
        let id = self.operations.allocate();
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: OperationResult::Scalar(value),
            kind,
        });
        self.bind_scalar_result(identities, coordinate, value)
    }
}
