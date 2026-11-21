#[path = "support/source.rs"]
mod sources;

use omega_preparation::prepare_source_package;
use omega_psi::{
    BlockTerminator, CleanupAction, DomainAtom, LoweredPackage, MachineTemplate, MachineTemplateBinder, Parameter, ParameterType, PredicateOperator, PredicateStep,
    ScalarType, TemplateCall, TemplateCallTarget, TemplateOperand, TypeExpression, TypeExpressionStep, TypeShape,
};
use omega_typer::ValueType;

#[test]
fn published_audible_preserves_the_domain_and_supplied_member_call() {
    let example = spec_corpus::published_chapter("language/modules.md")
        .unwrap()
        .into_iter()
        .find(|e| e.fence_line_number == 227)
        .expect("published Audio::Audible example at language/modules.md:227");
    let original = spec_corpus::parser_examples::parser_source(&example.source_text);
    // Same scalar field types and trait-member signatures. Only the selections differ.
    let twin = original.replace("volume: u8;", "volume: u8; level: u8;")
        .replace("(track: Audio)", "(shadow: Audio, track: Audio)")
        .replace("machine play(volume: u8);", "machine play(volume: u8); machine stop(volume: u8);")
        .replace("const Audio: Player", "const quiet: Player, const Audio: Player")
        .replace("Audio::play(track.volume)", "Audio::stop(track.level)");
    let swapped = twin.replace("self.volume", "self.level").replace("track.level", "shadow.volume");
    for (text, binder, member, predicate_field, subject, formal, actual) in [
        (&original, 0, "Player::play", "Audio::volume", 0, 0, "Audio::volume"),
        (&twin, 1, "Player::stop", "Audio::volume", 1, 1, "Audio::level"),
        (&swapped, 1, "Player::stop", "Audio::level", 1, 0, "Audio::volume"),
    ] {
        let files = [("published.omg", text.as_str())];
        let package = {
            let resolved = sources::resolve(&files);
            let prepared = prepare_source_package(&resolved).unwrap();
            let typed = prepared.typed();
            let test = typed.memberships.iter().find(|m| m.site.package == "app").unwrap();
            assert_eq!(test.domain.declaration.path, "Audio::Audible");
            assert_eq!(typed.domain_carriers.get(&test.domain.declaration), Some(&test.subject));
            assert!(matches!(&test.subject, ValueType::Nominal(id) if id.package == "app" && id.path == "Audio"));
            let call = typed.calls.iter().find(|c| c.site.package == "app").unwrap();
            assert_eq!(call.machine.path, member);
            assert!(matches!(&call.evidence, Some(ValueType::Parameter {owner, index}) if Some(owner.as_ref()) == call.caller.as_ref() && *index == binder));
            sources::lower_prepared(&prepared, &files).unwrap()
        };
        let [record] = package.data.as_slice() else { panic!("expected Audio") };
        let [domain] = package.domains.as_slice() else { panic!("expected Audible") };
        assert_eq!(record.declaration.path, "Audio");
        assert_eq!(domain.declaration.path, "Audio::Audible");
        let field = |path: &str| record.fields.iter().find(|f| f.declaration.path == path).unwrap().declaration.clone();
        let [clause] = domain.requires.as_slice() else { panic!("expected one authored predicate") };
        spec_corpus::observe_handoff!(psi "psi-output/published-audible-predicate", assert_eq!(
            (package.types.get(domain.carrier), clause.predicate.as_slice()),
            (Some(&TypeShape::Nominal(record.id)), [PredicateStep::SelfValue, PredicateStep::Field(field(predicate_field)),
                PredicateStep::Integer(0.into()), PredicateStep::Apply(PredicateOperator::GreaterThan)].as_slice())
        ));
        let [preview] = package.machine_templates.as_slice() else { panic!("expected an open machine, not a chosen implementation") };
        assert!(package.machines.is_empty() && package.conformances.is_empty());
        assert_eq!(preview.declaration.path, "preview");
        assert_eq!(preview.signature.parameters[subject].0, [TypeExpressionStep::Named { declaration: record.declaration.clone(), argument_count: 0 }]);
        spec_corpus::observe_handoff!(psi "psi-output/published-audible-precondition", assert_eq!(preview.signature.requires[0].predicate,
            [PredicateStep::Formal(subject), PredicateStep::InDomain(DomainAtom { declaration: domain.declaration.clone(), arguments: vec![] })]));
        let [player] = package.traits.as_slice() else { panic!("expected the supplied operation record bound") };
        assert_eq!(player.declaration.path, "Player");
        let MachineTemplateBinder::Implementation(slot) = &preview.parameters[binder] else { panic!("expected the implementation binder") };
        assert_eq!(slot.declaration, player.declaration);
        assert!(slot.arguments.is_empty());
        let requirement = player
            .requirements
            .iter()
            .find(|r| r.declaration.path == member)
            .unwrap();
        let [omega_psi::TemplateStep::Call(call)] = preview.body.as_slice() else {
            panic!("expected the selected member invocation")
        };
        spec_corpus::observe_handoff!(psi "psi-output/published-audible-call", assert_eq!((&call.target, call.operands.as_slice()),
            (&TemplateCallTarget::Member { binder, requirement: requirement.declaration.clone() }, [TemplateOperand::RecordField { formal, carrier: vec![], field: field(actual), argument: Parameter::Scalar(byte()) }].as_slice())));
    }
}

#[test]
fn a_scalar_audible_domain_lowers_with_its_carrier_and_predicate() {
    let package = sources::lower(&[("scalar.omg", "domain u8::Audible requires self > 0;")]).unwrap();
    let [domain] = package.domains.as_slice() else {
        panic!("expected one scalar domain");
    };
    let [clause] = domain.requires.as_slice() else {
        panic!("expected its requirement");
    };
    assert_eq!(domain.declaration.path, "u8::Audible");
    let byte = TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 8 });
    assert_eq!(package.types.get(domain.carrier), Some(&byte));
    spec_corpus::observe_handoff!(
        psi "psi-output/scalar-audible-predicate",
        assert_eq!(
            clause.predicate,
            [PredicateStep::SelfValue, PredicateStep::Integer(0.into()), PredicateStep::Apply(PredicateOperator::GreaterThan)]
        ),
    );
}

/// The published fence's body calls `Audio::play` through the implementation
/// binder. Typing records that binder as the call's evidence, and the call's
/// enclosing trait application as `Player`. L5 reads both to name the member.
#[test]
fn the_published_member_call_names_its_implementation_binder_as_evidence() {
    let example = spec_corpus::published_chapter("language/modules.md")
        .unwrap()
        .into_iter()
        .find(|e| e.fence_line_number == 227)
        .expect("published Audio::Audible example at language/modules.md:227");
    let text = spec_corpus::parser_examples::parser_source(&example.source_text);
    let package = sources::resolve(&[("published.omg", text.as_str())]);
    let prepared = prepare_source_package(&package).expect("the published example type-checks and prepares");
    let typed = prepared.typed();
    let [call] = typed.calls.as_slice() else {
        panic!("expected the one call in the fence")
    };
    assert_eq!(call.machine.path, "Player::play");
    assert_eq!(call.caller.as_ref().map(|c| c.path.as_str()), Some("preview"));
    assert!(matches!(&call.evidence, Some(ValueType::Parameter { owner, index: 0 }) if owner.path == "preview"));
    let [(record, arguments)] = call.enclosing_arguments.as_slice() else {
        panic!("expected the enclosing trait application")
    };
    assert!(record.path == "Player" && arguments.is_empty());
    let (_, binders) = typed
        .generic_parameters
        .iter()
        .find(|(id, _)| id.path == "preview")
        .unwrap();
    assert!(matches!(binders.parameters[0].carrier.as_deref(), Some(ValueType::Evidence(slot)) if slot.declaration.path == "Player"));
}

/// The open template publishes each implementation binder with its trait and names
/// the binder a call goes through. The two binders share a trait, so exchanging
/// them changes only the binder index on the call.
#[test]
fn an_open_template_calls_a_requirement_through_its_implementation_binder() {
    let source =
        "trait Player { machine play(volume: u8); }\nmachine preview<const Quiet: Player, const Loud: Player>(volume: u8)\n{\n    Loud::play(volume);\n}\n";
    for (text, binder) in [
        (source.to_owned(), 1),
        (source.replace("Loud::play", "Quiet::play"), 0),
    ] {
        let package = sources::lower(&[("template.omg", &text)]).unwrap();
        let [template] = package.machine_templates.as_slice() else {
            panic!("expected the open template")
        };
        assert_eq!(template.declaration.path, "preview");
        for parameter in &template.parameters {
            let MachineTemplateBinder::Implementation(slot) = parameter else {
                panic!("expected an implementation binder, found {parameter:?}")
            };
            assert_eq!((slot.declaration.path.as_str(), slot.arguments.len()), ("Player", 0));
        }
        let [omega_psi::TemplateStep::Call(call)] = template.body.as_slice() else {
            panic!("expected the one requirement call")
        };
        spec_corpus::observe_handoff!(
            psi "psi-output/implementation-binder-member-call",
            assert!(matches!(&call.target, TemplateCallTarget::Member { binder: b, requirement } if *b == binder && requirement.path == "Player::play")),
        );
        assert_eq!(call.operands, [TemplateOperand::Formal(0)]);
        assert!(template.invokes.is_empty() && template.result.is_none());
    }
}

/// A call through a binder of a different trait still selects that binder's
/// own requirement.
#[test]
fn a_requirement_call_names_its_trait() {
    let source = "trait Player { machine play(volume: u8); }\ntrait Mixer { machine mix(volume: u8); }\nmachine preview<const P: Player, const M: Mixer>(volume: u8)\n{\n    M::mix(volume);\n    P::play(volume);\n}\n";
    let package = sources::lower(&[("template.omg", source)]).unwrap();
    let targets: Vec<_> = package.machine_templates[0]
        .body
        .iter()
        .map(|step| match &step.call().expect("a call step").target {
            TemplateCallTarget::Member { binder, requirement } => (*binder, requirement.path.as_str()),
            other => panic!("unexpected target {other:?}"),
        })
        .collect();
    assert_eq!(targets, [(1, "Mixer::mix"), (0, "Player::play")]);
}

/// `const N: u8` binds a static value that the caller's application supplies.
/// The open template publishes that binder with its declared carrier and
/// returns the formal it reads, so a specializer can substitute any `u8` for
/// `N` without changing the result. Reading the binder itself returns that
/// value. A record carrier has no template binder form yet and still refuses.
#[test]
fn a_value_only_template_publishes_its_binder_and_its_formal_result() {
    use omega_psi::{TemplateResult, TypeExpression};
    use psi_handoff_consumer::machine_templates::{Application, Evaluation, evaluate};
    let byte = TypeExpression(vec![TypeExpressionStep::Scalar(ScalarType::Integer {
        signed: false,
        width: 8,
    })]);
    // The exact bytes of the readiness fixture `handoff/open template whose only static binder is a value`.
    let value = "machine keep<const N: u8>(volume: u8) -> u8 { volume }";
    let package = sources::lower(&[("value.omg", value)]).unwrap();
    assert!(package.machines.is_empty());
    let [keep] = package.machine_templates.as_slice() else {
        panic!("expected the open keep")
    };
    spec_corpus::observe_handoff!(
        psi "psi-output/value-binder-template",
        assert_eq!(
            (keep.parameters.as_slice(), keep.body.is_empty(), &keep.result, keep.signature.parameters.as_slice()),
            ([MachineTemplateBinder::Const(byte.clone())].as_slice(), true, &Some(TemplateResult::Operand(TemplateOperand::Formal(0))), [byte.clone()].as_slice())
        ),
    );
    assert!(keep.invokes.is_empty() && !keep.public);
    let none = |_: usize, _: &[u64]| unreachable!("keep calls nothing");
    // A specializer applies each template with the formal 7 and N = 0, then N = 255.
    let apply = |template, statics: &[u64]| {
        evaluate(
            template,
            &Application {
                formals: &[7],
                statics,
                select: &none,
            },
        )
    };
    let formal_result = [apply(keep, &[0]), apply(keep, &[255])];

    let binder = value.replace("{ volume }", "{ N }");
    let package = sources::lower(&[("value.omg", &binder)]).unwrap();
    let [read] = package.machine_templates.as_slice() else {
        panic!("expected the open keep")
    };
    let Some(TemplateResult::StaticParameter { owner, index, site }) = &read.result else {
        panic!("expected the binder read, got {:?}", read.result)
    };
    assert_eq!((owner, *index, site.span.slice(binder.as_bytes())), (&read.declaration, 0, Some(&b"N"[..])));
    let binder_result = [apply(read, &[0]), apply(read, &[255])];
    let returned = |a, b| [Some(Evaluation::Returned(a)), Some(Evaluation::Returned(b))];
    spec_corpus::observe_handoff!(
        psi "psi-output/value-binder-template-consumer",
        assert_eq!((formal_result, binder_result), (returned(7, 7), returned(0, 255))),
    );

    let record = format!("data P{{x:u8;}} {}", value.replace("const N: u8", "const N: P"));
    let message = "open generic machine requires a closed application before lowering";
    let error = sources::refuse(
        &[("value.omg", &record)],
        "value.omg",
        sources::unique_span(&record, "machine keep<const N: P>(volume: u8) -> u8 { volume }"),
        message,
    );
    assert_eq!(error.message, message);
}

/// `requires volume in u8::Audible` states a fact about the formal. Psi names
/// the tested domain after its subject; testing a different domain changes only
/// that declaration, and a case test on the same shape stays a case test.
#[test]
fn a_domain_test_in_a_precondition_names_its_subject_and_domain() {
    let source = "domain u8::Audible requires self > 0;\ndomain u8::Silent requires self == 0;\nmachine preview(volume: u8, other: u8)\nrequires other in u8::Audible\n{\n}\n";
    for (text, formal, domain) in [
        (source.to_owned(), 1, "u8::Audible"),
        (source.replace("other in u8::Audible", "volume in u8::Silent"), 0, "u8::Silent"),
    ] {
        let package = sources::lower(&[("contract.omg", &text)]).unwrap();
        let [machine] = package.machines.as_slice() else {
            panic!("expected the one machine")
        };
        let [clause] = machine.contract.requires.as_slice() else {
            panic!("expected the precondition")
        };
        spec_corpus::observe_handoff!(
            psi "psi-output/precondition-domain-test",
            assert!(matches!(
                clause.predicate.as_slice(),
                [PredicateStep::Formal(f), PredicateStep::InDomain(DomainAtom { declaration, arguments })]
                    if *f == formal && declaration.path == domain && arguments.is_empty()
            )),
        );
    }
    let case = "data Level { case Quiet; case Loud; }\nmachine preview(level: Level)\nrequires level in Level::Loud\n{\n}\n";
    let package = sources::lower(&[("case.omg", case)]).unwrap();
    let predicate = &package.machines[0].contract.requires[0].predicate;
    assert!(matches!(predicate.as_slice(), [PredicateStep::Formal(0), PredicateStep::IsCase(case)] if case.path == "Level::Loud"));
}

/// The published fence on a scalar carrier, with the formal passed directly.
/// Both its precondition and its member call reach Psi.
#[test]
fn the_published_shape_on_a_scalar_carrier_lowers_its_precondition_and_member_call() {
    let text = "domain u8::Audible requires self > 0;\ntrait Player { machine play(volume: u8); }\n\nmachine preview<const Audio: Player>(volume: u8)\nrequires volume in u8::Audible\n{\n    Audio::play(volume);\n}\n";
    let package = sources::lower(&[("scalar.omg", text)]).unwrap();
    let [template] = package.machine_templates.as_slice() else {
        panic!("expected the open template")
    };
    assert!(matches!(template.parameters.as_slice(), [MachineTemplateBinder::Implementation(slot)] if slot.declaration.path == "Player"));
    let [clause] = template.signature.requires.as_slice() else {
        panic!("expected the precondition")
    };
    assert!(matches!(clause.predicate.as_slice(), [PredicateStep::Formal(0), PredicateStep::InDomain(atom)] if atom.declaration.path == "u8::Audible"));
    assert!(matches!(&template.body[0].call().unwrap().target, TemplateCallTarget::Member { binder: 0, requirement } if requirement.path == "Player::play"));
    assert_eq!(template.body[0].call().unwrap().operands, [TemplateOperand::Formal(0)]);
}

fn byte() -> TypeExpression {
    TypeExpression(vec![TypeExpressionStep::Scalar(ScalarType::Integer { signed: false, width: 8 })])
}

/// The open template and its first call, which goes through the implementation binder to `Player::play`.
fn play_call(package: &LoweredPackage) -> (&MachineTemplate, &TemplateCall) {
    let [template] = package.machine_templates.as_slice() else { panic!("expected the open template") };
    let omega_psi::TemplateStep::Call(call) = &template.body[0] else { panic!("expected the requirement call first") };
    assert!(matches!(&call.target, TemplateCallTarget::Member { binder: 0, requirement } if requirement.path == "Player::play"));
    (template, call)
}

/// Consumer side, through public Psi only: follow the operand from its formal to
/// the published field and return that value's lane and type, alongside the
/// lane and type `Player::play` declares for its formal.
fn consumed(package: &LoweredPackage) -> (Result<ParameterType, &'static str>, ParameterType) {
    let (template, call) = play_call(package);
    let [operand] = call.operands.as_slice() else { panic!("expected one operand") };
    let [player] = package.traits.as_slice() else { panic!("expected Player") };
    let requirement = player.requirements.iter().find(|r| r.declaration.path == "Player::play").unwrap();
    let [formal] = requirement.parameter_types.as_slice() else { panic!("expected one requirement formal") };
    (psi_handoff_consumer::template_field_argument(package, template, operand), *formal)
}

fn field_id(package: &LoweredPackage, path: &str) -> omega_psi::DeclarationId {
    package.data.iter().flat_map(|d| &d.fields).find(|f| f.declaration.path == path).unwrap_or_else(|| panic!("missing {path}")).declaration.clone()
}

const MESSAGE: &str = "template call has unsupported operands, selection or obligations";

/// Without its record domain, the published fence still passes `track.volume`,
/// a scalar field of a formal record, and the template call publishes a copy of
/// that field as its operand: formal zero, no carrier path, `Audio::volume`, and
/// the `u8` scalar lane. A field read names only the selected field, so adding an
/// unused fixed-array or record sibling to `Audio`, before or after `volume`,
/// publishes the same operand. Selecting the scalar sibling `level` names that
/// field instead. A consumer of the Psi resolves each operand through the
/// formal's record to the field's `u8` carrier, the requirement's formal type.
#[test]
fn a_projected_operand_copies_its_selected_scalar_field_beside_unused_siblings() {
    let text = "data Audio { volume: u8; }
trait Player { machine play(volume: u8); }

machine preview<const Audio: Player>(track: Audio)
{
    Audio::play(track.volume);
}
";
    let package = sources::lower(&[("record.omg", text)]).unwrap();
    let [record] = package.data.as_slice() else { panic!("expected Audio") };
    let [volume] = record.fields.as_slice() else { panic!("expected Audio::volume") };
    assert_eq!(volume.declaration.path, "Audio::volume");
    let (template, call) = play_call(&package);
    assert_eq!(template.body.len(), 1);
    spec_corpus::observe_handoff!(
        psi "psi-output/projected-template-operand",
        assert_eq!(call.operands, [TemplateOperand::RecordField { formal: 0, carrier: vec![], field: volume.declaration.clone(), argument: Parameter::Scalar(byte()) }]),
    );

    // The exact input of the formerly refused case, then layout twins that keep the selection.
    let wide = text.replace("volume: u8; }", "volume: u8; name: [u8; 2]; }");
    let byte_shape = TypeShape::PrimitiveScalar(ScalarType::Integer { signed: false, width: 8 });
    for (file, source, selected) in [
        ("record.omg", wide.clone(), "Audio::volume"),
        ("swapped.omg", text.replace("volume: u8; }", "name: [u8; 2]; volume: u8; }"), "Audio::volume"),
        ("extent.omg", text.replace("volume: u8; }", "volume: u8; name: [u8; 3]; }"), "Audio::volume"),
        ("nested.omg", text.replace("data Audio { volume: u8; }", "data Tag { id: u16; }\ndata Audio { volume: u8; tag: Tag; }"), "Audio::volume"),
        ("level.omg", wide.replace("volume: u8; name", "volume: u8; level: u8; name").replace("track.volume", "track.level"), "Audio::level"),
    ] {
        let package = sources::lower(&[(file, &source)]).unwrap();
        let record = package.data.iter().find(|d| d.declaration.path == "Audio").expect("Audio reaches Psi");
        let field = record.fields.iter().find(|f| f.declaration.path == selected).expect("the selected field reaches Psi");
        // A layout twin must publish the original program's operand; the `level` control must not.
        let expected = if selected == "Audio::volume" { volume.declaration.clone() } else { field.declaration.clone() };
        assert_eq!((field.declaration == expected, expected == volume.declaration), (true, selected == "Audio::volume"));
        let (template, call) = play_call(&package);
        assert_eq!(template.body.len(), 1);
        spec_corpus::observe_handoff!(
            psi "psi-output/mixed-record-template-operand",
            assert_eq!(call.operands, [TemplateOperand::RecordField { formal: 0, carrier: vec![], field: expected.clone(), argument: Parameter::Scalar(byte()) }]),
        );
        let (read, formal) = consumed(&package);
        spec_corpus::observe_handoff!(
            psi "psi-output/mixed-record-operand-carrier",
            assert_eq!((read, package.types.get(formal.carrier())), (Ok(formal), Some(&byte_shape))),
        );
    }
}

/// `Audio::play(track.tag.id)` copies the scalar `Tag::id` found through the
/// record field `Audio::tag`. The operand keeps that whole path: the carrier
/// `[Audio::tag]` and the final field `Tag::id`. Reading the same field through a
/// second `Tag` sibling changes only the carrier step, and one more record level
/// adds one step. A Psi consumer follows each path through the published records
/// to the `u8` that `Player::play` takes.
#[test]
fn a_nested_template_operand_names_the_path_from_its_formal_to_the_scalar_field() {
    let source = "data Tag { id: u8; }
data Audio { volume: u8; tag: Tag; }
trait Player { machine play(volume: u8); }

machine preview<const Audio: Player>(track: Audio)
{
    Audio::play(track.tag.id);
}
";
    let sibling = source.replace("tag: Tag; }", "tag: Tag; other: Tag; }").replace("track.tag.id", "track.other.id");
    let deeper = source.replace("data Tag { id: u8; }", "data Inner { id: u8; }\ndata Tag { inner: Inner; }").replace("track.tag.id", "track.tag.inner.id");
    for (file, text, carrier, field) in [
        ("nested_path.omg", source.to_owned(), &["Audio::tag"][..], "Tag::id"),
        ("sibling_path.omg", sibling, &["Audio::other"][..], "Tag::id"),
        ("deeper_path.omg", deeper, &["Audio::tag", "Tag::inner"][..], "Inner::id"),
    ] {
        let package = sources::lower(&[(file, &text)]).unwrap();
        let (_, call) = play_call(&package);
        let carrier: Vec<_> = carrier.iter().map(|path| field_id(&package, path)).collect();
        spec_corpus::observe_handoff!(
            psi "psi-output/nested-template-operand-path",
            assert_eq!(call.operands, [TemplateOperand::RecordField { formal: 0, carrier, field: field_id(&package, field), argument: Parameter::Scalar(byte()) }]),
        );
        let (read, formal) = consumed(&package);
        spec_corpus::observe_handoff!(
            psi "psi-output/nested-template-operand-carrier",
            assert_eq!(read, Ok(formal)),
        );
    }
}

/// `Audio::play(track.name)` passes the `[u8; 2]` field to a `[u8; 2]` formal.
/// The template publishes a copy of that field in the structural lane with its
/// exact array type, the lane and type a consumer finds both on the published
/// field and on `Player::play`. A `[copy]` record field is copied the same way.
/// Without `[copy]` the record is affine: passing it would move it out of the
/// formal and leave a residual, which a template operand cannot state, so that
/// twin still refuses at the call.
#[test]
fn a_structural_field_operand_is_a_copy_and_an_affine_field_refuses_at_the_call() {
    let array = "data Audio { volume: u8; name: [u8; 2]; }
trait Player { machine play(name: [u8; 2]); }

machine preview<const Audio: Player>(track: Audio)
{
    Audio::play(track.name);
}
";
    let affine = "data Tag { id: u8; }
data Audio { volume: u8; tag: Tag; }
trait Player { machine play(tag: Tag); }

machine preview<const Audio: Player>(track: Audio)
{
    Audio::play(track.tag);
}
";
    let copy = affine.replace("data Tag {", "data Tag [copy] {");
    for (file, text, field) in [("array.omg", array.to_owned(), "Audio::name"), ("copy.omg", copy, "Audio::tag")] {
        let package = sources::lower(&[(file, &text)]).unwrap();
        let argument = match package.data.iter().find(|d| d.declaration.path == "Tag") {
            Some(tag) => vec![TypeExpressionStep::Named { declaration: tag.declaration.clone(), argument_count: 0 }],
            None => vec![TypeExpressionStep::Scalar(ScalarType::Integer { signed: false, width: 8 }), TypeExpressionStep::Array(2)],
        };
        let (_, call) = play_call(&package);
        spec_corpus::observe_handoff!(
            psi "psi-output/structural-template-operand",
            assert_eq!(call.operands, [TemplateOperand::RecordField { formal: 0, carrier: vec![], field: field_id(&package, field), argument: Parameter::Structural(TypeExpression(argument)) }]),
        );
        let (read, formal) = consumed(&package);
        spec_corpus::observe_handoff!(
            psi "psi-output/structural-template-operand-carrier",
            assert_eq!((read, matches!(formal, Parameter::Structural(_))), (Ok(formal), true)),
        );
        // The consumer refuses an operand whose published type or lane disagrees with the field it names.
        let [TemplateOperand::RecordField { formal, carrier, field, argument }] = call.operands.as_slice() else { unreachable!() };
        let [template] = package.machine_templates.as_slice() else { unreachable!() };
        let wider = TypeExpression(vec![TypeExpressionStep::Scalar(ScalarType::Integer { signed: false, width: 8 }), TypeExpressionStep::Array(3)]);
        let Parameter::Structural(exact) = argument else { unreachable!() };
        for (forged, error) in [(Parameter::Structural(wider), "published type disagrees with the field's type"), (Parameter::Scalar(exact.clone()), "published lane disagrees with the field's type")] {
            let operand = TemplateOperand::RecordField { formal: *formal, carrier: carrier.clone(), field: field.clone(), argument: forged };
            assert_eq!(psi_handoff_consumer::template_field_argument(&package, template, &operand), Err(error));
        }
    }
    let error = sources::refuse(&[("transfer.omg", affine)], "transfer.omg", sources::unique_span(affine, "Audio::play(track.tag)"), MESSAGE);
    assert_eq!(error.message, MESSAGE);
}

fn instance(package: &LoweredPackage) -> &omega_psi::LoweredMachine {
    package.machines.iter().find(|m| m.declaration.path == "preview").expect("the closed application lowers preview")
}

/// The cleanup each normal return of the closed `preview` performs.
fn return_cleanup(package: &LoweredPackage) -> Vec<&[CleanupAction]> {
    instance(package)
        .body
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            BlockTerminator::Return { cleanup, .. } => Some(cleanup.as_slice()),
            _ => None,
        })
        .collect()
}

/// `track.volume` is read beside a sibling `h: Handle` that has a cleanup
/// machine or is `[linear]`. The read is a copy of an unrestricted `u8`, so the
/// formal stays whole and every obligation of `track` is unchanged: the template
/// publishes the same operand as for the scalar-only record, and the same holds
/// when the receiver `Audio` itself has cleanup or is linear.
///
/// Lawful controls show where the siblings are really destroyed. When a caller
/// closes the application, the `preview` instance releases `track.h` at its
/// return, and it does so identically whether or not it read `track.volume`.
/// With a linear `h`, a lawful template must consume `track`: `finish(track)`
/// follows the read in the open template as a whole-formal operand, and in the
/// closed instance it moves the whole formal, again identically with or without
/// the read.
#[test]
fn a_field_read_beside_an_obligated_sibling_leaves_the_sibling_to_the_formal() {
    let text = "data Audio { volume: u8; }
trait Player { machine play(volume: u8); }

machine preview<const Audio: Player>(track: Audio)
{
    Audio::play(track.volume);
}
";
    let cleanup = "data Handle dropped by release{mark:u8;}
machine release(h:Handle){let Handle{mark as _}=h;}
";
    let linear = "data Handle [linear] { mark: u8; }
";
    let with_handle = text.replace("volume: u8; }", "volume: u8; h: Handle; }");
    let mut programs: Vec<_> = [cleanup, linear].iter().map(|prefix| ("obligated.omg", format!("{prefix}{with_handle}"))).collect();
    programs.push((
        "receiver.omg",
        format!("data Audio dropped by release {{ volume: u8; }}\nmachine release(a: Audio) {{ let Audio {{ volume as _ }} = a; }}\n{}", text.replace("data Audio { volume: u8; }\n", "")),
    ));
    programs.push(("receiver.omg", text.replace("data Audio { volume: u8; }", "data Audio [linear] { volume: u8; }")));
    for (file, source) in &programs {
        let package = sources::lower(&[(file, source)]).unwrap();
        let (template, call) = play_call(&package);
        assert_eq!(template.body.len(), 1);
        spec_corpus::observe_handoff!(
            psi "psi-output/obligated-sibling-template-operand",
            assert_eq!(call.operands, [TemplateOperand::RecordField { formal: 0, carrier: vec![], field: field_id(&package, "Audio::volume"), argument: Parameter::Scalar(byte()) }]),
        );
        assert!(package.machines.iter().all(|m| m.declaration.path != "preview"), "an open template has no instance body");
    }

    let close = "machine say(volume: u8) {}
Speaker satisfies Player { Player::play = say; }
machine main(track: Audio) { preview::<Speaker>(track); }
";
    // Cleanup sibling: the instance releases exactly `track.h` at return, with or without the read.
    let closed = format!("{cleanup}{with_handle}{close}");
    let unread = closed.replace("Audio::play(track.volume);", "Audio::play(7);");
    let [read, unread] = [&closed, &unread].map(|text| sources::lower(&[("closed.omg", text)]).unwrap());
    let release = read.machines.iter().find(|m| m.declaration.path == "release").unwrap().id;
    let Parameter::Structural(track) = instance(&read).body.parameters[0] else { panic!("expected the owned formal") };
    // Reverse declaration order: release `h`, then discard the plain `volume` residual.
    let at = |field: &str| omega_psi::ProjectedPlace { root: track, path: vec![omega_psi::PlaceProjection::Field(field_id(&read, field))] };
    spec_corpus::observe_handoff!(
        psi "psi-output/obligated-sibling-instance-cleanup",
        assert!(matches!(return_cleanup(&read).as_slice(), [[CleanupAction::Invoke { place: h, target, .. }, CleanupAction::DiscardResidual { place: volume, .. }]]
            if *target == release && *h == at("Audio::h") && *volume == at("Audio::volume")),
            "{:?}", return_cleanup(&read)),
    );
    assert_eq!(return_cleanup(&read), return_cleanup(&unread));

    // Linear sibling: the lawful template consumes the whole formal after the read.
    let lawful = format!(
        "{linear}{}",
        with_handle
            .replace("trait Player", "machine burn(h: Handle) { let Handle { mark as _ } = h; }\nmachine finish(a: Audio) { let Audio { volume as _, h } = a; burn(h); }\ntrait Player")
            .replace("Audio::play(track.volume);\n", "Audio::play(track.volume);\n    finish(track);\n")
    );
    let package = sources::lower(&[("lawful.omg", &lawful)]).unwrap();
    let (template, _) = play_call(&package);
    let finish = package.machines.iter().find(|m| m.declaration.path == "finish").unwrap().declaration.clone();
    spec_corpus::observe_handoff!(
        psi "psi-output/obligated-sibling-template-consumption",
        assert!(matches!(template.body.as_slice(), [_, omega_psi::TemplateStep::Call(call)] if call.target == TemplateCallTarget::Declaration(finish.clone()) && call.operands == [TemplateOperand::Formal(0)])),
    );
    let closed = format!("{lawful}{close}");
    let unread = closed.replace("Audio::play(track.volume);", "Audio::play(7);");
    let [read, unread] = [&closed, &unread].map(|text| sources::lower(&[("closed.omg", text)]).unwrap());
    // Every call to `finish`, as an operation or as the terminal call, with the cleanup of each normal exit.
    // The whole formal moves first into the place `finish` receives.
    let consumes = |package: &LoweredPackage| {
        let body = &instance(package).body;
        let finish = omega_psi::CallTarget::Machine(package.machines.iter().find(|m| m.declaration.path == "finish").unwrap().id);
        let Parameter::Structural(track) = body.parameters[0] else { panic!("expected the owned formal") };
        let moved: Vec<_> = body
            .operations
            .iter()
            .filter_map(|op| match (&op.kind, op.result) {
                (omega_psi::OperationKind::MoveStructural { source }, omega_psi::OperationResult::Structural(place)) => Some((source.clone(), place)),
                _ => None,
            })
            .collect();
        let [(source, whole)] = moved.as_slice() else { panic!("expected one move: {moved:?}") };
        assert_eq!(*source, omega_psi::ProjectedPlace { root: track, path: vec![] });
        let mut calls: Vec<_> = body
            .operations
            .iter()
            .filter_map(|op| match &op.kind {
                omega_psi::OperationKind::Call { callee, arguments, .. } if *callee == finish => Some(arguments.clone()),
                _ => None,
            })
            .collect();
        let mut exits = Vec::new();
        for block in &body.blocks {
            match &block.terminator {
                BlockTerminator::TerminalCall { callee, arguments, cleanup, .. } => {
                    calls.extend((*callee == finish).then(|| arguments.clone()));
                    exits.push(cleanup.clone());
                }
                BlockTerminator::Return { cleanup, .. } => exits.push(cleanup.clone()),
                _ => {}
            }
        }
        (calls == [vec![omega_psi::CallArgument::MoveWhole(*whole)]], exits)
    };
    spec_corpus::observe_handoff!(
        psi "psi-output/obligated-sibling-instance-consumption",
        assert_eq!((consumes(&read), consumes(&unread)), ((true, vec![vec![]]), (true, vec![vec![]]))),
    );
}

/// Integration control for two repairs that meet in one template. Templates
/// made the body an ordered list of steps and kept an exported wrapper's open
/// interface beside its local instance; fields made a call operand name the
/// whole path to a copied field. In `get(track.tag.id) + get(track.volume)`
/// each call keeps its own field path, and the integer step reads the two call
/// results, not the fields. The exported wrapper publishes the nested operand in
/// its template and still lowers its closed instance.
#[test]
fn template_steps_and_exported_wrappers_keep_each_field_operand_path() {
    let header = "data Tag { id: u8; }
data Audio { volume: u8; tag: Tag; }
trait Player { machine get(v: u8) -> u8; }
";
    let sum = format!("{header}\nmachine preview<const Audio: Player>(track: Audio) -> u8\n{{\n    Audio::get(track.tag.id) + Audio::get(track.volume)\n}}\n");
    let package = sources::lower(&[("sum.omg", &sum)]).unwrap();
    let [template] = package.machine_templates.as_slice() else { panic!("expected the open template") };
    let [omega_psi::TemplateStep::Call(nested), omega_psi::TemplateStep::Call(direct), omega_psi::TemplateStep::Integer(add)] = template.body.as_slice() else {
        panic!("expected two calls and their sum: {:?}", template.body)
    };
    let read = |carrier: &[&str], field: &str| TemplateOperand::RecordField {
        formal: 0,
        carrier: carrier.iter().map(|path| field_id(&package, path)).collect(),
        field: field_id(&package, field),
        argument: Parameter::Scalar(byte()),
    };
    assert_eq!((nested.operands.as_slice(), direct.operands.as_slice()), ([read(&["Audio::tag"], "Tag::id")].as_slice(), [read(&[], "Audio::volume")].as_slice()));
    assert_eq!(add.operands, [TemplateOperand::Step(0), TemplateOperand::Step(1)]);
    assert_eq!(template.result, Some(omega_psi::TemplateResult::Operand(TemplateOperand::Step(2))));

    let exported = format!(
        "{}\npublic machine preview<const Audio: Player>(track: Audio) -> u8\n{{\n    Audio::get(track.tag.id)\n}}\nmachine echo(v: u8) -> u8 {{ v }}\nSpeaker satisfies Player {{ Player::get = echo; }}\nmachine main(track: Audio) -> u8 {{ preview::<Speaker>(track) }}\n",
        header.replace("data ", "public data ").replace("trait ", "public trait ")
    );
    let package = sources::lower(&[("exported.omg", &exported)]).unwrap();
    let [template] = package.machine_templates.as_slice() else { panic!("expected the exported open template") };
    let [omega_psi::TemplateStep::Call(call)] = template.body.as_slice() else { panic!("expected the one call") };
    assert!(template.public);
    assert_eq!(
        call.operands,
        [TemplateOperand::RecordField { formal: 0, carrier: vec![field_id(&package, "Audio::tag")], field: field_id(&package, "Tag::id"), argument: Parameter::Scalar(byte()) }]
    );
    assert!(package.machines.iter().any(|m| m.declaration.path == "preview"), "the local instance still lowers");
}
