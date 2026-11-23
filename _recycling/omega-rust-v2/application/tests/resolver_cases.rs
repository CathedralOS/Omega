//! L2 resolver diagnostic twins through the CLI. Each test pairs a package that prepares with
//! a focused edit that the resolver refuses at one exact site. Each family is a separate test,
//! so a failing case does not hide the others. These cases sample the contract's distinct
//! refusal families; they do not claim every failure site of a rule.
#[path = "support/diagnostic_twins.rs"]
mod diagnostic_twins;
use diagnostic_twins::{Case, check};
use omega_resolver::NameErrorKind;

#[test]
fn an_applied_domain_carrier_requires_its_own_import() {
    let data = ("geometry/point.omg", "public data Point<T> {}");
    let domain = ("ready.omg", r"use geometry\Point; domain Point<u8>::Ready;");
    let caller = r"machine f(x: geometry\Point<u8> in Point<u8>::Ready) {}";
    let imported = format!(r"use geometry\Point; {caller}");
    check_resolver(NameErrorKind::UnresolvedApplicationOwner, Case {
        layer: "L2",
        producer: "resolver",
        good: &[data, domain, ("main.omg", &imported)],
        bad: &[data, domain, ("main.omg", caller)],
        file: "main.omg",
        occurrence: "Point",
        reason: "unresolved application owner Point",
        rule: "Namespaces and imports",
    });
}

/// The package identity the CLI gives to the files it is asked to read.
const ROOT_PACKAGE: &str = "root";

/// Resolve the refused files directly as the CLI's root package and pin the typed kind of the
/// refusal, then run the CLI twin. The kind is reported only after both paths agree.
fn check_resolver(
    kind: NameErrorKind,
    case: Case<'_>,
) {
    let (file, occurrence) = (case.file, case.occurrence);
    let error = diagnostic_twins::resolve_root_package(case.bad).expect_err("the refused package resolved");
    let (_, source) = case
        .bad
        .iter()
        .find(|(name, _)| *name == file)
        .expect("refusal belongs to the supplied package");
    let start = source
        .rfind(occurrence)
        .expect("expected occurrence is in the refused input");
    let end = start + occurrence.len();
    let reason = case.reason;
    check(case);
    spec_corpus::assert_resolver_refusal!(error, kind, ROOT_PACKAGE, file, omega_lexer::SourceSpan::new(start, end), reason);
}

// Namespaces and imports: a sibling file binds, a child directory introduces its own namespace.
const SCORE_CALLER: &str = "machine main() -> u32 {\n    score()\n}\n";
const SCORE: &str = "machine score() -> u32 { 7 }\n";

#[test]
fn a_module_qualified_machine_names_an_existing_directory() {
    let directory = ("tools/api.omg", "public data Entry {}");
    check_resolver(
        NameErrorKind::UnresolvedModuleOwner,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[("main.omg", r"machine tools\helper() {}"), directory],
            bad: &[("main.omg", r"machine absent\helper() {}"), directory],
            file: "main.omg",
            occurrence: r"absent\helper",
            reason: "unresolved module owner absent",
            rule: "Visibility",
        },
    );
}

#[test]
fn two_imported_modules_compete_before_the_member_is_selected() {
    let left = ("left/audio/items.omg", "public machine play() {}");
    let right = ("right/audio/items.omg", "public machine stop() {}");
    check_resolver(
        NameErrorKind::CompetingModulePaths,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[("main.omg", r"use left\audio; machine preview() { audio\play(); }"), left, right],
            bad: &[("main.omg", r"use left\audio; use right\audio; machine preview() { audio\play(); }"), left, right],
            file: "main.omg",
            occurrence: "audio",
            reason: "competing module paths audio",
            rule: "Dependency authority",
        },
    );
}

#[test]
fn missing_member_names_the_opened_module_and_package() {
    let helper = ("combat/score.omg", "public machine score() -> u32 { 7 }");
    check_resolver(
        NameErrorKind::UnresolvedName,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[
                ("main.omg", r"use combat; machine main() -> u32 { combat\score() }"),
                helper,
            ],
            bad: &[
                ("main.omg", r"use combat; machine main() -> u32 { combat\missing() }"),
                helper,
            ],
            file: "main.omg",
            occurrence: r"combat\missing",
            reason: r"unresolved name combat\missing in imported module combat of package root",
            rule: "Namespaces and imports",
        },
    );
}

#[test]
fn a_child_directory_declaration_is_not_a_sibling() {
    check_resolver(
        NameErrorKind::UnresolvedName,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[("combat/main.omg", SCORE_CALLER), ("combat/scoring.omg", SCORE)],
            bad: &[
                ("combat/main.omg", SCORE_CALLER),
                ("combat/ai/scoring.omg", SCORE),
            ],
            file: "combat/main.omg",
            occurrence: "score",
            reason: "unresolved name score",
            rule: "Namespaces and imports",
        },
    );
}

#[test]
fn a_local_cannot_shadow_a_visible_parameter() {
    check_resolver(
        NameErrorKind::ShadowedValueBinding,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[(
                "keep.omg",
                "machine keep(limit: u32) -> u32 {\n    let saved = limit;\n    let copy = saved;\n    saved\n}\n",
            )],
            bad: &[(
                "keep.omg",
                "machine keep(limit: u32) -> u32 {\n    let saved = limit;\n    let limit = saved;\n    saved\n}\n",
            )],
            file: "keep.omg",
            occurrence: "limit",
            reason: "local binding limit shadows a visible value binding",
            rule: "Lexical scope",
        },
    );
}

#[test]
fn an_open_integer_spelling_cannot_be_redeclared() {
    check_resolver(
        NameErrorKind::BuiltinTypeRedeclared,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[("types.omg", "data Sample { value: u32; }\nmachine level(value: u32) -> u32 { value }\n")],
            bad: &[("types.omg", "data u13 { value: u32; }\nmachine level(value: u32) -> u32 { value }\n")],
            file: "types.omg",
            occurrence: "u13",
            reason: "builtin type u13 cannot be redeclared",
            rule: "Type and value roles",
        },
    );
}

#[test]
fn a_case_and_an_attached_domain_on_one_owner_need_distinct_names() {
    check_resolver(
        NameErrorKind::DuplicateMember,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[("signal.omg", "data Signal { case Ready; case Idle; }\ndomain Signal::Armed;\n")],
            bad: &[("signal.omg", "data Signal { case Ready; case Idle; }\ndomain Signal::Ready;\n")],
            file: "signal.omg",
            occurrence: "Signal::Ready",
            reason: "duplicate member Signal::Ready",
            rule: "Type and value roles",
        },
    );
}

#[test]
fn a_domain_predicate_pattern_cannot_rebind_self() {
    check_resolver(
        NameErrorKind::PredicateSubjectRebound,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[("small.omg", "domain u8::Small requires self < 10;\n")],
            bad: &[("small.omg", "domain u8::Small requires match self { self -> true };\n")],
            file: "small.omg",
            occurrence: "self",
            reason: "the predicate subject self cannot be rebound",
            rule: "Predicate subject",
        },
    );
}

#[test]
fn a_public_signature_cannot_expose_a_private_type() {
    check_resolver(
        NameErrorKind::PrivateDeclarationInPublicSignature,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[(
                "secret.omg",
                "public data Secret { value: u32; }\npublic machine reveal(s: Secret) -> u32 { s.value }\n",
            )],
            bad: &[(
                "secret.omg",
                "data Secret { value: u32; }\npublic machine reveal(s: Secret) -> u32 { s.value }\n",
            )],
            file: "secret.omg",
            occurrence: "Secret",
            reason: "public signature exposes private declaration Secret",
            rule: "Visibility",
        },
    );
}

#[test]
fn a_data_carrier_cannot_qualify_a_top_level_machine() {
    check_resolver(
        NameErrorKind::IneligibleMachineOwner,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[("counter.omg", "data Counter { value: u32; }\nmachine read(c: Counter) -> u32 { c.value }\n")],
            bad: &[(
                "counter.omg",
                "data Counter { value: u32; }\nmachine Counter::read(c: Counter) -> u32 { c.value }\n",
            )],
            file: "counter.omg",
            occurrence: "Counter::read",
            reason: "a data carrier cannot own a machine",
            rule: "Visibility",
        },
    );
}

#[test]
fn an_ambiguous_machine_owner_identifies_both_domain_imports() {
    let left = ("left/domain.omg", "public domain u8::Ready;");
    let right = ("right/domain.omg", "public domain u16::Ready;");
    // Both imports are legal. Only qualifying the machine makes them compete.
    // Import in reverse identity order so the reported sites must follow their owners.
    check_resolver(
        NameErrorKind::AmbiguousMachineOwner,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[("main.omg", "use right; use left; machine run() {}"), right, left],
            bad: &[("main.omg", "use right; use left; machine Ready::run() {}"), right, left],
            file: "main.omg",
            occurrence: "Ready",
            reason: concat!(
                r"ambiguous machine owner Ready: competing declarations: root\left\u8::Ready, root\right\u16::Ready",
                r" (root\left\u8::Ready on u8, import main.omg@15..19)",
                r" (root\right\u16::Ready on u16, import main.omg@4..9)",
            ),
            rule: "Visibility",
        },
    );
}

const LABEL_USE: &str = "machine keep(x: u8 in Label<2>) -> u8 in Label<2> { x }\n";

#[test]
fn a_domain_head_index_names_its_own_binder() {
    check_resolver(
        NameErrorKind::DomainIndexNotOwnBinder,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[("label.omg", format!("domain<const N: u8> u8::Label<N>;\n{LABEL_USE}").as_str())],
            bad: &[("label.omg", format!("domain<const N: u8> u8::Label<M>;\n{LABEL_USE}").as_str())],
            file: "label.omg",
            occurrence: "M",
            reason: "domain head index must name one of its own binders",
            rule: "Domains",
        },
    );
}

const DRAW_PREFIX: &str = "machine draw(x: u32) -> u32 { x }\nmachine render(flag: bool) -> bool { draw(flag) }\n";

#[test]
fn a_repeated_domain_head_identifies_the_complete_written_carrier() {
    check_resolver(
        NameErrorKind::DuplicateDeclaration,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[("domains.omg", "domain u<8>::Ready; domain u<16>::Ready;")],
            bad: &[("domains.omg", "domain u<8>::Ready; domain u<8>::Ready;")],
            file: "domains.omg",
            occurrence: "u<8>::Ready",
            reason: "domain Ready is declared twice on this carrier",
            rule: "Domains",
        },
    );
}

#[test]
fn renaming_a_parameter_does_not_make_a_distinct_overload() {
    check_resolver(
        NameErrorKind::DuplicateDeclaration,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[("draw.omg", format!("{DRAW_PREFIX}machine draw(x: bool) -> bool {{ x }}\n").as_str())],
            bad: &[("draw.omg", format!("{DRAW_PREFIX}machine draw(value: u32) -> u32 {{ value }}\n").as_str())],
            file: "draw.omg",
            occurrence: "draw",
            reason: "duplicate declaration draw",
            rule: "Overloads and operators",
        },
    );
}

const TRAITS: &str = "trait Reader { machine read(x: u8) -> u8; }\ntrait Writer { machine write(x: u8) -> u8; }\nmachine copy(x: u8) -> u8 { x }\n";

#[test]
fn a_selection_row_names_a_requirement_of_the_implemented_trait() {
    check_resolver(
        NameErrorKind::RequirementOfDifferentTrait,
        Case {
            layer: "L2",
            producer: "resolver",
            good: &[("adapter.omg", format!("{TRAITS}Adapter satisfies Reader {{ Reader::read = copy; }}\n").as_str())],
            bad: &[(
                "adapter.omg",
                format!("{TRAITS}Adapter satisfies Reader {{ Writer::write = copy; }}\n").as_str(),
            )],
            file: "adapter.omg",
            occurrence: "Writer::write",
            reason: "selected requirement belongs to a different trait",
            rule: "Trait requirements",
        },
    );
}
