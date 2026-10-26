//! Decode the complete core profile value at its establishing consumer.
//!
//! The lifecycle plan authenticates the core result type before this reader is
//! entered. Core declaration order is its semantic carrier contract; the reader
//! follows retained field/case identities and actual stored discriminators,
//! never source names or a host-side shadow of a provider's return. Decoding
//! supplies data only. Resource normalization and exact extent admission remain
//! the consumer's next step, so an unused malformed profile stays ordinary data.

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::{
    StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalScalarValue,
    TerminalStructuralValue,
};
use access_plans::{
    AtomicCapability, AtomicPermissions, AtomicTransferRule, BoundaryReach, BoundaryServiceReachId,
    ExternalCapability, ExternalReadBehavior, PeerWritability, ResourceProfile, ResourceRegion,
    StableCapability, TransferRule,
};
use semantic_vocabulary::{IntegerValue, StructuralCaseId};
use terminal_psi::{StructuralFieldDeclaration, StructuralPathSegment, StructuralTypeShape};

pub(super) fn read(
    execution: &TerminalExecution<'_>,
    value: &TerminalStructuralValue,
) -> Result<ResourceProfile, TerminalInterpretError> {
    let profile = ProfileValue {
        execution,
        value: value.clone(),
        case: None,
    };
    profile.record(2)?;
    let regions = profile.counted_entries(32)?;
    let mut result = Vec::with_capacity(regions.len());
    for region in regions {
        region.record(6)?;
        let range = region.field(0)?;
        range.record(2)?;
        let peer = match region.field(1)?.tag(2)? {
            0 => PeerWritability::Exclusive,
            1 => PeerWritability::HostileShared,
            _ => return Err(malformed()),
        };
        let stable = match region.field(2)?.tag(4)? {
            0 => StableCapability::None,
            1 => StableCapability::Read,
            2 => StableCapability::Write,
            3 => StableCapability::ReadWrite,
            _ => return Err(malformed()),
        };
        result.push(ResourceRegion {
            offset: range.unsigned(0)?,
            length: range.unsigned(1)?,
            peer,
            stable,
            external: external(region.field(3)?)?,
            atomic: atomic(region.field(4)?)?,
            reach: reach(region.field(5)?)?,
        });
    }
    Ok(ResourceProfile { regions: result })
}

fn external(value: ProfileValue<'_, '_>) -> Result<ExternalCapability, TerminalInterpretError> {
    let (case, value) = value.selected_case(2)?;
    if case == 0 {
        value.payload(0)?;
        return Ok(ExternalCapability::None);
    }
    value.payload(3)?;
    let read = match value.field(0)?.tag(3)? {
        0 => ExternalReadBehavior::None,
        1 => ExternalReadBehavior::Repeatable,
        2 => ExternalReadBehavior::Destructive,
        _ => return Err(malformed()),
    };
    Ok(ExternalCapability::Access {
        read,
        write: value.boolean(1)?,
        transfers: value
            .field(2)?
            .counted_entries(16)?
            .into_iter()
            .map(transfer)
            .collect::<Result<_, _>>()?,
    })
}

fn transfer(value: ProfileValue<'_, '_>) -> Result<TransferRule, TerminalInterpretError> {
    value.record(2)?;
    Ok(TransferRule {
        width_bits: u16::try_from(value.unsigned(0)?).map_err(|_| {
            TerminalInterpretError::ContentTransformationMismatch(
                "profile transfer width exceeds the supported coordinate space",
            )
        })?,
        alignment_bytes: value.unsigned(1)?,
    })
}

fn atomic(value: ProfileValue<'_, '_>) -> Result<AtomicCapability, TerminalInterpretError> {
    let (case, value) = value.selected_case(2)?;
    if case == 0 {
        value.payload(0)?;
        return Ok(AtomicCapability::None);
    }
    value.payload(1)?;
    let mut transfers = Vec::new();
    for entry in value.field(0)?.counted_entries(16)? {
        entry.record(2)?;
        let operations = entry.field(1)?;
        operations.record(12)?;
        transfers.push(AtomicTransferRule {
            transfer: transfer(entry.field(0)?)?,
            operations: AtomicPermissions {
                load: operations.boolean(0)?,
                store: operations.boolean(1)?,
                fetch_add: operations.boolean(2)?,
                fetch_sub: operations.boolean(3)?,
                fetch_xor: operations.boolean(4)?,
                fetch_or: operations.boolean(5)?,
                fetch_and: operations.boolean(6)?,
                swap: operations.boolean(7)?,
                compare_exchange: operations.boolean(8)?,
                compare_exchange_once: operations.boolean(9)?,
                try_exchange: operations.boolean(10)?,
                try_exchange_once: operations.boolean(11)?,
            },
        });
    }
    Ok(AtomicCapability::Access { transfers })
}

fn reach(value: ProfileValue<'_, '_>) -> Result<BoundaryReach, TerminalInterpretError> {
    let services = value
        .counted_entries(32)?
        .into_iter()
        .map(|entry| {
            let scalar = entry
                .execution
                .structural_primitive_storage
                .get(&StructuralRuntimePlace::from(&entry.value))
                .map(crate::scalar_storage::ScalarCell::read)
                .transpose()?
                .ok_or_else(malformed)?;
            BoundaryServiceReachId::from_normalized_identity(unsigned(scalar)?).map_err(|_| {
                TerminalInterpretError::ContentTransformationMismatch(
                    "profile reach contains an invalid service identity",
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BoundaryReach::from_services(services))
}

fn unsigned(value: TerminalScalarValue) -> Result<u64, TerminalInterpretError> {
    match value {
        TerminalScalarValue::Integer {
            value: IntegerValue::Unsigned(value),
            ..
        } => u64::try_from(value).map_err(|_| malformed()),
        _ => Err(malformed()),
    }
}

struct ProfileValue<'execution, 'program> {
    execution: &'execution TerminalExecution<'program>,
    value: TerminalStructuralValue,
    case: Option<StructuralCaseId>,
}

impl<'execution, 'program> ProfileValue<'execution, 'program> {
    fn fields(&self) -> Result<&[StructuralFieldDeclaration], TerminalInterpretError> {
        self.execution
            .active_field_declarations(
                self.value.structural_type,
                &StructuralRuntimePlace::from(&self.value),
                self.case,
            )
            .map(|(fields, _)| fields)
    }

    fn record(&self, count: usize) -> Result<(), TerminalInterpretError> {
        if self.case.is_some() {
            return Err(malformed());
        }
        self.payload(count)
    }

    fn payload(&self, count: usize) -> Result<(), TerminalInterpretError> {
        let fields = self.fields()?;
        if fields.len() != count || fields.iter().any(|field| field.relevance.is_erased()) {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "profile lost its complete declared field roster",
            ));
        }
        Ok(())
    }

    fn scalar(&self, position: usize) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let field = self.fields()?.get(position).ok_or_else(malformed)?;
        let parent = StructuralRuntimePlace::from(&self.value);
        let cell = if self.case.is_some() {
            StructuralScalarRuntimeField::case_payload(parent, field.id)
        } else {
            StructuralScalarRuntimeField::member(parent, field.id)
        };
        let value = self
            .execution
            .structural_scalar_fields
            .get(&cell)
            .map(crate::scalar_storage::ScalarCell::read)
            .transpose()?
            .ok_or_else(malformed)?;
        if !value.satisfies_field(&field.field_type) {
            return Err(malformed());
        }
        Ok(value)
    }

    fn unsigned(&self, position: usize) -> Result<u64, TerminalInterpretError> {
        unsigned(self.scalar(position)?)
    }

    fn boolean(&self, position: usize) -> Result<bool, TerminalInterpretError> {
        match self.scalar(position)? {
            TerminalScalarValue::Boolean(value) => Ok(value),
            _ => Err(malformed()),
        }
    }

    fn field(&self, position: usize) -> Result<Self, TerminalInterpretError> {
        let field = self.fields()?.get(position).ok_or_else(malformed)?;
        let mut path = Vec::with_capacity(2);
        if let Some(case) = self.case {
            path.push(StructuralPathSegment::Case(case));
        }
        path.push(StructuralPathSegment::Field(field.identity));
        let value = crate::custody::project_structural_value(
            &self.execution.program.structural_types(),
            &self.value,
            &path,
        )?;
        Ok(Self {
            execution: self.execution,
            value,
            case: None,
        })
    }

    fn selected_case(mut self, count: usize) -> Result<(usize, Self), TerminalInterpretError> {
        let types = self.execution.program.structural_types();
        let declaration = types
            .get(&self.value.structural_type)
            .ok_or_else(malformed)?;
        let StructuralTypeShape::Sum { cases } = &declaration.shape else {
            return Err(malformed());
        };
        if cases.len() != count {
            return Err(malformed());
        }
        let stored = self
            .execution
            .structural_cases
            .get(&StructuralRuntimePlace::from(&self.value))
            .ok_or_else(malformed)?;
        if stored.structural_type != self.value.structural_type {
            return Err(malformed());
        }
        let position = cases
            .iter()
            .position(|case| case.id == stored.case)
            .ok_or_else(malformed)?;
        self.case = Some(stored.case);
        Ok((position, self))
    }

    fn tag(self, count: usize) -> Result<usize, TerminalInterpretError> {
        let (tag, selected) = self.selected_case(count)?;
        selected.payload(0)?;
        Ok(tag)
    }

    fn counted_entries(&self, capacity: u64) -> Result<Vec<Self>, TerminalInterpretError> {
        self.record(2)?;
        let count = self.unsigned(1)?;
        let array = self.field(0)?;
        let types = self.execution.program.structural_types();
        let StructuralTypeShape::FixedArray { length, .. } = types
            .get(&array.value.structural_type)
            .ok_or_else(malformed)?
            .shape
        else {
            return Err(malformed());
        };
        if length != capacity || count > capacity {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "profile count exceeds its complete supplied array",
            ));
        }
        (0..count)
            .map(|index| {
                let value = crate::custody::project_structural_value(
                    &types,
                    &array.value,
                    &[StructuralPathSegment::FixedIndex(index)],
                )?;
                Ok(Self {
                    execution: self.execution,
                    value,
                    case: None,
                })
            })
            .collect()
    }
}
