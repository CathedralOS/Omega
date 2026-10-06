//! Assignment captures its destination before evaluating the replacement.
//!
//! The retained cell or view projection is storage identity, not an expression
//! to replay after an RHS call changes a selector. Replacement values are fully
//! materialized before touching the old value. Representation differences
//! (packed bytes and recasts) affect the write, not the evaluation schedule.
use crate::interpreter::evaluator::{
    ArithmeticDomain, Cell, EvalResult, Evaluator, ExpressionHandle, ExpressionNode, Frame, Halt,
    MutableRecordRecastWrite, MutableScalarRecast, TypeReferenceNode, Value, trap,
};

enum AssignmentDestination {
    Cell(Cell),
    PackedByte {
        carrier: Cell,
        position: usize,
    },
    ScalarRecast {
        cell: Cell,
        recast: MutableScalarRecast,
    },
    RecordRecast(MutableRecordRecastWrite),
}

impl Evaluator<'_> {
    pub(in crate::interpreter::evaluator) fn assign(
        &mut self,
        target: ExpressionHandle,
        replacement: ExpressionHandle,
        frame: &mut Frame,
    ) -> EvalResult<()> {
        if self.assign_array_window(target, replacement, frame)? {
            return Ok(());
        }
        // Type queries must not execute a receiver or selector. Both coercion
        // and value-copy policy refer to the declared slot, not its old value.
        let declared_type = self.expression_type_reference(target, frame);
        let coercion = self.assignment_target_coercion(target, frame);
        let destination = self.capture_assignment_destination(target, frame)?;
        if let ExpressionNode::Atomic(atomic) = self
            .program
            .expression_table
            .expression(replacement)
            .clone()
            && matches!(
                atomic.ordering,
                language_core::AtomicOrderingPlan::ReadModifyWrite(_)
                    | language_core::AtomicOrderingPlan::Swap(_)
                    | language_core::AtomicOrderingPlan::CompareExchange { .. }
            )
        {
            if !atomic.result.is_valid() {
                return trap("atomic RMW carrier lost its result place");
            }
            let AssignmentDestination::Cell(cell) = &destination else {
                return trap("atomic RMW requires an ordinary scalar place");
            };
            let prior = cell.borrow().clone();
            let result = self.resolve_place(atomic.result, frame)?;
            let result = self.deref_cell(result);
            *result.borrow_mut() = prior;
        }
        let mut value_type = declared_type.unwrap_or_default();
        while let TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } = self.program.type_reference_table.type_reference(value_type)
        {
            value_type = *referee;
        }
        let value = self.eval_expression_at_type(replacement, value_type, frame)?;
        let copy_array = matches!(value, Value::Array(_))
            && declared_type
                .map(|target| self.declared_type_is_fixed_array(target))
                .unwrap_or(false);
        let value = if matches!(value, Value::Struct { .. }) || copy_array {
            value.deep_clone_with(&|value| self.allocate_cell(value))?
        } else {
            value
        };
        match destination {
            AssignmentDestination::RecordRecast(write) => {
                self.write_mutable_record_recast_target(write, value)
            }
            AssignmentDestination::ScalarRecast { cell, recast } => {
                let primitive = recast.target().ok_or_else(|| {
                    Halt::Trap("record recast reached the scalar write seam".to_owned())
                })?;
                let value = self.coerce_scalar_with(value, primitive, ArithmeticDomain::Exact)?;
                match recast {
                    MutableScalarRecast::Direct { source, .. } => {
                        *cell.borrow_mut() = self.eval_recast(value, Some(source))?;
                        Ok(())
                    }
                    MutableScalarRecast::ByteRegion {
                        cells,
                        offset,
                        target,
                    } => self.write_scalar_byte_region(&cells, offset, target, value),
                    MutableScalarRecast::AggregateByteRegion { .. }
                    | MutableScalarRecast::AggregateTyped { .. } => {
                        trap("aggregate recast reached the scalar write seam")
                    }
                }
            }
            destination => {
                let value = match coercion {
                    Some((primitive, domain)) => {
                        self.coerce_scalar_with(value, primitive, domain)?
                    }
                    None => value,
                };
                match destination {
                    AssignmentDestination::Cell(cell) => {
                        *cell.borrow_mut() = value;
                        Ok(())
                    }
                    AssignmentDestination::PackedByte { carrier, position } => {
                        let byte = value.as_int().ok_or_else(|| {
                            Halt::Trap("carrier byte write value is not an integer".to_owned())
                        })? as u8;
                        let carrier = carrier.borrow();
                        let Value::Str(text) = &*carrier else {
                            return trap("carrier byte write lost its backing storage");
                        };
                        text.write_byte(position, byte).map_err(|length| {
                            Halt::Trap(format!(
                                "carrier byte write index {position} out of bounds (len {length})"
                            ))
                        })
                    }
                    _ => trap("recast destination reached an ordinary write"),
                }
            }
        }
    }

    fn capture_assignment_destination(
        &mut self,
        target: ExpressionHandle,
        frame: &mut Frame,
    ) -> EvalResult<AssignmentDestination> {
        if let Some(write) = self.prepare_mutable_record_recast_write(target, frame)? {
            return Ok(AssignmentDestination::RecordRecast(write));
        }
        if let Some(recast) = self.mutable_scalar_recast_target(target, frame) {
            let cell = self.resolve_place(target, frame)?;
            return Ok(AssignmentDestination::ScalarRecast {
                cell: self.deref_cell(cell),
                recast,
            });
        }
        let cell = if let ExpressionNode::Indexed(indexed) =
            self.program.expression_table.expression(target).clone()
            && !matches!(
                self.program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) {
            let carrier = self.resolve_place(indexed.collection, frame)?;
            let carrier = self.deref_cell(carrier);
            let position = self.eval_index(indexed.index, frame)?;
            if matches!(&*carrier.borrow(), Value::Str(_)) {
                return Ok(AssignmentDestination::PackedByte { carrier, position });
            }
            self.element_cell(&carrier, position)?
        } else {
            self.resolve_place(target, frame)?
        };
        Ok(AssignmentDestination::Cell(self.deref_cell(cell)))
    }
}
