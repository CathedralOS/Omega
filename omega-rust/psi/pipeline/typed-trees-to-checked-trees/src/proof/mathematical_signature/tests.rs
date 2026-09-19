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
    let diagnostics = refuse("let bad(x: u64): u32 = x;");

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

#[test]
fn machine_expression_body_refuses() {
    let diagnostics = refuse("let p(limit: i32, value: i32): core::Strict<0> = value > limit;");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has no kernel denotation yet")),
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
