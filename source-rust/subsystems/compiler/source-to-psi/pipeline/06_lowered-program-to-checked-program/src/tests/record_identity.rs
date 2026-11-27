//! Identity of two records that have storage, from the identities of their
//! fields.
//!
//! A record whose fields all have a kernel carrier is the product of those
//! carriers, and an immutable input of it is the tuple of what its fields
//! held at entry. `a = b` at `Point` is proved field by field from the entry
//! requirements `a.x = b.x` and `a.y = b.y`; the kernel converts each tuple
//! back into its record. When one field identity is missing, the refusal
//! names that field ([record identity](wiki/spec/proofs/contracts.md#record-identity-from-field-identities)).

use super::*;

const FROM_FIELDS: &str =
    include_str!("../../tests/fixtures/record_identity/pass/point_from_fields/main.omg");
const MISSING_Y: &str =
    include_str!("../../tests/fixtures/record_identity/fail/point_missing_y/main.omg");

fn judged(source: &str) -> RouteVerdict {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[],
    );
    let prepared = crate::prepare_fixture(typed);
    let scope = source_provenance::DependencyScope::Product;
    let compilation = crate::lowering::prepare_compilation(
        &prepared,
        super::build_purpose::roster(&[], &[Some(super::build_purpose::package())], scope),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    let mut parts = crate::judge_compilation(compilation.program).into_iter();
    let verdict = parts.next().expect("the program's judgment").unwrap();
    assert!(parts.next().is_none(), "one judged part");
    verdict
}

#[test]
fn a_record_is_identical_when_every_field_is() {
    let verdict = judged(FROM_FIELDS);
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("the record identity must be proved: {verdict:?}")
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    let [(_, Ok(Discharged::Identity(judgment)))] = guarantees.as_slice() else {
        panic!("expected one judged Identity, got {guarantees:?}")
    };
    // Both entry requirements are cited, and nothing else.
    assert_eq!(judgment.citations.len(), 2);
    let MathematicalCoreDecision::Judged(receipt) = judgment.decision;
    // The two requirements are the proof's hypotheses; it assumes only the
    // `i32` carrier and the four values the fields held at entry.
    assert_eq!((receipt.context_depth, receipt.assumption_closure), (2, 5));
    let mut checked =
        terminal_codec::decode_mathematical_certificate(&judgment.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
    // The statement is kernel Id at the record, and the record is declared
    // as the product of its two `i32` fields.
    use proof_admission::Term;
    let Term::Id { ty, .. } = checked.arena.get(checked.certificate.expected) else {
        panic!("the contract must denote kernel Id")
    };
    let Term::Constant { declaration, .. } = checked.arena.get(ty) else {
        panic!("the record is a declared constant")
    };
    let Some(Term::Sigma { domain, codomain }) = checked.certificate.signature
        [declaration as usize]
        .body
        .map(|body| checked.arena.get(body))
    else {
        panic!("the record is the product of its fields")
    };
    assert!(checked.arena.structurally_equal(domain, codomain));
}

#[test]
fn a_record_whose_second_field_is_unproved_is_refused_at_that_field() {
    assert_eq!(
        MISSING_Y,
        FROM_FIELDS.replace("requires\n    |proof|::Identity::<i32>(a.y, b.y)\n", ""),
        "the twins differ only in the `y` requirement"
    );
    let verdict = judged(MISSING_Y);
    let RouteVerdict::Rejected {
        unproved,
        unestablished,
        ..
    } = verdict
    else {
        panic!("the record identity must be refused: {verdict:?}")
    };
    assert!(unestablished.is_empty(), "{unestablished:?}");
    let [refused] = unproved.as_slice() else {
        panic!("one unproved obligation: {unproved:?}")
    };
    assert_eq!(refused.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        refused.statement,
        "cannot prove ensures contract for exit from same_point at statement 0: \
         |proof|::Identity::<Point>(a, b)"
    );
    assert_eq!(
        refused.detail,
        "no available equality proves the stated Identity: \
         nothing proves the identity of field `y`"
    );
}

/// The same pair with `x` a record of its own: a requirement over the whole
/// field supplies it, and without one the refusal names the scalar inside.
fn nested(source: &str) -> String {
    format!(
        "data Coordinate {{\n    value: i32;\n}}\n\n{}",
        source.replace("    x: i32;", "    x: Coordinate;").replace(
            "Identity::<i32>(a.x, b.x)",
            "Identity::<Coordinate>(a.x, b.x)"
        )
    )
}

#[test]
fn a_record_field_of_a_record_is_identical_by_its_own_fields() {
    let verdict = judged(&nested(FROM_FIELDS));
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("the nested record identity must be proved: {verdict:?}")
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    let [(_, Ok(Discharged::Identity(judgment)))] = guarantees.as_slice() else {
        panic!("expected one judged Identity, got {guarantees:?}")
    };
    assert_eq!(judgment.citations.len(), 2);

    let without_x = nested(FROM_FIELDS).replace(
        "requires\n    |proof|::Identity::<Coordinate>(a.x, b.x)\n",
        "",
    );
    let RouteVerdict::Rejected { unproved, .. } = judged(&without_x) else {
        panic!("the nested record identity must be refused without `x`")
    };
    let [refused] = unproved.as_slice() else {
        panic!("one unproved obligation: {unproved:?}")
    };
    assert_eq!(refused.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        refused.detail,
        "no available equality proves the stated Identity: \
         nothing proves the identity of field `x.value`"
    );
}

#[test]
fn a_citation_supplies_a_missing_scalar_or_record_field_identity() {
    let scalar_citation = "
use {omega}/language/core/foundation.omg as proof;

        data Point {
            x: i32;
            y: i32;
        }
        boundary trait YProof {
            machine same_y(a: @Point, b: @Point)
            reaches none
            crashes none
            requires |proof|::Identity::<i32>(a.x, b.x)
            ensures |proof|::Identity::<i32>(a.y, b.y);
        }
        machine same_point(a: @Point, b: @Point)
        invokes YProof
        requires |proof|::Identity::<i32>(a.x, b.x)
        ensures |proof|::Identity::<Point>($a, $b)
        {
            YProof::same_y(a, b);
        }
    ";
    let verdict = judged(scalar_citation);
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("a cited scalar field identity must prove the record identity: {verdict:?}");
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    assert!(matches!(
        guarantees.as_slice(),
        [(_, Ok(Discharged::Identity(_)))]
    ));

    let record_citation = "
use {omega}/language/core/foundation.omg as proof;

        data Coordinate {
            value: i32;
        }
        data Point {
            x: Coordinate;
            y: i32;
        }
        boundary trait XProof {
            machine same_x(a: @Point, b: @Point)
            reaches none
            crashes none
            requires |proof|::Identity::<i32>(a.y, b.y)
            ensures |proof|::Identity::<Coordinate>(a.x, b.x);
        }
        machine same_point(a: @Point, b: @Point)
        invokes XProof
        requires |proof|::Identity::<i32>(a.y, b.y)
        ensures |proof|::Identity::<Point>($a, $b)
        {
            XProof::same_x(a, b);
        }
    ";
    let verdict = judged(record_citation);
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!(
            "a cited record field identity must prove the enclosing record identity: {verdict:?}"
        );
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    assert!(matches!(
        guarantees.as_slice(),
        [(_, Ok(Discharged::Identity(_)))]
    ));
}

#[test]
fn records_with_bounded_sum_and_array_fields_prove_when_all_fields_hold_and_name_unproved_fields() {
    let cases = [
        (
            "bounded",
            "use {omega}/language/core/foundation.omg as proof;

domain u8::Small requires self < 10;
             data Pair { x: i32; y: u8 in Small; }
             machine same(a: Pair, b: Pair)
             requires |proof|::Identity::<i32>(a.x, b.x)
             EXTRA
             ensures |proof|::Identity::<Pair>(a, b) {}",
            "requires (a.y as -Small) == (b.y as -Small)",
            "y",
        ),
        (
            "sum",
            "use {omega}/language/core/foundation.omg as proof;

data Tag { case Left; case Right; }
             data Tagged { x: i32; tag: Tag; }
             machine same(a: Tagged, b: Tagged)
             requires |proof|::Identity::<i32>(a.x, b.x)
             EXTRA
             ensures |proof|::Identity::<Tagged>(a, b) {}",
            "requires |proof|::Identity::<Tag>(a.tag, b.tag)",
            "tag",
        ),
        (
            "array",
            "use {omega}/language/core/foundation.omg as proof;

data Bucket { x: i32; items: [u8; 2]; }
             machine same(a: Bucket, b: Bucket)
             requires |proof|::Identity::<i32>(a.x, b.x)
             EXTRA
             ensures |proof|::Identity::<Bucket>(a, b) {}",
            "requires |proof|::Identity::<Bucket>(a, b)",
            "items",
        ),
    ];
    for (name, template, extra, missing_field) in cases {
        let positive = template.replace("EXTRA", extra);
        let verdict = judged(&positive);
        assert!(
            matches!(verdict, RouteVerdict::Accepted(_)),
            "{name}: supplying every field identity must accept: {verdict:?}"
        );
        let negative = template.replace("EXTRA", "");
        let verdict = judged(&negative);
        let RouteVerdict::Rejected { unproved, .. } = verdict else {
            panic!("{name}: omitting `{missing_field}` must be refused: {verdict:?}");
        };
        let [refused] = unproved.as_slice() else {
            panic!("{name}: expected one unproved obligation, got {unproved:?}");
        };
        assert_eq!(refused.cause, UnprovedCause::NoDerivation);
        assert_eq!(
            refused.detail,
            format!(
                "no available equality proves the stated Identity: \
                 nothing proves the identity of field `{missing_field}`"
            ),
            "{name}"
        );
    }
}

#[test]
fn records_with_reference_view_atomic_or_erased_fields_refuse_at_the_unproved_field() {
    // Records containing reference, element-view, atomic, or erased fields stay
    // decomposed into their fields rather than collapsing to one opaque type:
    // with `x` proved identical and the second field unproved, refusal names
    // the unproved field specifically.
    use terminal_psi::{StructuralAccess, StructuralFieldType, StructuralTypeShape};
    let typed = super::build_purpose::frontier_with_core(
        MISSING_Y,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[],
    );
    let prepared = crate::prepare_fixture(typed);
    let scope = source_provenance::DependencyScope::Product;
    let compilation = crate::lowering::prepare_compilation(
        &prepared,
        super::build_purpose::roster(&[], &[Some(super::build_purpose::package())], scope),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    let terminal_check_tables::TerminalCompilation::Prepared(prepared_comp) = compilation.program
    else {
        panic!("expected prepared compilation");
    };
    let base_elaboration = prepared_comp.closure.unwrap();
    let i32_scalar = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .unwrap(),
    );
    let template_decl = base_elaboration.module.types.structural_types[0].clone();

    for kind in ["reference", "view", "atomic", "erased"] {
        let mut elaboration = base_elaboration.clone();
        let mut identities = std::mem::take(&mut elaboration.module.identities).into_builder();
        let extra_identity = identities.intern(kind);
        let erased_ty_identity = identities.intern("core::u64");
        let extra_type_id = semantic_vocabulary::StructuralTypeId::new(
            elaboration.module.types.structural_types.len() as u64 + 1,
        )
        .unwrap();
        let field_type = match kind {
            "reference" => {
                let referent =
                    semantic_vocabulary::StructuralTypeId::new(extra_type_id.get() + 1).unwrap();
                let mut ref_decl = template_decl.clone();
                ref_decl.id = extra_type_id;
                ref_decl.identity = identities.intern("RefField");
                ref_decl.shape = StructuralTypeShape::Reference {
                    referent,
                    access: StructuralAccess::SharedBorrow,
                };
                let mut target_decl = template_decl.clone();
                target_decl.id = referent;
                target_decl.identity = identities.intern("RefTarget");
                target_decl.shape = StructuralTypeShape::PrimitiveScalar(i32_scalar);
                elaboration.module.types.structural_types.push(ref_decl);
                elaboration.module.types.structural_types.push(target_decl);
                StructuralFieldType::Structural(extra_type_id)
            }
            "view" => {
                let elem =
                    semantic_vocabulary::StructuralTypeId::new(extra_type_id.get() + 1).unwrap();
                let mut view_decl = template_decl.clone();
                view_decl.id = extra_type_id;
                view_decl.identity = identities.intern("ViewField");
                view_decl.shape = StructuralTypeShape::ElementView {
                    element: elem,
                    access: Some(StructuralAccess::SharedBorrow),
                };
                let mut elem_decl = template_decl.clone();
                elem_decl.id = elem;
                elem_decl.identity = identities.intern("ViewElem");
                elem_decl.shape = StructuralTypeShape::PrimitiveScalar(i32_scalar);
                elaboration.module.types.structural_types.push(view_decl);
                elaboration.module.types.structural_types.push(elem_decl);
                StructuralFieldType::Structural(extra_type_id)
            }
            "atomic" => StructuralFieldType::Atomic(i32_scalar),
            "erased" => StructuralFieldType::Erased {
                type_identity: erased_ty_identity,
                scalar: None,
            },
            _ => unreachable!(),
        };
        elaboration.module.identities = identities.build();
        let point = elaboration
            .module
            .types
            .structural_types
            .iter_mut()
            .find(|decl| {
                matches!(
                    &decl.shape,
                    StructuralTypeShape::Record { fields, .. } if fields.len() == 2
                )
            })
            .expect("Point record declaration");
        let StructuralTypeShape::Record { fields, .. } = &mut point.shape else {
            unreachable!();
        };
        fields[1].identity = extra_identity;
        fields[1].field_type = field_type;

        let machine_id = elaboration.module.machines[0].id;
        let tables = elaboration.tables.machines.get_mut(machine_id).unwrap();
        let lowered = &elaboration.module.machines[0];
        let formed = crate::formation::FormedModule::new(
            &elaboration.module,
            &elaboration.nominal_calls,
            &elaboration.static_schemas,
            &elaboration.domain_sources,
        );
        let prepared_machine = crate::prepare(formed, machine_id, tables).unwrap();
        let mut references =
            prepared_machine.references(&elaboration.module.types.structural_types);
        let storage = crate::places::Storage::new(
            machine_id,
            lowered,
            &elaboration.module,
            &prepared_machine.graph,
            tables,
            &prepared_machine.types,
            &mut references,
        )
        .unwrap();
        let (obligation, row) = tables
            .obligations
            .iter()
            .find(|(_, row)| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))
            .expect("NormalGuarantee obligation");
        let Proposition::DeclarationIdentity(goal) = crate::goal(
            &elaboration.module,
            lowered,
            (&storage, &mut references),
            machine_id,
            obligation,
            &row.kind,
            tables,
            &prepared_machine.types,
            &prepared_machine.ranking_bindings,
        )
        .unwrap() else {
            panic!("expected DeclarationIdentity goal");
        };
        let Err(refused) = crate::equality::discharge(
            &elaboration.module,
            lowered,
            tables,
            &prepared_machine.graph,
            &storage,
            row.created_at,
            obligation,
            &goal,
            &[],
            None,
            &mut storage.at_entry(&mut references, tables),
        ) else {
            panic!("{kind}: missing opaque field identity must be refused");
        };
        assert_eq!(
            refused.detail,
            format!(
                "no available equality proves the stated Identity: \
                 nothing proves the identity of field `{kind}`"
            ),
            "{kind}"
        );
    }
}

#[test]
fn equal_addresses_stale_observation_and_user_comparison_do_not_prove_record_identity() {
    // 1. Equal addresses do not substitute for a missing field identity.
    let equal_addresses = "
use {omega}/language/core/foundation.omg as proof;

        data Point { x: i32; y: i32; }
        machine same_point(a: Point, b: Point, addr_a: address, addr_b: address)
        requires |proof|::Identity::<i32>(a.x, b.x)
        requires addr_a == addr_b
        ensures |proof|::Identity::<Point>(a, b)
        {}
    ";
    let RouteVerdict::Rejected { unproved, .. } = judged(equal_addresses) else {
        panic!("equal addresses must not prove record identity");
    };
    let [refused] = unproved.as_slice() else {
        panic!("expected one unproved obligation: {unproved:?}");
    };
    assert_eq!(
        refused.detail,
        "no available equality proves the stated Identity: \
         nothing proves the identity of field `y`"
    );

    // 2. A stale observation (mutating `a.y` after entry) does not prove
    //    record identity at exit.
    let stale_observation = "
use {omega}/language/core/foundation.omg as proof;

        data Point { x: i32; y: i32; }
        machine mutate_y(a: @mutable Point, b: @Point)
        requires |proof|::Identity::<i32>(a.x, b.x)
        requires |proof|::Identity::<i32>(a.y, b.y)
        ensures |proof|::Identity::<Point>($a, $b)
        {
            a.y = a.y +% 1;
        }
    ";
    let verdict = judged(stale_observation);
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("mutating a field must invalidate its entry identity observation: {verdict:?}");
    };
    let [refused] = unproved.as_slice() else {
        panic!("expected one unproved obligation: {unproved:?}");
    };
    assert_eq!(
        refused.detail,
        "no available equality proves the stated Identity: \
         nothing proves the identity of field `y`"
    );

    // 3. A true user-defined comparison does not substitute for a missing
    //    field identity without an identity bridge.
    let user_comparison = "
use {omega}/language/core/foundation.omg as proof;

        data Point { x: i32; y: i32; }
        boundary trait UserCompare {
            machine eq(left: i32, right: i32) -> bool
            reaches none
            crashes none
            ensures result == true;
        }
        machine same_point(a: Point, b: Point)
        invokes UserCompare
        requires |proof|::Identity::<i32>(a.x, b.x)
        ensures |proof|::Identity::<Point>(a, b)
        {
            let _: bool = UserCompare::eq(a.y, b.y);
        }
    ";
    let RouteVerdict::Rejected { unproved, .. } = judged(user_comparison) else {
        panic!("a true user-defined comparison must not prove field identity");
    };
    let [refused] = unproved.as_slice() else {
        panic!("expected one unproved obligation: {unproved:?}");
    };
    assert_eq!(
        refused.detail,
        "no available equality proves the stated Identity: \
         nothing proves the identity of field `y`"
    );
}

#[test]
fn structural_result_identity_postconditions_and_twins() {
    // Direct return of `outer.inner`, caller restatement through `pick(outer)`,
    // and passing the unmodified call result to a helper whose `requires`
    // states `Identity::<Inner>(got, outer.inner)` all prove.
    let positive = "
use {omega}/language/core/foundation.omg as proof;

        data Inner [copy] { value: i32; }
        data Outer [copy] { inner: Inner; other: Inner; }

        machine pick(outer: Outer) -> Inner
        ensures |proof|::Identity::<Inner>(result, outer.inner)
        {
            outer.inner
        }

        machine check_same(got: Inner, outer: Outer) -> Inner
        requires |proof|::Identity::<Inner>(got, outer.inner)
        ensures |proof|::Identity::<Inner>(result, outer.inner)
        {
            got
        }

        machine direct_restatement(outer: Outer) -> Inner
        ensures |proof|::Identity::<Inner>(result, outer.inner)
        {
            let got: Inner = pick(outer);
            got
        }

        machine forward(outer: Outer) -> Inner
        ensures |proof|::Identity::<Inner>(result, outer.inner)
        {
            let got: Inner = pick(outer);
            check_same(got, outer)
        }
    ";
    let verdict = judged(positive);
    assert!(
        matches!(verdict, RouteVerdict::Accepted(_)),
        "expected structural result Identity postconditions to prove: {verdict:?}"
    );

    // Negative twin 1: mutating the local call result invalidates the imported
    // Identity guarantee from `pick(outer)`.
    let mutated_after_call = "
use {omega}/language/core/foundation.omg as proof;

        data Inner [copy] { value: i32; }
        data Outer [copy] { inner: Inner; other: Inner; }

        machine pick(outer: Outer) -> Inner
        ensures |proof|::Identity::<Inner>(result, outer.inner)
        {
            outer.inner
        }

        machine forward(outer: Outer) -> Inner
        ensures |proof|::Identity::<Inner>(result, outer.inner)
        {
            let mutable got: Inner = pick(outer);
            got.value = got.value +% 1;
            got
        }
    ";
    let verdict = judged(mutated_after_call);
    assert!(
        matches!(verdict, RouteVerdict::Rejected { .. }),
        "mutating a call result must invalidate its Identity guarantee: {verdict:?}"
    );

    // Negative twin 2: returning a different field does not prove
    // `Identity::<Inner>(result, outer.inner)`.
    let wrong_field = "
use {omega}/language/core/foundation.omg as proof;

        data Inner [copy] { value: i32; }
        data Outer [copy] { inner: Inner; other: Inner; }

        machine pick(outer: Outer) -> Inner
        ensures |proof|::Identity::<Inner>(result, outer.inner)
        {
            outer.other
        }
    ";
    let verdict = judged(wrong_field);
    assert!(
        matches!(verdict, RouteVerdict::Rejected { .. }),
        "returning another field must be refused: {verdict:?}"
    );
}
