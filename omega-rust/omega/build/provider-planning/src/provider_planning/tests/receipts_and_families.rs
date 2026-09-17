use super::{
    append_admitted_fact, bind_selected_provider_plan_facts_for_test, data_symbol,
    family_selection_from_typed, operator_coordinate_plan, operator_symbol_at_path,
    package_selection, push_boundary_requirement, selected_plan_names, selection_plan,
    set_exact_requirement, typed_fixture, typed_fixture_with_source,
};
use crate::provider_planning::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderSchemaDeclaration,
    ProviderSelectionProvenance, derive_satisfies_plans, select_derived_provider_plans,
    select_provider_plans, selected_provider_plan_facts,
};
use crate::{
    ProviderOperatorFamilyCoordinate, ProviderOperatorFamilySelection, ProviderPlanDerivation,
    ProviderSelectionSubject,
};

#[test]
fn admitted_receipt_owner_and_signature_custody_is_exact_and_atomic() {
    let owner_symbol = symbols::SymbolHandle::from_arena_index(7);
    let requirement_symbol = symbols::SymbolHandle::from_arena_index(10);
    let mut checked = checked_trees::CheckedTrees::default();
    let requirement_identity = push_boundary_requirement(
        &mut checked,
        owner_symbol,
        "PairBase",
        requirement_symbol,
        "first",
    );
    let valid = append_admitted_fact(
        &mut checked,
        symbols::SymbolHandle::from_arena_index(8),
        symbols::SymbolHandle::from_arena_index(9),
        owner_symbol,
        requirement_symbol,
    );
    append_admitted_fact(
        &mut checked,
        symbols::SymbolHandle::from_arena_index(11),
        symbols::SymbolHandle::from_arena_index(12),
        owner_symbol,
        symbols::SymbolHandle::from_arena_index(90),
    );
    let mut selected = selection_plan("FirstProvider", &["first"], &["first"]);
    set_exact_requirement(
        &mut selected,
        "PairChild",
        "PairBase",
        &requirement_identity,
    );

    let diagnostics = bind_selected_provider_plan_facts_for_test(
        &mut checked,
        std::slice::from_ref(&selected),
        effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&selected),
            &[selected.name.clone()],
        )
        .expect("selected provider"),
        &[selected.schema.trait_name.clone()],
    )
    .expect_err("late missing signature must reject every staged receipt");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("resolves to 0 exact typed signatures")
    }));
    assert_eq!(
        checked
            .facts
            .semantic
            .facts
            .get(valid)
            .evidence
            .receipt_identity,
        0,
        "late failure must not publish an earlier valid receipt",
    );

    let mut duplicate_owner = checked.clone();
    duplicate_owner
        .typed
        .push_trait_definition(typed_trees::trait_definition::TraitDefinition {
            symbol: owner_symbol,
            is_boundary: true,
            name: typed_trees::name::Identifier::generated("DuplicatePairBase"),
            ..Default::default()
        });
    let diagnostics = bind_selected_provider_plan_facts_for_test(
        &mut duplicate_owner,
        std::slice::from_ref(&selected),
        effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&selected),
            &[selected.name.clone()],
        )
        .expect("selected provider"),
        &[selected.schema.trait_name.clone()],
    )
    .expect_err("duplicate exact owner must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("resolves to 2 exact typed boundary requirement owners")
    }));

    let other_owner = symbols::SymbolHandle::from_arena_index(20);
    let other_requirement = symbols::SymbolHandle::from_arena_index(21);
    let mut cross_owned = checked_trees::CheckedTrees::default();
    push_boundary_requirement(
        &mut cross_owned,
        owner_symbol,
        "PairBase",
        requirement_symbol,
        "first",
    );
    push_boundary_requirement(
        &mut cross_owned,
        other_owner,
        "OtherBase",
        other_requirement,
        "other",
    );
    append_admitted_fact(
        &mut cross_owned,
        symbols::SymbolHandle::from_arena_index(22),
        symbols::SymbolHandle::from_arena_index(23),
        owner_symbol,
        other_requirement,
    );
    let diagnostics = bind_selected_provider_plan_facts_for_test(
        &mut cross_owned,
        std::slice::from_ref(&selected),
        effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&selected),
            &[selected.name.clone()],
        )
        .expect("selected provider"),
        &[selected.schema.trait_name.clone()],
    )
    .expect_err("cross-owned exact signature must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("belongs to exact trait") })
    );

    let mut duplicate_signature = checked_trees::CheckedTrees::default();
    push_boundary_requirement(
        &mut duplicate_signature,
        owner_symbol,
        "PairBase",
        requirement_symbol,
        "first",
    );
    push_boundary_requirement(
        &mut duplicate_signature,
        other_owner,
        "OtherBase",
        requirement_symbol,
        "duplicate",
    );
    append_admitted_fact(
        &mut duplicate_signature,
        symbols::SymbolHandle::from_arena_index(24),
        symbols::SymbolHandle::from_arena_index(25),
        owner_symbol,
        requirement_symbol,
    );
    let diagnostics = bind_selected_provider_plan_facts_for_test(
        &mut duplicate_signature,
        std::slice::from_ref(&selected),
        effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&selected),
            &[selected.name.clone()],
        )
        .expect("selected provider"),
        &[selected.schema.trait_name.clone()],
    )
    .expect_err("duplicate exact signature must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("resolves to 2 exact typed signatures")
    }));
}

#[test]
fn admitted_receipt_rejects_duplicate_exact_granted_plan_matches() {
    let owner_symbol = symbols::SymbolHandle::from_arena_index(7);
    let requirement_symbol = symbols::SymbolHandle::from_arena_index(10);
    let mut checked = checked_trees::CheckedTrees::default();
    let requirement_identity = push_boundary_requirement(
        &mut checked,
        owner_symbol,
        "PairBase",
        requirement_symbol,
        "first",
    );
    append_admitted_fact(
        &mut checked,
        symbols::SymbolHandle::from_arena_index(8),
        symbols::SymbolHandle::from_arena_index(9),
        owner_symbol,
        requirement_symbol,
    );
    let mut first = selection_plan("FirstProvider", &["first"], &["first"]);
    set_exact_requirement(&mut first, "PairChildA", "PairBase", &requirement_identity);
    let mut second = selection_plan("SecondProvider", &["first"], &["first"]);
    set_exact_requirement(&mut second, "PairChildB", "PairBase", &requirement_identity);

    let diagnostics = bind_selected_provider_plan_facts_for_test(
        &mut checked,
        &[first.clone(), second.clone()],
        effects::SelectedProviderPlanFacts::from_selection(
            &[first, second],
            &["FirstProvider".to_owned(), "SecondProvider".to_owned()],
        )
        .expect("distinct selected slots may retain duplicate requirement identities"),
        &["PairChildA".to_owned(), "PairChildB".to_owned()],
    )
    .expect_err("two granted exact matches must reject rather than choose by order");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("matches 2 granted selected provider plans")
    }));
}

#[test]
fn explicit_selection_resolves_covering_ambiguity_by_provider_type() {
    let plans = vec![
        selection_plan("FirstProvider", &["first"], &["first"]),
        selection_plan("SecondProvider", &["first"], &["first"]),
    ];
    let selected = select_provider_plans(
        &plans,
        target::NativeTarget::host(),
        &[],
        &[crate::ProviderSelection::exact_for_test(
            "Pair",
            "SecondProvider",
        )],
    )
    .expect("the build root owns the slot choice");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["SecondProvider".to_owned()]
    );
}

#[test]
fn operator_family_selection_atomically_selects_every_exact_coordinate() {
    let first_coordinate = "operator::Math::convert(i32)->i64";
    let second_coordinate = "operator::Math::convert(u32)->u64";
    let plans = [
        operator_coordinate_plan("signed-convert", first_coordinate, "MathProvider"),
        operator_coordinate_plan("unsigned-convert", second_coordinate, "MathProvider"),
    ];

    let selected = select_provider_plans(
        &plans,
        target::NativeTarget::host(),
        &[],
        &[crate::ProviderSelection::operator_family_for_test(
            "Math::convert",
            "MathProvider",
            &[second_coordinate, first_coordinate],
        )],
    )
    .expect("one family declaration selects its complete exact coordinate roster");

    assert_eq!(
        selected_plan_names(&selected),
        vec!["signed-convert".to_owned(), "unsigned-convert".to_owned()]
    );
}

#[test]
fn operator_family_selection_rejects_when_one_coordinate_lacks_the_provider() {
    let first_coordinate = "operator::Math::convert(i32)->i64";
    let second_coordinate = "operator::Math::convert(u32)->u64";
    let plans = [
        operator_coordinate_plan("signed-convert", first_coordinate, "MathProvider"),
        operator_coordinate_plan("unsigned-convert", second_coordinate, "OtherProvider"),
    ];

    let diagnostics = select_provider_plans(
        &plans,
        target::NativeTarget::host(),
        &[],
        &[crate::ProviderSelection::operator_family_for_test(
            "Math::convert",
            "MathProvider",
            &[first_coordinate, second_coordinate],
        )],
    )
    .expect_err("a family selection cannot admit only its satisfiable subset");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(second_coordinate)
            && diagnostic
                .message
                .contains("no candidate exists in the loaded dependency closure")
    }));
}

#[test]
fn operator_family_selection_canonicalizes_coordinate_order() {
    let first_coordinate = "operator::Math::convert(i32)->i64";
    let second_coordinate = "operator::Math::convert(u32)->u64";
    let selection = crate::ProviderSelection::operator_family_for_test(
        "Math::convert",
        "MathProvider",
        &[second_coordinate, first_coordinate],
    );
    let crate::ProviderSelectionSubject::BoundaryOperatorFamily(family) = selection.subject else {
        panic!("operator-family fixture must retain a family subject")
    };

    assert_eq!(
        family
            .coordinates()
            .iter()
            .map(|coordinate| coordinate.requirement_identity.as_str())
            .collect::<Vec<_>>(),
        vec![first_coordinate, second_coordinate]
    );
}

#[test]
fn operator_family_selection_rejects_an_unknown_exact_coordinate() {
    let known_coordinate = "operator::Math::convert(i32)->i64";
    let unknown_coordinate = "operator::Math::convert(u32)->u64";
    let plans = [operator_coordinate_plan(
        "signed-convert",
        known_coordinate,
        "MathProvider",
    )];

    let diagnostics = select_provider_plans(
        &plans,
        target::NativeTarget::host(),
        &[],
        &[crate::ProviderSelection::operator_family_for_test(
            "Math::convert",
            "MathProvider",
            &[known_coordinate, unknown_coordinate],
        )],
    )
    .expect_err("the complete compiler-derived family roster must exist");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(unknown_coordinate)
            && diagnostic.message.contains("subject `Math::convert`")
    }));
}

fn family_coordinate(identity: &str) -> ProviderOperatorFamilyCoordinate {
    ProviderOperatorFamilyCoordinate {
        symbol: symbols::SymbolHandle::invalid(),
        requirement_identity: identity.to_owned(),
        static_parameter_count: 0,
    }
}

#[test]
fn operator_family_roster_rejects_duplicate_and_empty_coordinates() {
    let coordinate = "operator::Math::convert(i32)->i64";
    let duplicate = ProviderOperatorFamilySelection::new(
        None,
        "Math::convert".to_owned(),
        "Math::convert".to_owned(),
        vec![family_coordinate(coordinate), family_coordinate(coordinate)],
    )
    .expect_err("a family roster is a semantic set, not a padded list");
    assert!(duplicate.contains("ambiguous coordinate"));
    assert!(duplicate.contains(coordinate));

    let empty = ProviderOperatorFamilySelection::new(
        None,
        "Math::convert".to_owned(),
        "Math::convert".to_owned(),
        Vec::new(),
    )
    .expect_err("a family with no applicable coordinate selects nothing");
    assert!(empty.contains("contains no applicable coordinates"));
}

#[test]
fn operator_family_selection_rejects_a_coordinate_covered_only_on_another_target() {
    let first_coordinate = "operator::Math::convert(i32)->i64";
    let second_coordinate = "operator::Math::convert(u32)->u64";
    let mut foreign_target =
        operator_coordinate_plan("unsigned-convert", second_coordinate, "MathProvider");
    foreign_target.target = if target::NativeTarget::host() == target::NativeTarget::windows_x64() {
        "linux_x86_64".to_owned()
    } else {
        "windows_x86_64".to_owned()
    };
    let plans = [
        operator_coordinate_plan("signed-convert", first_coordinate, "MathProvider"),
        foreign_target,
    ];

    let diagnostics = select_provider_plans(
        &plans,
        target::NativeTarget::host(),
        &[],
        &[crate::ProviderSelection::operator_family_for_test(
            "Math::convert",
            "MathProvider",
            &[first_coordinate, second_coordinate],
        )],
    )
    .expect_err("target coverage is its own axis: a foreign-target row covers nothing here");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(second_coordinate)
            && diagnostic
                .message
                .contains("no selected-target candidate exists")
    }));
}

#[test]
fn operator_family_selection_from_another_package_cannot_select_same_spelled_coordinates() {
    let family_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x41; 32])
        .expect("nonzero package identity");
    let other_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x42; 32])
        .expect("nonzero package identity");
    let coordinate = "operator::Math::convert(i32)->i64";
    let mut plan = operator_coordinate_plan("signed-convert", coordinate, "MathProvider");
    plan.schema.trait_package_identity = Some(family_package);
    plan.provider_type_package_identity = Some(family_package);
    let mut selection = crate::ProviderSelection::operator_family_for_test(
        "Math::convert",
        "MathProvider",
        &[coordinate],
    );
    let ProviderSelectionSubject::BoundaryOperatorFamily(family) = &mut selection.subject else {
        panic!("operator-family fixture must retain a family subject")
    };
    family.package = Some(other_package);
    selection.provider_type.package = Some(family_package);

    let diagnostics =
        select_provider_plans(&[plan], target::NativeTarget::host(), &[], &[selection])
            .expect_err("a same-spelled coordinate in another package is not this family");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(coordinate)
            && diagnostic.message.contains("unknown boundary coordinate")
    }));
}

#[test]
fn operator_family_declared_twice_rejects_every_coordinate_slot() {
    let first_coordinate = "operator::Math::convert(i32)->i64";
    let second_coordinate = "operator::Math::convert(u32)->u64";
    let plans = [
        operator_coordinate_plan("signed-convert", first_coordinate, "MathProvider"),
        operator_coordinate_plan("unsigned-convert", second_coordinate, "MathProvider"),
        operator_coordinate_plan("other-signed-convert", first_coordinate, "OtherProvider"),
        operator_coordinate_plan("other-unsigned-convert", second_coordinate, "OtherProvider"),
    ];

    let diagnostics = select_provider_plans(
        &plans,
        target::NativeTarget::host(),
        &[],
        &[
            crate::ProviderSelection::operator_family_for_test(
                "Math::convert",
                "MathProvider",
                &[first_coordinate, second_coordinate],
            ),
            crate::ProviderSelection::operator_family_for_test(
                "Math::convert",
                "OtherProvider",
                &[first_coordinate, second_coordinate],
            ),
        ],
    )
    .expect_err("one family cannot be selected twice");
    for coordinate in [first_coordinate, second_coordinate] {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "provider selection for slot `{coordinate}` more than once"
                ))
            }),
            "expected a duplicate-slot diagnostic for `{coordinate}`, got {diagnostics:#?}"
        );
    }
}

const FAMILY_SOURCE_SIGNED_FIRST: &str = r#"
    data Convert {}
    boundary operator Convert::apply(value: i32) -> i32;
    boundary operator Convert::apply(value: u32) -> u32;

    data Other {}
    boundary operator Other::apply(value: u8) -> u8;

    data ConvertProvider {}
    machine ConvertProvider::apply_i32(value: i32) -> i32
    satisfies Convert::apply
    {
        transition { _ -> (value) }
    }
    machine ConvertProvider::apply_u32(value: u32) -> u32
    satisfies Convert::apply
    {
        transition { _ -> (value) }
    }
    machine ConvertProvider::apply_other(value: u8) -> u8
    satisfies Other::apply
    {
        transition { _ -> (value) }
    }

    machine build_root() -> u8 { 0 }
"#;

const FAMILY_SOURCE_UNSIGNED_FIRST: &str = r#"
    data Convert {}
    boundary operator Convert::apply(value: u32) -> u32;
    boundary operator Convert::apply(value: i32) -> i32;

    data Other {}
    boundary operator Other::apply(value: u8) -> u8;

    data ConvertProvider {}
    machine ConvertProvider::apply_u32(value: u32) -> u32
    satisfies Convert::apply
    {
        transition { _ -> (value) }
    }
    machine ConvertProvider::apply_i32(value: i32) -> i32
    satisfies Convert::apply
    {
        transition { _ -> (value) }
    }
    machine ConvertProvider::apply_other(value: u8) -> u8
    satisfies Other::apply
    {
        transition { _ -> (value) }
    }

    machine build_root() -> u8 { 0 }
"#;

const CONVERT_I32: &str = "operator::Convert::apply(named(name(i32)))->named(name(i32))";
const CONVERT_U32: &str = "operator::Convert::apply(named(name(u32)))->named(name(u32))";
const OTHER_U8: &str = "operator::Other::apply(named(name(u8)))->named(name(u8))";

#[test]
fn operator_family_roster_is_independent_of_declaration_order() {
    let signed_first = typed_fixture(FAMILY_SOURCE_SIGNED_FIRST);
    let unsigned_first = typed_fixture(FAMILY_SOURCE_UNSIGNED_FIRST);
    let rosters = [&signed_first, &unsigned_first].map(|typed| {
        let family = ProviderOperatorFamilySelection::derive(
            typed,
            operator_symbol_at_path(typed, "Convert::apply"),
            "Convert::apply".to_owned(),
        )
        .expect("both declaration orders derive the same canonical family");
        assert_eq!(family.canonical_path, "Convert::apply");
        assert!(family.replay_against_typed(typed).is_empty());
        family
            .coordinates()
            .iter()
            .map(|coordinate| {
                (
                    coordinate.requirement_identity.clone(),
                    coordinate.static_parameter_count,
                )
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(rosters[0], rosters[1]);
    assert_eq!(
        rosters[0],
        vec![(CONVERT_I32.to_owned(), 0), (CONVERT_U32.to_owned(), 0)]
    );

    let neighbour = ProviderOperatorFamilySelection::derive(
        &signed_first,
        operator_symbol_at_path(&signed_first, "Other::apply"),
        "Other::apply".to_owned(),
    )
    .expect("the neighbouring family derives on its own");
    assert_eq!(
        neighbour
            .coordinates()
            .iter()
            .map(|coordinate| coordinate.requirement_identity.as_str())
            .collect::<Vec<_>>(),
        vec![OTHER_U8]
    );
}

fn replay_family_selection(
    typed: &typed_trees::TypedTrees,
    selection: crate::ProviderSelection,
) -> Result<Vec<String>, Vec<diagnostics::Diagnostic>> {
    let derived = derive_satisfies_plans(typed, ProviderPlanDerivation::unevaluated(None));
    assert_eq!(
        derived.len(),
        3,
        "two Convert coordinates and one Other coordinate"
    );
    let table = crate::evaluated_via_bindings::evaluate_via_bindings(typed, None, None)
        .expect("a fixture without `via` evaluates to an empty table");
    let selected =
        select_derived_provider_plans(&derived, target::NativeTarget::host(), &[], &[selection])?;
    let (facts, _) = selected_provider_plan_facts(typed, &table, selected)?;
    Ok(facts
        .plans()
        .iter()
        .map(|plan| plan.schema.trait_name.clone())
        .collect())
}

fn family_with_coordinates(
    selection: &crate::ProviderSelection,
    coordinates: Vec<ProviderOperatorFamilyCoordinate>,
) -> crate::ProviderSelection {
    let ProviderSelectionSubject::BoundaryOperatorFamily(family) = &selection.subject else {
        panic!("family fixture must retain a family subject")
    };
    let mut forged = selection.clone();
    forged.subject = ProviderSelectionSubject::BoundaryOperatorFamily(
        ProviderOperatorFamilySelection::new(
            family.package,
            family.canonical_path.clone(),
            family.authored_path.clone(),
            coordinates,
        )
        .expect("forged roster is well-formed on its own"),
    );
    forged
}

#[test]
fn operator_family_replay_rejects_partial_padded_stale_and_substituted_rosters() {
    let typed = typed_fixture_with_source("family_replay.omg", FAMILY_SOURCE_SIGNED_FIRST);
    let exact =
        family_selection_from_typed(&typed, "Convert::apply", "ConvertProvider", "build_root");
    let ProviderSelectionSubject::BoundaryOperatorFamily(family) = &exact.subject else {
        panic!("family fixture must retain a family subject")
    };
    let [signed, unsigned] = family.coordinates() else {
        panic!("Convert::apply has two canonical coordinates")
    };
    assert_eq!(signed.requirement_identity, CONVERT_I32);
    assert_eq!(unsigned.requirement_identity, CONVERT_U32);
    let other_symbol = operator_symbol_at_path(&typed, "Other::apply");

    let mut selected =
        replay_family_selection(&typed, exact.clone()).expect("the exact canonical roster replays");
    selected.sort_unstable();
    assert_eq!(selected, vec![CONVERT_I32, CONVERT_U32, OTHER_U8]);

    // Partial: the omitted coordinate has a unique covering candidate, so
    // slot selection alone would quietly complete the family. Replay names
    // the omitted member instead.
    let partial = family_with_coordinates(&exact, vec![signed.clone()]);
    let diagnostics =
        replay_family_selection(&typed, partial).expect_err("a partial family row selects nothing");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("omits coordinate")
                && diagnostic.message.contains(CONVERT_U32)
        }),
        "expected the omitted member to be named, got {diagnostics:#?}"
    );

    // Padded: a coordinate the provider does realize, but outside the family.
    let padded = family_with_coordinates(
        &exact,
        vec![
            signed.clone(),
            unsigned.clone(),
            ProviderOperatorFamilyCoordinate {
                symbol: other_symbol,
                requirement_identity: OTHER_U8.to_owned(),
                static_parameter_count: 0,
            },
        ],
    );
    let diagnostics =
        replay_family_selection(&typed, padded).expect_err("a padded family row selects nothing");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resolves to a declaration outside the family")
                && diagnostic.message.contains(OTHER_U8)
        }),
        "expected the padded member to be named, got {diagnostics:#?}"
    );

    // Substituted: the right coordinate spelling under another declaration.
    let substituted = family_with_coordinates(
        &exact,
        vec![
            ProviderOperatorFamilyCoordinate {
                symbol: other_symbol,
                ..signed.clone()
            },
            unsigned.clone(),
        ],
    );
    let diagnostics = replay_family_selection(&typed, substituted)
        .expect_err("a substituted coordinate symbol selects nothing");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("under a substituted declaration symbol")
                && diagnostic.message.contains(CONVERT_I32)
        }),
        "expected the substituted member to be named, got {diagnostics:#?}"
    );

    // Generic axis: a retained telescope arity is rechecked, never trusted.
    let generic_assertion = family_with_coordinates(
        &exact,
        vec![
            ProviderOperatorFamilyCoordinate {
                static_parameter_count: 1,
                ..signed.clone()
            },
            unsigned.clone(),
        ],
    );
    let diagnostics = replay_family_selection(&typed, generic_assertion)
        .expect_err("a coordinate cannot claim a generic telescope its declaration lacks");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("static-telescope arity 1, but its exact declaration has arity 0")
                && diagnostic.message.contains(CONVERT_I32)
        }),
        "expected the telescope drift to be named, got {diagnostics:#?}"
    );

    // Stale: a coordinate no declaration produces any more never reaches
    // slot selection; the roster replay still names it.
    let stale = family_with_coordinates(
        &exact,
        vec![
            signed.clone(),
            unsigned.clone(),
            family_coordinate("operator::Convert::apply(named(name(i64)))->named(name(i64))"),
        ],
    );
    let ProviderSelectionSubject::BoundaryOperatorFamily(stale_family) = &stale.subject else {
        panic!("stale fixture must retain a family subject")
    };
    let reasons = stale_family.replay_against_typed(&typed);
    assert_eq!(reasons.len(), 1, "{reasons:#?}");
    assert!(reasons[0].contains("no longer resolves to a boundary operator declaration"));
    assert!(reasons[0].contains("named(name(i64))"));
}

#[test]
fn operator_family_replay_requires_the_declared_provider_to_realize_each_coordinate() {
    let typed = typed_fixture_with_source("family_provider.omg", FAMILY_SOURCE_SIGNED_FIRST);
    let mut selection =
        family_selection_from_typed(&typed, "Convert::apply", "ConvertProvider", "build_root");
    let derived = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None));
    let table = crate::evaluated_via_bindings::evaluate_via_bindings(&typed, None, None)
        .expect("a fixture without `via` evaluates to an empty table");
    let selected = select_derived_provider_plans(
        &derived,
        target::NativeTarget::host(),
        &[],
        &[selection.clone()],
    )
    .expect("the exact family selects");

    // A retained provenance whose provider symbol is not the plans' provider
    // is a substitution of the provider axis, not a family the plans realize.
    selection.provider_type.symbol = data_symbol(&typed, "Other");
    let forged = selected
        .into_iter()
        .map(|mut plan| {
            if let ProviderSelectionProvenance::BuildOverride(declarations) = &mut plan.selected_by
            {
                for declaration in declarations {
                    declaration.provider_type.symbol = selection.provider_type.symbol;
                }
            }
            plan
        })
        .collect::<Vec<_>>();
    let diagnostics = selected_provider_plan_facts(&typed, &table, forged)
        .expect_err("a family whose provider the plans do not realize selects nothing");
    for coordinate in [CONVERT_I32, CONVERT_U32] {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(coordinate)
                    && diagnostic
                        .message
                        .contains("to 0 selected plans of provider `ConvertProvider`")
            }),
            "expected coordinate `{coordinate}` to be named, got {diagnostics:#?}"
        );
    }
}

#[test]
fn same_spelled_package_slots_and_providers_remain_distinct() {
    let first_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x61; 32])
        .expect("nonzero package identity");
    let second_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x62; 32])
        .expect("nonzero package identity");

    let mut first = selection_plan("first-plan", &["choose"], &["choose"]);
    first.schema.trait_name = "Shared".to_owned();
    first.schema.trait_package_identity = Some(first_package);
    first.provider_type = "Provider".to_owned();
    first.provider_type_package_identity = Some(first_package);

    let mut second = selection_plan("second-plan", &["choose"], &["choose"]);
    second.schema.trait_name = "Shared".to_owned();
    second.schema.trait_package_identity = Some(second_package);
    second.provider_type = "Provider".to_owned();
    second.provider_type_package_identity = Some(second_package);

    let plans = [first, second];
    let automatic = select_provider_plans(&plans, target::NativeTarget::host(), &[], &[])
        .expect("each exact slot has one covering provider");
    assert_eq!(
        selected_plan_names(&automatic),
        vec!["first-plan".to_owned(), "second-plan".to_owned()]
    );

    let selected = select_provider_plans(
        &plans,
        target::NativeTarget::host(),
        &[
            package_selection("Shared", first_package, "Provider", first_package),
            package_selection("Shared", second_package, "Provider", second_package),
        ],
        &[],
    )
    .expect("same readable paths in distinct packages select their exact plans");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["first-plan".to_owned(), "second-plan".to_owned()]
    );
}

#[test]
fn provider_from_another_package_cannot_satisfy_the_selected_slot() {
    let boundary_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x71; 32])
        .expect("nonzero package identity");
    let provider_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x72; 32])
        .expect("nonzero package identity");
    let mut plan = selection_plan("provider-plan", &["choose"], &["choose"]);
    plan.schema.trait_package_identity = Some(boundary_package);
    plan.provider_type = "Provider".to_owned();
    plan.provider_type_package_identity = Some(boundary_package);

    let diagnostics = select_provider_plans(
        &[plan],
        target::NativeTarget::host(),
        &[],
        &[package_selection(
            "Pair",
            boundary_package,
            "Provider",
            provider_package,
        )],
    )
    .expect_err("same provider spelling from another package is not the selected identity");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("no candidate exists in the loaded dependency closure")
    }));
}

#[test]
fn selection_does_not_fall_back_to_a_boundary_slot_leaf() {
    let mut first = selection_plan("FirstProvider", &["choose"], &["choose"]);
    first.schema.trait_name = "first::Pick".to_owned();
    let mut second = selection_plan("SecondProvider", &["choose"], &["choose"]);
    second.schema.trait_name = "second::Pick".to_owned();

    let diagnostics = select_provider_plans(
        &[first, second],
        target::NativeTarget::host(),
        &[],
        &[crate::ProviderSelection::exact_for_test(
            "Pick",
            "FirstProvider",
        )],
    )
    .expect_err("a readable leaf is not a boundary identity");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("unknown boundary slot `Pick`") }),
        "expected an exact-identity diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn exact_slot_identity_does_not_select_a_qualified_same_leaf_slot() {
    let exact = selection_plan("ExactProvider", &["choose"], &["choose"]);
    let mut qualified = selection_plan("QualifiedProvider", &["choose"], &[]);
    qualified.schema.trait_name = "package::Pair".to_owned();

    let selected = select_provider_plans(
        &[exact, qualified],
        target::NativeTarget::host(),
        &[],
        &[crate::ProviderSelection::exact_for_test(
            "Pair",
            "ExactProvider",
        )],
    )
    .expect("only the exact canonical slot participates in the declaration");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["ExactProvider".to_owned()]
    );
}

#[test]
fn exact_provider_identity_does_not_select_a_qualified_same_leaf_provider() {
    let exact = selection_plan("exact-plan", &["choose"], &["choose"]);
    let mut qualified = selection_plan("qualified-plan", &["choose"], &["choose"]);
    qualified.provider_type = "package::exact-plan".to_owned();

    let selected = select_provider_plans(
        &[exact, qualified],
        target::NativeTarget::host(),
        &[],
        &[crate::ProviderSelection::exact_for_test(
            "Pair",
            "exact-plan",
        )],
    )
    .expect("only the exact canonical provider identity matches");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["exact-plan".to_owned()]
    );
}

#[test]
fn canonical_slot_resolution_catches_duplicate_selection_spellings() {
    let mut plan = selection_plan("FirstProvider", &["choose"], &["choose"]);
    plan.schema.trait_name = "package::Pick".to_owned();

    let diagnostics = select_provider_plans(
        &[plan],
        target::NativeTarget::host(),
        &[],
        &[
            crate::ProviderSelection::exact_for_test("package::Pick", "FirstProvider"),
            crate::ProviderSelection::exact_for_test("package::Pick", "SecondProvider"),
        ],
    )
    .expect_err("one canonical slot cannot be selected twice through aliases");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("provider selection for slot `package::Pick` more than once")),
        "expected canonical duplicate-slot diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn target_default_does_not_fall_back_to_a_boundary_slot_leaf() {
    let mut first = selection_plan("FirstProvider", &["choose"], &["choose"]);
    first.schema.trait_name = "first::Pick".to_owned();
    let mut second = selection_plan("SecondProvider", &["choose"], &["choose"]);
    second.schema.trait_name = "second::Pick".to_owned();

    let diagnostics = select_provider_plans(
        &[first, second],
        target::NativeTarget::host(),
        &[crate::ProviderSelection::exact_for_test(
            "Pick",
            "FirstProvider",
        )],
        &[],
    )
    .expect_err("a target default must name one canonical slot");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "target package selects provider `FirstProvider` for unknown boundary slot `Pick`"
            ))
    );
}

#[test]
fn explicit_selection_refuses_partial_provider() {
    let plans = vec![selection_plan(
        "PartialProvider",
        &["first", "second"],
        &["first"],
    )];
    let diagnostics = select_provider_plans(
        &plans,
        target::NativeTarget::host(),
        &[],
        &[crate::ProviderSelection::exact_for_test(
            "Pair",
            "PartialProvider",
        )],
    )
    .expect_err("selection never manufactures missing rows");
    assert!(diagnostics[0].message.contains("is partial"));
}

#[test]
fn target_default_resolves_covering_ambiguity() {
    let plans = vec![
        selection_plan("FirstProvider", &["first"], &["first"]),
        selection_plan("SecondProvider", &["first"], &["first"]),
    ];
    let selected = select_provider_plans(
        &plans,
        target::NativeTarget::host(),
        &[crate::ProviderSelection::exact_for_test(
            "Pair",
            "FirstProvider",
        )],
        &[],
    )
    .expect("the selected target package supplies the slot default");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["FirstProvider".to_owned()]
    );
}

#[test]
fn duplicate_exact_target_defaults_do_not_conflict() {
    let mut plan = selection_plan("package-provider", &["first"], &["first"]);
    plan.provider_type = "package::FirstProvider".to_owned();
    let defaults = [
        crate::ProviderSelection::exact_for_test("Pair", "package::FirstProvider"),
        crate::ProviderSelection::exact_for_test("Pair", "package::FirstProvider"),
    ];
    let selected = select_provider_plans(
        &[plan.clone()],
        target::NativeTarget::host(),
        &defaults,
        &[],
    )
    .expect("duplicate declarations of one exact provider identity are one target default");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["package-provider".to_owned()]
    );
    let selected = select_derived_provider_plans(
        &[DerivedProviderPlan {
            plan,
            provenance: ProviderPlanProvenance {
                schema: ProviderSchemaDeclaration::BoundaryTrait(symbols::SymbolHandle::invalid()),
                provider_type: None,
                row_requirements: Vec::new(),
                row_realizations: Vec::new(),
                row_target_machine_origins: Vec::new(),
            },
        }],
        target::NativeTarget::host(),
        &defaults,
        &[],
    )
    .expect("selection carries every accepted duplicate default site");
    let [selected] = selected.as_slice() else {
        panic!("one selected provider")
    };
    let ProviderSelectionProvenance::TargetDefault(declarations) = &selected.selected_by else {
        panic!("target-default provenance")
    };
    assert_eq!(declarations.len(), 2);
}

#[test]
fn build_override_wins_over_target_default() {
    let plans = vec![
        selection_plan("FirstProvider", &["first"], &["first"]),
        selection_plan("SecondProvider", &["first"], &["first"]),
    ];
    let selected = select_provider_plans(
        &plans,
        target::NativeTarget::host(),
        &[crate::ProviderSelection::exact_for_test(
            "Pair",
            "FirstProvider",
        )],
        &[crate::ProviderSelection::exact_for_test(
            "Pair",
            "SecondProvider",
        )],
    )
    .expect("the build root owns the final slot choice");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["SecondProvider".to_owned()]
    );
}

#[test]
fn conflicting_target_defaults_are_loud() {
    let plans = vec![
        selection_plan("FirstProvider", &["first"], &["first"]),
        selection_plan("SecondProvider", &["first"], &["first"]),
    ];
    let diagnostics = select_provider_plans(
        &plans,
        target::NativeTarget::host(),
        &[
            crate::ProviderSelection::exact_for_test("Pair", "FirstProvider"),
            crate::ProviderSelection::exact_for_test("Pair", "SecondProvider"),
        ],
        &[],
    )
    .expect_err("a target has one default provider per slot");
    assert!(
        diagnostics[0]
            .message
            .contains("conflicting target-package defaults")
    );
}
