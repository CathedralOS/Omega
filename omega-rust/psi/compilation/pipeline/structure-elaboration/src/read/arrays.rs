//! Resolve an array's actual element type and closed extent once. Array
//! elements keep the canonical indexed namespace; they never acquire invented
//! field declarations or a nominal schema.

use crate::body::Qualification;
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::ArrayExtent;
use typed_trees::expression::{ExpressionHandle, TableIndexedExpression};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle};

use super::authority::Access;
use super::{Program, Slot, places::RecordSource};
use crate::{
    Expression, ExpressionKind, RecordType, SourceOrigin, StorageShape, Unserved, ValueType, refuse,
};

impl Program<'_> {
    pub(super) fn array_literal(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        supplied: arena::HandleSpan<ExpressionHandle>,
        expected: Option<ValueType>,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        // A literal constructs the bare carrier, whatever its context
        // expects: a qualified destination then refuses the plain value.
        let Some(ValueType::Record(array, _, regions)) = expected else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("array literal without a closed contextual type"),
            ));
        };
        let (element, extent) = self.array_element(machine, array)?;
        // An array value holds its elements' regions (`array_type`).
        let declared =
            super::places::with_regions(self.storage_value_type(machine, element)?, regions);
        let supplied = self.typed.expression_table.expression_handles(supplied);
        if u64::try_from(supplied.len()).ok() != Some(extent) {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("array literal extent"),
            ));
        }
        let mut elements = Vec::with_capacity(supplied.len());
        for handle in supplied {
            let element = self.expression(machine, slots, *handle, Some(declared))?;
            if !self.agrees(element.result, declared) {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Typing("array literal element"),
                ));
            }
            elements.push(element);
        }
        Ok(Expression {
            kind: ExpressionKind::Array { elements },
            result: Some(ValueType::Record(array, Qualification::NONE, regions)),
            source,
        })
    }

    pub(super) fn indexed(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        indexed: &TableIndexedExpression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let base = self.expression(machine, slots, indexed.collection, None)?;
        let Some(array) = self.opened(machine, base.result, Access::Read)? else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("index into non-array storage"),
            ));
        };
        let (element, _) = self.array_element(machine, array)?;
        let regions = base
            .result
            .map_or(crate::body::Regions::NONE, ValueType::regions);
        let result =
            super::places::with_regions(self.storage_value_type(machine, element)?, regions);
        let index_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| {
            refuse(
                machine,
                Construct::Machine,
                Unserved::Type("u64 index carrier"),
            )
        })?;
        let index = self.expression(
            machine,
            slots,
            indexed.index,
            Some(ValueType::Scalar(
                ScalarType::Integer(index_type),
                Qualification::NONE,
            )),
        )?;
        if !matches!(index.result.and_then(ValueType::scalar), Some(ScalarType::Integer(integer)) if !integer.is_address())
        {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("array index is not a fixed integer"),
            ));
        }
        Ok(Expression {
            kind: ExpressionKind::Index {
                base: Box::new(base),
                index: Box::new(index),
            },
            result: Some(result),
            source,
        })
    }

    fn array_element(
        &self,
        machine: MachineId,
        array: StructuralTypeId,
    ) -> Checked<(StructuralTypeId, u64)> {
        match &self.data_type(machine, array)?.shape {
            StorageShape::FixedArray {
                element,
                length: ArrayExtent::Literal(length),
            } => Ok((*element, *length)),
            _ => Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("index or literal of a non-array type"),
            )),
        }
    }

    fn storage_value_type(
        &self,
        machine: MachineId,
        storage: StructuralTypeId,
    ) -> Checked<ValueType> {
        Ok(match self.data_type(machine, storage)?.shape {
            StorageShape::PrimitiveScalar {
                scalar_type,
                policy: None,
            } => ValueType::Scalar(scalar_type, Qualification::NONE),
            StorageShape::PrimitiveScalar {
                scalar_type,
                policy: Some(policy),
            } => ValueType::Scalar(scalar_type, Qualification::of_policy(policy)),
            _ => ValueType::record(storage),
        })
    }

    pub(super) fn array_type(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
        element: TypeReferenceHandle,
        extent: &FixedArrayLength,
    ) -> Checked<ValueType> {
        let FixedArrayLength::Literal(length) = extent else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("unresolved array extent"),
            ));
        };
        let length = u64::try_from(*length).map_err(|_| {
            refuse(
                machine,
                Construct::Machine,
                Unserved::Type("array extent outside u64"),
            )
        })?;
        let value = self.value_type(machine, element)?;
        let element = self.storage_type(machine, element, value)?;
        let extent = ArrayExtent::Literal(length);
        if let Some((id, _)) = self.records.iter().find(|(_, row)| {
            matches!(&row.shape, StorageShape::FixedArray { element: known, length }
                if *known == element && *length == extent)
        }) {
            return Ok(ValueType::Record(id, Qualification::NONE, value.regions()));
        }
        let copyable = self.data_type(machine, element)?.copyable;
        let identity = self.storage_identity(machine, reference)?;
        let id = self.records.push(RecordType {
            copyable,
            identity,
            shape: StorageShape::FixedArray {
                element,
                length: extent,
            },
            requires: Vec::new(),
        });
        self.record_sources.push(physical_source(reference));
        // The row is the element's carrier; the regions its elements hold
        // belong to the array value's type, as a record's do (`regions`).
        Ok(ValueType::Record(id, Qualification::NONE, value.regions()))
    }

    fn storage_type(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
        value: ValueType,
    ) -> Checked<StructuralTypeId> {
        let (scalar_type, policy) = match value {
            ValueType::Scalar(scalar, qualification) if qualification.domains.is_empty() => {
                (scalar, qualification.policy)
            }
            ValueType::Scalar(..) => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("array of elements that carry a declared domain"),
                ));
            }
            ValueType::Record(id, Qualification::NONE, _) => return Ok(id),
            ValueType::Record(..) => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("array of elements that carry a declared domain"),
                ));
            }
            ValueType::Rigid(_) => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("rigid array element"),
                ));
            }
        };
        if let Some((id, _)) = self.records.iter().find(|(_, row)| {
            matches!(row.shape, StorageShape::PrimitiveScalar { scalar_type: known, policy: prior }
                if known == scalar_type && prior == policy)
        }) {
            return Ok(id);
        }
        let identity = self.storage_identity(machine, reference)?;
        let id = self.records.push(RecordType {
            copyable: true,
            identity,
            shape: StorageShape::PrimitiveScalar {
                scalar_type,
                policy,
            },
            requires: Vec::new(),
        });
        self.record_sources.push(physical_source(reference));
        Ok(id)
    }

    fn storage_identity(
        &self,
        machine: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<String> {
        self.typed
            .type_identity_text(self.typed.normalized_type_identity(reference))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| {
                refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Spelling("array storage type identity"),
                )
            })
    }
}

fn physical_source(reference: TypeReferenceHandle) -> RecordSource {
    RecordSource {
        declaration: SymbolHandle::invalid(),
        application: Some(typed_trees::SourceTypeApplication {
            reference,
            substitutions: Vec::new(),
        }),
        region_owner: SymbolHandle::invalid(),
        fields: Vec::new(),
        cases: Vec::new(),
    }
}
