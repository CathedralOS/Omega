use super::fixture::{
    ARM64_CHAIN_USAGE, ARM64_EXACT_USAGE, X64_CHAIN_USAGE, X64_EXACT_USAGE, analyze, chain,
    chain_exact_budget, exact_budget, generous_budget, source,
};
use crate::tests::NativeTarget;

#[test]
fn chained_forwarded_parameter_partitions_across_a_fragment_chain() {
    for (target, expected_usage) in [
        (NativeTarget::linux_x64(), X64_CHAIN_USAGE),
        (NativeTarget::linux_arm64(), ARM64_CHAIN_USAGE),
    ] {
        let fixture = chain(target);
        let first = analyze(&fixture, chain_exact_budget(target)).unwrap();
        let second = analyze(&fixture, chain_exact_budget(target)).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().target(), target);
        assert_eq!(first.receipt().function_count(), 1);
        assert_eq!(first.receipt().register_count(), 3);
        assert_eq!(first.receipt().fragment_count(), 5);
        assert_eq!(first.receipt().segment_count(), 5);
        assert_eq!(first.receipt().incompatible_fixed_use_boundary_count(), 0);
        assert_eq!(first.receipt().usage(), expected_usage);
        assert_eq!(
            first.receipt().identity(),
            register_homes::fixed_precolored_split_requirement_plan_identity(first.plan())
        );

        let registers = &first.plan().functions[0].registers;
        let forwarded = &registers[1];
        assert_eq!(forwarded.fragments.len(), 3);
        for fragment in &forwarded.fragments {
            assert_eq!(fragment.segments.len(), 1);
            assert!(fragment.segments[0].candidates.len() > 1);
        }
        // The chain's distinguishing shape: the leaf fragment's incoming
        // connector originates at the middle fragment, not the source block.
        let connector_of = |index: usize| match forwarded.fragments[index].segments[0].opening {
            register_homes::FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 {
                connector,
            } => connector,
            _ => panic!("non-source fragment opens across an incoming edge"),
        };
        let middle = connector_of(1);
        assert_eq!(middle.source, forwarded.fragments[0].block);
        assert_eq!(middle.target, forwarded.fragments[1].block);
        let leaf = connector_of(2);
        assert_eq!(leaf.source, forwarded.fragments[1].block);
        assert_eq!(leaf.target, forwarded.fragments[2].block);
        assert_ne!(leaf.source, forwarded.fragments[0].block);

        let replayed =
            selected_instructions_to_register_homes::validate_fixed_precolored_split_requirements(
                fixture.source.live_range_stage().ranges(),
                fixture.source.legality(),
                &fixture.fixed,
                first.plan().clone(),
            )
            .unwrap();
        assert_eq!(replayed, first);
        assert_eq!(
            first.receipt().fixed_intervals(),
            fixture.fixed.receipt().identity()
        );
        assert_eq!(
            first.receipt().ranges(),
            fixture
                .source
                .live_range_stage()
                .ranges()
                .receipt()
                .identity()
        );
        assert_eq!(
            first.receipt().legality(),
            fixture.source.legality().receipt().identity()
        );
    }
}

#[test]
fn chained_forwarded_parameter_shares_one_home_domain_across_the_chain() {
    use crate::tests::stage_optimized_fixed_precolored_segment_homes;

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged =
            stage_optimized_fixed_precolored_segment_homes(chain(target).source, generous_budget())
                .unwrap();
        let assignments = &staged.segment_homes().plan().functions[0].assignments;
        let forwarded = assignments
            .iter()
            .filter(|assignment| assignment.virtual_register.0 == 1)
            .collect::<Vec<_>>();
        // All three chain fragments land in one allocation domain on one view:
        // the middle fragment's domain is the parent of the leaf's.
        assert_eq!(forwarded.len(), 3);
        assert_eq!(forwarded[0].view, forwarded[1].view);
        assert_eq!(forwarded[0].view, forwarded[2].view);
        assert_eq!(
            forwarded[0].allocation_domain,
            forwarded[1].allocation_domain
        );
        assert_eq!(
            forwarded[0].allocation_domain,
            forwarded[2].allocation_domain
        );
        assert_eq!(
            staged.segment_homes().receipt().split_requirements(),
            staged.split_requirements().receipt().identity()
        );
    }
}

#[test]
fn chained_forwarded_parameter_completes_the_leaf_local_fixed_view_sequence() {
    use crate::tests::stage_leaf_local_fixed_view_register_allocation;

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        // Before the tree-topology admission this route stopped inside the
        // fixed segment preflight with UnsupportedCrossBlockRange; the whole
        // sequence now assigns homes through the reanalyzed chain.
        let homes = stage_leaf_local_fixed_view_register_allocation(chain(target).source).unwrap();
        let custody = homes.custody().source().source();
        assert_eq!(
            custody.policy(),
            crate::tests::FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1
        );
        assert_eq!(custody.copy_count(), 0);
    }
}

#[test]
fn forwarded_conditional_keeps_abi_transfers_outside_semantic_value_domains() {
    for (target, entry_name, result_name) in [
        (NativeTarget::linux_x64(), "rsi", "rax"),
        (NativeTarget::linux_arm64(), "x1", "x0"),
    ] {
        let fixture = source(target);
        let first = analyze(&fixture, exact_budget(target)).unwrap();
        let second = analyze(&fixture, exact_budget(target)).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.receipt().target(), target);
        assert_eq!(first.receipt().function_count(), 1);
        assert_eq!(first.receipt().register_count(), 6);
        assert_eq!(first.receipt().fragment_count(), 8);
        assert_eq!(first.receipt().segment_count(), 8);
        assert_eq!(first.receipt().incompatible_fixed_use_boundary_count(), 0);
        assert_eq!(
            first.receipt().usage(),
            if target == NativeTarget::linux_x64() {
                X64_EXACT_USAGE
            } else {
                ARM64_EXACT_USAGE
            }
        );
        assert_eq!(
            first.receipt().identity(),
            register_homes::fixed_precolored_split_requirement_plan_identity(first.plan())
        );
        let environment = fixture
            .source
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment();
        let named = |name| environment.physical().model().view_named(name).unwrap().id;
        let registers = &first.plan().functions[0].registers;
        assert_eq!(
            registers[1].fragments[0].segments[0].candidates,
            [named(entry_name)]
        );
        let forwarded = &registers[3];
        assert_eq!(forwarded.fragments.len(), 3);
        for fragment in &forwarded.fragments {
            assert_eq!(fragment.segments.len(), 1);
            assert!(fragment.segments[0].candidates.len() > 1);
        }
        for fragment in &forwarded.fragments[1..] {
            assert!(matches!(
                fragment.segments[0].opening,
                register_homes::FixedPrecoloredSourceSegmentOpening::IncomingSourceEdgeV1 { .. }
            ));
        }
        for returned in &registers[4..] {
            assert_eq!(
                returned.fragments[0].segments[0].candidates,
                [named(result_name)]
            );
        }

        let replayed =
            selected_instructions_to_register_homes::validate_fixed_precolored_split_requirements(
                fixture.source.live_range_stage().ranges(),
                fixture.source.legality(),
                &fixture.fixed,
                first.plan().clone(),
            )
            .unwrap();
        assert_eq!(replayed, first);
        assert_eq!(
            first.receipt().fixed_intervals(),
            fixture.fixed.receipt().identity()
        );
        assert_eq!(
            first.receipt().ranges(),
            fixture
                .source
                .live_range_stage()
                .ranges()
                .receipt()
                .identity()
        );
        assert_eq!(
            first.receipt().legality(),
            fixture.source.legality().receipt().identity()
        );
    }
}
