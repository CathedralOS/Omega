use crate::{FuelSettlement, NodeLocation};
use semantic_vocabulary::{BlockId, EdgeId, MachineId};

use super::*;

#[test]
fn human_projection_names_ownership_frontier_facts_explicitly() {
    let identity = optimization_core::OwnershipFrontierFactIdentity::from_canonical_bytes(
        b"ownership-render-test",
    );
    assert_eq!(
        render_fact(OptimizationFactReference::OwnershipFrontier(identity)),
        format!("ownership-frontier:{}", hex(&identity.bytes()))
    );
}

#[test]
fn human_projection_names_value_range_facts_explicitly() {
    let identity =
        optimization_core::ValueRangeFactIdentity::from_canonical_bytes(b"value-range-render-test");
    assert_eq!(
        render_fact(OptimizationFactReference::ValueRange(identity)),
        format!("value-range:{}", hex(&identity.bytes()))
    );
}

#[test]
fn human_projection_distinguishes_charged_and_unreachable_source_fuel() {
    let location = NodeLocation {
        machine: MachineId::new(1).unwrap(),
        block: BlockId::new(2).unwrap(),
        node: 3,
    };
    let site = PsiRealizationSite::Node(location);
    let source = PsiProvenance::Edge(EdgeId::new(4).unwrap());
    let render = |disposition| {
        let mut text = String::new();
        render_provenance_rewrite(
            &mut text,
            &ProvenanceRewrite {
                input: site,
                disposition,
                sources: vec![source],
                fuel: vec![FuelSettlement {
                    site: source,
                    units: 1,
                }],
            },
        );
        text
    };
    let realized = render(ProvenanceDisposition::RealizedAt(site));
    assert!(realized.contains("realized-at: node:machine=1,block=2,node=3"));
    assert!(realized.contains("runtime-charge=1"));
    let unreachable = render(ProvenanceDisposition::ProvenUnreachableAt(site));
    assert!(unreachable.contains("proven-unreachable-at: node:machine=1,block=2,node=3"));
    assert!(unreachable.contains("runtime-charge=none reason=proven-unreachable"));
}
