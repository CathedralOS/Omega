//! Receiver-free logical `Extent` values bound to installed root authority.
//!
//! This module stops before physical realization. It retains ordinary
//! `base`/`length` observations and their exact checked field layout beside the
//! non-duplicable authority carrier; it does not encode bytes, populate an ABI
//! location, emit a wrapper or call, or claim native execution.

use super::{
    ProgramEntrySourceExtentFieldRole, ProgramEntrySourceExtentValueLayout,
    ProgramLocalStorageCustody, ProgramLocalStorageCustodyError, ProgramStorageEntryDiagnostic,
    ProgramStorageEntryRootRole, ProgramStorageEntryWholeRootArgumentCarrier,
};

#[derive(Debug)]
pub struct ProgramStorageEntryExtentLogicalValue {
    role: ProgramStorageEntryRootRole,
    visible_parameter_index: usize,
    call_parameter_index: usize,
    base: u64,
    length: u64,
    layout: ProgramEntrySourceExtentValueLayout,
}

impl ProgramStorageEntryExtentLogicalValue {
    pub const fn role(&self) -> ProgramStorageEntryRootRole {
        self.role
    }

    pub const fn visible_parameter_index(&self) -> usize {
        self.visible_parameter_index
    }

    pub const fn call_parameter_index(&self) -> usize {
        self.call_parameter_index
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn layout(&self) -> &ProgramEntrySourceExtentValueLayout {
        &self.layout
    }
}

/// Two logical source values that remain inseparable from their installed
/// receiver-free authorities and exact continuation argument plan.
#[derive(Debug)]
pub struct ProgramStorageEntryWholeRootLogicalValueCarrier {
    arguments: ProgramStorageEntryWholeRootArgumentCarrier,
    values: [ProgramStorageEntryExtentLogicalValue; 2],
}

impl ProgramStorageEntryWholeRootLogicalValueCarrier {
    pub const fn arguments(&self) -> &ProgramStorageEntryWholeRootArgumentCarrier {
        &self.arguments
    }

    pub const fn values(&self) -> &[ProgramStorageEntryExtentLogicalValue; 2] {
        &self.values
    }

    pub fn into_arguments(self) -> ProgramStorageEntryWholeRootArgumentCarrier {
        self.arguments
    }
}

pub fn bind_program_storage_entry_whole_root_logical_values(
    arguments: ProgramStorageEntryWholeRootArgumentCarrier,
) -> Result<
    ProgramStorageEntryWholeRootLogicalValueCarrier,
    ProgramStorageEntryWholeRootLogicalValueError,
> {
    match validate_logical_values(&arguments) {
        Ok(values) => Ok(ProgramStorageEntryWholeRootLogicalValueCarrier { arguments, values }),
        Err(diagnostic) => Err(ProgramStorageEntryWholeRootLogicalValueError {
            arguments,
            diagnostic,
        }),
    }
}

pub fn bind_program_local_storage_entry_whole_root_logical_values<'root, 'code>(
    custody: ProgramLocalStorageCustody<'root, 'code, ProgramStorageEntryWholeRootArgumentCarrier>,
) -> Result<
    ProgramLocalStorageCustody<'root, 'code, ProgramStorageEntryWholeRootLogicalValueCarrier>,
    ProgramLocalStorageCustodyError<'root, 'code, ProgramStorageEntryWholeRootArgumentCarrier>,
> {
    let (arguments, registry) = custody.into_parts();
    match bind_program_storage_entry_whole_root_logical_values(arguments) {
        Ok(values) => Ok(ProgramLocalStorageCustody::new(values, registry)),
        Err(error) => {
            let diagnostic = error.diagnostic().clone();
            Err(ProgramLocalStorageCustodyError::new(
                ProgramLocalStorageCustody::new(error.into_arguments(), registry),
                diagnostic,
            ))
        }
    }
}

fn validate_logical_values(
    carrier: &ProgramStorageEntryWholeRootArgumentCarrier,
) -> Result<[ProgramStorageEntryExtentLogicalValue; 2], ProgramStorageEntryDiagnostic> {
    if carrier.target().pointer_size != 8 || carrier.target().pointer_alignment != 8 {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage Extent logical values require the retained x86-64 address layout"
                .into(),
        ));
    }
    let expected_roles = [
        ProgramStorageEntryRootRole::Image,
        ProgramStorageEntryRootRole::InitialStorage,
    ];
    let mut values = Vec::with_capacity(2);
    for (index, (expected_role, argument)) in expected_roles
        .into_iter()
        .zip(carrier.arguments())
        .enumerate()
    {
        let layout = argument.extent_value_layout();
        let [base, length] = layout.fields();
        let scalar = omega_calling_conventions::ValueShape::integer(8, 8);
        if argument.role() != expected_role
            || argument.visible_parameter_index() != index
            || argument.call_parameter_index() != index
            || argument.shape() != layout.shape()
            || base.role() != ProgramEntrySourceExtentFieldRole::Base
            || base.byte_offset() != 0
            || base.shape() != scalar
            || length.role() != ProgramEntrySourceExtentFieldRole::Length
            || length.byte_offset() != 8
            || length.shape() != scalar
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "program-storage {expected_role:?} logical value drifted from its exact Extent argument layout"
            )));
        }
        let authority = carrier.root_authority(expected_role);
        if authority.base().checked_add(authority.length()).is_none() {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "program-storage {expected_role:?} logical Extent geometry wraps the target address space"
            )));
        }
        values.push(ProgramStorageEntryExtentLogicalValue {
            role: expected_role,
            visible_parameter_index: index,
            call_parameter_index: index,
            base: authority.base(),
            length: authority.length(),
            layout: layout.clone(),
        });
    }
    values.try_into().map_err(|_| {
        ProgramStorageEntryDiagnostic(
            "program-storage logical value carrier lost its exact two root rows".into(),
        )
    })
}

#[derive(Debug)]
pub struct ProgramStorageEntryWholeRootLogicalValueError {
    arguments: ProgramStorageEntryWholeRootArgumentCarrier,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStorageEntryWholeRootLogicalValueError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_arguments(self) -> ProgramStorageEntryWholeRootArgumentCarrier {
        self.arguments
    }
}

impl std::fmt::Display for ProgramStorageEntryWholeRootLogicalValueError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageEntryWholeRootLogicalValueError {}
