//! Complete storage geometry for one private callback address.
//!
//! A callback remains one sealed source when its address occupies several bit
//! fragments. These checks close that source against the target pointer width;
//! callers then compare the returned changed-bit ranges with neighboring slots.

use crate::{LayoutPlacementReport, validate_fragment};

/// Validate one complete private address placement and return its changed-bit
/// ranges relative to the containing record. The full accessed containers must
/// fit even when only some of their bits change.
pub fn validate_private_callback_placements(
    placements: &[LayoutPlacementReport],
    pointer_size: u64,
    pointer_alignment: u64,
    storage_size: u64,
    storage_alignment: u64,
) -> Result<Vec<(u64, u64)>, String> {
    if pointer_size == 0 || pointer_size > 8 || !pointer_alignment.is_power_of_two() {
        return Err("private callback source has invalid target pointer geometry".into());
    }
    let source_bits = pointer_size * 8;
    if let [LayoutPlacementReport::At { offset }] = placements {
        if storage_alignment < pointer_alignment || !offset.is_multiple_of(pointer_alignment) {
            return Err("private callback whole-pointer placement is not aligned".into());
        }
        let end = offset
            .checked_add(pointer_size)
            .filter(|end| *end <= storage_size)
            .ok_or("private callback whole-pointer placement exceeds its storage")?;
        return Ok(vec![(
            offset
                .checked_mul(8)
                .ok_or("private callback bit offset overflows")?,
            end.checked_mul(8)
                .ok_or("private callback bit extent overflows")?,
        )]);
    }
    if placements.is_empty() {
        return Err("private callback slot has no placement".into());
    }
    let storage_size =
        usize::try_from(storage_size).map_err(|_| "private callback storage exceeds host range")?;
    let mut sources = Vec::with_capacity(placements.len());
    let mut destinations = Vec::with_capacity(placements.len());
    for placement in placements {
        let LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb,
            source_lsb,
            width,
        } = *placement
        else {
            return Err("private callback slots require one whole pointer or a complete set of Bits fragments; narrowed IntegerAt storage requires an address-fit contract".into());
        };
        let narrow = |value| {
            u16::try_from(value)
                .map_err(|_| "private callback fragment exceeds the supported scalar width")
        };
        validate_fragment(
            storage_size,
            "private callback",
            container,
            narrow(container_width)?,
            narrow(destination_lsb)?,
            narrow(source_lsb)?,
            narrow(width)?,
        )
        .map_err(|error| error.to_string())?;
        let source_end = source_lsb
            .checked_add(width)
            .filter(|end| *end <= source_bits)
            .ok_or("private callback fragment exceeds its target pointer width")?;
        let destination_start = container
            .checked_mul(8)
            .and_then(|base| base.checked_add(destination_lsb))
            .ok_or("private callback fragment destination overflows")?;
        let destination_end = destination_start
            .checked_add(width)
            .ok_or("private callback fragment destination overflows")?;
        sources.push((source_lsb, source_end));
        destinations.push((destination_start, destination_end));
    }
    sources.sort_unstable();
    let mut covered = 0;
    for (start, end) in sources {
        if start != covered {
            return Err(
                "private callback fragments have missing or overlapping source bits".into(),
            );
        }
        covered = end;
    }
    if covered != source_bits {
        return Err("private callback fragments do not cover the complete target pointer".into());
    }
    destinations.sort_unstable();
    if destinations.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err("private callback fragments overlap in destination storage".into());
    }
    Ok(destinations)
}

/// Add an inline record's byte base to every placement, preserving source bits.
pub fn compose_private_callback_placements(
    placements: &[LayoutPlacementReport],
    base: u64,
) -> Result<Vec<LayoutPlacementReport>, String> {
    placements
        .iter()
        .copied()
        .map(|mut placement| {
            let offset = match &mut placement {
                LayoutPlacementReport::At { offset }
                | LayoutPlacementReport::IntegerAt { offset, .. } => offset,
                LayoutPlacementReport::Bits { container, .. } => container,
            };
            *offset = offset
                .checked_add(base)
                .ok_or("private callback composed offset overflows")?;
            Ok(placement)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fragment(
        container: u64,
        container_width: u64,
        source_lsb: u64,
        width: u64,
    ) -> LayoutPlacementReport {
        LayoutPlacementReport::Bits {
            container,
            container_width,
            destination_lsb: 0,
            source_lsb,
            width,
        }
    }

    fn split_gate() -> [LayoutPlacementReport; 3] {
        [
            fragment(0, 16, 0, 16),
            fragment(6, 16, 16, 16),
            fragment(8, 32, 32, 32),
        ]
    }

    #[test]
    fn split_address_leaves_descriptor_fields_outside_changed_ranges() {
        let ranges = validate_private_callback_placements(&split_gate(), 8, 8, 16, 8).unwrap();
        assert_eq!(ranges, [(0, 16), (48, 64), (64, 96)]);
        // Selector, stack index, attributes and reserved word remain untouched.
        for (start, end) in [(16, 32), (32, 40), (40, 48), (96, 128)] {
            assert!(
                ranges
                    .iter()
                    .all(|(written_start, written_end)| *written_end <= start
                        || end <= *written_start)
            );
        }
        let nested = compose_private_callback_placements(&split_gate(), 24).unwrap();
        assert_eq!(
            validate_private_callback_placements(&nested, 8, 8, 40, 8).unwrap(),
            [(192, 208), (240, 256), (256, 288)]
        );
    }

    #[test]
    fn source_coverage_rejects_missing_repeated_and_target_excess_bits() {
        let mut placements = split_gate();
        placements[1] = fragment(6, 16, 17, 15);
        assert!(
            validate_private_callback_placements(&placements, 8, 8, 16, 8)
                .unwrap_err()
                .contains("missing or overlapping")
        );
        placements[1] = fragment(6, 16, 0, 16);
        assert!(
            validate_private_callback_placements(&placements, 8, 8, 16, 8)
                .unwrap_err()
                .contains("missing or overlapping")
        );
        assert!(
            validate_private_callback_placements(&split_gate(), 4, 4, 16, 8)
                .unwrap_err()
                .contains("target pointer width")
        );
    }

    #[test]
    fn container_bounds_and_changed_bit_overlap_are_separate_obligations() {
        let disjoint = [
            LayoutPlacementReport::Bits {
                container: 0,
                container_width: 64,
                destination_lsb: 0,
                source_lsb: 0,
                width: 32,
            },
            LayoutPlacementReport::Bits {
                container: 0,
                container_width: 64,
                destination_lsb: 32,
                source_lsb: 32,
                width: 32,
            },
        ];
        assert_eq!(
            validate_private_callback_placements(&disjoint, 8, 8, 8, 1).unwrap(),
            [(0, 32), (32, 64)]
        );
        let mut overlap = disjoint;
        overlap[1] = fragment(0, 64, 32, 32);
        assert!(
            validate_private_callback_placements(&overlap, 8, 8, 8, 8)
                .unwrap_err()
                .contains("overlap in destination")
        );
        // Only the low bits change, but the declared 64-bit access still must fit.
        let oversized = [fragment(0, 64, 0, 32)];
        assert!(validate_private_callback_placements(&oversized, 4, 4, 4, 4).is_err());
        assert!(compose_private_callback_placements(&split_gate(), u64::MAX).is_err());
    }

    #[test]
    fn whole_pointer_still_requires_alignment_and_full_extent() {
        assert!(
            validate_private_callback_placements(
                &[LayoutPlacementReport::At { offset: 6 }],
                8,
                8,
                16,
                8
            )
            .is_err()
        );
        assert!(
            validate_private_callback_placements(
                &[LayoutPlacementReport::At { offset: 8 }],
                8,
                8,
                12,
                8
            )
            .is_err()
        );
        assert!(
            validate_private_callback_placements(
                &[LayoutPlacementReport::At { offset: 0 }],
                8,
                8,
                8,
                1
            )
            .is_err()
        );
        assert!(validate_private_callback_placements(&[], 8, 8, 16, 8).is_err());
    }
}
