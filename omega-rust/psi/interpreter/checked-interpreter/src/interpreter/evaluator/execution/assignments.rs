//! Assignment captures its destination before evaluating the replacement.
//!
//! The retained cell or view projection is storage identity, not an expression
//! to replay after an RHS call changes a selector. Replacement values are fully
//! materialized before touching the old value. Representation differences
//! (packed bytes and recasts) affect the write, not the evaluation schedule.
use super::replacement_cleanup::ReplacementStorage;
use crate::interpreter::evaluator::{
    ArithmeticDomain, Cell, EvalResult, Evaluator, ExpressionHandle, ExpressionNode, Frame, Halt,
    MutableRecordRecastWrite, ReferenceStorage, Value, trap,
};

enum AssignmentDestination {
    Cell(Cell),
    ArrayWindow {
        carrier: Cell,
        start: usize,
        end: usize,
        element_type: typed_trees::types::TypeReferenceHandle,
    },
    PackedByte {
        carrier: Cell,
        position: usize,
    },
    ScalarRecast {
        cell: Cell,
        recast: ReferenceStorage,
    },
    RecordRecast(MutableRecordRecastWrite),
}

impl Evaluator<'_> {
    pub(in crate::interpreter::evaluator) fn assign(
        &mut self,
        statement_index: usize,
        assignment: &typed_trees::statement::TableAssignment,
        frame: &mut Frame,
    ) -> EvalResult<()> {
        let target = assignment.target;
        let replacement = assignment.value;
        // Type queries must not execute a receiver or selector. Both coercion
        // and value-copy policy refer to the declared slot, not its old value.
        let receiving = self
            .program
            .machine_by_symbol(frame.machine_symbol)
            .and_then(|machine| {
                self.program
                    .state_in_machine(machine, frame.state_symbol)
                    .and_then(|state| {
                        validation::assignment_stored_value_type(
                            self.program,
                            machine,
                            state,
                            assignment,
                        )
                    })
            })
            .ok_or_else(|| {
                Halt::Unsupported("assignment has no selected receiving type".to_owned())
            })?;
        let coercion = if validation::reference_shell(self.program, receiving).is_none() {
            self.assignment_target_coercion(target, frame)
        } else {
            None
        };
        let destination = self.capture_assignment_destination(target, receiving, frame)?;
        let cleanup = self.capture_replacement_cleanup(
            statement_index,
            match &destination {
                AssignmentDestination::Cell(cell) => Some(ReplacementStorage::Cell(cell.clone())),
                AssignmentDestination::ArrayWindow {
                    carrier,
                    start,
                    end,
                    ..
                } => Some(ReplacementStorage::ArrayWindow {
                    carrier: carrier.clone(),
                    start: *start,
                    end: *end,
                }),
                _ => None,
            },
            frame,
        )?;
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
        let value_type = self
            .program
            .type_reference_table
            .peeled_type_reference(receiving);
        let value = match &destination {
            AssignmentDestination::ArrayWindow { element_type, .. } => {
                if matches!(
                    self.program.expression_table.expression(replacement),
                    ExpressionNode::ArrayLiteral(_)
                ) {
                    self.eval_array_literal_at_element_type(replacement, *element_type, frame)?
                } else {
                    self.eval_expression_at_type(replacement, value_type, frame)?
                }
            }
            _ => self.eval_expression_at_type(replacement, value_type, frame)?,
        };
        let copy_array =
            matches!(value, Value::Array(_)) && self.declared_type_is_fixed_array(receiving);
        let value = if matches!(value, Value::Struct { .. })
            || copy_array
            || matches!(destination, AssignmentDestination::ArrayWindow { .. })
        {
            value.deep_clone_with(&|value| self.allocate_cell(value))?
        } else {
            value
        };
        if let AssignmentDestination::ArrayWindow { start, end, .. } = &destination {
            let Value::Array(elements) = &value else {
                return trap("array window replacement is not an array value");
            };
            if elements.len() != end - start {
                return trap("array window replacement has a different element count");
            }
        }
        self.execute_replacement_cleanup(cleanup)?;
        match destination {
            AssignmentDestination::ArrayWindow {
                carrier,
                start,
                end,
                ..
            } => self.write_array_window(carrier, start, end, value),
            AssignmentDestination::RecordRecast(write) => {
                self.write_mutable_record_recast_target(write, value)
            }
            AssignmentDestination::ScalarRecast { cell, recast } => {
                let primitive = recast.target().ok_or_else(|| {
                    Halt::Trap("record recast reached the scalar write seam".to_owned())
                })?;
                let value = self.coerce_scalar_with(value, primitive, ArithmeticDomain::Exact)?;
                match recast {
                    ReferenceStorage::Direct { source, .. } => {
                        *cell.borrow_mut() = self.eval_recast(value, Some(source))?;
                        Ok(())
                    }
                    ReferenceStorage::ByteRegion {
                        cells,
                        offset,
                        target,
                    } => self.write_scalar_byte_region(&cells, offset, target, value),
                    ReferenceStorage::AggregateByteRegion { .. }
                    | ReferenceStorage::AggregateTyped { .. } => {
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
        receiving: typed_trees::types::TypeReferenceHandle,
        frame: &mut Frame,
    ) -> EvalResult<AssignmentDestination> {
        if let Some((carrier, start, end, element_type)) =
            self.capture_array_window(target, frame)?
        {
            return Ok(AssignmentDestination::ArrayWindow {
                carrier,
                start,
                end,
                element_type,
            });
        }
        if let Some(write) = self.prepare_mutable_record_recast_write(target, frame)? {
            return Ok(AssignmentDestination::RecordRecast(write));
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
        if validation::reference_shell(self.program, receiving).is_some() {
            // Store a descriptor in its selected home, not through the old
            // descriptor into its former pointee. A borrowed parameter can
            // itself reach a descriptor home: cross only the declared outer
            // references until the selected receiving type is reached.
            let mut cell = cell;
            let mut declared = self
                .program
                .machine_by_symbol(frame.machine_symbol)
                .and_then(|machine| {
                    self.program
                        .state_in_machine(machine, frame.state_symbol)
                        .and_then(|state| {
                            validation::declared_place_type_raw(
                                self.program,
                                machine,
                                Some(state),
                                target,
                            )
                        })
                })
                .ok_or_else(|| {
                    Halt::Unsupported(
                        "reference replacement has no declared destination type".to_owned(),
                    )
                })?;
            while self.program.normalized_type_identity(declared)
                != self.program.normalized_type_identity(receiving)
            {
                let shell =
                    validation::reference_shell(self.program, declared).ok_or_else(|| {
                        Halt::Unsupported(
                            "reference replacement lost its destination projection".to_owned(),
                        )
                    })?;
                let value = cell.borrow().clone();
                let Value::Ref(reference) = value else {
                    return Err(Halt::Unsupported(
                        "reference destination has no retained outer borrow".to_owned(),
                    ));
                };
                cell = reference.cell;
                declared = shell.referee;
            }
            return Ok(AssignmentDestination::Cell(cell));
        }
        if let Value::Ref(reference) = cell.borrow().clone()
            && let Some(storage) = reference.storage
        {
            return Ok(match storage {
                recast
                @ (ReferenceStorage::Direct { .. } | ReferenceStorage::ByteRegion { .. }) => {
                    AssignmentDestination::ScalarRecast {
                        cell: reference.cell,
                        recast,
                    }
                }
                recast @ (ReferenceStorage::AggregateByteRegion { projection, .. }
                | ReferenceStorage::AggregateTyped { projection, .. }) => {
                    AssignmentDestination::RecordRecast(MutableRecordRecastWrite {
                        recast,
                        projection,
                    })
                }
            });
        }
        Ok(AssignmentDestination::Cell(self.deref_cell(cell)))
    }
}
