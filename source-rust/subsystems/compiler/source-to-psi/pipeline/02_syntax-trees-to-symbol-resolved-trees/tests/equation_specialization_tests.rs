use super::source_request;
use crate::resolution::resolve;

#[test]
fn closed_data_equations_recover_an_omitted_static_argument() {
    let source = "
        data Mode<const Inner: u32> [copy] { value: u32; }
        data Box<Payload [copy], const Count: u32> [copy]
        requires Payload == Mode<Count>
        { value: Payload; }
        data Container { item: Box<Mode<4> >; }
    ";
    for source in [
        source.to_owned(),
        source.replace("Box<Mode<4> >", "Box<Mode<4>, 4>"),
    ] {
        resolve(source_request(&source)).unwrap_or_else(|errors| panic!("{errors:?}"));
    }
    for name in ["IntervalSet", "CountedQuantity"] {
        let renamed = source.replace("Box", name);
        resolve(source_request(&renamed)).unwrap_or_else(|errors| panic!("{name}: {errors:?}"));
    }
    let conflicting = source.replace("Box<Mode<4> >", "Box<Mode<4>, 5>");
    let errors = resolve(source_request(&conflicting)).expect_err("conflicting value accepted");
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("binds `Count` to 4 but its explicit argument is 5")),
        "{errors:?}"
    );
}

#[test]
fn nested_const_landing_errors_reach_the_stage_result() {
    let source = "
        data Mode<const Inner: u32> { value: u32; }
        data Box<Payload, const Count: u32> requires Payload == Mode<Count> { value: Payload; }
        data Container { item: Box<Mode<((7 / 2) * 2)> >; }
    ";
    let errors = resolve(source_request(source)).expect_err("implicit fractional landing accepted");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("fractional intermediate")),
        "{errors:?}"
    );
}

#[test]
fn integer_operands_do_not_prove_a_generic_case_impossible() {
    use crate::source_resolved::data::DataMember;

    for (operand, can_keep) in [
        ("!false", true),
        ("!true", false),
        ("!5", true),
        ("!1", true),
        ("0", true),
        ("!340282366920938463463374607431768211455", true),
    ] {
        let source = format!(
            "
            data Mode<const Inner: u32> [copy] {{ value: u32; }}
            data Choice<Payload [copy], const Count: u32> [copy]
                requires Payload == Mode<Count>
                requires (self in Choice::Other) || {operand}
            {{ case Keep(value: Payload); case Other; }}
            data Demand {{ item: Choice<Mode<4> >; }}
        "
        );
        let trees = resolve(source_request(&source))
            .unwrap_or_else(|diagnostics| panic!("{operand}: {diagnostics:?}"));
        let case = trees
            .data_definitions
            .iter()
            .filter(|data| data.generic_instance.is_some())
            .flat_map(|data| trees.data_members(data.members))
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.name.as_str() == "Keep" => Some(variant),
                _ => None,
            })
            .expect("the demanded instance retains its Keep case");
        assert_eq!(
            case.requires_facts.is_empty(),
            can_keep,
            "{operand}: the resolver may mark Keep impossible only from a Boolean predicate"
        );
    }
}

#[test]
fn content_algebra_source_requires_the_core_owner_and_bundled_file() {
    use language_core::bundled_files::BundledFile;
    use language_core::core_vocabulary::CoreSource;
    use source_admission::SourceOrigin;

    let core = std::str::from_utf8(bundled_declarations::bytes(BundledFile::CoreContent)).unwrap();
    let edited = format!("{core}\n// changed\n");
    let mut sources = crate::source_inputs::SourceMap::default();
    for (path, origin, source, expected) in [
        (
            "library/content.omg",
            SourceOrigin::Toolchain,
            core,
            Some(CoreSource::Content),
        ),
        ("library/content.omg", SourceOrigin::User, core, None),
        ("library/other.omg", SourceOrigin::Toolchain, core, None),
        (
            "library/content.omg",
            SourceOrigin::Toolchain,
            edited.as_str(),
            None,
        ),
    ] {
        let file = sources.add_with_metadata(
            path.into(),
            source.to_owned(),
            "library".into(),
            None,
            origin,
        );
        assert_eq!(
            crate::source_symbols::core_source_at(file),
            expected,
            "{path} {origin:?}"
        );
    }
}
