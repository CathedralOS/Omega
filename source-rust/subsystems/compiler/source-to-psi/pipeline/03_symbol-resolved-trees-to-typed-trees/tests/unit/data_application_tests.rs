use typed_trees::data::DataMember;

#[test]
fn a_named_layout_closes_only_its_own_data_application() {
    let declarations = r#"
data Word { bits: u64; }
data Other { other: u8; }
trait Layout<T> { machine measure(value: T) -> u64; }
WordLayout satisfies Layout<Word> { machine measure(value: Word) -> u64 { value.bits } }
OtherLayout satisfies Layout<Other> { machine measure(value: Other) -> u64 { 0u64 } }
"#;
    for parameter in ["Policy satisfies Layout<T>", "const Policy: Layout<T>"] {
        let source = format!("{declarations}data Storage<T, {parameter}> {{ value: T; }}");
        for (holder, closed) in [
            (
                "data Holder { storage: Storage<Word, WordLayout>; }".to_owned(),
                true,
            ),
            (
                format!("data Holder<T, {parameter}> {{ field: Storage<T, Policy>; }}"),
                false,
            ),
        ] {
            let resolved = crate::resolve_fixture(&format!("{source}\n{holder}"));
            let program = crate::lower_symbol_resolved_trees(&resolved).unwrap();
            let holder = program
                .data_definitions()
                .iter()
                .find(|d| d.name.as_str() == "Holder")
                .unwrap();
            let DataMember::Field(field) = &program.data_members(holder)[0] else {
                panic!("Holder.storage");
            };
            assert_eq!(
                program.data_application_is_closed(field.type_reference),
                closed,
                "{parameter}"
            );
        }
        let resolved = crate::resolve_fixture(&format!(
            "{source}\ndata Holder {{ storage: Storage<Word, OtherLayout>; }}"
        ));
        let error = crate::lower_symbol_resolved_trees(&resolved).expect_err("wrong layout");
        assert!(
            error.message.contains("OtherLayout")
                && error.message.contains("Layout")
                && error.message.contains("Storage"),
            "{}",
            error.message
        );
    }
}

#[test]
fn a_case_payload_can_use_its_enclosing_machine_lifetime() {
    let text = r#"
data Found<'input> { position: u64; bytes: @'input [u8]; }
data Maybe<T> { case Missing; case Present(value: T); }
machine position<'input>(found: Maybe<Found<'input>>) -> u64 {
    transition found {
        Maybe::Present { value } -> value.position,
        Maybe::Missing -> 0u64
    }
}
"#;
    let resolved = crate::resolve_fixture(text);
    crate::lower_symbol_resolved_trees(&resolved).unwrap();
    let unbound = text.replace("Maybe<Found<'input>>", "Maybe<Found<'missing>>");
    let diagnostic = crate::lower_symbol_resolved_trees(&crate::resolve_fixture(&unbound))
        .expect_err("the payload's lifetime must belong to a declaration");
    assert!(
        diagnostic.message.contains("lifetime argument `'missing`"),
        "{diagnostic:?}"
    );
}

#[test]
fn a_specialized_member_prepares_its_nested_domain_before_its_outer_coordinate() {
    let text = "
data Word { value: u64; }
domain<T> T::Placed;
domain<T> T::Other;
data Run<P,T> { count: u64; }
domain<P,T> Run<P,T>::Live<P,T>;
data Cells<P,T> = Run<P,T> in Run::Live<P,T>;
data Vec<T> { live: Cells<T in Placed,T>; }
machine pack<T>(live: Cells<T in Placed,T>) -> Vec<T> { Vec { live: live } }
machine carry(live: Cells<Word in Placed,Word>) -> Vec<Word> { pack::<Word>(live) }
machine other(live: Cells<Word in Other,Word>) {}";
    let resolved = crate::resolve_fixture(text);
    let program = crate::lower_symbol_resolved_trees(&resolved).unwrap();
    let mut placements = Vec::new();
    for applied in program
        .applied_domain_applications
        .iter()
        .filter(|row| row.is_closed)
    {
        if program
            .domain_definition(applied.declaration)
            .unwrap()
            .name
            .as_str()
            != "Run::Live"
        {
            continue;
        }
        let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program
            .type_reference_table
            .type_reference(applied.arguments[0])
        else {
            continue;
        };
        if program.display_type_reference_with_constraints(*base_type) != "Word" {
            continue;
        }
        let [typed_trees::types::TypeConstraintNode::Domain(placement)] =
            program.type_reference_table.constraints(*constraints)
        else {
            continue;
        };
        let Some([subject]) =
            program.domain_arguments_for_instance(placement.symbol, placement.semantic_id)
        else {
            continue;
        };
        assert_eq!(
            program.normalized_type_identity(*subject),
            program.normalized_type_identity(*base_type),
            "a closed placement on Word must retain Word as the domain's carrier"
        );
        placements.push(placement.semantic_id);
    }
    placements.sort_by_key(|id| id.0);
    placements.dedup();
    assert_eq!(
        placements.len(),
        2,
        "both distinct concrete placements are retained"
    );
    assert_ne!(
        placements[0], placements[1],
        "distinct placement domains keep distinct indices"
    );
}
