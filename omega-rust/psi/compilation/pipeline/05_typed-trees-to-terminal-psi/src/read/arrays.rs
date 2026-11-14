//! Resolve an array's actual element type and closed extent once. Array
//! elements keep the canonical indexed namespace; they never acquire invented
//! field declarations or a nominal schema.

use crate::body::Qualification;
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType, StructuralTypeId};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::ArrayExtent;
use terminal_semantics::CollectionExtent;
use typed_trees::expression::{ExpressionHandle, TableIndexedExpression};
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle};

use super::authority::Access;
use super::{Program, Slot, places::RecordSource};
use crate::{
    Expression, ExpressionKind, RecordType, SourceOrigin, StorageShape, Unserved, ValueType, refuse,
};

// Retain the same scoped type/region identity used by byte views. A bare
// sequence is affine; an explicit shared loan is unrestricted. Neither kind
// supplies elements or backing merely by appearing in a declaration.
pub(super) fn source_sequence_view(
    typed: &typed_trees::TypedTrees,
    reference: TypeReferenceHandle,
    lexical_owner: SymbolHandle,
    machine: MachineId,
) -> Checked<
    Option<(
        TypeReferenceHandle,
        Option<terminal_psi::StructuralAccess>,
        String,
    )>,
> {
    use typed_trees::types::TypeReferenceNode;
    let fail = |reason| refuse(machine, Construct::Machine, Unserved::Type(reason));
    if !typed
        .type_reference_table
        .contains_type_reference(reference)
    {
        return Err(fail("sequence lost its retained type reference"));
    }
    let (slice, access) = match typed.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Slice { .. } => (reference, None),
        TypeReferenceNode::Reference {
            referee, access, ..
        } => {
            if !typed.type_reference_table.contains_type_reference(*referee) {
                return Err(fail("sequence loan lost its retained referent"));
            }
            if !matches!(
                typed.type_reference_table.type_reference(*referee),
                TypeReferenceNode::Slice { .. }
            ) {
                return Ok(None);
            }
            if super::bytes::source_byte_view(typed, reference, lexical_owner, machine)?.is_some() {
                return Ok(None);
            }
            if *access != language_core::ReferenceAccess::Shared {
                return Err(fail(
                    "mutable typed sequence requires its actual exclusive backing and loan",
                ));
            }
            (*referee, Some(terminal_psi::StructuralAccess::SharedBorrow))
        }
        _ => return Ok(None),
    };
    let TypeReferenceNode::Slice { element_type } =
        typed.type_reference_table.type_reference(slice)
    else {
        return Err(fail("sequence lost its exact slice declaration"));
    };
    if !typed
        .type_reference_table
        .contains_type_reference(*element_type)
    {
        return Err(fail("sequence lost its retained element type"));
    }
    let identity = if access.is_some() {
        let retained = typed.type_reference_table.operand_lifetime_owner(reference);
        let owner = if retained.is_valid() {
            retained
        } else {
            lexical_owner
        };
        typed_trees::type_identity::applied_type_identity_in_region_scope(
            typed,
            reference,
            owner,
            owner,
            &[],
            &[],
        )
        .and_then(|identity| typed.type_identity_text(identity))
    } else {
        typed.type_identity_text(typed.normalized_type_identity(reference))
    }
    .filter(|identity| !identity.is_empty())
    .ok_or_else(|| fail("sequence has no complete owned type/region identity"))?;
    Ok(Some((*element_type, access, identity)))
}

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
        let element_regions = self.collection_element_regions(machine, array, regions)?;
        let declared = super::places::with_regions(
            self.storage_value_type(machine, element)?,
            element_regions,
        );
        let supplied = self.typed.expression_table.expression_handles(supplied);
        if let CollectionExtent::Fixed(extent) = extent
            && u64::try_from(supplied.len()).ok() != Some(extent)
        {
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
        let (element, extent) = self.array_element(machine, array)?;
        if !matches!(extent, CollectionExtent::Fixed(_)) {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("sequence index needs its current length and bounds judgment"),
            ));
        }
        let regions = base
            .result
            .map_or(crate::body::Regions::NONE, ValueType::regions);
        let element_regions = self.collection_element_regions(machine, array, regions)?;
        let result = super::places::with_regions(
            self.storage_value_type(machine, element)?,
            element_regions,
        );
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

    pub(super) fn array_element(
        &self,
        machine: MachineId,
        array: StructuralTypeId,
    ) -> Checked<(StructuralTypeId, CollectionExtent)> {
        match &self.data_type(machine, array)?.shape {
            StorageShape::FixedArray {
                element,
                length: ArrayExtent::Literal(length),
            } => Ok((*element, CollectionExtent::Fixed(*length))),
            StorageShape::ElementView { element, .. } => {
                Ok((*element, CollectionExtent::StoredLength))
            }
            _ => Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Typing("index or literal of a non-array type"),
            )),
        }
    }

    pub(super) fn storage_value_type(
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

    /// A source sequence keeps its exact element and type identity. This row
    /// supplies no elements, lifetime or backing: those belong to construction
    /// and transport through the existing canonical ElementView operations.
    pub(super) fn sequence_type(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
        element: TypeReferenceHandle,
    ) -> Checked<ValueType> {
        let Some((declared, access, identity)) =
            source_sequence_view(self.typed, reference, self.lifetime_owner, machine)?
        else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("sequence without its source declaration"),
            ));
        };
        if declared != element {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Type("sequence selects another element occurrence"),
            ));
        }
        let mut tuple = Vec::new();
        if access.is_some() {
            tuple.push(self.borrowed_view_region(
                machine,
                reference,
                self.lifetime_owner,
                &identity,
            )?);
        }
        let value = self.value_type(machine, element)?;
        tuple.extend_from_slice(self.region_tuple(value.regions()));
        let regions = self.intern_regions(tuple);
        let element = self.storage_type(machine, element, value)?;
        let shape = StorageShape::ElementView { element, access };
        if let Some((id, _)) = self.records.iter().find(|(_, row)| row.shape == shape) {
            return Ok(ValueType::Record(id, Qualification::NONE, regions));
        }
        let id = self.records.push(RecordType {
            owns_linear_claim: false,
            copyable: self.typed.stored_value_multiplicity(reference)
                == language_semantics::Multiplicity::Unrestricted,
            identity,
            shape,
            requires: Vec::new(),
        });
        let mut source = physical_source(reference);
        source.region_owner = self.lifetime_owner;
        self.record_sources.push(source);
        Ok(ValueType::Record(id, Qualification::NONE, regions))
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
        let owns_linear_claim = self.data_type(machine, element)?.owns_linear_claim;
        let identity = self.storage_identity(machine, reference)?;
        let id = self.records.push(RecordType {
            owns_linear_claim,
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

    pub(super) fn storage_type(
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
            owns_linear_claim: false,
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

#[cfg(test)]
mod sequence_source_tests {
    use super::*;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{TypedTrees, name::Identifier, types::TypeReferenceNode};

    #[test]
    fn a_sequence_cannot_rebind_its_source_or_element_to_equal_width_storage() {
        // This is a typed-metadata falsifier, not source or authority admission.
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = symbols.insert_children(root, symbols::builtin_type_symbols());
        let unsigned = children
            .handles()
            .nth(
                symbols::BuiltinTypeAtom::Integer(
                    numerics::integer_carrier::FixedInteger::unsigned(32),
                )
                .ordinal(),
            )
            .unwrap();
        let signed = children
            .handles()
            .nth(
                symbols::BuiltinTypeAtom::Integer(numerics::integer_carrier::FixedInteger::signed(
                    32,
                ))
                .ordinal(),
            )
            .unwrap();
        let mut typed = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let unsigned = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: unsigned,
            name: Identifier::generated("u32"),
        });
        let signed = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: signed,
            name: Identifier::generated("i32"),
        });
        let first = typed.type_reference_table.insert(TypeReferenceNode::Slice {
            element_type: unsigned,
        });
        let second = typed.type_reference_table.insert(TypeReferenceNode::Slice {
            element_type: signed,
        });
        let machine = MachineId::new(1).unwrap();
        let mut program = Program::new(&typed, false);
        let ValueType::Record(first_row, _, _) =
            program.sequence_type(machine, first, unsigned).unwrap()
        else {
            unreachable!()
        };
        let ValueType::Record(second_row, _, _) =
            program.sequence_type(machine, second, signed).unwrap()
        else {
            unreachable!()
        };
        assert_ne!(first_row, second_row);
        assert!(
            program
                .source_record_correspondence(machine)
                .unwrap()
                .is_empty()
        );
        let original = program.records.get(first_row).unwrap().shape.clone();
        let StorageShape::ElementView { element: wrong, .. } =
            program.records.get(second_row).unwrap().shape
        else {
            unreachable!()
        };
        let StorageShape::ElementView { element, .. } =
            &mut program.records.get_mut(first_row).unwrap().shape
        else {
            unreachable!()
        };
        *element = wrong;
        assert!(
            program.source_record_correspondence(machine).is_err(),
            "same geometry is not an element declaration"
        );
        program.records.get_mut(first_row).unwrap().shape = original;
        program
            .record_sources
            .get_mut(first_row)
            .unwrap()
            .application
            .as_mut()
            .unwrap()
            .reference = second;
        assert!(
            program.source_record_correspondence(machine).is_err(),
            "an unrelated retained source cannot relabel the row"
        );
    }
}
