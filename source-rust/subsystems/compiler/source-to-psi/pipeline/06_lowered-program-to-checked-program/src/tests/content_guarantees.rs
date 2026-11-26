//! Calling a boundary keeps the law it declared before the call can use its result.
use super::build_purpose::{frontier_with_core, package, roster, symbol};
use semantic_vocabulary::{
    ContentConservation, ContentPlaceVersion, ContentTerm, Proposition, StructuralPlaceKind,
};
use source_provenance::DependencyScope;
use terminal_psi::{BoundaryContentGuarantee, StructuralPlaceDeclaration};

const SOURCE: &str = r#"
use {omega}/language/core/content.omg;
use {omega}/language/core/nat.omg;
use {omega}/language/core/foundation.omg as proof;
data Bytes {}
data Region { length: u64; }
domain Region::Owned;
machine Owned::measure(subject: @Region) -> |content|::CountedQuantity<Bytes>
satisfies |content|::Content<Region, |content|::CountedQuantity<Bytes>>::project {
    |content|::CountedQuantity { magnitude: embed(subject.length) as |nat|::Nat }
}
public boundary trait Transfer {
    machine pass(input: Region in Owned) -> Region
    reaches none
    ensures result in Owned, |proof|::Identity::<|content|::CountedQuantity<Bytes>>(
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
            ("content.omg", "IntervalSet", CONTENT),
            ("nat.omg", "Nat", NAT),
            ("algebra.omg", "CommutativeSemiring", ALGEBRA),
        ],
    )
}

#[test]
fn checked_conservation_follows_the_owned_input_through_an_explicit_erasure() {
    let (definitions, _) = SOURCE.split_once("public boundary trait Transfer").unwrap();
    let source = format!(r#"{}machine relay(input: Region in Owned) -> Region
        ensures result in Owned, |proof|::Identity::<|content|::CountedQuantity<Bytes>>(
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
    assert_eq!(returns, [(0, vec![machine.entry_claims[0].claim])]);
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
        reaches none ensures result.left in Owned, |proof|::Identity::<|content|::CountedQuantity<Bytes>>(
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

#[test]
fn opening_a_qualified_partition_keeps_each_projection_on_its_original_claim() {
    let lower = |source| {
        let typed = with_content(source);
        let entry = symbol(&typed, "partition");
        let prepared = crate::prepare_fixture(typed);
        crate::lowering::elaborate_for(
            prepared.trees(),
            roster(&[entry], &[], DependencyScope::Product),
            terminal_codec::TerminalPublication::Invocation,
        )
    };
    let mut lowered = lower(include_str!(
        "../../../../../../../../samples/cli/facts/independent_content_partition/local/main.omg"
    )).unwrap();
    let machine = &lowered.module.machines[0];
    let id = machine.id;
    let [entry] = machine.content_entry_claims.as_slice() else {
        panic!("the two domains qualify one owned input")
    };
    assert_eq!(entry.projections.len(), 2);
    assert_eq!(machine.entry_claims[0].claim, entry.claim);
    terminal_semantics::plain_affine::verify_claims_in(&lowered.module, id).unwrap();

    lowered.module.machines[0].contract.content_entry_claims.clear();
    assert!(terminal_semantics::plain_affine::verify_claims_in(&lowered.module, id).is_err());

    let missing = lower(include_str!(
        "../../../../../../../../samples/cli/facts/independent_content_partition/invalid_local/main.omg"
    ))
    .expect_err("one law cannot account for both domains");
    assert!(missing.detail.contains("partition law for each independent input projection"));
}

#[test]
fn independently_qualified_intervals_cover_the_consumed_range_on_every_return() {
    const SOURCE: &str = include_str!(
        "../../../../../../../../samples/cli/facts/independent_content_partition/local/main.omg"
    );
    let verify = |source: &str| -> Result<(), String> {
        let typed = with_content(source);
        let entry = symbol(&typed, "partition");
        let prepared = crate::prepare_fixture(typed);
        let lowered = crate::lowering::elaborate_for(
            prepared.trees(),
            roster(&[entry], &[], DependencyScope::Product),
            terminal_codec::TerminalPublication::Invocation,
        ).map_err(|error| error.detail.to_string())?;
        let module = &lowered.module;
        let machine = module.machines.iter().find(|machine| !machine.content_entry_claims.is_empty()).unwrap();
        let claim = machine.content_entry_claims[0].claim;
        let returns = terminal_semantics::content_transformations::forwarding::checked_returns(module, machine)?;
        assert_eq!(returns, [(0, vec![claim]), (1, vec![claim])]);
        terminal_semantics::structural_qualifications::validate_shape_for_bodies(module, &[machine.id])
            .map_err(|error| error.detail.to_owned())
    };
    verify(SOURCE).unwrap();
    let branched = SOURCE.replace(
        "whole.base == 10 && whole.length == 48 && requested == 16", "requested <= whole.length",
    ).replace("let { base, length } = whole;", r#"let { base, length } = whole;
        let rest_base: u64 = match requested < length {
            true -> base + requested,
            false -> base,
        };"#).replace("right: Region { base: base + requested", "right: Region { base: rest_base");
    verify(&branched).unwrap();
    for wrong in ["base + requested - 1", "base + requested + 1"] {
        let corrupted = SOURCE.replace("right: Region { base: base + requested", &format!("right: Region {{ base: {wrong}"));
        assert!(verify(&corrupted).unwrap_err().contains("overlap or leave an unproved gap"));
    }
    let short = SOURCE.replace("length: length - requested", "length: length - requested - 1");
    assert!(
        verify(&short)
            .unwrap_err()
            .contains("do not reach the whole endpoint")
    );
}

#[test]
fn joining_owned_intervals_keeps_every_constituent_claim_for_each_domain() {
    let split = include_str!(
        "../../../../../../../../samples/cli/facts/independent_content_partition/local/main.omg"
    );
    let declarations = split.split_once("data Parts").unwrap().0;
    let join = r#"
machine join(left: Region in First & Second, right: Region in First & Second) -> Region in First & Second
requires
    embed(left.length) + embed(right.length) <= 18446744073709551615,
    left.length == 0 || right.length == 0 || embed(right.base) == embed(left.base) + embed(left.length)
ensures
    core\Identity::<IntervalSet<Nat>>(
        separate(First::content(old(@left)), First::content(old(@right))), First::content(@result)),
    core\Identity::<IntervalSet<Nat>>(
        separate(Second::content(old(@left)), Second::content(old(@right))), Second::content(@result))
{
    let { base as a, length as x } = left;
    let { base as b, length as y } = right;
    let start: u64 = match x == 0 { true -> b, false -> a, };
    Region { base: start, length: x + y } as +First +Second
}
"#;
    let verify = |body: &str| -> Result<(), String> {
        let typed = with_content(&format!("{declarations}{body}"));
        let entry = symbol(&typed, "join");
        let prepared = crate::prepare_fixture(typed);
        let lowered = crate::lowering::elaborate_for(
            prepared.trees(),
            roster(&[entry], &[], DependencyScope::Product),
            terminal_codec::TerminalPublication::Portable,
        )
        .map_err(|err| err.detail.to_string())?;
        let module = &lowered.module;
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.content_entry_claims.len() == 2)
            .unwrap();
        let mut claims: Vec<_> = machine
            .content_entry_claims
            .iter()
            .map(|binding| binding.claim)
            .collect();
        claims.sort();
        assert_ne!(claims[0], claims[1]);
        let returns = terminal_semantics::content_transformations::forwarding::checked_returns(
            module, machine,
        )?;
        assert_eq!(returns, [(0, claims.clone()), (1, claims)]);
        terminal_semantics::structural_qualifications::validate_shape_for_bodies(
            module,
            &[machine.id],
        )
        .map_err(|err| err.detail.to_owned())?;
        let verdict =
            crate::verdict::judge(Ok(lowered)).map_err(|failure| failure.detail.to_string())?;
        let RouteVerdict::Accepted(accepted) = verdict else {
            return Err(format!("{verdict:?}"));
        };
        let artifact = accepted
            .publish()
            .map_err(|failure| format!("{failure:?}"))?
            .artifact;
        let bytes = artifact.to_bytes();
        let restored = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
            .map_err(|failure| format!("{failure:?}"))?;
        assert_eq!(restored.to_bytes(), bytes);
        Ok(())
    };
    verify(join).unwrap();
    assert!(verify(&join.replace("length: x + y", "length: x")).is_err());
    assert!(verify(&join.replace("true -> b", "true -> a")).is_err());
    assert!(
        verify(&join.replace(
            "Second::content(old(@right))",
            "Second::content(old(@left))"
        ))
        .is_err()
    );
    assert!(
        verify(&join.replace(
            " || embed(right.base) == embed(left.base) + embed(left.length)",
            " || right.length > 0"
        ))
        .is_err()
    );
}

#[test]
fn a_qualified_local_receives_the_content_of_the_value_stored_in_it() {
    let (declarations, _) = include_str!(
        "../../../../../../../../samples/cli/facts/independent_content_partition/local/main.omg"
    )
    .split_once("data Parts")
    .unwrap();
    let check = |incoming: &str| {
        let source = format!(
            "{declarations}
machine identity(input: Region in {incoming}) -> Region in {incoming} {{ input }}
machine hold(input: Region in {incoming}) -> Region in First & Second {{
    let held: Region in First & Second = identity(input);
    held
}}"
        );
        let typed = with_content(&source);
        let entry = symbol(&typed, "hold");
        let prepared = crate::prepare_fixture(typed);
        let lowered = crate::lowering::elaborate_for(
            prepared.trees(),
            roster(&[entry], &[], DependencyScope::Product),
            terminal_codec::TerminalPublication::Invocation,
        );
        crate::verdict::judge(lowered)
    };
    let accepted = check("First & Second").unwrap();
    assert!(matches!(accepted, RouteVerdict::Accepted(_)), "{accepted:?}");
    let missing = check("First");
    assert!(!matches!(missing, Ok(RouteVerdict::Accepted(_))), "{missing:?}");
}

#[test]
fn a_returned_record_retires_after_its_qualified_fields_move() {
    let (partition, _) = include_str!(
        "../../../../../../../../samples/cli/facts/independent_content_partition/local/main.omg"
    )
    .split_once("machine exercise(")
    .unwrap();
    let check = |body| {
        let typed = with_content(&format!(
            "{partition}
machine consume(a: Region in First & Second, b: Region in First & Second) {{
    drop_region(a as -First -Second);
    drop_region(b as -First -Second);
}}
machine run(whole: Region in First & Second)
requires whole.base == 10 && whole.length == 48 {{
    let parts: Parts = partition(whole, 16);
    {body}
}}"
        ));
        let entry = symbol(&typed, "run");
        let prepared = crate::prepare_fixture(typed);
        crate::verdict::judge(crate::lowering::elaborate_for(
            prepared.trees(),
            roster(&[entry], &[], DependencyScope::Product),
            terminal_codec::TerminalPublication::Invocation,
        ))
    };
    let RouteVerdict::Accepted(checked) = check("consume(parts.left, parts.right);").unwrap()
    else {
        panic!("both qualified components have moved to the callee");
    };
    checked.publish_invocation().unwrap();
    for invalid in [
        "consume(parts.left, parts.left);",
        "drop_region(parts.left as -First -Second);",
    ] {
        let verdict = check(invalid);
        assert!(
            !matches!(verdict, Ok(RouteVerdict::Accepted(_))),
            "{verdict:?}"
        );
    }
}
