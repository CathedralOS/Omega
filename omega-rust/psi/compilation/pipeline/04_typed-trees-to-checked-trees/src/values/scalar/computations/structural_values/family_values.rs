//! Structural value production by family: fixed and scalar arrays, records,
//! and case constructions each establish their own contents.

use super::super::{
    Builder, CheckedScalarExpressionRole, ExpressionHandle, ExpressionNode, TypeReferenceNode,
};
use super::value_predicates::{zeroed_integer_literal, zeroed_scalar_array_parts};
use checked_trees::CheckedStructuralValueKind;
use typed_trees::types::TypeReferenceHandle;

impl Builder<'_, '_> {
    /// Element-wise array literal establishment: each element registers as
    /// its own structural value under the declared element type, exactly as
    /// record fields do inside a constructor.
    pub(super) fn fixed_array_value(
        &mut self,
        expected: TypeReferenceHandle,
        literal: arena::HandleSpan<ExpressionHandle>,
    ) -> Option<CheckedStructuralValueKind> {
        if validation::is_closed_primitive_array_type(self.program, expected) {
            return None;
        }
        let reference = validation::unwrapped_type_reference(self.program, expected)?;
        let TypeReferenceNode::FixedArray { element_type, .. } =
            self.program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let mut elements = Vec::new();
        for element in self.program.expression_table.expression_handles(literal) {
            elements.push(self.structural_value(*element, *element_type)?);
        }
        Some(CheckedStructuralValueKind::FixedArray { elements })
    }

    /// A closed primitive array supplied as one member of a structural value.
    /// The ordinary scalar-array producer flattens it to its row-major leaves;
    /// each leaf evaluates in authored order as its own scalar computation,
    /// keyed by this array expression the way record fields key by their
    /// literal. Lowering establishes the whole roster as one scalar array, so
    /// the array contributes no element custody of its own. Constant-array
    /// projections retain their exact builtin indexing selection.
    /// Element-wise array literal establishment: each element registers as
    /// its own structural value under the declared element type, exactly as
    /// record fields do inside a constructor.
    pub(super) fn scalar_array_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let array =
            validation::scalar_array_elements(self.program, self.machine, expression, expected)?;
        if array.projections.iter().any(|projection| {
            self.operators
                .expression_use(*projection)
                .is_some_and(|selected| {
                    selected.spelling != language_core::OperatorSpelling::Index
                        || selected.selected_operator_symbol.is_valid()
                        || selected.candidate_count != 0
                        || !matches!(
                            selected.status,
                            checked_trees::CheckedOperatorResolutionStatus::Missing
                                | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                        )
                })
        }) {
            return None;
        }
        let mut elements = Vec::with_capacity(array.elements.len());
        for (element_ordinal, (leaf, primitive_type)) in array.elements.into_iter().enumerate() {
            let root = self.expression(leaf, primitive_type)?;
            self.plans.nodes.get_mut(root).authored_root = leaf;
            self.push_scalar_root(
                CheckedScalarExpressionRole::StructuralArrayElement {
                    expression,
                    element_ordinal: u32::try_from(element_ordinal).ok()?,
                },
                root,
            )?;
            elements.push(root);
        }
        Some(CheckedStructuralValueKind::ScalarArray { elements })
    }

    pub(super) fn record_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression).clone()
        else {
            return None;
        };
        let plain_contents = |reference| {
            validation::has_contents(
                self.program,
                reference,
                validation::ContentsRequirement::NumericOwned,
            ) || validation::has_contents(
                self.program,
                reference,
                validation::ContentsRequirement::StructuralConstruction,
            ) || validation::has_contents(
                self.program,
                reference,
                validation::ContentsRequirement::LinearOwned,
            ) || validation::reference_result_custody::is_reference_record(self.program, reference)
                || crate::execution::terminal_unit::types::record_with_owned_or_shared_view_fields(
                    self.program,
                    reference,
                )
                || validation::has_contents(
                    self.program,
                    reference,
                    validation::ContentsRequirement::OwnedOrSharedView,
                )
        };
        // A custody domain on the declared type (`Region in Owned`) qualifies
        // the binding, not the record's storage: the shape question is about
        // the underlying record. Domain-qualified storage that is not a plain
        // owned shape still refuses on the unwrapped check.
        if !plain_contents(expected)
            && !validation::unwrapped_type_reference(self.program, expected)
                .is_some_and(|unwrapped| unwrapped != expected && plain_contents(unwrapped))
        {
            return None;
        }
        let reference = validation::unwrapped_type_reference(self.program, expected)?;
        let data = self
            .program
            .closed_data_constructor_definition(reference, literal.type_symbol)?;
        if literal.case_name.is_some() {
            return None;
        }
        let data_symbol = data.symbol;
        let declared = self.program.data_members(data);
        if declared
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            return None;
        }
        let mut authored = self
            .program
            .expression_table
            .struct_fields(literal.fields)
            .to_vec();
        // Consume the prepared schema, including nested applications and const
        // extents. Source fields may still select the template; the retained
        // member correspondence, not a name-only search, translates that use.
        for initializer in &mut authored {
            initializer.field_symbol = self
                .program
                .closed_data_constructor_field(
                    reference,
                    literal.type_symbol,
                    initializer.field_symbol,
                )?
                .symbol;
        }
        let relevant = self.program.live_data_fields(declared).collect::<Vec<_>>();
        let erased = declared
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.relevance.is_erased() => {
                    Some(field)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        // An ordinary runtime field may be omitted only when its zero value
        // satisfies the declared carrier — today that means a `[scalar; N]`
        // leaf whose element is a plain integer primitive. Every authored
        // initializer must still name a declared member exactly once.
        let omitted = relevant
            .iter()
            .filter(|field| {
                authored
                    .iter()
                    .all(|initializer| initializer.field_symbol != field.symbol)
            })
            .copied()
            .collect::<Vec<_>>();
        if authored.len() != relevant.len() + erased.len()
            && (authored.len() > relevant.len() + erased.len()
                || authored.iter().any(|initializer| {
                    declared.iter().all(|member| match member {
                        typed_trees::data::DataMember::Field(field) => {
                            field.symbol != initializer.field_symbol
                        }
                        _ => true,
                    })
                })
                || omitted.iter().any(|field| {
                    zeroed_scalar_array_parts(self.program, field.type_reference).is_none()
                        && !validation::zeroed_structural_parts(self.program, field.type_reference)
                }))
        {
            return None;
        }
        let mut fields = Vec::new();
        for (ordinal, initializer) in authored.into_iter().enumerate() {
            let erased_field = erased
                .iter()
                .find(|field| field.symbol == initializer.field_symbol);
            let field = if let Some(field) = erased_field {
                *field
            } else {
                *relevant
                    .iter()
                    .find(|field| field.symbol == initializer.field_symbol)?
            };
            if erased_field.is_some() {
                // The erased member stays in the checked record's field list —
                // it carries semantic content but no runtime storage, so the
                // checked value keeps its exact initializer for interpretation
                // while emission skips it below the lowered representations.
                let value = checked_trees::CheckedStructuralRecordFieldValue::Structural(
                    self.structural_value(initializer.value, field.type_reference)?,
                );
                fields.push(checked_trees::CheckedStructuralRecordField {
                    field: field.symbol,
                    expression: initializer.value,
                    type_reference: field.type_reference,
                    value,
                });
                continue;
            }
            if fields
                .iter()
                .any(|prior: &checked_trees::CheckedStructuralRecordField| {
                    prior.field == field.symbol
                })
            {
                return None;
            }
            // A quoted literal for a shared byte view member lends immutable
            // image storage. Retain its octets as an ordinary descriptor
            // operand, completed before the enclosing record is established.
            // The literal must grant every declared domain of the view.
            if let ExpressionNode::String(bytes) =
                self.program.expression_table.expression(initializer.value)
                && matches!(
                    crate::execution::terminal_unit::types::byte_sequence_carrier(
                        self.program,
                        field.type_reference,
                        &[],
                    ),
                    Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView {
                        access: Some(checked_trees::CheckedStructuralAccess::SharedBorrow),
                    })
                )
            {
                if !validation::field_domains::string_literal_grants_view_domains(
                    self.program,
                    field.type_reference,
                    initializer.value,
                ) {
                    return None;
                }
                let node = self.publish_structural_value(
                    initializer.value,
                    field.type_reference,
                    CheckedStructuralValueKind::ByteLiteralView {
                        bytes: bytes.to_vec(),
                    },
                )?;
                fields.push(checked_trees::CheckedStructuralRecordField {
                    field: field.symbol,
                    expression: initializer.value,
                    type_reference: field.type_reference,
                    value: checked_trees::CheckedStructuralRecordFieldValue::Structural(node),
                });
                continue;
            }
            let declared = field.type_reference;
            // A stored reference owns its descriptor, not the primitive
            // referent's value. The declared-storage query retains reference
            // shells; referent normalization is not a field-value classifier.
            let value = if let Some(primitive) = self.program.primitive_type_reference(declared) {
                let root = self.expression(initializer.value, primitive)?;
                self.plans.nodes.get_mut(root).authored_root = initializer.value;
                self.push_scalar_root(
                    CheckedScalarExpressionRole::RecordField {
                        expression,
                        field_ordinal: u32::try_from(ordinal).ok()?,
                    },
                    root,
                )?;
                checked_trees::CheckedStructuralRecordFieldValue::Scalar(root)
            } else {
                checked_trees::CheckedStructuralRecordFieldValue::Structural(
                    self.structural_value(initializer.value, declared)?,
                )
            };
            fields.push(checked_trees::CheckedStructuralRecordField {
                field: field.symbol,
                expression: initializer.value,
                type_reference: declared,
                value,
            });
        }
        for field in omitted {
            let declared = field.type_reference;
            if zeroed_scalar_array_parts(self.program, declared).is_none() {
                // Structural members carry no element roster: the emitted
                // EstablishZeroedStructural owns the whole shape's zeros.
                let node = self.publish_structural_value(
                    expression,
                    declared,
                    CheckedStructuralValueKind::ZeroedStructural {},
                )?;
                fields.push(checked_trees::CheckedStructuralRecordField {
                    field: field.symbol,
                    expression,
                    type_reference: declared,
                    value: checked_trees::CheckedStructuralRecordFieldValue::Structural(node),
                });
                continue;
            }
            let (primitive_type, element_count) =
                zeroed_scalar_array_parts(self.program, declared)?;
            let zero = zeroed_integer_literal(primitive_type)?;
            let element = self
                .plans
                .nodes
                .append(checked_trees::CheckedScalarComputation {
                    authored_root: ExpressionHandle::invalid(),
                    value_source: ExpressionHandle::invalid(),
                    primitive_type,
                    kind: checked_trees::CheckedScalarComputationKind::Value(zero),
                });
            // This literal has no authored scalar occurrence. The zeroed array
            // owns its node; structural replay checks the declared carrier,
            // element count and literal zero before emission.
            let node = self.publish_structural_value(
                expression,
                declared,
                CheckedStructuralValueKind::ZeroedScalarArray {
                    element,
                    element_count,
                },
            )?;
            fields.push(checked_trees::CheckedStructuralRecordField {
                field: field.symbol,
                expression,
                type_reference: declared,
                value: checked_trees::CheckedStructuralRecordFieldValue::Structural(node),
            });
        }
        Some(CheckedStructuralValueKind::Record {
            data_symbol,
            fields: self.structural_values.record_fields.insert_many(fields),
        })
    }

    /// A case literal composes its payload fields exactly like a record's:
    /// authored initializers pair against the selected case's declarations in
    /// authored order, and each operand is a scalar computation or a nested
    /// structural value. The case tag stays with the construction, so the
    /// value keeps its discriminated identity a record cannot spell.
    pub(super) fn structural_case_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let constructor =
            validation::structural_case_constructor(self.program, expression, expected)?;
        let data_symbol = self
            .program
            .closed_data_definition(constructor.type_reference)?
            .symbol;
        if self
            .program
            .normalized_type_identity(constructor.type_reference)
            != self.program.normalized_type_identity(expected)
        {
            return None;
        }
        let mut fields = Vec::with_capacity(constructor.fields.len());
        for (ordinal, (symbol, value_expression, declared)) in
            constructor.fields.into_iter().enumerate()
        {
            // Case payloads keep the same declared storage distinction as
            // record fields: references remain structural descriptors.
            let value = if let Some(primitive) = self.program.primitive_type_reference(declared) {
                let root = self.expression(value_expression, primitive)?;
                self.plans.nodes.get_mut(root).authored_root = value_expression;
                self.push_scalar_root(
                    CheckedScalarExpressionRole::StructuralValueField {
                        expression,
                        field_ordinal: u32::try_from(ordinal).ok()?,
                    },
                    root,
                )?;
                checked_trees::CheckedStructuralRecordFieldValue::Scalar(root)
            } else {
                checked_trees::CheckedStructuralRecordFieldValue::Structural(
                    self.structural_value(value_expression, declared)?,
                )
            };
            fields.push(checked_trees::CheckedStructuralRecordField {
                field: symbol,
                expression: value_expression,
                type_reference: declared,
                value,
            });
        }
        Some(CheckedStructuralValueKind::StructuralCase {
            data_symbol,
            case: constructor.case,
            fields: self.structural_values.record_fields.insert_many(fields),
        })
    }
}
