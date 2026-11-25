//! Calling a boundary keeps the law it declared before the call can use its result.
use super::build_purpose::{frontier_with_core, package, roster, symbol};
use semantic_vocabulary::{
    ContentConservation, ContentPlaceVersion, ContentTerm, Proposition, StructuralPlaceKind,
};
use source_provenance::DependencyScope;
use terminal_psi::{BoundaryContentGuarantee, StructuralPlaceDeclaration};

const SOURCE: &str = r#"
use omega\language\core\Content;
use omega\language\core\CountedQuantity;
use omega\language\core\Nat;
data Bytes {}
data Region { length: u64; }
domain Region::Owned;
machine Owned::measure(subject: @Region) -> CountedQuantity<Bytes>
satisfies Content<Region, CountedQuantity<Bytes>>::project {
    CountedQuantity { magnitude: embed(subject.length) as Nat }
}
public boundary trait Transfer {
    machine pass(input: Region in Owned) -> Region
    reaches none
    ensures result in Owned, core\Identity::<CountedQuantity<Bytes>>(
        Owned::measure(old(@input)), Owned::measure(@result))
    crashes none;
}
machine relay(input: Region in Owned) -> Region { Transfer::pass(input) }
"#;

#[test]
fn the_boundary_and_declaration_keep_the_same_input_and_result_content() {
    const CONTENT: &str = include_str!("../../../../../../../../source/library/core/content.omg");
    const NAT: &str = include_str!("../../../../../../../../source/library/core/nat.omg");
    const ALGEBRA: &str = include_str!("../../../../../../../../source/library/core/algebra.omg");
    let typed = frontier_with_core(
        SOURCE,
        "machine builder() {}",
        DependencyScope::Product,
        &[
            ("content.omg", "Content", CONTENT),
            ("content.omg", "CountedQuantity", CONTENT),
            ("nat.omg", "Nat", NAT),
            ("algebra.omg", "CommutativeSemiring", ALGEBRA),
        ],
    );
    let entry = symbol(&typed, "relay");
    let prepared = crate::prepare_fixture(typed);
    let lowered = crate::lowering::elaborate_for(
        prepared.trees(),
        roster(&[entry], &[Some(package())], DependencyScope::Product),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    let module = &lowered.module;
    let [boundary] = module.interfaces.boundary_machines.as_slice() else {
        panic!("one boundary invocation")
    };
    let [BoundaryContentGuarantee::Conservation(guarantee)] =
        boundary.content_guarantees.as_slice()
    else {
        panic!("one authored conservation law")
    };
    let [input] = boundary.structural_parameters.as_slice() else {
        panic!("one structural input")
    };
    let (result, _) = boundary.structural_contract_result().unwrap();
    assert_eq!(
        guarantee.structural_places,
        [
            StructuralPlaceDeclaration {
                id: input.place,
                kind: StructuralPlaceKind::Parameter {
                    position: input.position
                }
            },
            StructuralPlaceDeclaration {
                id: result,
                kind: StructuralPlaceKind::Result
            },
        ]
    );
    let owner = module
        .types
        .structural_domains
        .iter()
        .find_map(|row| row.content_projection.as_ref())
        .unwrap();
    assert_eq!(guarantee.conservation.algebra(), &owner.algebra);
    let mut subjects = [
        guarantee.conservation.left(),
        guarantee.conservation.right(),
    ]
    .map(|term| {
        let ContentTerm::Projection {
            projection,
            subject,
        } = term
        else {
            panic!("one projection per side")
        };
        assert_eq!(*projection, owner.identity);
        assert!(subject.segments.is_empty());
        (subject.version, subject.root)
    });
    subjects.sort();
    assert_eq!(
        subjects,
        [
            (ContentPlaceVersion::Entry, input.place),
            (ContentPlaceVersion::Current, result)
        ]
    );
    let formed = |candidate: &terminal_psi::TerminalModule| {
        super::super::formation::module(
            candidate,
            &lowered.predicate_sources,
            &mut Default::default(),
        )
    };
    formed(module).unwrap();
    let mut changed = module.clone();
    let law = &guarantee.conservation;
    let published = changed
        .execution
        .declaration_contracts
        .iter_mut()
        .flat_map(|contract| &mut contract.ensures)
        .find_map(|clause| match &mut clause.proposition {
            Proposition::ContentConservation(stored) if stored == law => Some(stored),
            _ => None,
        })
        .expect("the declaration owns the same law as its boundary");
    let mut algebra = law.algebra().clone();
    algebra.parameter.push_str("OtherUnit");
    *published = ContentConservation::new(algebra, law.left().clone(), law.right().clone());
    assert!(
        formed(&changed)
            .unwrap_err()
            .detail
            .contains("declared projection and algebra")
    );

    let mut unqualified = module.clone();
    unqualified.interfaces.boundary_machines[0].structural_parameters[0]
        .qualifications
        .clear();
    assert!(
        formed(&unqualified)
            .unwrap_err()
            .detail
            .contains("exact qualification on its observed place")
    );

    let mut swapped = module.clone();
    let BoundaryContentGuarantee::Conservation(report) =
        &mut swapped.interfaces.boundary_machines[0].content_guarantees[0]
    else {
        unreachable!()
    };
    report.structural_places[0].kind = StructuralPlaceKind::Result;
    assert!(
        formed(&swapped)
            .unwrap_err()
            .detail
            .contains("differs from its declared input and result places")
    );
}
