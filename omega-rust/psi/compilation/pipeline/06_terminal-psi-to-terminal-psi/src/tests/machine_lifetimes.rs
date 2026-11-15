//! Machines that declare lifetime parameters, and calls that pass byte
//! views. Read from source, as a Build program: the toolchain's
//! `dependency::from_source<'storage>` is such a machine, and every
//! `Build::dependency` call goes through it.
//!
//! lifetimes.md: "Lifetime arguments are erased regions, separate from
//! runtime generic arity, layout and monomorphization." A machine with
//! lifetime parameters is therefore one machine, judged once: each parameter
//! is a region the body knows nothing else about, named by the type of every
//! view written with it. A call stands a region of the caller in for each.

use super::build_purpose::judge_build;
use super::*;

/// `accepted` for a program the checker accepts and can publish, `rejected`
/// for one it judges wrong, else the refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected { .. }) => "rejected".to_owned(),
        other => panic!("{other:?}"),
    }
}

/// Compare the actual declaration promise before and after receiving its wire
/// form. Module-local identity indices and source declaration coordinates are
/// deliberately excluded from comparisons between separately read programs.
fn published_label_signature(source: &str) -> (Vec<String>, String, [u8; 32]) {
    // Portable receiving checks start from package-owned Product source.
    // Invocation-scoped code deliberately cannot be serialized as an artifact.
    let typed = super::build_purpose::frontier_of(source, "machine unused_build() {}");
    let entry = super::build_purpose::symbol(&typed, "build_entry");
    let input = typed_trees_to_terminal_psi::prepare_compilation(
        &typed,
        super::build_purpose::roster(
            &[entry],
            &[Some(super::build_purpose::package())],
            source::DependencyScope::Product,
        ),
        terminal_codec::TerminalPublication::Portable,
    );
    let mut verdicts = crate::judge_compilation(input.program);
    assert_eq!(verdicts.len(), 1);
    let verdict = verdicts.remove(0);
    let Ok(RouteVerdict::Accepted(accepted)) = verdict else {
        panic!("the complete declared signature must be judged: {verdict:?}");
    };
    let publication = accepted.publish().unwrap();
    let signature = |module: &terminal_psi::TerminalModule| {
        let mut declarations = module
            .machines
            .iter()
            .filter_map(|machine| machine.declaration.as_ref())
            .filter(|declaration| {
                module
                    .identities
                    .text(declaration.callable)
                    .is_some_and(|identity| identity.contains("label"))
            });
        let declaration = declarations.next().expect("label's published header");
        assert!(declarations.next().is_none());
        (
            declaration
                .parameter_types
                .iter()
                .map(|identity| module.identities.text(*identity).unwrap().to_owned())
                .collect::<Vec<_>>(),
            module
                .identities
                .text(declaration.result_type)
                .unwrap()
                .to_owned(),
            declaration.contract,
        )
    };
    let module = publication.artifact.view().module();
    let original = signature(module);
    let bytes = terminal_codec::encode_module(module).unwrap();
    let received = terminal_codec::decode_module(&bytes).unwrap();
    assert_eq!(
        signature(&received),
        original,
        "receiving retains the promise"
    );
    original
}

/// Identical image-backed bodies can implement different public lifetime
/// promises. A receiver must distinguish those promises after source erasure.
#[test]
fn an_ordinary_published_header_retains_its_declared_result_lifetime_relation() {
    let declared_signature = |binders: &str, input: &str, result: &str| {
        let source = format!(
            "machine build_entry() {{\n\
                 let output: &[u8] = label(\"input\");\n\
                 take(output);\n\
             }}\n\
             machine label{binders}(value: {input}) -> {result} {{\n\
                 \"label\"\n\
             }}\n\
             machine take(view: &[u8]) {{ }}"
        );
        published_label_signature(&source)
    };
    let related = declared_signature("<'input, 'other>", "&'input [u8]", "&'input [u8]");
    let chosen = declared_signature("<'input, 'other>", "&'input [u8]", "&'other [u8]");
    assert_eq!(related.0, chosen.0, "the input promise is unchanged");
    assert_ne!(
        related.1, chosen.1,
        "the result carries its own declared relation"
    );
    assert_ne!(
        related.2, chosen.2,
        "an input-related promise must not become a caller-chosen promise through erasure"
    );
    assert_eq!(
        related,
        declared_signature("<'source, 'independent>", "&'source [u8]", "&'source [u8]"),
        "renaming binders preserves the declared relation"
    );
    assert_eq!(
        chosen,
        declared_signature(
            "<'source, 'independent>",
            "&'source [u8]",
            "&'independent [u8]"
        ),
        "renaming a caller-chosen binder preserves the promise"
    );
    assert_eq!(
        declared_signature("<'input>", "&'input [u8]", "&'input [u8]"),
        declared_signature("", "&[u8]", "&[u8]"),
        "elision publishes the same unique input relation"
    );
}

/// Omitted data lifetime arguments are signature occurrences too. They must
/// retain the same relation and binder order as explicitly written arguments.
#[test]
fn an_ordinary_record_signature_retains_its_elided_lifetime_relation() {
    let signature =
        |binders: &str, formal: &str, result: &str, argument: &str, body: &str, consumer: &str| {
            published_label_signature(&format!(
                r#"
            data View<'storage> {{ bytes: &'storage [u8]; }}
            machine build_entry() {{
                let value: View = View {{ bytes: "input" }};
                take(label({argument}));
            }}
            machine label{binders}(value: {formal}) -> {result} {{ {body} }}
            machine take(value: {consumer}) {{}}
            "#
            ))
        };
    let related = signature(
        "<'input>",
        "View<'input>",
        "View<'input>",
        "value",
        "View { bytes: \"label\" }",
        "View",
    );
    assert_eq!(
        related,
        signature(
            "",
            "View",
            "View",
            "value",
            "View { bytes: \"label\" }",
            "View"
        ),
        "a carried reference input supplies the elided result relation"
    );
    let chosen = signature(
        "<'input, 'output>",
        "View<'input>",
        "View<'output>",
        "value",
        "View { bytes: \"label\" }",
        "View",
    );
    assert_eq!(related.0, chosen.0);
    assert_ne!(related.1, chosen.1);
    assert_ne!(related.2, chosen.2);
    assert_eq!(
        signature(
            "<'outer, 'inner>",
            "&'outer View<'inner>",
            "u32",
            "&value",
            "7u32",
            "u32"
        ),
        signature("", "&View", "u32", "&value", "7u32", "u32"),
        "the outer reference is numbered before its carried reference"
    );
    let no_input = |binders: &str, result: &str| {
        published_label_signature(&format!(
            r#"
            data Pair<'left, 'right> {{ left: &'left [u8]; right: &'right [u8]; }}
            machine build_entry() {{ take(label()); }}
            machine label{binders}() -> {result} {{ Pair {{ left: "a", right: "b" }} }}
            machine take(value: Pair) {{}}
            "#
        ))
    };
    assert_eq!(
        no_input("<'output>", "Pair<'output, 'output>"),
        no_input("", "Pair"),
        "all no-input result elision shares its one caller-chosen lifetime"
    );
}

/// The one statement a program judged wrong breaks.
fn violation(build: &str) -> String {
    let verdict = judge_build(build);
    let Ok(RouteVerdict::Rejected { violations, .. }) = verdict else {
        panic!("expected a rejection: {verdict:?}");
    };
    let [violation] = violations.as_slice() else {
        panic!("expected one violation: {violations:?}");
    };
    violation.statement.clone()
}

/// lifetimes.md: "Duplicate binders and undeclared tags reject."
fn undeclared(tag: &str, machine: &str) -> String {
    format!(
        "this type names the lifetime `'{tag}`, which its declaration does not declare: \
         undeclared lifetime tags reject while reading the signature of `{machine}`"
    )
}

/// "An explicit result lifetime relates its reference leaves to the
/// corresponding input lifetime." The body may return the view it was given
/// for that lifetime and no other: "A signature does not authorize returning
/// unrelated storage".
#[test]
fn a_machine_returns_only_a_view_of_the_region_its_result_names() {
    assert_eq!(
        verdict(
            r#"
machine build_entry(name: &[u8]) { let kept: &[u8] = same(name); }
machine same<'a>(view: &'a [u8]) -> &'a [u8] { view }
"#
        ),
        "accepted"
    );
    assert_eq!(
        verdict(
            r#"
machine build_entry(name: &[u8], second: &[u8]) { let kept: &[u8] = pick(name, second); }
machine pick<'a, 'b>(view: &'a [u8], other: &'b [u8]) -> &'a [u8] { other }
"#
        ),
        "rejected"
    );
}

/// The call gives the lifetime the region of the view it passes, and the
/// result is a view of that region: a view taken from a record's field comes
/// back as a value of that field, and a view of any other region does not.
/// The two formals state that they are at one region; elided, each would be
/// at its own (lifetimes.md: "Several unnamed carried references do not
/// become one source merely because they occupy one parameter"), and
/// `other`'s view would not be a value of `session`'s field.
#[test]
fn a_call_gives_a_lifetime_the_region_of_the_view_it_passes() {
    assert_eq!(
        verdict(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry<'s>(session: &mut Session<'s>, other: &Session<'s>) { session.name = same(other.name); }
machine same<'a>(view: &'a [u8]) -> &'a [u8] { view }
"#
        ),
        "accepted"
    );
    assert_eq!(
        verdict(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &mut Session, other: &Session) { session.name = same(other.name); }
machine same<'a>(view: &'a [u8]) -> &'a [u8] { view }
"#
        ),
        "typing call that gives one lifetime two regions while reading the body of `build_entry`"
    );
    assert_eq!(
        verdict(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &mut Session, name: &[u8]) { session.name = same(name); }
machine same<'a>(view: &'a [u8]) -> &'a [u8] { view }
"#
        ),
        "typing call that gives one lifetime two regions while reading the body of `build_entry`"
    );
}

/// One lifetime stands for one region at a call. There are no "general
/// authored outlives bounds" to relate two, so views of two regions passed
/// for one lifetime are refused; a literal is within every region.
#[test]
fn one_lifetime_is_not_given_two_regions() {
    let both = "machine both<'a>(left: &'a [u8], right: &'a [u8]) -> &'a [u8] { left }";
    assert_eq!(
        verdict(&format!(
            r#"
data Session<'storage> {{ ready: bool; name: &'storage [u8]; }}
machine build_entry<'s>(session: &mut Session<'s>, other: &Session<'s>) {{ session.name = both(other.name, "literal"); }}
{both}
"#
        )),
        "accepted"
    );
    assert_eq!(
        verdict(&format!(
            r#"
data Session<'storage> {{ ready: bool; name: &'storage [u8]; }}
machine build_entry<'s>(session: &mut Session<'s>, other: &Session<'s>, name: &[u8]) {{ session.name = both(other.name, name); }}
{both}
"#
        )),
        "typing call that gives one lifetime two regions while reading the body of `build_entry`"
    );
}

/// "For an elided output, one unambiguous reference input supplies the
/// relation. ... Ambiguous elision requires an explicit relation, not a guess
/// by name or position." A machine with one input view returns a view of it;
/// one with two must say which, or its result is refused wherever it is
/// declared.
#[test]
fn a_result_without_a_lifetime_needs_one_input_lifetime() {
    assert_eq!(
        verdict(
            r#"
machine build_entry(name: &[u8]) { let kept: &[u8] = same(name); }
machine same(view: &[u8]) -> &[u8] { view }
"#
        ),
        "accepted"
    );
    assert_eq!(
        verdict(
            r#"
machine build_entry(name: &[u8], second: &[u8]) { let kept: &[u8] = pick(name, second); }
machine pick(view: &[u8], other: &[u8]) -> &[u8] { other }
"#
        ),
        "typing result without a lifetime of a signature with several input lifetimes while reading \
         the signature of `pick`"
    );
    // Two regions among the views passed matter to nothing else.
    assert_eq!(
        verdict(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &Session, name: &[u8]) { take(session.name, name); }
machine take(view: &[u8], other: &[u8]) { }
"#
        ),
        "accepted"
    );
}

/// lifetimes.md, "Caller-chosen result lifetimes": "A lifetime that a
/// machine binds and no input carries is chosen independently by each
/// caller: the call instantiates it from the result's use". A quoted
/// literal is valid at every choice, so a machine may return one at such a
/// lifetime, written out or left off, and so may a machine that forwards
/// another's. The caller's choice comes from the place the result meets: a
/// local, or the formal it is passed for.
#[test]
fn a_result_lifetime_no_input_carries_is_chosen_by_the_caller() {
    for build in [
        // Named, into a local.
        r#"
machine build_entry() { let kept: &[u8] = made(); }
machine made<'a>() -> &'a [u8] { "made" }
"#,
        // The spec's `package_name`: left off, with no reference input.
        r#"
machine build_entry() { take(package_name()); }
machine package_name() -> &[u8] { "my_library" }
machine take(view: &[u8]) { }
"#,
        // The spec's `kind_name`: the result does not keep `value` borrowed.
        r#"
machine build_entry(value: &[u8]) { take(kind_name(value)); }
machine kind_name<'name>(value: &[u8]) -> &'name [u8] { "record" }
machine take(view: &[u8]) { }
"#,
        // "a view projected from another caller-chosen result" qualifies.
        r#"
machine build_entry() { take(forward()); }
machine forward() -> &[u8] { package_name() }
machine package_name() -> &[u8] { "my_library" }
machine take(view: &[u8]) { }
"#,
    ] {
        assert_eq!(verdict(build), "accepted", "{build}");
    }
}

const LITERAL_MATCH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../tests/fixtures/run/calls/caller_chosen_literal_match/main.omg"
));

fn literal_match_source() -> String {
    LITERAL_MATCH.replace("machine main()", "machine build_entry()")
}

/// A selected literal retains its image backing when it arrives through a
/// value-match join; the result's lifetime does not depend on its Boolean input.
#[test]
fn a_literal_match_keeps_each_incoming_view_for_a_caller_chosen_result() {
    let verdict = judge_build(&literal_match_source());
    let Ok(RouteVerdict::Accepted(accepted)) = verdict else {
        panic!("the literal match must be judged: {verdict:?}");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit
            )
        ),
        "both match arms must retain their original bytes: {status:?}"
    );
}

/// A place declaration alone cannot supply a descriptor value. Its exact
/// unique producer supplies the source contract; missing or duplicate producer
/// rows, and roots that denote only a result or unestablished storage, do not.
#[test]
fn a_literal_arrival_source_retains_its_exact_establishment() {
    let Ok(RouteVerdict::Accepted(accepted)) = judge_build(&literal_match_source()) else {
        panic!("the literal match must be judged");
    };
    let publication = accepted.publish_invocation().unwrap();
    let module = publication.artifact.view().module();
    let machine = module
        .machines
        .iter()
        .find(|machine| {
            machine.structural_places.iter().any(|root| {
                matches!(
                    root.kind,
                    semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral { .. }
                )
            })
        })
        .unwrap();
    let root = machine
        .structural_places
        .iter()
        .find(|root| {
            matches!(
                root.kind,
                semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral { .. }
            )
        })
        .unwrap();
    let structural_type = root.kind.produced_structural_type().unwrap();
    let source = terminal_semantics::structural_argument_source(machine, root.id, 0).unwrap();
    assert_eq!(source.structural_type, structural_type);
    assert_eq!(source.access, terminal_psi::StructuralAccess::Owned);
    assert_eq!(
        source.multiplicity,
        terminal_psi::StructuralMultiplicity::Unrestricted
    );
    let matches = |operation: &&terminal_psi::Operation| matches!(operation.kind, terminal_psi::OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == root.id);
    let establishment = machine.operations().find(matches).unwrap().clone();
    let mut missing = machine.clone();
    for block in &mut missing.blocks {
        block
            .operations
            .retain(|operation| operation.id != establishment.id);
    }
    assert!(terminal_semantics::structural_argument_source(&missing, root.id, 0).is_none());
    let mut duplicate = machine.clone();
    let mut second = establishment;
    second.id = semantic_vocabulary::OperationId::new(
        machine
            .operations()
            .map(|operation| operation.id.get())
            .max()
            .unwrap()
            + 1,
    )
    .unwrap();
    duplicate.blocks[0].operations.push(second);
    assert!(terminal_semantics::structural_argument_source(&duplicate, root.id, 0).is_none());
    for kind in [
        semantic_vocabulary::StructuralPlaceKind::Result,
        semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: 0,
            structural_type,
            construction: None,
        },
    ] {
        let mut other = machine.clone();
        other
            .structural_places
            .iter_mut()
            .find(|candidate| candidate.id == root.id)
            .unwrap()
            .kind = kind;
        assert!(terminal_semantics::structural_argument_source(&other, root.id, 0).is_none());
    }
}

#[test]
fn a_caller_chosen_result_is_not_a_view_of_a_local_even_when_empty() {
    let local_view = |result: &str| {
        format!(
            "machine build_entry() {{ take(leak()); }}\n\
             machine leak<'out>() -> &'out [u8] {{\n\
                 let bytes: [u8; 2] = [98u8, 98u8];\n\
                 let view: &[u8] = &bytes[..];\n\
                 {result}\n\
             }}\n\
             machine take(view: &[u8]) {{ }}"
        )
    };
    let local_match = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/fixtures/fail/calls/caller_chosen_local_match/main.omg"
    ))
    .replace("machine main()", "machine build_entry()");
    for source in [
        local_view("view"),
        local_view("view[0u64..0u64]"),
        local_match,
    ] {
        let verdict = judge_build(&source);
        let Ok(RouteVerdict::Rejected { violations, .. }) = &verdict else {
            panic!("local backing must receive a lifetime rejection: {source}\n{verdict:?}");
        };
        assert!(
            violations.iter().any(|violation| {
                violation
                    .statement
                    .contains("local storage cannot establish")
            }),
            "the rejected obligation must be the local backing lifetime: {violations:?}"
        );
    }
}

/// The twins return a view that is valid only for an input's lifetime: the
/// spec's `leak`, and a view an input record holds. "It cannot assume the
/// choice coincides with an input lifetime", so each machine is rejected,
/// whatever its callers do with the result.
#[test]
fn a_caller_chosen_result_is_not_a_view_of_an_input() {
    for build in [
        r#"
machine build_entry(text: &[u8]) { take(leak(text)); }
machine leak<'name>(text: &[u8]) -> &'name [u8] { text }
machine take(view: &[u8]) { }
"#,
        r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &Session) { take(held(session)); }
machine held<'name>(session: &Session) -> &'name [u8] { session.name }
machine take(view: &[u8]) { }
"#,
    ] {
        assert_eq!(verdict(build), "rejected", "{build}");
    }
}

/// What a signature may write with a lifetime: one its machine declares, on
/// a view or on a reference to one place.
#[test]
fn a_signature_names_only_lifetimes_its_machine_declares() {
    assert_eq!(
        violation(
            r#"
machine build_entry(name: &[u8]) { take(name); }
machine take(view: &'missing [u8]) { }
"#
        ),
        undeclared("missing", "take")
    );
    assert_eq!(
        violation(
            r#"
machine build_entry() { let count: u32 = 1; take(&count); }
machine take(count: &'missing u32) { }
"#
        ),
        undeclared("missing", "take")
    );
    assert_eq!(
        verdict(
            r#"
machine build_entry() { let count: u32 = 1; take(&count); }
machine take<'a>(count: &'a u32) { }
"#
        ),
        "accepted"
    );
}

const DATA_QUALIFIED_MACHINE: &str = r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &mut Session, name: &[u8]) { Session::keep(session, name); }
machine Session::keep(session: &mut Session, view: &'storage [u8]) { session.name = view; }
"#;

#[test]
fn a_data_qualified_machine_is_rejected_at_its_declaration() {
    let verdict = judge_build(DATA_QUALIFIED_MACHINE);
    let Ok(RouteVerdict::Rejected { violations, .. }) = verdict else {
        panic!("expected a data attachment violation: {verdict:?}");
    };
    let [violation] = violations.as_slice() else {
        panic!("expected one data attachment violation: {violations:?}");
    };
    assert_eq!(
        violation.statement,
        "a data declaration cannot own a machine; declare an ordinary static machine"
    );
    let site = violation.site.expect("the machine declaration is authored");
    assert_eq!(
        &DATA_QUALIFIED_MACHINE[site.span.start..site.span.end],
        "Session::keep"
    );
}

#[test]
fn a_static_machine_must_declare_the_lifetime_its_signature_uses() {
    // Removing the retired attachment lets the signature reader reach
    // the view whose lifetime is declared only on the data type.
    let ordinary = DATA_QUALIFIED_MACHINE.replace("Session::keep", "keep");
    assert_eq!(violation(&ordinary), undeclared("storage", "keep"));
    let bound = ordinary
        .replace("machine build_entry(", "machine build_entry<'storage>(")
        .replace("machine keep(", "machine keep<'storage>(")
        .replace("&mut Session,", "&mut Session<'storage>,")
        .replace("name: &[u8]", "name: &'storage [u8]");
    assert_eq!(verdict(&bound), "accepted");
}

/// The toolchain's `dependency::from_source` and the call that consumes its
/// result: data applied at the machine's lifetime is the declaration's one
/// record, a literal fills a stored view, and a stored view is passed to a
/// view formal beside its sibling fields.
#[test]
fn a_description_is_made_from_a_source_and_taken_apart_for_a_request() {
    let program = |request: &str| {
        format!(
            r#"
data Source<'storage> {{ location: &'storage [u8]; }}
data Description<'storage> {{ source: Source<'storage>; explicit_alias: bool; alias: &'storage [u8]; }}
machine build_entry() {{ register(from_source(Source {{ location: "std" }})); }}
machine from_source<'storage>(source: Source<'storage>) -> Description<'storage> {{
    Description {{ source: source, explicit_alias: false, alias: "" }}
}}
machine register(description: Description) {{ {request} }}
machine request(explicit_alias: bool, alias: &[u8], source: Source) {{
    path(explicit_alias, alias, source.location);
}}
machine path(explicit_alias: bool, alias: &[u8], location: &[u8]) {{ }}
machine whole(alias: &[u8], description: Description) {{ }}
machine whole_source(source: &Source, description: Description) {{ }}
"#
        )
    };
    assert_eq!(
        verdict(&program(
            "request(description.explicit_alias, description.alias, description.source);"
        )),
        "accepted"
    );
    // The first argument captures the shared descriptor before the second
    // transfers its carrier. Its backing remains the original literal;
    // moving the carrier does not consume those bytes.
    assert_eq!(
        verdict(&program("whole(description.alias, description);")),
        "accepted"
    );
    // A reference to an inline record still refers to its carrier's
    // storage. Snapshotting shared views must not remove the source guard
    // against transferring that same storage in another argument.
    assert_eq!(
        verdict(&program("whole_source(&description.source, description);")),
        "call argument that names storage another argument lends while reading the body of \
         `register`"
    );
}

/// A machine's lifetime applied to data makes the data's stored views views
/// of that lifetime, so `dependency::with_alias` stores a view of its own
/// lifetime into a description at it. Data applied at another lifetime does
/// not take it.
#[test]
fn a_view_of_a_machine_lifetime_is_stored_in_data_applied_at_it() {
    assert_eq!(
        verdict(
            r#"
data Description<'storage> { explicit_alias: bool; alias: &'storage [u8]; }
machine build_entry() { }
machine with_alias<'storage>(description: &mut Description<'storage>, alias: &'storage [u8]) {
    description.explicit_alias = true;
    description.alias = alias;
}
"#
        ),
        "accepted"
    );
    assert_eq!(
        verdict(
            r#"
data Description<'storage> { explicit_alias: bool; alias: &'storage [u8]; }
machine build_entry() { }
machine with_alias<'storage, 'other>(description: &mut Description<'storage>, alias: &'other [u8]) {
    description.alias = alias;
}
"#
        ),
        "typing assigned value type while reading the body of `with_alias`"
    );
}

/// A case written with its sum, `Shape::Dot { at }`, heads a pattern.
#[test]
fn a_pattern_is_headed_by_a_case_written_with_its_sum() {
    assert_eq!(
        verdict(
            r#"
data Shape [copy] { case Dot(at: u32); case Pair(left: u32, right: u32); }
machine build_entry() { let total: u32 = weigh(Shape::Dot { at: 1 }); }
machine weigh(shape: Shape) -> u32 {
    transition shape {
        Shape::Dot { at } -> at
        Shape::Pair { left, right } -> left
    }
}
"#
        ),
        "accepted"
    );
}

/// Elision is judged at the declaration that owns it. lifetimes.md:
/// "Ambiguous elision requires an explicit relation, not a guess by name or
/// position." Each elided input is its own lifetime of `pick`, so `pick`
/// alone, never called, is refused whichever input it returns; the
/// diagnostic does not wait for a caller that passes two regions.
#[test]
fn an_ambiguous_elided_result_is_refused_without_a_caller() {
    for returned in ["view", "other"] {
        assert_eq!(
            verdict(&format!(
                "machine build_entry() {{ }}\n\
                 machine pick(view: &[u8], other: &[u8]) -> &[u8] {{ {returned} }}\n"
            )),
            "typing result without a lifetime of a signature with several input lifetimes while \
             reading the signature of `pick`",
            "{returned}"
        );
    }
}
