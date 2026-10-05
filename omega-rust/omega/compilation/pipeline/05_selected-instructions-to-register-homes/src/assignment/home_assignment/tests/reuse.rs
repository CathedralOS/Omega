use super::{compute_function, fixtures::*};
use crate::assignment::home_assignment::compute::FunctionHomeReuse;
use register_model::TargetRegisterEnvironmentIdentity;

#[test]
fn reused_homes_equal_fresh_homes_and_changed_rows_recompute() {
    let physical = physical();
    let environment = TargetRegisterEnvironmentIdentity::from_bytes([1; 32]);
    let mut legality = legality(&[(0, 2), (0, 2), (0, 2)]);
    set_candidates(&mut legality, 1, &[0]);
    let separate = ranges(3, &[(0, 1)]);
    let mut reuse = FunctionHomeReuse::default();

    let fresh = compute_function(0, &legality, &separate, &physical);
    assert_eq!(
        reuse.compute(environment, 0, &legality, &separate, &physical),
        fresh
    );
    assert_eq!(reuse.reused(), 0);
    assert_eq!(
        reuse.compute(environment, 0, &legality, &separate, &physical),
        fresh
    );
    assert_eq!(reuse.reused(), 1);

    // A rewrite that changes this function's rows computes afresh.
    let interfering = ranges(3, &[(0, 1), (1, 2)]);
    assert_eq!(
        reuse.compute(environment, 0, &legality, &interfering, &physical),
        compute_function(0, &legality, &interfering, &physical)
    );
    assert_eq!(reuse.reused(), 1);

    // Another register environment discards every earlier row.
    let other = TargetRegisterEnvironmentIdentity::from_bytes([2; 32]);
    assert_eq!(
        reuse.compute(other, 0, &legality, &interfering, &physical),
        compute_function(0, &legality, &interfering, &physical)
    );
    assert_eq!(reuse.reused(), 1);
}
