//! Run the emitted supply checks without allocating the described backing.
//! A billion cells must cost only the bounded profile walk, while missing
//! permission at the last cell still prevents the transfer.

use super::*;
use compatibility::run::{RunGeometry, SupplyRole};

#[test]
fn run_supply_checks_cover_the_last_cell_without_walking_the_run() {
    let placement = initialization::aligned_placement();
    let footprint = placement.layout().size.unwrap();
    let mut declarations = Types::default();
    let profile_type = declarations.profile();
    declarations.expose_scalar_fields();
    let layout = ProfileLayout::derive(&declarations.0, profile_type).unwrap();

    for target in targets() {
        for (count, base, length) in [
            (1_000_000_000, 4096, 1_000_000_000 * footprint),
            (1, 4097, footprint),
            (2, 4096, footprint),
            (1, u64::MAX - 7, footprint),
            (0, 0, 0),
        ] {
            with_builder(target, |builder, row| {
                let profile = profile_home(builder, row);
                let scratch = ImageHome::scratch(
                    builder,
                    row,
                    profile.place,
                    compatibility::run::SCRATCH_BYTES,
                )
                .unwrap();
                let geometry = RunGeometry {
                    count: fragmented_scalar::constant(builder, row, profile.place, count).unwrap(),
                    base: fragmented_scalar::constant(builder, row, profile.place, base).unwrap(),
                    length: fragmented_scalar::constant(builder, row, profile.place, length)
                        .unwrap(),
                };
                let assessments = [SupplyRole::Source, SupplyRole::Destination].map(|role| {
                    compatibility::run::assess(
                        &declarations.0,
                        profile_type,
                        builder,
                        row,
                        profile,
                        scratch,
                        geometry,
                        &placement,
                        role,
                    )
                    .unwrap()
                });
                let valid = ResourceProfile {
                    regions: if length == 0 {
                        vec![]
                    } else {
                        vec![region(0, length)]
                    },
                };
                let mut profiles = vec![valid.clone()];
                if length >= footprint {
                    for stable in [StableCapability::Read, StableCapability::Write] {
                        let mut last = region(length - footprint, footprint);
                        last.stable = stable;
                        let mut regions = vec![last];
                        if length > footprint {
                            regions.push(region(0, length - footprint));
                        }
                        profiles.push(ResourceProfile { regions });
                    }
                }
                let source = access_plans::validate_resource_profile(valid, length).unwrap();
                for profile in profiles {
                    let normalized =
                        access_plans::validate_resource_profile(profile.clone(), length).unwrap();
                    let geometry_valid = count.checked_mul(footprint) == Some(length)
                        && base.checked_add(length).is_some();
                    let source_valid = geometry_valid
                        && access_plans::require_stable_region_supply(
                            &normalized,
                            0,
                            length,
                            true,
                            false,
                            placement.reach(),
                            "test source",
                        )
                        .is_ok();
                    let destination_valid = geometry_valid
                        && access_plans::validate_run_relocation_resources(
                            &placement,
                            count,
                            base,
                            &source,
                            &normalized,
                        )
                        .is_ok();
                    let mut bytes = encode(&layout, &profile);
                    bytes.resize(
                        layout.bytes as usize + compatibility::run::SCRATCH_BYTES as usize + 16,
                        0xd7,
                    );
                    let registers = run_values(builder, &mut bytes, u64::from(layout.bytes) + 8);
                    for (assessment, expected) in
                        assessments.iter().zip([source_valid, destination_valid])
                    {
                        assert_eq!(registers[assessment.well_formed.0 as usize], 1);
                        assert_eq!(
                            registers[assessment.compatible.0 as usize],
                            u64::from(expected),
                            "count={count}, base={base}, length={length}, profile={profile:?}"
                        );
                    }
                }
            });
        }
    }
}
