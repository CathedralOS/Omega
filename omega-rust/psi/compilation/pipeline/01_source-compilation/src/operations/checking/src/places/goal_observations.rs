//! Sufficient scalar goals after an indexed store. A selected cell contains
//! either its prior value or the stored operand. Requiring a predicate for
//! both preserves it without inventing an equality or assuming a domain from
//! the destination's type. Ordinary reads still require one exact identity.

use super::*;

impl Held<'_, '_> {
    pub(super) fn observation_alternatives(
        &mut self,
        leaf: &Lineage,
        scalar_type: ScalarType,
    ) -> Result<Vec<ScalarTerm>, crate::checking::Unbound> {
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
                    Err(crate::checking::Unbound::Unproved(_)) => {}
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
                    .map_err(crate::checking::Unbound::Refused)?;
                let begin = storage.log.partition_point(|row| row.block < self.block);
                let end = storage.log.partition_point(|row| row.block <= self.block);
                // The write owner resolved this backing at the store. Its
                // original descriptor path remains separate for permissions.
                let row = storage.log[begin..end]
                    .iter()
                    .rev()
                    .find(|row| row.position <= self.up_to && overlap(&row.backing, &backed))
                    .ok_or(crate::checking::Unbound::Unproved(
                        "the scalar observation has no local indexed store correspondence",
                    ))?;
                let Stored::Value(value) = row.stored else {
                    return Err(crate::checking::Unbound::Unproved(
                        "the last overlapping store supplies no scalar operand",
                    ));
                };
                let written = row.backing.single_place().ok_or(crate::checking::Unbound::Unproved(
                    "an indexed store has more than one possible cell",
                ))?;
                let selected = backed.single_place().ok_or(crate::checking::Unbound::Unproved(
                    "an indexed observation has more than one possible cell",
                ))?;
                if written.root != selected.root
                    || written.segments.len() != selected.segments.len()
                {
                    return Err(crate::checking::Unbound::Unproved(
                        "the store and observed cell have different storage paths",
                    ));
                }
                let mut uncertain = false;
                for (written, observed) in written.segments.iter().zip(&selected.segments) {
                    if written == observed {
                        continue;
                    }
                    if !uncertain
                        && matches!(
                            (written, observed),
                            (Segment::RuntimeIndex { .. }, Segment::FixedIndex(_))
                        )
                    {
                        uncertain = true;
                    } else {
                        return Err(crate::checking::Unbound::Unproved(
                            "the indexed store has no single-cell correspondence to the observation",
                        ));
                    }
                }
                if !uncertain {
                    return Err(crate::checking::Unbound::Unproved(
                        "the scalar store has no uncertain selector",
                    ));
                }
                let supplied = terminal_check_tables::row(
                    &self.tables.values,
                    value,
                    storage.machine,
                    self.concerns,
                )
                .map_err(crate::checking::Unbound::Refused)?
                .scalar_type;
                if supplied != scalar_type {
                    return Err(crate::checking::Unbound::Unsupported(
                        "an indexed store operand has another scalar carrier",
                    ));
                }
                let position = row
                    .position
                    .checked_sub(1)
                    .ok_or(crate::checking::Unbound::Unsupported(
                        "an indexed store has no prestate",
                    ))?;
                let block = storage.block(self.block).map_err(crate::checking::Unbound::Refused)?;
                let before = position
                    .checked_sub(1)
                    .map_or(ProgramPoint::BlockEntry(self.block), |previous| {
                        ProgramPoint::AfterOperation(block.operations[previous].id)
                    });
                alternatives.push(ScalarTerm::value(value, scalar_type));
                self.up_to = position;
                self.point = Some(before);
                observed = backed.place;
            }
        })();
        (self.up_to, self.point) = saved;
        result
    }
}
