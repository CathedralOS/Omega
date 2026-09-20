use super::{CheckedMathematicalSignature, check_mathematical_signature};
use proof_admission::{Level, Sort, Term};

fn typed_program(source: &str) -> typed_trees::TypedTrees {
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

fn signature(source: &str) -> CheckedMathematicalSignature {
    check_mathematical_signature(&typed_program(source)).expect("elaborate")
}

fn refuse(source: &str) -> Vec<diagnostics::Diagnostic> {
    match check_mathematical_signature(&typed_program(source)) {
        Ok(_) => panic!("must refuse"),
        Err(diagnostics) => diagnostics,
    }
}

/// `let double(x: u64): u64 = x;` elaborates to a carrier assumption
/// `u64 : Type 0` plus `double : Π(_ : u64). u64 := λ(_ : u64). x`.
#[test]
fn plain_definition_elaborates_to_kernel_terms() {
    let signature = signature("let double(x: u64): u64 = x;");

    assert_eq!(signature.authored(), &[1]);
    let declarations = signature.signature().declarations();
    assert_eq!(declarations.len(), 2);

    let carrier = &declarations[0];
    assert_eq!(carrier.level_arity, 0);
    assert!(carrier.body.is_none());
    assert_eq!(
        signature.term(carrier.ty),
        Term::Sort(Sort::Type(Level::Constant(0)))
    );

    let double = &declarations[1];
    assert_eq!(double.level_arity, 0);
    let Term::Pi { domain, codomain } = signature.term(double.ty) else {
        panic!("expected Pi, got {:?}", signature.term(double.ty));
    };
    assert_eq!(
        signature.term(domain),
        Term::Constant {
            declaration: 0,
            levels: Vec::new()
        }
    );
    assert_eq!(signature.term(codomain), signature.term(domain));
    let Term::Lambda {
        domain: lambda_domain,
        body,
    } = signature.term(double.body.expect("definition body"))
    else {
        panic!("expected Lambda");
    };
    assert_eq!(signature.term(lambda_domain), signature.term(domain));
    assert_eq!(signature.term(body), Term::Variable(0));
}

/// A `boundary let` is a named assumption: same statement, no body.
#[test]
fn boundary_let_is_a_named_assumption() {
    let signature = signature("boundary let choose(x: u64): u64;");

    let declarations = signature.signature().declarations();
    assert_eq!(declarations.len(), 2);
    assert_eq!(signature.authored(), &[1]);
    assert!(declarations[1].is_assumption());
    // The statement is the same Π(_ : u64). u64 a definition carries;
    // only the absent body distinguishes the assumption.
    let Term::Pi { .. } = signature.term(declarations[1].ty) else {
        panic!("expected Pi");
    };
}

/// A universe-polymorphic family declaration: `u: core::Level` claims
/// level parameter 0, `A`/`B` bind at `Type u`, the telescope and
/// dependent result elaborate to nested `Pi`, and the body `f(x)` checks
/// against it under the kernel's own rules.
#[test]
fn polymorphic_dependent_declaration_checks() {
    let signature = signature(
        "let apply<u: core::Level, A: core::Type<u>, B: core::Type<u>>(f: A -> B, x: A): B = f(x);",
    );

    assert_eq!(signature.authored(), &[0]);
    let declarations = signature.signature().declarations();
    assert_eq!(declarations.len(), 1);
    let apply = &declarations[0];
    assert_eq!(apply.level_arity, 1);
    // Π(A : Type u). Π(B : Type u). Π(f : Π(_ : A). B). Π(x : A). B
    let Term::Pi { domain, codomain } = signature.term(apply.ty) else {
        panic!("expected Pi");
    };
    assert_eq!(
        signature.term(domain),
        Term::Sort(Sort::Type(Level::Parameter(0)))
    );
    let Term::Pi { domain, codomain } = signature.term(codomain) else {
        panic!("expected Pi");
    };
    assert_eq!(
        signature.term(domain),
        Term::Sort(Sort::Type(Level::Parameter(0)))
    );
    let Term::Pi { domain, codomain } = signature.term(codomain) else {
        panic!("expected Pi");
    };
    // f : Π(_ : A). B — A elaborates under [A, B] to Variable(1); the
    // anonymous codomain lives under the Pi binder, so under [A, B, _]
    // B is Variable(1) as well.
    let Term::Pi {
        domain: f_domain,
        codomain: f_codomain,
    } = signature.term(domain)
    else {
        panic!("expected f : Pi");
    };
    assert_eq!(signature.term(f_domain), Term::Variable(1));
    assert_eq!(signature.term(f_codomain), Term::Variable(1));
    let Term::Pi { domain, codomain } = signature.term(codomain) else {
        panic!("expected Pi");
    };
    // x : A — under [A, B, f] A is Variable(2).
    assert_eq!(signature.term(domain), Term::Variable(2));
    // result B — under [A, B, f, x] B is Variable(2).
    assert_eq!(signature.term(codomain), Term::Variable(2));
}

/// An earlier declaration is referenced through `Term::Constant` and a
/// carrier interns once into the shared prefix.
#[test]
fn declaration_references_and_shared_carriers() {
    let signature =
        signature("let double(x: u64): u64 = x;\nlet quad(x: u64): u64 = double(double(x));");

    assert_eq!(signature.authored(), &[1, 2]);
    let declarations = signature.signature().declarations();
    assert_eq!(declarations.len(), 3);
    let Term::Lambda { body, .. } = signature.term(declarations[2].body.expect("quad body")) else {
        panic!("expected Lambda");
    };
    // double(double(x)) — the callee is Constant(1), the argument a
    // Variable for x.
    let Term::Apply { function, argument } = signature.term(body) else {
        panic!("expected Apply");
    };
    assert_eq!(
        signature.term(function),
        Term::Constant {
            declaration: 1,
            levels: Vec::new()
        }
    );
    let Term::Apply {
        function: inner_function,
        argument: inner_argument,
    } = signature.term(argument)
    else {
        panic!("expected inner Apply");
    };
    assert_eq!(signature.term(inner_function), signature.term(function));
    assert_eq!(signature.term(inner_argument), Term::Variable(0));
}

/// A family parameter applied in the result: `F(value)` elaborates to
/// `Apply` over the arrow domain's scope, so a `boundary let` can name a
/// dependent proposition.
#[test]
fn dependent_application_in_result() {
    let signature = signature(
        "boundary let apply<u: core::Level, A: core::Type<u>>(F: (value: A) -> core::Type<u>, x: A): F(x);",
    );

    let declarations = signature.signature().declarations();
    assert_eq!(declarations.len(), 1);
    assert_eq!(declarations[0].level_arity, 1);
    assert!(declarations[0].is_assumption());
    // Π(A : Type u). Π(F : Π(value : A). Type u). Π(x : A). F x
    let Term::Pi { codomain, .. } = signature.term(declarations[0].ty) else {
        panic!("expected Pi");
    };
    let Term::Pi { codomain, .. } = signature.term(codomain) else {
        panic!("expected Pi");
    };
    let Term::Pi { codomain, .. } = signature.term(codomain) else {
        panic!("expected Pi");
    };
    let Term::Apply { function, argument } = signature.term(codomain) else {
        panic!("expected Apply result, got {:?}", signature.term(codomain));
    };
    // F is entry 1 of 3 → index 1; x is innermost → index 0.
    assert_eq!(signature.term(function), Term::Variable(1));
    assert_eq!(signature.term(argument), Term::Variable(0));
}

/// A strict proposition result at a literal level: `core::Strict<0>` is
/// `Sort::Strict(Level::Constant 0)`.
#[test]
fn strict_result_at_literal_level() {
    let signature = signature("boundary let gt(limit: i32, value: i32): core::Strict<0>;");

    let declarations = signature.signature().declarations();
    // i32 carrier + the authored assumption.
    assert_eq!(declarations.len(), 2);
    let gt = &declarations[1];
    let Term::Pi { codomain, .. } = signature.term(gt.ty) else {
        panic!("expected Pi");
    };
    let Term::Pi { codomain, .. } = signature.term(codomain) else {
        panic!("expected Pi");
    };
    assert_eq!(
        signature.term(codomain),
        Term::Sort(Sort::Strict(Level::Constant(0)))
    );
}

/// A bare type binder generalizes a universe parameter deterministically.
#[test]
fn bare_type_binder_generalizes_a_level() {
    let signature = signature("boundary let id<T>(x: T): T;");

    let declarations = signature.signature().declarations();
    assert_eq!(declarations.len(), 1);
    assert_eq!(declarations[0].level_arity, 1);
    let Term::Pi { domain, codomain } = signature.term(declarations[0].ty) else {
        panic!("expected Pi");
    };
    // T binds at a generalized universe parameter — the declaration's
    // only level parameter.
    assert_eq!(
        signature.term(domain),
        Term::Sort(Sort::Type(Level::Parameter(0)))
    );
    let Term::Pi { domain, codomain } = signature.term(codomain) else {
        panic!("expected Pi");
    };
    // x : T — under [T] T is Variable(0); result under [T, x] is
    // Variable(1).
    assert_eq!(signature.term(domain), Term::Variable(0));
    assert_eq!(signature.term(codomain), Term::Variable(1));
}

#[test]
fn body_type_mismatch_fails_kernel_checking() {
    // Fixed integer carriers share `Int`, so `x: u64` inhabits `u32`'s
    // denotation too — the mismatch needs a genuinely distinct carrier.
    let diagnostics = refuse("let bad(x: u64): bool = x;");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("fails kernel checking")
                && diagnostic.message.contains("bad")
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn unbound_body_name_refuses() {
    let diagnostics = refuse("let f(x: u64): u64 = term;");

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("unresolved mathematical name `term`")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn unbound_result_family_refuses() {
    let diagnostics = refuse("let f(x: u64): F(x) = x;");

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("unresolved mathematical type `F`")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

/// The spec's canonical machine-valued body: `value > limit` denotes
/// `Squash (IntLt limit value)` at the `Strict 0` boundary, with the
/// `i32` telescope sharing the `Int` carrier.
#[test]
fn machine_comparison_denotes_squashed_integer_order() {
    let signature = signature("let gt(limit: i32, value: i32): core::Strict<0> = value > limit;");

    crate::lower_typed_trees(typed_program(
        "let gt(limit: i32, value: i32): core::Strict<0> = value > limit;",
    ))
    .expect("machine comparison body checks");

    assert_eq!(signature.authored(), &[2]);
    let declarations = signature.signature().declarations();
    // `Int : Type 0`, `IntLt : Π(_ : Int). Π(_ : Int). Type 0`, `gt`.
    assert_eq!(declarations.len(), 3);
    let gt = &declarations[2];
    // Π(_ : Int). Π(_ : Int). Strict 0
    let Term::Pi { domain, codomain } = signature.term(gt.ty) else {
        panic!("expected Pi, got {:?}", signature.term(gt.ty));
    };
    assert_eq!(
        signature.term(domain),
        Term::Constant {
            declaration: 0,
            levels: Vec::new()
        }
    );
    let Term::Pi { codomain, .. } = signature.term(codomain) else {
        panic!("expected inner Pi");
    };
    assert_eq!(
        signature.term(codomain),
        Term::Sort(Sort::Strict(Level::Constant(0)))
    );
    // λ(_ : Int). λ(_ : Int). Squash (IntLt limit value) — `value`
    // is innermost (index 0), `limit` index 1.
    let mut body = gt.body.expect("definition body");
    while let Term::Lambda { body: inner, .. } = signature.term(body) {
        body = inner;
    }
    let Term::Squash { ty } = signature.term(body) else {
        panic!("expected Squash, got {:?}", signature.term(body));
    };
    let Term::Apply {
        function: application,
        argument,
    } = signature.term(ty)
    else {
        panic!("expected IntLt application, got {:?}", signature.term(ty));
    };
    assert_eq!(signature.term(argument), Term::Variable(0));
    let Term::Apply {
        function: relation,
        argument: left,
    } = signature.term(application)
    else {
        panic!("expected IntLt head, got {:?}", signature.term(application));
    };
    assert_eq!(
        signature.term(relation),
        Term::Constant {
            declaration: 1,
            levels: Vec::new()
        }
    );
    assert_eq!(signature.term(left), Term::Variable(1));
}

/// `x == y` denotes `Id Int y x` — endpoints order canonically so both
/// spellings share one denotation.
#[test]
fn machine_equality_denotes_id_over_int() {
    let signature = signature("let eq(a: u64, b: u64): core::Strict<0> = a == b;");
    crate::lower_typed_trees(typed_program(
        "let eq(a: u64, b: u64): core::Strict<0> = a == b;",
    ))
    .expect("machine equality body checks");

    let declarations = signature.signature().declarations();
    // `Int` carrier + `eq`.
    assert_eq!(declarations.len(), 2);
    let mut body = declarations[1].body.expect("definition body");
    while let Term::Lambda { body: inner, .. } = signature.term(body) {
        body = inner;
    }
    let Term::Squash { ty } = signature.term(body) else {
        panic!("expected Squash, got {:?}", signature.term(body));
    };
    let Term::Id { ty, left, right } = signature.term(ty) else {
        panic!("expected Id, got {:?}", signature.term(ty));
    };
    assert_eq!(
        signature.term(ty),
        Term::Constant {
            declaration: 0,
            levels: Vec::new()
        }
    );
    // Canonical order: `a` (index 1) and `b` (index 0) sort to (b, a).
    assert_eq!(signature.term(left), Term::Variable(0));
    assert_eq!(signature.term(right), Term::Variable(1));
}

/// `x <= limit && y <= limit` denotes a right-nested `Σ` of `IntLe`
/// propositions under `Squash`.
#[test]
fn machine_conjunction_denotes_nested_sigma() {
    let signature = signature(
        "let within(x: u64, y: u64, limit: u64): core::Strict<0> = x <= limit && y <= limit;",
    );

    let declarations = signature.signature().declarations();
    // `Int`, `IntLe`, `within`.
    assert_eq!(declarations.len(), 3);
    let mut body = declarations[2].body.expect("definition body");
    while let Term::Lambda { body: inner, .. } = signature.term(body) {
        body = inner;
    }
    let Term::Squash { ty } = signature.term(body) else {
        panic!("expected Squash, got {:?}", signature.term(body));
    };
    // Both conjuncts denote `IntLe _ limit` — canonical term order
    // places `y <= limit` first (y is the shallower variable: `x`,
    // `y`, `limit` are indices 2, 1, 0).
    let Term::Sigma { domain, codomain } = signature.term(ty) else {
        panic!("expected Sigma, got {:?}", signature.term(ty));
    };
    let Term::Apply { function, argument } = signature.term(domain) else {
        panic!("expected Apply");
    };
    assert_eq!(signature.term(argument), Term::Variable(0));
    let Term::Apply {
        function: relation,
        argument: left,
    } = signature.term(function)
    else {
        panic!("expected IntLe head");
    };
    assert_eq!(
        signature.term(relation),
        Term::Constant {
            declaration: 1,
            levels: Vec::new()
        }
    );
    assert_eq!(signature.term(left), Term::Variable(1));
    // `x <= limit` under one binder: x shifts to index 3, limit to 1.
    let Term::Apply { function, argument } = signature.term(codomain) else {
        panic!(
            "expected Apply codomain, got {:?}",
            signature.term(codomain)
        );
    };
    assert_eq!(signature.term(argument), Term::Variable(1));
    let Term::Apply { argument: left, .. } = signature.term(function) else {
        panic!("expected IntLe head");
    };
    assert_eq!(signature.term(left), Term::Variable(3));
}

/// `a < b || c < d` denotes `Σ(t : Two). caseTwo(M, _, _, t)` under
/// `Squash` — the bounded vocabulary's tagged sum.
#[test]
fn machine_disjunction_denotes_tagged_sum() {
    let signature =
        signature("let either(a: i64, b: i64, c: i64, d: i64): core::Strict<0> = a < b || c < d;");

    let declarations = signature.signature().declarations();
    // `Int`, `IntLt`, `either`.
    assert_eq!(declarations.len(), 3);
    let mut body = declarations[2].body.expect("definition body");
    while let Term::Lambda { body: inner, .. } = signature.term(body) {
        body = inner;
    }
    let Term::Squash { ty } = signature.term(body) else {
        panic!("expected Squash, got {:?}", signature.term(body));
    };
    let Term::Sigma { domain, codomain } = signature.term(ty) else {
        panic!("expected Sigma, got {:?}", signature.term(ty));
    };
    assert_eq!(signature.term(domain), Term::Two);
    let Term::CaseTwo { scrutinee, .. } = signature.term(codomain) else {
        panic!("expected CaseTwo, got {:?}", signature.term(codomain));
    };
    assert_eq!(signature.term(scrutinee), Term::Variable(0));
}

/// A bare `bool` subject denotes `x = true` — the same proposition
/// `lower_proposition` forms — interned once at the `bool` carrier.
#[test]
fn machine_boolean_subject_denotes_true_identity() {
    let signature = signature("let ready(flag: bool): core::Strict<0> = flag;");

    let declarations = signature.signature().declarations();
    // `bool` carrier, `true` literal, `ready`.
    assert_eq!(declarations.len(), 3);
    let mut body = declarations[2].body.expect("definition body");
    while let Term::Lambda { body: inner, .. } = signature.term(body) {
        body = inner;
    }
    let Term::Squash { ty } = signature.term(body) else {
        panic!("expected Squash, got {:?}", signature.term(body));
    };
    let Term::Id { ty, left, right } = signature.term(ty) else {
        panic!("expected Id, got {:?}", signature.term(ty));
    };
    assert_eq!(
        signature.term(ty),
        Term::Constant {
            declaration: 0,
            levels: Vec::new()
        }
    );
    assert_eq!(signature.term(left), Term::Variable(0));
    assert_eq!(
        signature.term(right),
        Term::Constant {
            declaration: 1,
            levels: Vec::new()
        }
    );
}

/// Closed integer arithmetic evaluates to one literal `Int` constant —
/// `2 + 3 == 5` denotes `Id Int five five`, which `refl` proves.
#[test]
fn closed_machine_arithmetic_denotes_exact_literal() {
    let signature = signature("let closed(): core::Strict<0> = 2 + 3 == 5;");

    let declarations = signature.signature().declarations();
    // `Int`, the `5` literal, `closed`.
    assert_eq!(declarations.len(), 3);
    let body = declarations[2].body.expect("definition body");
    let Term::Squash { ty } = signature.term(body) else {
        panic!("expected Squash, got {:?}", signature.term(body));
    };
    let Term::Id { left, right, .. } = signature.term(ty) else {
        panic!("expected Id, got {:?}", signature.term(ty));
    };
    assert_eq!(
        signature.term(left),
        Term::Constant {
            declaration: 1,
            levels: Vec::new()
        }
    );
    assert_eq!(signature.term(left), signature.term(right));
}

/// Open `+` composes through `IntAdd`; `x + 0 == x` denotes
/// `Squash (Id Int (IntAdd x zero) x)` — not reflexive, a real
/// proposition.
#[test]
fn open_addition_composes_through_the_shared_function() {
    let signature = signature("let lemma(x: u64): core::Strict<0> = x + 0 == x;");
    crate::lower_typed_trees(typed_program(
        "let lemma(x: u64): core::Strict<0> = x + 0 == x;",
    ))
    .expect("open addition body checks");

    let declarations = signature.signature().declarations();
    // `Int`, `IntAdd`, the `0` literal, `lemma`.
    assert_eq!(declarations.len(), 4);
    let mut body = declarations[3].body.expect("definition body");
    while let Term::Lambda { body: inner, .. } = signature.term(body) {
        body = inner;
    }
    let Term::Squash { ty } = signature.term(body) else {
        panic!("expected Squash, got {:?}", signature.term(body));
    };
    let Term::Id { left, right, .. } = signature.term(ty) else {
        panic!("expected Id, got {:?}", signature.term(ty));
    };
    // `x` (Variable 0) sorts before the application.
    assert_eq!(signature.term(left), Term::Variable(0));
    let Term::Apply { function, argument } = signature.term(right) else {
        panic!(
            "expected IntAdd application, got {:?}",
            signature.term(right)
        );
    };
    assert_eq!(
        signature.term(argument),
        Term::Constant {
            declaration: 2,
            levels: Vec::new()
        }
    );
    let Term::Apply {
        function: operation,
        argument: operand,
    } = signature.term(function)
    else {
        panic!("expected IntAdd head");
    };
    assert_eq!(
        signature.term(operation),
        Term::Constant {
            declaration: 1,
            levels: Vec::new()
        }
    );
    assert_eq!(signature.term(operand), Term::Variable(0));
}

/// A machine-valued body over an earlier declaration composes: `half(x)`
/// applies the declaration constant inside the `Int` relation.
#[test]
fn machine_call_operand_denotes_application() {
    let signature = signature(
        "let double(x: u64): u64 = x + x;\nlet exceeds(x: u64): core::Strict<0> = double(x) > x;",
    );
    crate::lower_typed_trees(typed_program(
        "let double(x: u64): u64 = x + x;\nlet exceeds(x: u64): core::Strict<0> = double(x) > x;",
    ))
    .expect("call operand checks");

    let declarations = signature.signature().declarations();
    // `Int`, `IntAdd`, `double`, `IntLt`, `exceeds`.
    assert_eq!(declarations.len(), 5);
    assert_eq!(signature.authored(), &[2, 4]);
}

#[test]
fn open_multiplication_refuses() {
    let diagnostics = refuse("let product(x: u64, y: u64): u64 = x * y;");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("no bounded denotation")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn negated_equality_refuses() {
    let diagnostics = refuse("let different(x: u64, y: u64): core::Strict<0> = x != y;");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("no negation")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn order_over_non_integer_carriers_refuses() {
    for source in [
        "let lt(a: bool, b: bool): core::Strict<0> = a < b;",
        "let lt(a: addr, b: addr): core::Strict<0> = a < b;",
    ] {
        let diagnostics = refuse(source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("non-address integers")),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn equality_across_distinct_carriers_refuses() {
    let diagnostics = refuse("let bad(x: u64, b: bool): core::Strict<0> = x == b;");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("different scalar carrier")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn level_is_not_a_term_type() {
    let diagnostics = refuse("boundary let f(x: core::Level): u64;");

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`core::Level` is a universe level")),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

#[test]
fn forward_declaration_reference_refuses() {
    let diagnostics = refuse("let a(x: u64): u64 = b(x);\nlet b(y: u64): u64 = y;");

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("references itself or a later declaration")
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

const EXPLICIT_LEVEL_IDENTITY: &str =
    "let identity<u: core::Level, A: core::Type<u>>(x: A): A = x;";

#[test]
fn explicit_levels_and_generic_types_reach_checked_admission() {
    let source = format!(
        "{EXPLICIT_LEVEL_IDENTITY}
         let relay<v: core::Level, B: core::Type<v>>(x: B): B = identity<v,B>(x);
         let closed(B: core::Type<0>, x: B): B = identity<0,B>(x);"
    );
    crate::lower_typed_trees(typed_program(&source)).expect("checked polymorphic applications");
    let checked = signature(&source);
    for (index, level) in [(1, Level::Parameter(0)), (2, Level::Constant(0))] {
        let declaration = &checked.signature().declarations()[checked.authored()[index] as usize];
        let mut body = declaration.body.expect("transparent body");
        while let Term::Lambda { body: inner, .. } = checked.term(body) {
            body = inner;
        }
        let Term::Apply { function, .. } = checked.term(body) else {
            panic!("ordinary argument")
        };
        let Term::Apply { function, .. } = checked.term(function) else {
            panic!("generic type argument")
        };
        assert_eq!(
            checked.term(function),
            Term::Constant {
                declaration: checked.authored()[0],
                levels: vec![level]
            }
        );
    }
}

#[test]
fn explicit_levels_follow_the_ordered_mixed_generic_telescope() {
    let source = "let pick<u: core::Level, A: core::Type<u>, v: core::Level, B: core::Type<v>>(x: A, y: B): B = y;
        let relay<v: core::Level, u: core::Level, A: core::Type<u>, B: core::Type<v>>(x: A, y: B): B = pick<u,A,v,B>(x,y);";
    crate::lower_typed_trees(typed_program(source)).expect("interleaved level and type arguments");
    let checked = signature(source);
    let mut body = checked.signature().declarations()[checked.authored()[1] as usize]
        .body
        .unwrap();
    loop {
        match checked.term(body) {
            Term::Lambda { body: inner, .. } => body = inner,
            Term::Apply { function, .. } => body = function,
            term => {
                assert_eq!(
                    term,
                    Term::Constant {
                        declaration: checked.authored()[0],
                        levels: vec![Level::Parameter(1), Level::Parameter(0)]
                    }
                );
                break;
            }
        }
    }
}

#[test]
fn explicit_wrong_level_is_rejected_by_kernel_application_checking() {
    let source = format!(
        "{EXPLICIT_LEVEL_IDENTITY}
        let wrong<v: core::Level, A: core::Type<v>>(x: A): A = identity<0,A>(x);"
    );
    let diagnostics = crate::lower_typed_trees(typed_program(&source)).expect_err("wrong level");
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("fails kernel checking")
                && diagnostic.message.contains("wrong")
        ),
        "{diagnostics:?}"
    );
}

#[test]
fn explicit_type_cannot_supply_a_level_argument() {
    let source = format!(
        "{EXPLICIT_LEVEL_IDENTITY}
        let wrong<v: core::Level, A: core::Type<v>>(x: A): A = identity<A,v>(x);"
    );
    let diagnostics =
        crate::lower_typed_trees(typed_program(&source)).expect_err("wrong argument kinds");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("core::Level")),
        "{diagnostics:?}"
    );
}

#[test]
fn explicit_mathematical_arguments_do_not_bypass_executable_call_admission() {
    use typed_trees::expression::ExpressionNode;
    use typed_trees::mathematical::MathematicalBody;
    use typed_trees::statement::StatementNode;

    let source = format!(
        "{EXPLICIT_LEVEL_IDENTITY}
        let relay<v: core::Level, A: core::Type<v>>(x: A): A = identity<v,A>(x);
        machine ordinary(x: u64) -> u64 {{ x }}
        machine executable(x: u64) -> u64 {{ ordinary(x) }}"
    );
    let original = typed_program(&source);
    let MathematicalBody::Definition(mathematical) = original.mathematical_definitions()[1].body
    else {
        panic!("mathematical body");
    };
    let ExpressionNode::Call(mathematical_call) =
        original.expression_table.expression(mathematical)
    else {
        panic!("mathematical application");
    };
    let runtime = original
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            matches!(node, ExpressionNode::Call(call) if call.target.as_str() == "ordinary")
                .then_some(handle)
        })
        .expect("runtime call");
    for shared in [false, true] {
        let mut program = original.clone();
        if shared {
            let statements = program.machines().iter().flat_map(|machine| program.machine_states(machine))
                .flat_map(|state| program.statement_table.iter_statements(state.statement_nodes))
                .filter_map(|(handle, node)| matches!(node, StatementNode::LocalData(local) if local.initial_value == runtime).then_some(handle))
                .collect::<Vec<_>>();
            assert!(!statements.is_empty(), "hoisted runtime call initializer");
            for statement in statements {
                if let StatementNode::LocalData(local) =
                    program.statement_table.statement_mut(statement)
                {
                    local.initial_value = mathematical;
                }
            }
        } else {
            // Same target and arguments, distinct executable occurrence.
            *program.expression_table.expression_mut(runtime) =
                ExpressionNode::Call(mathematical_call.clone());
        }
        let diagnostics = validation::validate_static_machine_selections(&program)
            .expect_err("runtime application has no executable selection");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("generic callee did not resolve")),
            "shared={shared}: {diagnostics:?}"
        );
    }
}

#[test]
fn explicit_generic_argument_arity_and_scope_are_checked() {
    for arguments in ["v", "v,A,A", "missing,A", "4294967296,A"] {
        let source = format!(
            "{EXPLICIT_LEVEL_IDENTITY}
            let wrong<v: core::Level, A: core::Type<v>>(x: A): A = identity<{arguments}>(x);"
        );
        let diagnostics = crate::lower_typed_trees(typed_program(&source)).expect_err(arguments);
        assert!(
            diagnostics.iter().any(
                |diagnostic| diagnostic.message.contains("generic arguments")
                    || diagnostic.message.contains("core::Level")
                    || diagnostic.message.contains("level range")
            ),
            "{arguments}: {diagnostics:?}"
        );
    }
}

#[test]
fn explicit_generic_application_preserves_partial_ordinary_application() {
    let source = format!(
        "{EXPLICIT_LEVEL_IDENTITY}
        let partial<v: core::Level, A: core::Type<v>>(): A -> A = identity<v,A>();"
    );
    crate::lower_typed_trees(typed_program(&source)).expect("remaining ordinary Pi argument");
}

#[test]
fn implicit_static_arguments_preserve_existing_ordinary_prefix_application() {
    let source = "let generic<T: u64>(x: u64): u64 = x;
        let partial(x: u64): u64 -> u64 = generic(x);
        let empty(): u64 -> u64 -> u64 = generic();";
    crate::lower_typed_trees(typed_program(source)).expect("ordinary prefix remains a Pi term");
}

#[test]
fn explicit_application_keeps_declaration_order_and_generalized_level_fences() {
    for source in [
        "let recursive<u: core::Level, A: core::Type<u>>(x: A): A = recursive<u,A>(x);",
        "let first<u: core::Level, A: core::Type<u>>(x: A): A = second<u,A>(x);
         let second<u: core::Level, A: core::Type<u>>(x: A): A = x;",
    ] {
        let diagnostics =
            crate::lower_typed_trees(typed_program(source)).expect_err("ordered signature");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("references itself or a later declaration")),
            "{diagnostics:?}"
        );
    }
}

/// An omitted universe argument of a generalized binder infers from the
/// supplied type argument's own sort: inside `use`, `A` binds at
/// `Type u`, so `inferred<A>` instantiates `inferred`'s generalized
/// parameter at `u`.
#[test]
fn generalized_level_infers_from_the_type_argument() {
    let source = "let inferred<A>(x: A): A = x;
        let use<u: core::Level, A: core::Type<u>>(x: A): A = inferred<A>(x);";
    crate::lower_typed_trees(typed_program(source)).expect("inferred level application");
    let checked = signature(source);
    let mut body = checked.signature().declarations()[checked.authored()[1] as usize]
        .body
        .unwrap();
    while let Term::Lambda { body: inner, .. } = checked.term(body) {
        body = inner;
    }
    let Term::Apply { function, .. } = checked.term(body) else {
        panic!("ordinary argument")
    };
    let Term::Apply { function, .. } = checked.term(function) else {
        panic!("generic type argument")
    };
    assert_eq!(
        checked.term(function),
        Term::Constant {
            declaration: checked.authored()[0],
            levels: vec![Level::Parameter(0)]
        }
    );
}

/// A closed carrier argument infers a closed generalized level:
/// `u64` interned at `Type 0` instantiates the parameter at level `0`.
#[test]
fn generalized_level_infers_a_closed_carrier_level() {
    let source = "let inferred<A>(x: A): A = x;
        let closed(x: u64): u64 = inferred<u64>(x);";
    crate::lower_typed_trees(typed_program(source)).expect("closed level inference");
    let checked = signature(source);
    let mut body = checked.signature().declarations()[checked.authored()[1] as usize]
        .body
        .unwrap();
    while let Term::Lambda { body: inner, .. } = checked.term(body) {
        body = inner;
    }
    let Term::Apply { function, .. } = checked.term(body) else {
        panic!("ordinary argument")
    };
    let Term::Apply { function, .. } = checked.term(function) else {
        panic!("generic type argument")
    };
    assert_eq!(
        checked.term(function),
        Term::Constant {
            declaration: checked.authored()[0],
            levels: vec![Level::Constant(0)]
        }
    );
}

/// Inference is bounded to what the explicit application supplies: a
/// non-type argument cannot determine a generalized level, and omitting
/// the whole generic argument list stays an annotation requirement.
#[test]
fn generalized_level_inference_stays_bounded() {
    let diagnostics = crate::lower_typed_trees(typed_program(
        "let inferred<A>(x: A): A = x;
         let f(x: u64): u64 = inferred<x>(x);",
    ))
    .expect_err("a value cannot determine a universe");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot infer the generalized universe argument")),
        "{diagnostics:?}"
    );
    let diagnostics = crate::lower_typed_trees(typed_program(
        "let inferred<A>(x: A): A = x;
         let use<A>(x: A): A = inferred(x);",
    ))
    .expect_err("omitted generic arguments stay refused");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("explicit level arguments")),
        "{diagnostics:?}"
    );
}

#[test]
fn explicit_application_keeps_term_type_checking() {
    let source = format!(
        "{EXPLICIT_LEVEL_IDENTITY}
        let wrong<v: core::Level, A: core::Type<v>, B: core::Type<v>>(x: B): A = identity<v,A>(x);"
    );
    let diagnostics =
        crate::lower_typed_trees(typed_program(&source)).expect_err("distinct type arguments");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("fails kernel checking")),
        "{diagnostics:?}"
    );
}
