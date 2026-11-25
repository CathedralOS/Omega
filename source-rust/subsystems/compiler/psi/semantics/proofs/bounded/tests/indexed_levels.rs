//! Universe-polymorphic native inductive families: a single declaration
//! `Vec[u] : Π(E : Type u). Π(n : Nat). Type u` checked once at level arity 1
//! and instantiated at `u = 0` (`Type 0` vector of `Two`) and `u = 1`
//! (`Type 1` vector of `Type 0`) inside one judgment.

use crate::common::*;
use proof_admission::{
    Context, CoreError, Declaration, Level, Signature, Sort, TermArena, TermHandle,
    check_signature, check_type, convertible, declare_inductive_family, infer_sort, infer_type,
};

const VEC: u32 = 3;
const VEC_NIL: u32 = 4;
const VEC_CONS: u32 = 5;
const VEC_REC: u32 = 6;

fn level_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = nat_succ_declarations(arena);
    let u = Level::Parameter(0);
    let type_u = sort_level(arena, u.clone());
    let nat_ty = nat(arena);

    // `Vec[u] : Π(E : Type u). Π(n : Nat). Type u` (1 uniform parameter `E`, 1 index `n`).
    let vec_ty = {
        let over_n = pi(arena, nat_ty, type_u);
        pi(arena, type_u, over_n)
    };

    // `nil[u] : Π(E : Type u). Vec[u] E zeroN`
    let nil_ty = {
        let head = constant(arena, VEC, vec![u.clone()]);
        let e = variable(arena, 0);
        let zero = nat_zero(arena);
        let ret = apps(arena, head, &[e, zero]);
        pi(arena, type_u, ret)
    };

    // `cons[u] : Π(E : Type u). Π(e : E). Π(n : Nat). Π(t : Vec[u] E n). Vec[u] E (succN n)`
    let cons_ty = {
        let head = constant(arena, VEC, vec![u]);
        let e_ty = variable(arena, 0);
        let tail_ty = {
            let e_param = variable(arena, 2);
            let n = variable(arena, 0);
            apps(arena, head, &[e_param, n])
        };
        let ret = {
            let e_param = variable(arena, 3);
            let n = variable(arena, 1);
            let succ_n = nat_succ(arena, n);
            apps(arena, head, &[e_param, succ_n])
        };
        let over_tail = pi(arena, tail_ty, ret);
        let over_n = pi(arena, nat_ty, over_tail);
        let over_e = pi(arena, e_ty, over_n);
        pi(arena, type_u, over_e)
    };

    declarations.extend(declare_inductive_family(
        arena,
        VEC,
        1,
        1,
        vec_ty,
        &[nil_ty, cons_ty],
    ));
    declarations
}

fn level_signature(arena: &mut TermArena) -> Signature {
    let declarations = level_declarations(arena);
    check_signature(arena, &declarations, &mut budget()).unwrap()
}

fn vec_at(arena: &mut TermArena, u: Level, element: TermHandle, length: TermHandle) -> TermHandle {
    let family = constant(arena, VEC, vec![u]);
    apps(arena, family, &[element, length])
}

#[test]
fn the_polymorphic_family_declaration_checks_once_and_instantiates_at_each_level() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let declarations = level_declarations(&mut arena);
    assert_eq!(declarations.len(), 7);

    let signature = check_signature(&mut arena, &declarations, &mut budget).unwrap();
    assert_eq!(signature.len(), 7);

    let index_type = nat(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(index_type);
    let length = variable(&mut arena, 0);

    let element0 = two(&mut arena);
    let vec_two = vec_at(&mut arena, Level::Constant(0), element0, length);
    let sort = infer_sort(&mut arena, &context, vec_two, &mut budget).unwrap();
    assert_eq!(sort, Sort::Type(Level::Constant(0)));

    let element1 = type_sort(&mut arena, 0);
    let vec_types = vec_at(&mut arena, Level::Constant(1), element1, length);
    let sort = infer_sort(&mut arena, &context, vec_types, &mut budget).unwrap();
    assert_eq!(sort, Sort::Type(Level::Constant(1)));

    // Construct values at both levels using the same polymorphic constructors.
    let zero = nat_zero(&mut arena);
    let one = nat_succ(&mut arena, zero);
    let nil0_head = constant(&mut arena, VEC_NIL, vec![Level::Constant(0)]);
    let nil0 = apply(&mut arena, nil0_head, element0);
    let cons0_head = constant(&mut arena, VEC_CONS, vec![Level::Constant(0)]);
    let z_two = two_zero(&mut arena);
    let cons0 = apps(&mut arena, cons0_head, &[element0, z_two, zero, nil0]);
    let expected0 = vec_at(&mut arena, Level::Constant(0), element0, one);
    check_type(&mut arena, &context, cons0, expected0, &mut budget).unwrap();

    let nil1_head = constant(&mut arena, VEC_NIL, vec![Level::Constant(1)]);
    let nil1 = apply(&mut arena, nil1_head, element1);
    let cons1_head = constant(&mut arena, VEC_CONS, vec![Level::Constant(1)]);
    let cons1 = apps(&mut arena, cons1_head, &[element1, element0, zero, nil1]);
    let expected1 = vec_at(&mut arena, Level::Constant(1), element1, one);
    check_type(&mut arena, &context, cons1, expected1, &mut budget).unwrap();

    // The level-0 instantiation is not an operand at the level-1 shared type.
    let shared_one = type_sort(&mut arena, 1);
    assert!(matches!(
        convertible(
            &mut arena,
            &context,
            vec_two,
            vec_types,
            shared_one,
            &mut budget
        ),
        Err(CoreError::TypeMismatch { .. })
    ));
}

#[test]
fn level_instantiation_rejects_the_wrong_universe() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let signature = level_signature(&mut arena);
    let index_type = nat(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(index_type);

    // `Vec[1] Two n` — `Two : Type 0` is one universe too low for `Type 1`.
    let length = variable(&mut arena, 0);
    let element = two(&mut arena);
    let low = vec_at(&mut arena, Level::Constant(1), element, length);
    let error = infer_type(&mut arena, &context, low, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::ArgumentTypeMismatch { .. }));

    // `Vec` takes one level argument; supplying none is malformed.
    let bare = constant(&mut arena, VEC, Vec::new());
    let error = infer_type(&mut arena, &context, bare, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::DeclarationArityMismatch {
            declaration: VEC,
            expected: 1,
            supplied: 0,
        }
    );

    // A `Parameter` level argument under a closed context is out of scope.
    let closed = Context::empty().with_signature(context.signature().clone());
    let escaped = constant(&mut arena, VEC, vec![Level::Parameter(0)]);
    let error = infer_type(&mut arena, &closed, escaped, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::UnboundLevelParameter { index: 0, arity: 0 }
    );

    // `Vec.rec` is polymorphic over `[u, w]`: supplying only 1 level argument fails.
    let short = constant(&mut arena, VEC_REC, vec![Level::Constant(0)]);
    let error = infer_type(&mut arena, &context, short, &mut budget).unwrap_err();
    assert_eq!(
        error,
        CoreError::DeclarationArityMismatch {
            declaration: VEC_REC,
            expected: 2,
            supplied: 1,
        }
    );

    // Universe predicativity: an inductive in `Type 0` cannot store a field of
    // type `Type 0 : Type 1`.
    let type0 = type_sort(&mut arena, 0);
    let bad_head = constant(&mut arena, 0, Vec::new());
    let bad_ctor = pi(&mut arena, type0, bad_head);
    let bad_decls = declare_inductive_family(&mut arena, 0, 0, 0, type0, &[bad_ctor]);
    let err = check_signature(&mut arena, &bad_decls, &mut budget).unwrap_err();
    assert!(matches!(err, CoreError::InductiveUniverseViolation));
}
