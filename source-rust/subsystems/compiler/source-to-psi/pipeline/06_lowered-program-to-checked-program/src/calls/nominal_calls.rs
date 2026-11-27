//! Authenticate open nominal call correspondence before using call contracts.

use crate::{Elaboration, refusal};
use semantic_vocabulary::{MachineId, TargetMachineCallable};
use terminal_check_tables::{Checked, Construct, RefusalReason};
use terminal_psi::generic_calls::CallTypePosition;

pub(crate) fn validate(elaboration: &Elaboration) -> Checked<()> {
    if elaboration.nominal_types.is_empty() && elaboration.nominal_calls.is_empty() {
        return Ok(());
    }
    let module = &elaboration.module;
    if module
        .machines
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(refusal(
            RefusalReason::ForeignIdentity,
            MachineId::new(1).expect("one is nonzero"),
            Construct::Machine,
            "nominal calls require an ordered machine roster",
        ));
    }
    let rows = &elaboration.nominal_calls;
    let fault = |machine, operation, detail| {
        refusal(
            RefusalReason::ForeignIdentity,
            machine,
            Construct::Operation(operation),
            detail,
        )
    };
    if elaboration
        .nominal_types
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(refusal(
            RefusalReason::ForeignIdentity,
            MachineId::new(1).expect("one is nonzero"),
            Construct::Machine,
            "nominal input types are not uniquely ordered",
        ));
    }
    if rows.windows(2).any(|pair| {
        (pair[0].caller, pair[0].operation, pair[0].position)
            >= (pair[1].caller, pair[1].operation, pair[1].position)
    }) {
        let row = &rows[0];
        return Err(fault(
            row.caller,
            row.operation,
            "nominal call correspondence is duplicated or unordered",
        ));
    }
    for (owner, storage) in &elaboration.nominal_types {
        let valid = module
            .machines
            .binary_search_by_key(owner, |machine| machine.id)
            .ok()
            .and_then(|position| module.machines.get(position))
            .is_some_and(|machine| {
                (machine
                    .structural_parameters
                    .iter()
                    .any(|formal| formal.structural_type == *storage)
                    || machine
                        .result
                        .structural()
                        .is_some_and(|result| result.structural_type == *storage))
                    && elaboration
                        .value_parameters
                        .get(*owner)
                        .is_some_and(|count| *count > 0)
            });
        if !valid {
            return Err(refusal(
                RefusalReason::ForeignIdentity,
                *owner,
                Construct::Machine,
                "nominal type roster names no const-dependent parameter or result",
            ));
        }
    }
    let valid_key = |key| {
        module.identities.text(key).is_some_and(|text| {
            application_identity::type_identity::validate_source_type_description(text).is_ok()
        })
    };
    let mut observed = 0;
    for caller in &module.machines {
        for operation in caller.operations() {
            let Some(call) = operation.declared_invocation() else {
                continue;
            };
            let TargetMachineCallable::Machine(callee_id) = call.callee else {
                continue;
            };
            let callee = module
                .machines
                .binary_search_by_key(&callee_id, |callee| callee.id)
                .ok()
                .and_then(|position| module.machines.get(position))
                .ok_or_else(|| {
                    fault(caller.id, operation.id, "nominal call has no exact callee")
                })?;
            let parameters = call
                .structural_arguments
                .iter()
                .zip(&callee.structural_parameters)
                .enumerate()
                .map(|(index, (actual, formal))| {
                    let actual_type = caller.structural_place_type(actual.place).and_then(|root| {
                        terminal_semantics::runtime_structural_path_tip(
                            module.types.structural_types.iter(),
                            root,
                            &actual.path,
                        )
                    });
                    (
                        CallTypePosition::Parameter(index as u32),
                        actual_type.map(|ty| ty.id),
                        formal.structural_type,
                        actual.access == formal.access,
                    )
                });
            let result = callee.result.structural().map(|formal| {
                let actual = operation.result.as_structural();
                (
                    CallTypePosition::Result,
                    actual.map(|r| r.structural_type),
                    formal.structural_type,
                    actual.is_some_and(|result| {
                        result.multiplicity == formal.multiplicity
                            && result.qualifications == formal.qualifications
                            && result.projected_qualifications == formal.projected_qualifications
                    }),
                )
            });
            for (position, actual_type, formal_type, presentation_matches) in parameters.chain(result) {
                let key = (caller.id, operation.id, position);
                let row = rows
                    .binary_search_by_key(&key, |row| (row.caller, row.operation, row.position))
                    .ok()
                    .map(|position| &rows[position]);
                let required = elaboration
                    .nominal_types
                    .binary_search(&(callee.id, formal_type))
                    .is_ok();
                let Some(row) = row else {
                    if required {
                        return Err(fault(
                            caller.id,
                            operation.id,
                            "nominal call type has no complete type correspondence",
                        ));
                    }
                    continue;
                };
                observed += 1;
                let count = elaboration
                    .value_parameters
                    .get(callee.id)
                    .copied()
                    .unwrap_or(0);
                let start = callee.parameters.len().checked_sub(count).ok_or_else(|| {
                    fault(
                        caller.id,
                        operation.id,
                        "nominal const input count exceeds the callee",
                    )
                })?;
                if !required
                    || row.callee != callee.id
                    || row.expected != formal_type
                    || actual_type != Some(row.actual)
                    || !presentation_matches
                    || row
                        .const_arguments
                        .iter()
                        .any(|binding| (binding.parameter as usize) < start)
                    || !valid_key(row.actual_application)
                    || !valid_key(row.expected_application)
                    || !terminal_semantics::calls::nominal_call_type_matches(
                        module,
                        caller,
                        callee,
                        call.arguments,
                        row,
                    )
                {
                    return Err(fault(
                        caller.id,
                        operation.id,
                        "nominal call type changed its complete type or const actuals",
                    ));
                }
            }
        }
    }
    if observed != rows.len() {
        let row = &rows[0];
        return Err(fault(
            row.caller,
            row.operation,
            "nominal type correspondence has no emitted call position",
        ));
    }
    Ok(())
}
