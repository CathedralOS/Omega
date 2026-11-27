//! A sum input of a theorem is one value of its inductive declaration: its
//! case tests and payload reads denote through the recursor, and an Identity
//! of two sum inputs is the kernel's identity at the sum.

use proof_admission::{
    BoundedImplicationType, BoundedTheoremError, Budget, Context, DataFieldShape, DataSubject,
    DataSubjectShape, DeclarationKind, Level, PrimitiveJudgment, ProofNode, ProofRule, Signature,
    Sort, Term, TermHandle, check_signature, convertible, denote_bounded_implication_type,
    infer_sort, instantiate, verify_bounded_theorem,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ClauseCarrier, ClauseIdentity, ClauseOperand, ClauseValue,
    IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, PlaceId, Proposition,
    ScalarReadProjection, ScalarTerm, ScalarType, StructuralCaseId, StructuralCaseSubject,
    StructuralFieldId, StructuralTypeId,
};

fn u8_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
}

fn case(raw: u64) -> StructuralCaseId {
    StructuralCaseId::new(raw).unwrap()
}

fn field(raw: u64) -> StructuralFieldId {
    StructuralFieldId::new(raw).unwrap()
}

fn place(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}

/// `data Choice { case One(x: u8); case Two(y: u8); }`, with `x` as field 1
/// of case 1 and `y` as field 2 of case 2.
fn choice(payload: ScalarType) -> DataSubjectShape {
    DataSubjectShape {
        structural_type: StructuralTypeId::new(7).unwrap(),
        cases: vec![
            (case(1), vec![(field(1), DataFieldShape::Scalar(payload))]),
            (case(2), vec![(field(2), DataFieldShape::Scalar(payload))]),
        ],
    }
}

/// `data Nat { case Zero; case Next(previous: Nat); }`
fn nat() -> DataSubjectShape {
    DataSubjectShape {
        structural_type: StructuralTypeId::new(8).unwrap(),
        cases: vec![
            (case(3), Vec::new()),
            (case(4), vec![(field(3), DataFieldShape::Recursive)]),
        ],
    }
}

fn subject(raw: u64, shape: DataSubjectShape) -> DataSubject {
    DataSubject {
        place: place(raw),
        shape,
    }
}

fn is_case(raw: u64, tested: StructuralCaseId) -> Proposition {
    Proposition::StructuralCaseMembership {
        subject: StructuralCaseSubject::new(place(raw), Vec::new()),
        case: tested,
    }
}

fn read(raw: u64, of: StructuralCaseId, at: StructuralFieldId, ty: ScalarType) -> ScalarTerm {
    ScalarTerm::StructuralRead {
        root: place(raw),
        path: vec![
            ScalarReadProjection::Static(CanonicalStructuralPathSegment::Case(of)),
            ScalarReadProjection::Static(CanonicalStructuralPathSegment::Field(at)),
        ],
        scalar_type: ty,
    }
}

fn nonzero(term: ScalarTerm) -> Proposition {
    let zero = ScalarTerm::integer(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerValue::Unsigned(0),
    )
    .unwrap();
    Proposition::LessThan(zero, term)
}

fn identity(left: u64, right: u64, ty: StructuralTypeId) -> Proposition {
    let input = |raw| ClauseValue {
        carrier: ClauseCarrier::Structural(ty),
        operand: ClauseOperand::Input(StructuralCaseSubject::new(place(raw), Vec::new())),
    };
    Proposition::DeclarationIdentity(ClauseIdentity {
        values: vec![input(left), input(right)],
        left: 0,
        right: 1,
    })
}

fn denote(
    data: &[DataSubject],
    premises: &[Proposition],
    conclusion: &Proposition,
) -> Result<BoundedImplicationType, BoundedTheoremError> {
    denote_bounded_implication_type(&[], data, &[], premises, conclusion, &mut Budget::default())
}

/// The type checks against its own signature, and binds the sum inputs,
/// each followed by the two carrier bounds of each of its integer payloads,
/// and then each premise.
fn assert_well_formed(mut ty: BoundedImplicationType, binders: usize) {
    check_signature(&mut ty.arena, &ty.signature, &mut Budget::default())
        .expect("the sum's declaration is a checked inductive block");
    let context =
        Context::empty().with_signature(Signature::from_declarations(ty.signature.clone()));
    assert!(
        infer_sort(&mut ty.arena, &context, ty.ty, &mut Budget::default())
            .expect("the implication is a type")
            .is_strict()
    );
    let (mut body, mut count) = (ty.ty, 0);
    while let Term::Pi { codomain, .. } = ty.arena.get(body) {
        count += 1;
        body = codomain;
    }
    assert_eq!(count, binders);
}

fn refusal(result: Result<BoundedImplicationType, BoundedTheoremError>) -> String {
    match result {
        Ok(_) => panic!("the implication was formed"),
        Err(error) => format!("{error:?}"),
    }
}

#[test]
fn a_case_test_and_its_payload_read_form_one_implication_over_the_sum() {
    let premise = is_case(1, case(1));
    let conclusion = nonzero(read(1, case(1), field(1), u8_type()));
    let ty = denote(&[subject(1, choice(u8_type()))], &[premise], &conclusion).unwrap();
    // The input, two bounds on each of `x` and `y`, and the premise.
    assert_well_formed(ty, 6);
}

#[test]
fn a_payload_read_names_a_field_of_its_own_case() {
    let premise = is_case(1, case(1));
    let conclusion = nonzero(read(1, case(1), field(2), u8_type()));
    let refused = refusal(denote(
        &[subject(1, choice(u8_type()))],
        &[premise],
        &conclusion,
    ));
    assert!(refused.contains("a field its case lacks"), "{refused}");
}

#[test]
fn a_boolean_payload_is_read_at_its_carrier() {
    let conclusion = Proposition::Equal(
        read(1, case(2), field(2), ScalarType::Boolean),
        ScalarTerm::Boolean(true),
    );
    let ty = denote(&[subject(1, choice(ScalarType::Boolean))], &[], &conclusion).unwrap();
    assert_well_formed(ty, 1);
}

#[test]
fn a_payload_read_is_its_field_in_its_case() {
    let premise = Proposition::Equal(
        read(1, case(2), field(2), ScalarType::Boolean),
        ScalarTerm::Boolean(true),
    );
    let mut ty = denote(
        &[subject(1, choice(ScalarType::Boolean))],
        &[premise],
        &Proposition::Truth,
    )
    .unwrap();
    let (one, two) = (constructor(&mut ty, 0), constructor(&mut ty, 1));
    let (zero, set) = (
        ty.arena.insert(Term::TwoZero),
        ty.arena.insert(Term::TwoOne),
    );
    let two_set = ty.arena.insert(Term::Apply {
        function: two,
        argument: set,
    });
    let one_set = ty.arena.insert(Term::Apply {
        function: one,
        argument: set,
    });
    // `Two(true)` reads `true`.
    assert!(premise_is(&mut ty, two_set, set, set));
    assert!(!premise_is(&mut ty, two_set, zero, set));
    // `One(true)` has no `y`; the read is the carrier's zero.
    assert!(premise_is(&mut ty, one_set, zero, set));
    assert!(!premise_is(&mut ty, one_set, set, set));
}

#[test]
fn a_payload_read_at_another_carrier_is_refused() {
    let conclusion = nonzero(read(1, case(2), field(2), u8_type()));
    let refused = refusal(denote(
        &[subject(1, choice(ScalarType::Boolean))],
        &[],
        &conclusion,
    ));
    assert!(refused.contains("its field's scalar carrier"), "{refused}");
}

/// A float payload is a value of its format's carrier, as a float subject
/// is, and an equality of it is read through the recursor.
#[test]
fn a_float_payload_is_read_at_its_carrier() {
    let float = ScalarType::IeeeFloat(IeeeFloatFormat::Binary32);
    let premise = Proposition::Equal(
        read(1, case(1), field(1), float),
        ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0x3f80_0000)),
    );
    let ty = denote(
        &[subject(1, choice(float))],
        &[premise],
        &is_case(1, case(1)),
    )
    .unwrap();
    // The input and the premise: a float payload has no integer bounds.
    assert_well_formed(ty, 2);
}

/// A read of a float payload at the other format is not its field.
#[test]
fn a_float_payload_read_at_another_format_is_refused() {
    let premise = Proposition::Equal(
        read(
            1,
            case(1),
            field(1),
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
        ),
        ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0x3ff0_0000_0000_0000)),
    );
    let refused = refusal(denote(
        &[subject(
            1,
            choice(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
        )],
        &[premise],
        &is_case(1, case(1)),
    ));
    assert!(refused.contains("its field's scalar carrier"), "{refused}");
}

#[test]
fn a_recursive_sum_is_one_value_of_its_inductive_declaration() {
    let ty = denote(
        &[subject(1, nat())],
        &[is_case(1, case(4))],
        &is_case(1, case(4)),
    )
    .unwrap();
    assert_well_formed(ty, 2);
}

/// The constructor of case `index` in the type's signature.
fn constructor(ty: &mut BoundedImplicationType, index: u32) -> TermHandle {
    let declaration = ty
        .signature
        .iter()
        .position(|declaration| {
            matches!(declaration.kind, DeclarationKind::Constructor { constructor_index, .. }
                if constructor_index == index)
        })
        .expect("the sum's constructor") as u32;
    ty.arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}

/// Whether the premise's proposition, at the given value of the sum input,
/// is the identity of `found` with `0` in `Two`.
fn premise_at(ty: &mut BoundedImplicationType, value: TermHandle, found: TermHandle) -> bool {
    let zero = ty.arena.insert(Term::TwoZero);
    premise_is(ty, value, found, zero)
}

/// Whether the premise's proposition, at the given value of the sum input,
/// is the identity of `left` with `right` in `Two`.
fn premise_is(
    ty: &mut BoundedImplicationType,
    value: TermHandle,
    left: TermHandle,
    right: TermHandle,
) -> bool {
    let Term::Pi { codomain, .. } = ty.arena.get(ty.ty) else {
        panic!("the sum input is bound first");
    };
    let Term::Pi { domain, .. } = ty.arena.get(codomain) else {
        panic!("the premise is bound next");
    };
    let Term::Squash { ty: proposition } = ty.arena.get(domain) else {
        panic!("a premise is a squashed proposition");
    };
    let at = instantiate(&mut ty.arena, proposition, &[value]);
    let two = ty.arena.insert(Term::Two);
    let expected = ty.arena.insert(Term::Id {
        ty: two,
        left,
        right,
    });
    let universe = ty.arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
    let signature = check_signature(&mut ty.arena, &ty.signature, &mut Budget::default())
        .expect("the sum's declaration is a checked inductive block");
    let context = Context::empty().with_signature(signature);
    convertible(
        &mut ty.arena,
        &context,
        at,
        expected,
        universe,
        &mut Budget::default(),
    )
    .unwrap()
}

#[test]
fn a_case_test_computes_through_the_recursor() {
    let mut ty = denote(
        &[subject(1, nat())],
        &[is_case(1, case(3))],
        &Proposition::Truth,
    )
    .unwrap();
    let zero_value = constructor(&mut ty, 0);
    let next = constructor(&mut ty, 1);
    let one_value = ty.arena.insert(Term::Apply {
        function: next,
        argument: zero_value,
    });
    let (holds, fails) = (
        ty.arena.insert(Term::TwoZero),
        ty.arena.insert(Term::TwoOne),
    );
    assert!(premise_at(&mut ty, zero_value, holds));
    assert!(!premise_at(&mut ty, zero_value, fails));
    assert!(premise_at(&mut ty, one_value, fails));
    assert!(!premise_at(&mut ty, one_value, holds));
}

#[test]
fn a_case_test_names_a_case_of_the_declaration() {
    let refused = refusal(denote(&[subject(1, nat())], &[], &is_case(1, case(1))));
    assert!(
        refused.contains("a case its declaration lacks"),
        "{refused}"
    );
}

#[test]
fn a_field_of_a_recursive_case_is_not_a_scalar_payload() {
    let conclusion = nonzero(read(1, case(4), field(3), u8_type()));
    let refused = refusal(denote(&[subject(1, nat())], &[], &conclusion));
    assert!(refused.contains("its field's scalar carrier"), "{refused}");
}

#[test]
fn two_inputs_of_one_sum_share_its_declaration_and_compare_by_identity() {
    let shape = choice(u8_type());
    let ty_id = shape.structural_type;
    let ty = denote(
        &[subject(1, shape.clone()), subject(2, shape)],
        &[identity(1, 2, ty_id)],
        &identity(2, 1, ty_id),
    )
    .unwrap();
    // The two inputs, each with two bounds on each of `x` and `y`, and the
    // premise.
    assert_well_formed(ty, 11);
}

#[test]
fn an_identity_of_inputs_of_two_sums_is_refused() {
    let left = choice(u8_type());
    let refused = refusal(denote(
        &[subject(1, left.clone()), subject(2, nat())],
        &[],
        &identity(1, 2, left.structural_type),
    ));
    assert!(
        refused.contains("at other than their one declaration"),
        "{refused}"
    );
}

#[test]
fn different_sums_give_different_implications() {
    let conclusion = is_case(1, case(1));
    let one = denote(&[subject(1, choice(u8_type()))], &[], &conclusion).unwrap();
    let other = denote(&[subject(1, choice(ScalarType::Boolean))], &[], &conclusion).unwrap();
    let same = denote(&[subject(1, choice(u8_type()))], &[], &conclusion).unwrap();
    let key = |ty: &BoundedImplicationType| format!("{:?}", ty.signature);
    assert_eq!(key(&one), key(&same));
    assert_ne!(key(&one), key(&other));
}

#[test]
fn a_sum_input_is_named_once() {
    let refused = refusal(denote(
        &[subject(1, nat()), subject(1, nat())],
        &[],
        &is_case(1, case(3)),
    ));
    assert!(!refused.is_empty(), "one place is one input");
}

fn u8_literal(value: u64) -> ScalarTerm {
    ScalarTerm::integer(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerValue::Unsigned(value.into()),
    )
    .unwrap()
}

fn carrier_proof(goal: &Proposition) -> Result<usize, String> {
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound),
    };
    let theorem = verify_bounded_theorem(
        &[],
        &[subject(1, choice(u8_type()))],
        &[],
        goal,
        &[is_case(1, case(1))],
        &[],
        &proof,
        &mut Budget::default(),
    )
    .map_err(|error| format!("{error:?}"))?;
    assert!(theorem.denotation.certificate.context.is_empty());
    let (mut body, mut count) = (theorem.denotation.certificate.expected, 0);
    while let Term::Pi { codomain, .. } = theorem.denotation.arena.get(body) {
        count += 1;
        body = codomain;
    }
    Ok(count)
}

/// A payload holds a value of its carrier, so its carrier bound proves
/// through the binder the sum input brings for it.
#[test]
fn a_payload_read_has_its_carrier_bounds() {
    let x = read(1, case(1), field(1), u8_type());
    let lower = Proposition::LessOrEqual(u8_literal(0), x.clone());
    let upper = Proposition::LessOrEqual(x, u8_literal(255));
    // The input, two bounds on each of `x` and `y`, and the premise.
    assert_eq!(carrier_proof(&lower), Ok(6));
    assert_eq!(carrier_proof(&upper), Ok(6));
}

#[test]
fn a_payload_read_has_no_bound_inside_its_carrier() {
    let x = read(1, case(1), field(1), u8_type());
    let refused = carrier_proof(&Proposition::LessOrEqual(u8_literal(1), x)).unwrap_err();
    assert!(
        refused.contains("JudgmentDoesNotEstablishGoal"),
        "{refused}"
    );
}
