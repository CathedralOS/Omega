//! Atomic syntax names an event, not an arithmetic expression over a prior
//! load. Decode its retained carrier once, keeping the storage path separate
//! from the operands and binding the prior only to the event's actual result.

use language_core::atomic::{AtomicExpressionResultCustody, AtomicOrderingPlan};
use semantic_vocabulary::{AtomicCellType, IntegerSign, IntegerType, MachineId, ScalarType};
use symbols::BuiltinTypeAtom;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{AtomicAccessEvent, AtomicReadModifyWrite};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableAtomicExpression,
};
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::authority::Access;
use super::{Program, Slot};
use crate::body::{Expression, ExpressionKind, RecordType, Statement, StorageShape, ValueType};
use crate::{SourceOrigin, Unserved, refuse, reject};

impl Program<'_> {
    pub(super) fn atomic_type(
        &mut self,
        machine: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<Option<ValueType>> {
        if !self
            .typed
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Ok(None);
        }
        let TypeReferenceNode::Named { symbol, .. } =
            self.typed.type_reference_table.type_reference(reference)
        else {
            return Ok(None);
        };
        let cell = match self.typed.symbols.builtin_type_atom(*symbol) {
            Some(BuiltinTypeAtom::AtomicBool) => AtomicCellType::Boolean,
            Some(BuiltinTypeAtom::AtomicU32) => AtomicCellType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 32).expect("u32 width"),
            ),
            Some(BuiltinTypeAtom::AtomicU64) => AtomicCellType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 width"),
            ),
            _ => return Ok(None),
        };
        if let Some((id, _)) = self
            .records
            .iter()
            .find(|(_, row)| row.shape == StorageShape::AtomicCell(cell))
        {
            return Ok(Some(ValueType::record(id)));
        }
        let identity = self
            .typed
            .type_identity_text(self.typed.normalized_type_identity(reference))
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Type("atomic storage without its builtin identity"),
                )
            })?;
        let id = self.records.push(RecordType {
            owns_linear_claim: false,
            copyable: false,
            identity,
            shape: StorageShape::AtomicCell(cell),
            requires: Vec::new(),
        });
        self.record_sources
            .push(super::arrays::physical_source(reference));
        Ok(Some(ValueType::record(id)))
    }

    fn atomic_place(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        target: ExpressionHandle,
        read: bool,
        write: bool,
    ) -> Checked<(Expression, ScalarType)> {
        let mut place = self.place(
            machine,
            slots,
            target,
            None,
            if write { Access::Write } else { Access::Read },
        )?;
        if let Some(value_type) = place.result
            && let Some((referent, _)) = self.reference_referent(machine, value_type)?
        {
            let source = place.source;
            place = Expression {
                call_context: None,
                kind: ExpressionKind::Referent(Box::new(place)),
                result: Some(referent),
                source,
            };
        }
        let Some(ValueType::Record(record, _, _)) = place.result else {
            return Err(reject(
                machine,
                place.source,
                "an atomic event requires dedicated atomic storage",
            ));
        };
        let StorageShape::AtomicCell(cell) = self.data_type(machine, record)?.shape else {
            return Err(reject(
                machine,
                place.source,
                "an atomic event requires dedicated atomic storage",
            ));
        };
        self.reference_place_access(
            machine,
            slots,
            &place,
            language_core::ReferenceAccess::Shared,
        )?;
        let root = super::storage_path::root_slot(machine, &place, &self.captures)?;
        if let Some(root) = root
            && slots.get(root).is_some_and(|slot| {
                slot.borrowed == Some(language_core::ReferenceAccess::WriteOnly)
            })
        {
            return Err(reject(
                machine,
                place.source,
                "an atomic event requires shared atomic access, not a write-only borrow",
            ));
        }
        let value_type = place.result.expect("atomic place has a type");
        if read {
            self.authorized(machine, value_type, Access::Read)?;
            let mut captures = super::storage_path::Captures::new(machine, &self.captures);
            super::storage_path::walk(
                &place,
                |capture| captures.resolve(capture),
                |_, base| {
                    if let Some(value_type) = base.result {
                        self.authorized(machine, value_type, Access::Read)?;
                    }
                    Ok(())
                },
            )?;
        }
        if write {
            self.authorized(machine, value_type, Access::Write)?;
            self.written_through(machine, &place)?;
        }
        Ok((place, cell.scalar_type()))
    }

    pub(super) fn atomic_load(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        atomic: &TableAtomicExpression,
        source: SourceOrigin,
    ) -> Checked<Expression> {
        let AtomicOrderingPlan::Load(ordering) = atomic.ordering else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Expression("atomic writing event outside its retained assignment"),
            ));
        };
        if atomic.result.is_valid()
            || atomic.result_custody != AtomicExpressionResultCustody::Scalar
            || !ordering.valid_for_load()
        {
            return Err(reject(
                machine,
                source,
                "malformed atomic load or illegal ordering",
            ));
        }
        let (place, scalar) = self.atomic_place(machine, slots, atomic.value, true, false)?;
        Ok(Expression {
            call_context: None,
            kind: ExpressionKind::Atomic(crate::body::AtomicOperation::Access {
                place: Box::new(place),
                event: AtomicAccessEvent::Load { ordering },
                operands: Vec::new(),
            }),
            result: Some(ValueType::plain(scalar)),
            source,
        })
    }

    pub(super) fn atomic_fence(
        &self,
        id: MachineId,
        target: symbols::SymbolHandle,
        arguments: &[ExpressionHandle],
        source: SourceOrigin,
    ) -> Checked<Option<Expression>> {
        use language_core::sealed_intrinsics::{CompilerPrimitive, SealedIntrinsic};
        use terminal_psi::FenceOrdering;

        let Some(machine) = self.typed.machine_by_symbol(target) else {
            return Ok(None);
        };
        if self
            .typed
            .symbols
            .sealed_declaration(target, machine.name.as_str())
            != Some(SealedIntrinsic::CompilerPrimitive(
                CompilerPrimitive::AtomicFence,
            ))
        {
            return Ok(None);
        }
        let reject_ordering = || {
            reject(
                id,
                source,
                "an atomic fence needs a known case of FenceOrdering",
            )
        };
        let (_, entry) = self.declaration(id, target)?;
        let [parameter] = self.typed.state_parameters(entry) else {
            return Err(reject_ordering());
        };
        let [argument] = arguments else {
            return Err(reject_ordering());
        };
        let definition = self
            .typed
            .closed_data_definition(parameter.type_reference)
            .ok_or_else(reject_ordering)?;
        let case_symbol = match self.node(id, *argument)? {
            ExpressionNode::Name(path) => path.symbol,
            ExpressionNode::StructLiteral(literal)
                if literal.type_symbol == definition.symbol
                    && self
                        .typed
                        .expression_table
                        .struct_fields(literal.fields)
                        .is_empty() =>
            {
                literal.case_symbol.ok_or_else(reject_ordering)?
            }
            _ => return Err(reject_ordering()),
        };
        let case = self
            .typed
            .data_members(definition)
            .iter()
            .filter_map(|member| member.as_variant())
            .find(|case| {
                case.symbol == case_symbol && self.typed.data_payload_fields(case).is_empty()
            })
            .ok_or_else(reject_ordering)?;
        let ordering = match case.name.as_str() {
            "Receive" => FenceOrdering::Receive,
            "Publish" => FenceOrdering::Publish,
            "ReceivePublish" => FenceOrdering::ReceivePublish,
            _ => return Err(reject_ordering()),
        };
        Ok(Some(Expression {
            call_context: None,
            kind: ExpressionKind::Atomic(crate::body::AtomicOperation::Fence(ordering)),
            result: None,
            source,
        }))
    }

    pub(super) fn atomic_store(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        target: ExpressionHandle,
        atomic: &TableAtomicExpression,
        source: SourceOrigin,
    ) -> Checked<Statement> {
        let AtomicOrderingPlan::Store(ordering) = atomic.ordering else {
            return Err(refuse(
                machine,
                Construct::Machine,
                Unserved::Statement("atomic update without its observed-result binding"),
            ));
        };
        if atomic.result.is_valid()
            || atomic.result_custody != AtomicExpressionResultCustody::Scalar
            || !ordering.valid_for_store()
        {
            return Err(reject(
                machine,
                source,
                "malformed atomic store or illegal ordering",
            ));
        }
        let (place, scalar) = self.atomic_place(machine, slots, target, false, true)?;
        let operand = self.atomic_operand(machine, slots, atomic.value, scalar)?;
        Ok(Statement::Evaluate {
            expression: Expression {
                call_context: None,
                kind: ExpressionKind::Atomic(crate::body::AtomicOperation::Access {
                    place: Box::new(place),
                    event: AtomicAccessEvent::Store { value: 0, ordering },
                    operands: vec![operand],
                }),
                result: None,
                source,
            },
            discards_result: false,
        })
    }

    fn atomic_operand(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        handle: ExpressionHandle,
        scalar: ScalarType,
    ) -> Checked<Expression> {
        let value = self.expression(machine, slots, handle, Some(ValueType::plain(scalar)))?;
        if value.result != Some(ValueType::plain(scalar)) {
            return Err(reject(
                machine,
                value.source,
                "atomic operand must have the cell's exact resident type",
            ));
        }
        Ok(value)
    }

    pub(super) fn atomic_binding(
        &mut self,
        machine: MachineId,
        slots: &[Slot],
        local: &TableLocalData,
        following: Option<&StatementNode>,
    ) -> Checked<(Slot, Statement)> {
        let source = SourceOrigin::of(self.typed.expression_table.source_span(local.initial_value));
        let invalid = || {
            reject(
                machine,
                source,
                "atomic observed binding has no matching event",
            )
        };
        if !local.symbol.is_valid()
            || local.relevance != language_core::BindingRelevance::Relevant
            || !matches!(self.node(machine, local.initial_value)?, ExpressionNode::Integer(value) if *value == numerics::literals::IntegerLiteral::zero())
            || !local.pattern_fields.is_empty()
            || local.pattern_subject.is_valid()
        {
            return Err(invalid());
        }
        let Some(StatementNode::Assignment(assignment)) = following else {
            return Err(invalid());
        };
        let ExpressionNode::Atomic(atomic) = self.node(machine, assignment.value)? else {
            return Err(invalid());
        };
        let atomic = *atomic;
        let same_result = |handle| matches!(self.node(machine, handle), Ok(ExpressionNode::Name(path)) if path.resolved_symbol() == Some(local.symbol));
        if !atomic.result.is_valid()
            || !same_result(atomic.result)
            || atomic.result_custody != AtomicExpressionResultCustody::Scalar
        {
            return Err(invalid());
        }
        let (event, operand) = match atomic.ordering {
            AtomicOrderingPlan::ReadModifyWrite(ordering) => {
                let ExpressionNode::Binary(binary) = self.node(machine, atomic.value)? else {
                    return Err(invalid());
                };
                if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact
                    || !same_result(binary.left)
                {
                    return Err(invalid());
                }
                let operation = match binary.operator {
                    BinaryOperator::Add => AtomicReadModifyWrite::FetchAdd,
                    BinaryOperator::Subtract => AtomicReadModifyWrite::FetchSub,
                    BinaryOperator::BitwiseAnd => AtomicReadModifyWrite::FetchAnd,
                    BinaryOperator::BitwiseOr => AtomicReadModifyWrite::FetchOr,
                    BinaryOperator::BitwiseXor => AtomicReadModifyWrite::FetchXor,
                    _ => return Err(invalid()),
                };
                (
                    AtomicAccessEvent::ReadModifyWrite {
                        operation,
                        operand: 0,
                        ordering,
                    },
                    binary.right,
                )
            }
            AtomicOrderingPlan::Swap(ordering) => {
                (AtomicAccessEvent::Swap { value: 0, ordering }, atomic.value)
            }
            _ => {
                return Err(refuse(
                    machine,
                    Construct::Machine,
                    Unserved::Statement("atomic exchange result custody"),
                ));
            }
        };
        let (place, scalar) = self.atomic_place(machine, slots, assignment.target, true, true)?;
        if !matches!(scalar, ScalarType::Integer(_)) {
            return Err(reject(
                machine,
                source,
                "atomic fetch and swap require an admitted integer resident",
            ));
        }
        let declared = self.local_type(machine, local.type_reference)?;
        if declared != ValueType::plain(scalar) {
            return Err(reject(
                machine,
                source,
                "atomic observed result must have the cell's exact resident type",
            ));
        }
        let operand = self.atomic_operand(machine, slots, operand, scalar)?;
        let value = Expression {
            call_context: None,
            kind: ExpressionKind::Atomic(crate::body::AtomicOperation::Access {
                place: Box::new(place),
                event,
                operands: vec![operand],
            }),
            result: Some(declared),
            source,
        };
        Ok((
            Slot {
                symbol: local.symbol,
                value_type: declared,
                assignable: local.is_mutable,
                homed: false,
                erased: false,
                borrowed: None,
                borrowed_regions: crate::body::Regions::NONE,
            },
            Statement::Bind {
                slot: slots.len(),
                value,
            },
        ))
    }
}
