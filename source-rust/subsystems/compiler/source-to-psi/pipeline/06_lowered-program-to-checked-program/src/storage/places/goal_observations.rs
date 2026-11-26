//! Sufficient scalar goals after an indexed store. A selected cell contains
//! either its prior value or the stored operand. Requiring a predicate for
//! both preserves it when the index is unknown. A captured constant index
//! selects just the matching cell. Ordinary reads still require one exact identity.

use super::*;

impl<'a> Storage<'a> {
    pub(super) fn resolve_write_indices(
        &self,
        references: &mut DescriptorAnalysis<'a>,
        tables: &MachineTables,
        types: &IdTable<ValueId, ScalarType>,
    ) -> Checked<IdTable<ValueId, Option<u64>>> {
        use semantic_vocabulary::IntegerValue;
        use terminal_semantics::{ScalarConstantOrigin, ScalarConstants};

        let mut fixed = IdTable::default();
        let mut constants = None;
        let mut incoming = None;
        for selector in self
            .log
            .iter()
            .filter_map(|row| row.backing.single_place())
            .flat_map(|place| &place.segments)
            .filter_map(|step| match step {
                Segment::RuntimeIndex { selector } => Some(*selector),
                _ => None,
            })
        {
            let constants = constants.get_or_insert_with(|| {
                ScalarConstants::for_values(tables.values.iter().map(|(id, _)| id))
            });
            let mut refusal = None;
            let known = constants.value(selector, |id| {
                let row = tables.values.get(id)?;
                if let ValueOrigin::BlockParameter { .. } = row.origin {
                    let arrivals = incoming
                        .get_or_insert_with(|| {
                            terminal_semantics::MachineScalarIndex::new(self.lowered)
                        })
                        .incoming_values(id)?;
                    if arrivals
                        .iter()
                        .any(|(_, value)| types.get(*value) != Some(&row.scalar_type))
                    {
                        return None;
                    }
                    return Some(ScalarConstantOrigin::Incoming(
                        arrivals.into_iter().map(|(_, value)| value).collect(),
                    ));
                }
                let ValueOrigin::Operation(operation) = row.origin else {
                    return None;
                };
                let op = self.lowered.operation(operation)?;
                if let Some(origin) = terminal_semantics::scalar_operation_origin(op, id, types) {
                    return Some(origin);
                }
                // A storage read names the contents at the operation that captured
                // the selector, even if the field changes before the array is used.
                let Some(Proposition::Equal(left, right)) = self
                    .read_equation(references, tables, op)
                    .map_err(|error| refusal = Some(error))
                    .ok()?
                else {
                    return None;
                };
                (left == ScalarTerm::value(id, row.scalar_type))
                    .then_some(ScalarConstantOrigin::Computed(right))
            });
            if let Some(error) = refusal {
                return Err(error);
            }
            if let Some((ScalarType::Integer(_), value)) = known {
                let position = match value {
                    IntegerValue::Signed(value) => u64::try_from(value).ok(),
                    IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
                };
                if let Some(position) = position {
                    if fixed.is_empty() {
                        fixed = tables.values.map(|_, _| None);
                    }
                    *fixed.get_mut(selector).expect("registered selector") = Some(position);
                }
            }
        }
        Ok(fixed)
    }
}

impl Held<'_, '_> {
    pub(super) fn observation_alternatives(
        &mut self,
        leaf: &Lineage,
        scalar_type: ScalarType,
    ) -> Result<Vec<ScalarTerm>, crate::Unbound> {
        let saved = (self.up_to, self.point);
        let mut observed = leaf.clone();
        let mut alternatives = Vec::new();
        let result = (|| {
            // Positions strictly decrease. Multiple stores add a linear roster
            // of operands, without recursive calls or cell-assignment products.
            loop {
                match self.observation(&observed, scalar_type) {
                    Ok(value) => {
                        alternatives.push(value);
                        alternatives.sort();
                        alternatives.dedup();
                        return Ok(alternatives);
                    }
                    Err(crate::Unbound::Unproved(_)) => {}
                    Err(error) => return Err(error),
                }
                let storage = self.storage;
                let backed = storage
                    .backing(
                        self.references,
                        &observed,
                        self.block,
                        self.up_to,
                        self.concerns,
                    )
                    .map_err(crate::Unbound::Refused)?;
                let begin = storage.log.partition_point(|row| row.block < self.block);
                let end = storage.log.partition_point(|row| row.block <= self.block);
                // The write owner resolved this backing at the store. Its
                // original descriptor path remains separate for permissions.
                let row = storage.log[begin..end]
                    .iter()
                    .rev()
                    .find(|row| row.position <= self.up_to && overlap(&row.backing, &backed))
                    .ok_or(crate::Unbound::Unproved(
                        "the scalar observation has no local indexed store correspondence",
                    ))?;
                let Stored::Value(value) = row.stored else {
                    return Err(crate::Unbound::Unproved(
                        "the last overlapping store supplies no scalar operand",
                    ));
                };
                let written = row.backing.single_place().ok_or(crate::Unbound::Unproved(
                    "an indexed store has more than one possible cell",
                ))?;
                let selected = backed.single_place().ok_or(crate::Unbound::Unproved(
                    "an indexed observation has more than one possible cell",
                ))?;
                if written.root != selected.root
                    || written.segments.len() != selected.segments.len()
                {
                    return Err(crate::Unbound::Unproved(
                        "the store and observed cell have different storage paths",
                    ));
                }
                let mut selector = None;
                for (written, observed) in written.segments.iter().zip(&selected.segments) {
                    if written == observed {
                        continue;
                    }
                    if selector.is_none()
                        && let (Segment::RuntimeIndex { selector: id }, Segment::FixedIndex(index)) =
                            (written, observed)
                    {
                        selector = Some((*id, *index));
                    } else {
                        return Err(crate::Unbound::Unproved(
                            "the indexed store has no single-cell correspondence to the observation",
                        ));
                    }
                }
                let (selector, observed_index) = selector.ok_or(crate::Unbound::Unproved(
                    "the scalar store has no uncertain selector",
                ))?;
                let written_index = storage.fixed_write_indices.get(selector).copied().flatten();
                let supplied = terminal_check_tables::row(
                    &self.tables.values,
                    value,
                    storage.machine,
                    self.concerns,
                )
                .map_err(crate::Unbound::Refused)?
                .scalar_type;
                if supplied != scalar_type {
                    return Err(crate::Unbound::Unsupported(
                        "an indexed store operand has another scalar carrier",
                    ));
                }
                let position = row
                    .position
                    .checked_sub(1)
                    .ok_or(crate::Unbound::Unsupported(
                        "an indexed store has no prestate",
                    ))?;
                let block = storage.block(self.block).map_err(crate::Unbound::Refused)?;
                let before = position
                    .checked_sub(1)
                    .map_or(ProgramPoint::BlockEntry(self.block), |previous| {
                        ProgramPoint::AfterOperation(block.operations[previous].id)
                    });
                if written_index.is_none_or(|index| index == observed_index) {
                    alternatives.push(ScalarTerm::value(value, scalar_type));
                    if written_index.is_some()
                        && selected.segments.iter().all(|segment| {
                            !matches!(segment, Segment::RuntimeIndex { .. } | Segment::Referent)
                        })
                    {
                        alternatives.sort();
                        alternatives.dedup();
                        return Ok(alternatives);
                    }
                }
                self.up_to = position;
                self.point = Some(before);
                observed = backed.place;
            }
        })();
        (self.up_to, self.point) = saved;
        result
    }
}
