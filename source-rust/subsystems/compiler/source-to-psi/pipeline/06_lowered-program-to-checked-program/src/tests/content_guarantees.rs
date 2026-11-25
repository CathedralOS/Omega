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
    let typed = with_content(SOURCE);
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

fn with_content(source: &str) -> crate::source_state::TypedTrees {
    const CONTENT: &str = include_str!("../../../../../../../../source/library/core/content.omg");
    const NAT: &str = include_str!("../../../../../../../../source/library/core/nat.omg");
    const ALGEBRA: &str = include_str!("../../../../../../../../source/library/core/algebra.omg");
    frontier_with_core(
        source,
        "machine builder() {}",
        DependencyScope::Product,
        &[
            ("content.omg", "Content", CONTENT),
            ("content.omg", "CountedQuantity", CONTENT),
            ("nat.omg", "Nat", NAT),
            ("algebra.omg", "CommutativeSemiring", ALGEBRA),
        ],
    )
}

#[test]
fn checked_conservation_follows_the_owned_input_through_an_explicit_erasure() {
    let (definitions, _) = SOURCE.split_once("public boundary trait Transfer").unwrap();
    let source = format!(r#"{}machine relay(input: Region in Owned) -> Region
        ensures result in Owned, core\Identity::<CountedQuantity<Bytes>>(
            Owned::measure(old(@input)), Owned::measure(@result))
        {{ input as -Owned }}"#, definitions.replace("data Region {", "data Region [linear] {"));
    let typed = with_content(&source);
    let entry = symbol(&typed, "relay");
    let prepared = crate::prepare_fixture(typed);
    let lowered = crate::lowering::elaborate_for(
        prepared.trees(),
        roster(&[entry], &[Some(package())], DependencyScope::Product),
        terminal_codec::TerminalPublication::Invocation,
    ).unwrap();
    let crate::verdict::RouteVerdict::Accepted(accepted) = crate::verdict::judge(Ok(lowered)).unwrap()
        else { panic!("an unchanged qualified input establishes its exact normal content") };
    let module = &accepted.elaboration.module;
    let machine = module.machines.iter().find(|machine| machine.ensures.iter().any(|clause|
        matches!(clause.proposition, Proposition::ContentConservation(_))
    )).unwrap();
    let returns = terminal_semantics::content_transformations::forwarding::checked_returns(module, machine).unwrap();
    assert_eq!(returns, [(0, machine.entry_claims[0].claim)]);
    let mut changed = module.clone();
    let candidate = changed.machines.iter_mut().find(|body| body.id == machine.id).unwrap();
    for block in &mut candidate.blocks {
        if let terminal_psi::Terminator::ReturnStructural { returned_claims, .. } = &mut block.terminator {
            returned_claims[0].claim = semantic_vocabulary::ClaimId::new(999).unwrap();
        }
    }
    assert!(terminal_semantics::content_transformations::forwarding::checked_returns(
        &changed, changed.machine(machine.id).unwrap()).is_err());
}

#[test]
fn selected_provider_must_return_the_promised_input_under_its_result_field() {
    let (definitions, _) = SOURCE.split_once("public boundary trait Transfer").unwrap();
    let definitions = definitions.replace("data Region {", "data Region [linear] {");
    let header = r#"machine pass(first: Region in Owned, second: Region in Owned) -> Pair
        reaches none ensures result.left in Owned, core\Identity::<CountedQuantity<Bytes>>(
            Owned::measure(old(@first)), Owned::measure(@result.left)) crashes none"#;
    let checked = |supplied: &str, left: &str, right: &str| {
        let source = format!(r#"{definitions}
            data Pair {{ left: Region in Owned; right: Region in Owned; }}
            boundary trait Transfer {{ {header}; }}
            Native satisfies Transfer {{ {supplied} {{ Pair {{ left: {left}, right: {right} }} }} }}
            machine entry() {{}}
        "#);
        super::provider_supplies::supplied_trees(
            with_content(&source), "Transfer", &[Some(package())],
        )
    };
    let selected = checked(header, "first", "second").unwrap();
    assert!(selected.iter().any(|name| name.ends_with("Native::pass")), "{selected:?}");
    let refused = checked(header, "second", "first").unwrap_err();
    assert!(refused.contains("normal return does not forward the content claim of the promised input"), "{refused}");

    let different_input = header.replace("old(@first)", "old(@second)");
    let refused = checked(&different_input, "second", "first").unwrap_err();
    assert!(
        refused.contains("does not promise the exact declared content equation")
            || refused.contains("does not forward the content claim of the promised input"),
        "{refused}"
    );
}
