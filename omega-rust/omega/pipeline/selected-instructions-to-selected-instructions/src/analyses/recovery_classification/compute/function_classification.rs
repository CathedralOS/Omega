//! Per-function spill-choice and victim-role reconstruction.

use crate::{
    FunctionRecoveryClassification, PressureRecoveryClassification, RecoveryClassificationError,
    RecoveryVictimRole,
};

pub(super) fn classify(
    function: usize,
    selected: &selected_instructions::SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    legality: &crate::FunctionAllocationLegality,
    choices: &crate::FunctionSpillChoices,
) -> Result<FunctionRecoveryClassification, RecoveryClassificationError> {
    if selected.machine != ranges.machine
        || selected.machine != legality.machine
        || selected.machine != choices.machine
        || selected.virtual_registers.len() != ranges.virtual_registers.len()
        || selected.virtual_registers.len() != legality.virtual_registers.len()
    {
        return Err(RecoveryClassificationError::FunctionMismatch { function });
    }
    let Some(choice) = &choices.choice else {
        return Ok(FunctionRecoveryClassification {
            machine: selected.machine,
            classification: None,
        });
    };
    let victim = selected
        .virtual_registers
        .iter()
        .find(|register| register.id == choice.selected_victim)
        .ok_or(RecoveryClassificationError::VictimMismatch {
            function,
            register: choice.selected_victim.0,
        })?;
    let range = ranges
        .virtual_registers
        .iter()
        .find(|range| range.virtual_register == victim.id && range.class == victim.class)
        .ok_or(RecoveryClassificationError::VictimMismatch {
            function,
            register: victim.id.0,
        })?;
    if !legality
        .virtual_registers
        .iter()
        .any(|row| row.virtual_register == victim.id && row.class == victim.class)
    {
        return Err(RecoveryClassificationError::VictimMismatch {
            function,
            register: victim.id.0,
        });
    }
    let role = victim_role(function, choice)?;
    let classification =
        super::immediate_eligibility::classify(function, selected, ranges, choice, victim, range)?;
    Ok(FunctionRecoveryClassification {
        machine: selected.machine,
        classification: Some(PressureRecoveryClassification {
            block: choice.block,
            point: choice.point,
            victim: victim.id,
            role,
            scalar_type: victim.scalar_type,
            class: victim.class,
            origin: victim.origin,
            definition_site: victim.definition_site,
            classification,
        }),
    })
}

fn victim_role(
    function: usize,
    choice: &crate::SpillChoice,
) -> Result<RecoveryVictimRole, RecoveryClassificationError> {
    if choice.selected_victim == choice.incoming {
        let contender = choice
            .contenders
            .iter()
            .find(|row| row.virtual_register == choice.selected_victim)
            .ok_or(RecoveryClassificationError::ChoiceMismatch { function })?;
        if contender.reclaimed_view.is_some() {
            return Err(RecoveryClassificationError::ChoiceMismatch { function });
        }
        return Ok(RecoveryVictimRole::Incoming);
    }
    let resident = choice
        .active_residents
        .iter()
        .find(|row| row.virtual_register == choice.selected_victim)
        .ok_or(RecoveryClassificationError::ChoiceMismatch { function })?;
    let reclaimed_view = choice
        .contenders
        .iter()
        .find(|row| row.virtual_register == choice.selected_victim)
        .and_then(|row| row.reclaimed_view)
        .ok_or(RecoveryClassificationError::ChoiceMismatch { function })?;
    Ok(RecoveryVictimRole::ActiveResident {
        current_view: resident.view,
        reclaimed_view,
    })
}
