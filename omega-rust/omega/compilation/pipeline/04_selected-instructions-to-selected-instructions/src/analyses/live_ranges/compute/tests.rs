//! Focused live-range computation fixtures.

use selected_instructions::VirtualRegisterId;

use super::{derive_early_clobbers, derive_tied_pairs};
use selected_instructions::LiveRangePoint;

#[test]
fn component_tied_early_clobber_keeps_transitive_ties_and_only_unrelated_hazards() {
    let selected =
        crate::analyses::liveness::tests::supported_component_tied_early_clobber_function();
    let live = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    let ties = derive_tied_pairs(0, &live).unwrap();
    let rows = derive_early_clobbers(0, &live).unwrap();

    assert_eq!(ties.len(), 2);
    assert_eq!(
        ties.iter()
            .map(|tie| (tie.use_virtual_register, tie.def_virtual_register))
            .collect::<Vec<_>>(),
        vec![
            (VirtualRegisterId(0), VirtualRegisterId(1)),
            (VirtualRegisterId(1), VirtualRegisterId(3)),
        ]
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].def_virtual_register, VirtualRegisterId(3));
    assert_eq!(rows[0].early_point, LiveRangePoint(2));
    assert_eq!(rows[0].def_point, LiveRangePoint(3));
    assert_eq!(
        rows[0]
            .uses
            .iter()
            .map(|used| used.virtual_register)
            .collect::<Vec<_>>(),
        vec![VirtualRegisterId(2)]
    );

    let multiple =
        crate::analyses::liveness::tests::supported_multiple_component_tied_early_clobber_function(
        );
    let live = crate::analyses::liveness::compute::compute_function(0, &multiple).unwrap();
    assert_eq!(derive_tied_pairs(0, &live).unwrap().len(), 4);
    assert_eq!(derive_early_clobbers(0, &live).unwrap().len(), 2);
}
