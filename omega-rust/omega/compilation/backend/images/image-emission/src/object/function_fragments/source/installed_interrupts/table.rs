//! Check each installed address bit without changing neighboring table bits.
use super::{Binary, Error, Expr, Graph, Word, Writes};
use layout_plans::LayoutPlacementReport;
use selected_instructions::SelectedInstructionId;

pub(super) fn gate(placements: &[LayoutPlacementReport], table_bytes: u32) -> Result<u32, Error> {
    let mut covered = [false; 64];
    let mut base = None;
    for placement in placements {
        let LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } = *placement
        else {
            return Err(Error::Mismatch("installed address has a non-bit placement"));
        };
        if width == 0
            || container_width == 0
            || container_width > 64
            || container_width % 8 != 0
            || source_lsb.checked_add(width).is_none_or(|end| end > 64)
            || destination_lsb
                .checked_add(width)
                .is_none_or(|end| end > container_width)
            || container
                .checked_add(container_width / 8)
                .is_none_or(|end| end > u64::from(table_bytes))
        {
            return Err(Error::Mismatch(
                "installed address bit placement exceeds its exact container",
            ));
        }
        for position in 0..width {
            let source = source_lsb + position;
            if std::mem::replace(&mut covered[source as usize], true) {
                return Err(Error::Mismatch("installed address source bit is repeated"));
            }
            let destination = container
                .checked_mul(8)
                .and_then(|value| value.checked_add(destination_lsb + position))
                .ok_or(Error::Overflow)?;
            let hardware = if source < 16 { source } else { source + 32 };
            let current = destination.checked_sub(hardware).ok_or(Error::Mismatch(
                "installed address bit precedes its hardware gate",
            ))?;
            if current % 128 != 0 || base.is_some_and(|base| base != current) {
                return Err(Error::Mismatch(
                    "installed address bits name different hardware gates",
                ));
            }
            base = Some(current);
        }
    }
    if covered.iter().any(|value| !*value) {
        return Err(Error::Mismatch(
            "installed address omits an executable pointer bit",
        ));
    }
    let gate =
        u32::try_from(base.ok_or(Error::Mismatch("installed address has no placement"))? / 8)
            .map_err(|_| Error::Overflow)?;
    if gate.checked_add(16).is_none_or(|end| end > table_bytes) {
        return Err(Error::Mismatch("installed gate exceeds its exact table"));
    }
    Ok(gate)
}
fn op(kind: Binary, left: Expr, right: Expr) -> Expr {
    Expr::binary(kind, left, right)
}
pub(super) fn materialization(
    placements: &[LayoutPlacementReport],
    table: &Expr,
    address: &Expr,
    validate: SelectedInstructionId,
    graph: &Graph<'_>,
    writes: &mut Writes<'_>,
) -> Result<(), Error> {
    // The storage vocabulary canonically sorts fragments by their exact five
    // coordinates. This is input normalization, not execution replay.
    let mut fragments = placements
        .iter()
        .map(|placement| match *placement {
            LayoutPlacementReport::Bits {
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            } => Ok((
                container,
                container_width,
                destination_lsb,
                source_lsb,
                width,
            )),
            _ => Err(Error::Mismatch(
                "installed address lost its fragment representation",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    fragments.sort_unstable();
    let mut previous = None;
    let mut previous_fragment = None;
    for (container, container_width, destination, source, width) in fragments {
        let pointer = table
            .clone()
            .offset(u32::try_from(container).map_err(|_| Error::Overflow)?);
        let bytes = u8::try_from(container_width / 8).map_err(|_| Error::Overflow)?;
        let source_mask = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        let mask = source_mask
            .checked_shl(u32::try_from(destination).map_err(|_| Error::Overflow)?)
            .ok_or(Error::Overflow)?;
        let preserved = op(
            Binary::And,
            pointer.clone().load(bytes),
            Expr::constant(!mask),
        );
        let supplied = op(
            Binary::Left,
            op(
                Binary::And,
                op(Binary::Right, address.clone(), Expr::constant(source)),
                Expr::constant(source_mask),
            ),
            Expr::constant(destination),
        );
        let merged = op(Binary::Or, preserved, supplied);
        let current = writes.take_bytes(pointer.clone(), merged, bytes)?;
        let first = *current
            .first()
            .ok_or(Error::Mismatch("installed address fragment has no writes"))?;
        for write in current {
            if !graph.precedes(write, validate)? {
                return Err(Error::Mismatch(
                    "installed address must be written before validation",
                ));
            }
            if let Some(prior) = previous
                && !graph.precedes(prior, write)?
            {
                return Err(Error::Mismatch(
                    "installed address writes changed their order",
                ));
            }
            fresh_container(
                writes.value(write)?,
                &pointer,
                bytes,
                previous_fragment,
                first,
                graph,
            )?;
            previous = Some(write);
        }
        previous_fragment = previous;
    }
    Ok(())
}

// A preserving write must observe the container after the previous fragment.
// Equal-looking loads from an earlier storage version cannot preserve its bits.
fn fresh_container(
    value: &Expr,
    container: &Expr,
    bytes: u8,
    previous: Option<SelectedInstructionId>,
    first_write: SelectedInstructionId,
    graph: &Graph<'_>,
) -> Result<(), Error> {
    match value.word() {
        Word::Load {
            address,
            bytes: loaded,
            instruction,
        } => {
            if super::publication::within(address, *loaded, container, u32::from(bytes)) {
                let load = instruction.ok_or(Error::Mismatch(
                    "installed container observation has no instruction",
                ))?;
                if !graph.precedes(load, first_write)? {
                    return Err(Error::Mismatch(
                        "installed preserving write reads its container after writing",
                    ));
                }
                if let Some(previous) = previous
                    && !graph.precedes(previous, load)?
                {
                    return Err(Error::Mismatch(
                        "installed preserving write reuses stale container bytes",
                    ));
                }
            }
            fresh_container(address, container, bytes, previous, first_write, graph)?;
        }
        Word::Binary(_, left, right) => {
            fresh_container(left, container, bytes, previous, first_write, graph)?;
            fresh_container(right, container, bytes, previous, first_write, graph)?;
        }
        _ => {}
    }
    Ok(())
}
