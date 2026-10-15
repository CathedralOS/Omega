//! Bind checked port events to the exact selected instruction span.
//! This retains source service custody across placement; downstream physical
//! review independently rejoins the same event and checks final image bytes.
use super::{Error, attribution, host, source};
use crate::ObjectPortEffect;
use abstract_operations::AbstractOperation;
use machine_code::{PortEffectRecord, SemanticCodeSite};
use object_file::StagedOptimizedRelocationFreeObjectContainer;
use selected_instructions::{MachineAlternativeFamily, MachineAlternativeKey};

pub(super) fn produce(
    container: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<Vec<ObjectPortEffect>, Error> {
    let mut effects = Vec::new();
    for placed in &container.source().text_section().functions {
        let (function, _) = source::function(container, placed.machine)?;
        let fragment = source::fragments(container)
            .function(placed.machine)
            .ok_or(Error::Mismatch(
                "port event lost its current function fragment",
            ))?;
        for operation in &function.operations {
            let AbstractOperation::PortWrite {
                psi_operation,
                service,
                port,
                value,
            } = operation
            else {
                continue;
            };
            let mut matching = fragment.instructions().filter(|span| {
                span.alternative
                    == MachineAlternativeKey {
                        family: MachineAlternativeFamily::PortWrite,
                        variant: 0,
                    }
                    && span.provenance.operations.as_slice() == [*psi_operation]
            });
            let span = matching
                .next()
                .ok_or(Error::Mismatch("port event has no exact selected span"))?;
            if matching.next().is_some() {
                return Err(Error::Mismatch("port event has two selected spans"));
            }
            let code_offset = host(span.offset)?;
            effects.push(ObjectPortEffect {
                machine: placed.machine,
                text_offset: host(placed.section_offset)?
                    .checked_add(code_offset)
                    .ok_or(Error::Overflow)?,
                effect: PortEffectRecord {
                    psi_operation: *psi_operation,
                    service: *service,
                    port: *port,
                    value: *value,
                    operation_ordinal: attribution::ordinal(
                        function,
                        SemanticCodeSite::Operation(*psi_operation),
                    )?,
                    code_offset,
                    byte_count: span.bytes.len(),
                },
            });
        }
    }
    Ok(effects)
}
