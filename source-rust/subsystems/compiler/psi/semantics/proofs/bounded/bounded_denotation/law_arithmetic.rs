/*
    The integer model's laws are assumptions the kernel believes without a
    proof (kernel_metatheory.md, "Assumed laws of the integer model"). These
    tests check every one of them against computed arithmetic, so a law whose
    statement is wrong fails here instead of letting the kernel accept a false
    integer goal.

    The tests read the statement the kernel actually receives: each law is
    declared through `Denotation::law`, and its declared type is evaluated,
    not the formula text it was built from. An arithmetic model gives every
    integer declaration its computed meaning (the carrier is the integers,
    `double(x) = 2x`, `odd(x) = 2x + 1`, `negate(x) = -x`, the order relations
    and `+ - *` are the integers' own, a fixed carrier's exact division is the
    truncating quotient and a widening or exact cast is the identity) and the
    law must hold at every assignment of its integer variables:

    - over the integers in `-8..=8`, which exercises signs and zero on every
      side of every law;
    - for each division law, over every value of the 4-bit carriers (and of
      the 8-bit carriers for the two-variable laws), with
      the quotient computed by the carrier's own `IntegerType::exact_div`
      (an assignment whose division the carrier refuses has no computed value
      and is skipped);
    - for each cast law, over every value of the source carrier, with the
      result computed by `IntegerType::widen_value_to` or
      `IntegerType::exact_cast_value_to`.

    Each positive check has a negative twin. A law with premises is checked
    again with one premise at a time replaced by a premise that always holds;
    the model must then find an assignment where the weakened statement fails
    or has no value, which shows the law states every side condition it
    depends on. A law with no premise is paired with a wrong statement of the
    same shape, built by the same formula reader, that must fail.
*/

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

use super::{
    ArithKind, ArithPolicy, BinaryOp, CORE_LAW_FORMULAS, CoreLaw, Denotation, IntegerOperation,
    NumeralCtor, Term, TermHandle, UnaryOp,
};

const WINDOW: std::ops::RangeInclusive<i128> = -8..=8;

/// Every law the integer model can declare. The match in `family` lists each
/// `CoreLaw` variant, so a new law does not compile until it is placed here.
fn every_law() -> Vec<CoreLaw> {
    let mut laws: Vec<CoreLaw> = CORE_LAW_FORMULAS.iter().map(|&(law, ..)| law).collect();
    laws.push(CoreLaw::LtIrrefl);
    laws.push(CoreLaw::DivisorCaseSplit);
    for ty in division_carriers() {
        for (a, b) in [(false, false), (false, true), (true, false), (true, true)] {
            laws.push(CoreLaw::DivBound {
                ty,
                lower: a,
                positive: b,
            });
            laws.push(CoreLaw::QuotBound {
                ty,
                upper: a,
                positive: b,
            });
        }
    }
    laws.extend(cast_operations().into_iter().map(CoreLaw::CastId));
    for &law in &laws {
        let _ = family(law);
    }
    laws
}

#[derive(Debug, PartialEq, Eq)]
enum Family {
    Formula,
    Irreflexive,
    CaseSplit,
    Division(IntegerType),
    Cast(IntegerOperation),
}

fn family(law: CoreLaw) -> Family {
    use CoreLaw as L;
    match law {
        L::EqToLe
        | L::LtToLe
        | L::LeTrans
        | L::LtTrans
        | L::LtLeTrans
        | L::LeLtTrans
        | L::LtSubstLeft
        | L::LtSubstRight
        | L::LeSubstLeft
        | L::LeSubstRight
        | L::DoubleZero
        | L::NegateZero
        | L::EvenBeforeOdd
        | L::OddBeforeNextEven
        | L::NegateOrder
        | L::AddMonotone
        | L::AddCancelSub
        | L::AddMonotoneBoth
        | L::AddZeroRight
        | L::AddZeroLeft
        | L::SubZero
        | L::SubAntitone
        | L::SubSelfZero
        | L::SubLeAntitone
        | L::SubCancelAddLeft
        | L::SubCancelAddRight
        | L::SubMonotoneAntitone
        | L::MulDistributeAdd
        | L::MulMonotonePos
        | L::MulAntitoneNeg
        | L::AddRightInv
        | L::SubRightInv
        | L::LeAntisym => Family::Formula,
        L::LtIrrefl => Family::Irreflexive,
        L::DivisorCaseSplit => Family::CaseSplit,
        L::DivBound { ty, .. } | L::QuotBound { ty, .. } => Family::Division(ty),
        L::CastId(op) => Family::Cast(op),
    }
}

fn int_ty(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).expect("closed integer width")
}

/// Small carriers whose every value the division laws are evaluated at.
fn division_carriers() -> [IntegerType; 4] {
    [
        int_ty(IntegerSign::Signed, 4),
        int_ty(IntegerSign::Unsigned, 4),
        int_ty(IntegerSign::Signed, 8),
        int_ty(IntegerSign::Unsigned, 8),
    ]
}

/// The cast laws are declared only for widening and exact casts
/// (the cast chain in `bounded_denotation.rs` builds no other edge).
fn cast_operations() -> Vec<IntegerOperation> {
    let (i4, u4, i8, u8) = (
        int_ty(IntegerSign::Signed, 4),
        int_ty(IntegerSign::Unsigned, 4),
        int_ty(IntegerSign::Signed, 8),
        int_ty(IntegerSign::Unsigned, 8),
    );
    vec![
        IntegerOperation::Unary(UnaryOp::Widen(i8), i4),
        IntegerOperation::Unary(UnaryOp::Widen(i8), u4),
        IntegerOperation::Unary(UnaryOp::Widen(u8), u4),
        IntegerOperation::Unary(UnaryOp::ExactCast(u4), i8),
        IntegerOperation::Unary(UnaryOp::ExactCast(i4), u8),
        IntegerOperation::Unary(UnaryOp::ExactCast(i8), u8),
    ]
}

fn carrier_values(ty: IntegerType) -> Vec<i128> {
    let low = to_i128(ty.minimum_value().expect("closed width"));
    let high = to_i128(ty.maximum_value().expect("closed width"));
    (low..=high).collect()
}

fn to_i128(value: IntegerValue) -> i128 {
    match value {
        IntegerValue::Signed(v) => v,
        IntegerValue::Unsigned(v) => i128::try_from(v).expect("small carrier value"),
    }
}

/// The carrier's own representation of `v`, or none outside its range.
fn in_carrier(ty: IntegerType, v: i128) -> Option<IntegerValue> {
    let value = match ty.sign() {
        IntegerSign::Signed => IntegerValue::Signed(v),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::try_from(v).ok()?),
    };
    ty.admits(value).then_some(value)
}

#[derive(Clone, Copy)]
enum Arithmetic {
    /// The integers: `/` is the truncating quotient, a cast is the identity.
    Integers,
    /// Each fixed operation is computed by its carrier and has no value where
    /// the carrier refuses it.
    Carriers,
}

#[derive(Clone, Copy)]
enum Meaning {
    Carrier,
    Less,
    LessOrEqual,
    Binary(BinaryOp),
    Zero,
    Numeral(NumeralCtor),
    Operation(IntegerOperation),
    Defined(TermHandle),
}

/// What an evaluation found over every assignment.
#[derive(Debug, Default, PartialEq, Eq)]
struct Outcome {
    held: u64,
    failed: u64,
    undefined: u64,
}

struct Model<'a> {
    denotation: &'a Denotation,
    meanings: Vec<Option<Meaning>>,
    arithmetic: Arithmetic,
    values: Vec<i128>,
}

impl<'a> Model<'a> {
    fn new(denotation: &'a Denotation, arithmetic: Arithmetic, values: Vec<i128>) -> Self {
        let t = &denotation.tables;
        let mut meanings = vec![None; denotation.declarations.len()];
        let mut give = |p: Option<u32>, m: Meaning| {
            if let Some(p) = p {
                meanings[p as usize] = Some(m);
            }
        };
        give(t.integer, Meaning::Carrier);
        give(t.integer_less_than, Meaning::Less);
        give(t.integer_less_or_equal, Meaning::LessOrEqual);
        for op in [BinaryOp::Add, BinaryOp::Sub, BinaryOp::Mul] {
            give(t.binary_ops[op.slot()], Meaning::Binary(op));
        }
        give(t.numeral_zero, Meaning::Zero);
        for c in [NumeralCtor::Double, NumeralCtor::Odd, NumeralCtor::Negate] {
            give(t.numeral_ctors[c.slot()], Meaning::Numeral(c));
        }
        for (&op, &p) in &t.integer_operations {
            give(Some(p), Meaning::Operation(op));
        }
        for &p in t.numeral_prefixes.values() {
            let body = denotation.declarations[p as usize]
                .body
                .expect("a numeral prefix is a definition");
            give(Some(p), Meaning::Defined(body));
        }
        Self {
            denotation,
            meanings,
            arithmetic,
            values,
        }
    }

    fn meaning(&self, term: TermHandle) -> Option<Meaning> {
        match self.denotation.arena.get(term) {
            Term::Constant { declaration, .. } => {
                self.meanings.get(declaration as usize).copied().flatten()
            }
            _ => None,
        }
    }

    fn spine(&self, mut term: TermHandle) -> (TermHandle, Vec<TermHandle>) {
        let mut args = Vec::new();
        while let Term::Apply { function, argument } = self.denotation.arena.get(term) {
            args.push(argument);
            term = function;
        }
        args.reverse();
        (term, args)
    }

    fn unreadable(&self, term: TermHandle) -> ! {
        panic!(
            "the arithmetic model cannot read this law term: {:?}",
            self.denotation.arena.get(term)
        )
    }

    /// Evaluate `statement` at every assignment of its integer binders.
    fn evaluate(&self, statement: TermHandle) -> Outcome {
        let mut outcome = Outcome::default();
        let mut env = Vec::new();
        self.evaluate_under(statement, &mut env, &mut outcome);
        outcome
    }

    fn evaluate_under(
        &self,
        statement: TermHandle,
        env: &mut Vec<Option<i128>>,
        outcome: &mut Outcome,
    ) {
        if let Term::Pi { domain, codomain } = self.denotation.arena.get(statement)
            && matches!(self.meaning(domain), Some(Meaning::Carrier))
        {
            for &v in &self.values {
                env.push(Some(v));
                self.evaluate_under(codomain, env, outcome);
                env.pop();
            }
            return;
        }
        match self.holds(statement, env) {
            Some(true) => outcome.held += 1,
            Some(false) => outcome.failed += 1,
            None => outcome.undefined += 1,
        }
    }

    /// Whether a proposition holds; none when an operation in it has no
    /// computed value.
    fn holds(&self, term: TermHandle, env: &mut Vec<Option<i128>>) -> Option<bool> {
        match self.denotation.arena.get(term) {
            Term::Empty => Some(false),
            Term::Pi { domain, codomain } => {
                if matches!(self.meaning(domain), Some(Meaning::Carrier)) {
                    self.unreadable(term);
                }
                if !self.holds(domain, env)? {
                    return Some(true);
                }
                env.push(None);
                let held = self.holds(codomain, env);
                env.pop();
                held
            }
            Term::Sigma { domain, codomain } => {
                let Term::Two = self.denotation.arena.get(domain) else {
                    self.unreadable(term);
                };
                let Term::CaseTwo {
                    zero_branch,
                    one_branch,
                    scrutinee,
                    ..
                } = self.denotation.arena.get(codomain)
                else {
                    self.unreadable(term);
                };
                if self.denotation.arena.get(scrutinee) != Term::Variable(0) {
                    self.unreadable(term);
                }
                env.push(None);
                let held = match self.holds(zero_branch, env) {
                    Some(true) => Some(true),
                    zero => match (zero, self.holds(one_branch, env)) {
                        (_, Some(true)) => Some(true),
                        (Some(false), Some(false)) => Some(false),
                        _ => None,
                    },
                };
                env.pop();
                held
            }
            Term::Id { ty, left, right } => {
                if !matches!(self.meaning(ty), Some(Meaning::Carrier)) {
                    self.unreadable(term);
                }
                Some(self.value(left, env)? == self.value(right, env)?)
            }
            Term::Apply { .. } => {
                let (head, args) = self.spine(term);
                let [l, r] = args[..] else {
                    self.unreadable(term);
                };
                let (l, r) = (self.value(l, env)?, self.value(r, env)?);
                match self.meaning(head) {
                    Some(Meaning::Less) => Some(l < r),
                    Some(Meaning::LessOrEqual) => Some(l <= r),
                    _ => self.unreadable(term),
                }
            }
            _ => self.unreadable(term),
        }
    }

    fn value(&self, term: TermHandle, env: &[Option<i128>]) -> Option<i128> {
        match self.denotation.arena.get(term) {
            Term::Variable(i) => {
                let slot = env.len().checked_sub(1 + i as usize);
                match slot.map(|s| env[s]) {
                    Some(Some(v)) => Some(v),
                    _ => self.unreadable(term),
                }
            }
            Term::Constant { .. } => match self.meaning(term) {
                Some(Meaning::Zero) => Some(0),
                Some(Meaning::Defined(body)) => self.value(body, &[]),
                _ => self.unreadable(term),
            },
            Term::Apply { .. } => {
                let (head, args) = self.spine(term);
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.value(a, env)?);
                }
                match (self.meaning(head), &vals[..]) {
                    (Some(Meaning::Binary(op)), &[l, r]) => Some(op.eval_i128(l, r).unwrap()),
                    (Some(Meaning::Numeral(NumeralCtor::Double)), &[x]) => Some(2 * x),
                    (Some(Meaning::Numeral(NumeralCtor::Odd)), &[x]) => Some(2 * x + 1),
                    (Some(Meaning::Numeral(NumeralCtor::Negate)), &[x]) => Some(-x),
                    (Some(Meaning::Operation(op)), vals) => self.operation(op, vals),
                    _ => self.unreadable(term),
                }
            }
            _ => self.unreadable(term),
        }
    }

    fn operation(&self, op: IntegerOperation, vals: &[i128]) -> Option<i128> {
        match (op, vals, self.arithmetic) {
            (
                IntegerOperation::Arith(ArithPolicy::Exact, ArithKind::Div, _),
                &[n, d],
                Arithmetic::Integers,
            ) => (d != 0).then(|| n / d),
            (
                IntegerOperation::Arith(ArithPolicy::Exact, ArithKind::Div, ty),
                &[n, d],
                Arithmetic::Carriers,
            ) => ty
                .exact_div(in_carrier(ty, n)?, in_carrier(ty, d)?)
                .map(to_i128),
            (
                IntegerOperation::Unary(UnaryOp::Widen(_) | UnaryOp::ExactCast(_), _),
                &[x],
                Arithmetic::Integers,
            ) => Some(x),
            (
                IntegerOperation::Unary(UnaryOp::Widen(target), source),
                &[x],
                Arithmetic::Carriers,
            ) => source
                .widen_value_to(target, in_carrier(source, x)?)
                .map(to_i128),
            (
                IntegerOperation::Unary(UnaryOp::ExactCast(target), source),
                &[x],
                Arithmetic::Carriers,
            ) => source
                .exact_cast_value_to(target, in_carrier(source, x)?)
                .map(to_i128),
            _ => panic!("the arithmetic model has no meaning for {op:?}"),
        }
    }
}

fn declared(d: &mut Denotation, law: CoreLaw) -> TermHandle {
    d.law(law, &[]).expect("law declares");
    let p = d.tables.laws[&law];
    d.declarations[p as usize].ty
}

/// The premises of a statement `Π(x : Int)… Π(_ : P₁)… Q`, outermost first.
fn premise_count(d: &Denotation, mut ty: TermHandle) -> usize {
    let int = d.tables.integer.expect("integer carrier");
    let mut n = 0;
    while let Term::Pi { domain, codomain } = d.arena.get(ty) {
        match d.arena.get(domain) {
            Term::Constant { declaration, .. } if declaration == int => {}
            _ => n += 1,
        }
        ty = codomain;
    }
    n
}

/// `ty` with its `index`th premise replaced by `Empty → Empty`, which always
/// holds. The binder stays, so no variable of the statement moves.
fn weaken_premise(d: &mut Denotation, ty: TermHandle, index: usize) -> TermHandle {
    let int = d.tables.integer.expect("integer carrier");
    let Term::Pi { domain, codomain } = d.arena.get(ty) else {
        panic!("premise {index} is past the statement's conclusion");
    };
    let is_binder = matches!(
        d.arena.get(domain),
        Term::Constant { declaration, .. } if declaration == int
    );
    let (domain, codomain) = match (is_binder, index) {
        (true, _) => (domain, weaken_premise(d, codomain, index)),
        (false, 0) => {
            let empty = d.arena.insert(Term::Empty);
            (d.pi(empty, empty), codomain)
        }
        (false, _) => (domain, weaken_premise(d, codomain, index - 1)),
    };
    d.pi(domain, codomain)
}

fn integers(d: &Denotation) -> Model<'_> {
    Model::new(d, Arithmetic::Integers, WINDOW.collect())
}

fn assert_holds(model: &Model<'_>, ty: TermHandle, what: &str) {
    let outcome = model.evaluate(ty);
    assert!(
        outcome.failed == 0 && outcome.undefined == 0 && outcome.held > 0,
        "{what} does not hold over computed arithmetic: {outcome:?}"
    );
}

fn assert_refuted(model: &Model<'_>, ty: TermHandle, what: &str) {
    let outcome = model.evaluate(ty);
    assert!(
        outcome.failed + outcome.undefined > 0,
        "{what} holds over computed arithmetic, so it does not show the law exact: {outcome:?}"
    );
}

#[test]
fn every_integer_law_holds_over_the_integers() {
    let mut d = Denotation::new();
    let laws = every_law();
    let statements: Vec<(CoreLaw, TermHandle)> = laws
        .iter()
        .map(|&law| (law, declared(&mut d, law)))
        .collect();
    let model = integers(&d);
    for (law, ty) in statements {
        assert_holds(&model, ty, &format!("{law:?}"));
    }
}

#[test]
fn division_laws_hold_at_every_value_of_small_carriers() {
    for ty in division_carriers() {
        let mut d = Denotation::new();
        // A quotient law has three variables; at eight bits that is 2^24
        // assignments per law, so only the two-variable laws run there.
        let narrow = ty.bits() == Some(4);
        let statements: Vec<(CoreLaw, TermHandle)> = every_law()
            .into_iter()
            .filter(|&law| family(law) == Family::Division(ty))
            .filter(|&law| narrow || matches!(law, CoreLaw::DivBound { .. }))
            .map(|law| (law, declared(&mut d, law)))
            .collect();
        assert_eq!(statements.len(), if narrow { 8 } else { 4 });
        let model = Model::new(&d, Arithmetic::Carriers, carrier_values(ty));
        for (law, statement) in statements {
            let outcome = model.evaluate(statement);
            assert!(
                outcome.failed == 0 && outcome.held > 0,
                "{law:?} fails at a computed value of {ty:?}: {outcome:?}"
            );
        }
    }
}

#[test]
fn cast_laws_hold_at_every_value_of_their_source_carrier() {
    for op in cast_operations() {
        let IntegerOperation::Unary(_, source) = op else {
            unreachable!("casts are unary")
        };
        let mut d = Denotation::new();
        let statement = declared(&mut d, CoreLaw::CastId(op));
        let model = Model::new(&d, Arithmetic::Carriers, carrier_values(source));
        let outcome = model.evaluate(statement);
        assert!(
            outcome.failed == 0 && outcome.held > 0,
            "{op:?} changes a computed value: {outcome:?}"
        );
    }
}

#[test]
fn every_premise_of_every_integer_law_is_needed() {
    let mut d = Denotation::new();
    let mut weakened = Vec::new();
    let mut premise_free = Vec::new();
    for law in every_law() {
        let ty = declared(&mut d, law);
        let count = premise_count(&d, ty);
        if count == 0 {
            premise_free.push(law);
        }
        for index in 0..count {
            weakened.push((law, index, weaken_premise(&mut d, ty, index)));
        }
    }
    let model = integers(&d);
    for (law, index, ty) in weakened {
        assert_refuted(&model, ty, &format!("{law:?} without premise {index}"));
    }
    premise_free.sort();
    premise_free.dedup();
    let mut expected = vec![
        CoreLaw::DoubleZero,
        CoreLaw::NegateZero,
        CoreLaw::EvenBeforeOdd,
        CoreLaw::AddCancelSub,
        CoreLaw::AddZeroRight,
        CoreLaw::AddZeroLeft,
        CoreLaw::SubZero,
        CoreLaw::SubSelfZero,
        CoreLaw::MulDistributeAdd,
        CoreLaw::DivisorCaseSplit,
    ];
    expected.extend(cast_operations().into_iter().map(CoreLaw::CastId));
    expected.sort();
    assert_eq!(
        premise_free, expected,
        "each law with no premise has a wrong twin in `premise_free_laws_have_refuted_twins`"
    );
}

#[test]
fn premise_free_laws_have_refuted_twins() {
    let mut d = Denotation::new();
    // Each pair is a premise-free law's declared formula and a near miss of
    // the same arity that the model must refute.
    let twins: [(u8, &str, &str); 9] = [
        (0, "d(z)=z", "d(#1)=#1"),
        (0, "n(z)=z", "n(#1)=#1"),
        (1, "d(0)<o(0)", "o(0)<d(0)"),
        (2, "((0-1)+1)=0", "((0-1)-1)=0"),
        (1, "(0+#0)=0", "(0+#1)=0"),
        (1, "(#0+0)=0", "(#1+0)=0"),
        (1, "(0-#0)=0", "(0-#1)=0"),
        (1, "(0-0)=#0", "(0-0)=#1"),
        (3, "(0*(1+2))=((0*1)+(0*2))", "(0*(1+2))=((0*1)+2)"),
    ];
    let mut built = Vec::new();
    for (arity, law, twin) in twins {
        assert!(
            CORE_LAW_FORMULAS
                .iter()
                .any(|&(_, a, f)| a == arity && f == law),
            "{law} is a declared law"
        );
        built.push((
            law,
            d.build_law_formula(arity, law, None).unwrap(),
            twin,
            d.build_law_formula(arity, twin, None).unwrap(),
        ));
    }

    // `DivisorCaseSplit` without its `d = 0` case, and a cast that doubles.
    let split = declared(&mut d, CoreLaw::DivisorCaseSplit);
    let int = d.integer_constant().unwrap();
    let (one, m1, m2) = (
        d.binary_term(false, 1).unwrap(),
        d.binary_term(true, 1).unwrap(),
        d.binary_term(true, 2).unwrap(),
    );
    let (d1, d2) = (d.var(1), d.var(2));
    let cm2 = d.order_term(false, d1, m2).unwrap();
    let cm1 = d.id(int, d2, m1);
    let positive = d.order_term(false, one, d2).unwrap();
    let rest = d.tagged_sum(cm1, positive);
    let top = d.tagged_sum(cm2, rest);
    let split_without_zero = d.pi(int, top);

    let op = cast_operations()[0];
    let cast = declared(&mut d, CoreLaw::CastId(op));
    let cast_fn = d.integer_operation(op).unwrap();
    let (f, x) = (d.constant(cast_fn), d.var(0));
    let app = d.app(f, x);
    let doubler = d.numeral_ctor(NumeralCtor::Double).unwrap();
    let dh = d.constant(doubler);
    let doubled = d.app(dh, x);
    let eq = d.id(int, app, doubled);
    let doubling_cast = d.pi(int, eq);

    let model = integers(&d);
    for (law, law_ty, twin, twin_ty) in built {
        assert_holds(&model, law_ty, law);
        assert_refuted(&model, twin_ty, twin);
    }
    assert_holds(&model, split, "DivisorCaseSplit");
    assert_refuted(&model, split_without_zero, "DivisorCaseSplit without d = 0");
    assert_holds(&model, cast, "CastId");
    assert_refuted(&model, doubling_cast, "a cast that doubles");
}

#[test]
fn a_division_law_with_the_wrong_rounding_fails_at_a_carrier_value() {
    // `DivideUpperPositive` for a truncating quotient, and the same statement
    // with the bound reversed, which holds only for a quotient rounded up.
    let ty = int_ty(IntegerSign::Signed, 4);
    let mut d = Denotation::new();
    let law = declared(
        &mut d,
        CoreLaw::DivBound {
            ty,
            lower: false,
            positive: true,
        },
    );
    let rounded_up = d
        .build_law_formula(2, "#0<=0 -> #1<=1 -> 0<=((0/1)*1)", Some(ty))
        .unwrap();
    let model = Model::new(&d, Arithmetic::Carriers, carrier_values(ty));
    let held = model.evaluate(law);
    assert!(held.failed == 0 && held.held > 0, "{held:?}");
    let refuted = model.evaluate(rounded_up);
    assert!(refuted.failed > 0, "{refuted:?}");
}
