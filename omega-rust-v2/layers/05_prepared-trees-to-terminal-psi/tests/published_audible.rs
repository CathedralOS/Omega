#[path = "support/source.rs"]
mod sources;

use omega_preparation::prepare_source_package;
use omega_psi::{DomainAtom, MachineTemplateBinder, PredicateOperator, PredicateStep, ScalarType, TemplateCallTarget, TemplateOperand, TypeExpressionStep, TypeShape};
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
        let requirement = player.requirements.iter().find(|r| r.declaration.path == member).unwrap();
        let [call] = preview.body.as_slice() else { panic!("expected the selected member invocation") };
        spec_corpus::observe_handoff!(psi "psi-output/published-audible-call", assert_eq!((&call.target, call.operands.as_slice()),
            (&TemplateCallTarget::Member { binder, requirement: requirement.declaration.clone() }, [TemplateOperand::RecordField { formal, field: field(actual) }].as_slice())));
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
        let [call] = template.body.as_slice() else {
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
/// own requirement. A template whose only static parameter is a value keeps
/// refusing, because it has no binder for a caller to supply.
#[test]
fn a_requirement_call_names_its_trait_and_a_value_only_template_refuses() {
    let source = "trait Player { machine play(volume: u8); }\ntrait Mixer { machine mix(volume: u8); }\nmachine preview<const P: Player, const M: Mixer>(volume: u8)\n{\n    M::mix(volume);\n    P::play(volume);\n}\n";
    let package = sources::lower(&[("template.omg", source)]).unwrap();
    let targets: Vec<_> = package.machine_templates[0]
        .body
        .iter()
        .map(|call| match &call.target {
            TemplateCallTarget::Member { binder, requirement } => (*binder, requirement.path.as_str()),
            other => panic!("unexpected target {other:?}"),
        })
        .collect();
    assert_eq!(targets, [(1, "Mixer::mix"), (0, "Player::play")]);

    let value = "machine keep<const N: u8>(volume: u8) -> u8 { volume }";
    let message = "open generic machine requires a closed application before lowering";
    let error = sources::refuse(&[("value.omg", value)], "value.omg", sources::unique_span(value, value), message);
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
    assert!(matches!(&template.body[0].target, TemplateCallTarget::Member { binder: 0, requirement } if requirement.path == "Player::play"));
    assert_eq!(template.body[0].operands, [TemplateOperand::Formal(0)]);
}

/// Without its record domain, the published fence still passes `track.volume`,
/// a scalar field of a formal record. The template call publishes that field
/// read as its operand. The twin adds a fixed-array field to `Audio`; a record
/// with a non-scalar field is not a plain field read, so L5 still refuses at
/// the same call with the operand message.
#[test]
fn a_projected_operand_reads_its_scalar_field_and_a_non_scalar_record_refuses_at_the_call() {
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
    let [template] = package.machine_templates.as_slice() else { panic!("expected the open template") };
    let [call] = template.body.as_slice() else { panic!("expected the one requirement call") };
    assert!(matches!(&call.target, TemplateCallTarget::Member { binder: 0, requirement } if requirement.path == "Player::play"));
    spec_corpus::observe_handoff!(
        psi "psi-output/projected-template-operand",
        assert_eq!(call.operands, [TemplateOperand::RecordField { formal: 0, field: volume.declaration.clone() }]),
    );

    let wide = text.replace("volume: u8; }", "volume: u8; name: [u8; 2]; }");
    let message = "template call has unsupported operands, selection or obligations";
    let error = sources::refuse(
        &[("record.omg", &wide)],
        "record.omg",
        sources::unique_span(&wide, "Audio::play(track.volume)"),
        message,
    );
    assert_eq!(error.message, message);
}
