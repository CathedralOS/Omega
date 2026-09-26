use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::{
    checked_program, checked_program_result, typed_program, typed_program_from_source_map,
    typed_program_with_resolution,
};
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionTarget,
};

mod indexed_operators;
mod projected_receivers;

fn typed_root_binding_fixture(source: &str, toolchain: bool) -> typed_trees::TypedTrees {
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            std::path::PathBuf::from("<build-prelude>"),
            source.to_owned(),
            std::path::PathBuf::from("."),
            None,
            if toolchain {
                source::SourceOrigin::Toolchain
            } else {
                source::SourceOrigin::User
            },
        )
        .source_id;
    typed_program_from_source_map(sources, &[(source_id, source)])
}

#[test]
fn root_binding_has_no_value_context_or_ordinary_bind_call_exception() {
    let ordinary = "data Build {} machine bind() {} machine build(builder: &mut Build) { bind(); }";
    lower_typed_trees(
        typed_root_binding_fixture(ordinary, true),
        &CheckingRequest::settled(),
    )
    .expect("ordinary bind is an ordinary call");
    let source = "data Build {} machine build(builder: &mut Build) { let value: u64 = builder.roots.bind(Target::ProgramEntry, Product::start); }";
    assert!(
        lower_typed_trees(
            typed_root_binding_fixture(source, true),
            &CheckingRequest::settled()
        )
        .is_err(),
        "root binding cannot yield a value"
    );
    let source = "data Roots {} data Carrier { roots: Roots; } machine Roots::bind(&mut self) {} machine inspect(carrier: &mut Carrier) { carrier.roots.bind(Target::ProgramEntry, Product::start); }";
    let mut typed = typed_root_binding_fixture(source, true);
    let diagnostic = crate::authored_selections::finalize_checked_authored_selections(
        &mut typed,
        &checked_trees::CheckFacts::default(),
    )
    .expect_err("same-shaped ordinary receiver has no Build authority");
    assert!(diagnostic.message.contains("compiler-issued &mut Build"));
}

#[test]
fn root_binding_helper_mutation_preserves_caller_loan_exclusion() {
    for helper_body in [
        "builder.roots.bind(Target::ProgramEntry, Product::start);",
        "let delegated: &mut Build = &mut builder; delegated.roots.bind(Target::ProgramEntry, Product::start);",
        "retain(builder).roots.bind(Target::ProgramEntry, Product::start);",
        "let returned: &mut Build = retain(builder); returned.roots.bind(Target::ProgramEntry, Product::start);",
    ] {
        let source = format!(
            "data Build {{}} machine retain(builder: &mut Build) -> &mut Build {{ transition {{ _ -> (builder) }} }} machine configure(builder: &mut Build) {{ {helper_body} }} machine build(builder: &mut Build) {{ let alias: &mut Build = &mut builder; configure(builder); alias.roots.bind(Target::ProgramEntry, Product::start); }}"
        );
        let diagnostics = lower_typed_trees(
            typed_root_binding_fixture(&source, true),
            &CheckingRequest::settled(),
        )
        .expect_err("helper root binding must expose its exclusive receiver use to the caller");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("still active")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn root_binding_computed_index_operand_retains_its_collection_access() {
    let program = typed_root_binding_fixture(
        "data Build {} data ProductEntryRef [copy] {}
         machine entries(builder: &mut Build) -> [ProductEntryRef; 1] { [ProductEntryRef {}] }
         machine build(builder: &mut Build) {
             builder.roots.bind(Target::ProgramEntry, entries(builder)[0]);
         }",
        true,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("build machine");
    let state = &program.machine_states(machine)[0];
    let builder = program.state_parameters(state)[0].symbol;
    let typed_trees::statement::StatementNode::RootBinding(binding) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("root binding");
    };
    let mut segments = arena::Arena::new();
    let mut accesses = arena::Arena::new();
    let span = crate::borrow::accesses::collect_call_argument_accesses(
        &program,
        &mut segments,
        &mut accesses,
        &[binding.implementation_operand],
        state.symbol,
        0,
        machine.symbol,
    );
    assert!(
        accesses
            .span_or_empty(span)
            .iter()
            .any(|access| access.root_symbol == builder),
        "the computed collection still evaluates its Build argument"
    );
}

#[test]
fn root_binding_rejects_uninitialized_receiver_and_live_shared_loan() {
    let source = "data Build {} machine build(builder: &mut Build) { let alias: &mut Build; alias.roots.bind(Target::ProgramEntry, Product::start); }";
    let diagnostics = lower_typed_trees(
        typed_root_binding_fixture(source, true),
        &CheckingRequest::settled(),
    )
    .expect_err("a type annotation cannot manufacture a live Build loan");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("available loan authority")),
        "{diagnostics:?}"
    );

    let source = "data Build {} machine inspect(builder: &Build) {} machine build(builder: &mut Build) { let shared: &Build = &builder; builder.roots.bind(Target::ProgramEntry, Product::start); inspect(shared); }";
    let diagnostics = lower_typed_trees(
        typed_root_binding_fixture(source, true),
        &CheckingRequest::settled(),
    )
    .expect_err("binding must exclude a shared loan used afterwards");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("still active")),
        "{diagnostics:?}"
    );
}

#[test]
fn entry_record_types_do_not_finalize_implicit_state_captures() {
    let source = r#"
        data Packet { value: u32; }
        machine main() -> u32 {
            let packet: Packet = Packet { value: 300 };
            transition { _ -> inspect() }
            state inspect() -> u32 { packet.value }
        }
    "#;
    let mut typed = typed_program(source);
    let diagnostic = crate::authored_selections::finalize_checked_authored_selections(
        &mut typed,
        &checked_trees::CheckFacts::default(),
    )
    .expect_err("entry-local types cannot resolve a sibling state's receiver");
    assert!(
        diagnostic.message.contains("remained unresolved"),
        "{diagnostic:?}"
    );
}

#[test]
fn successful_checking_finalizes_nominal_calls_in_proof_owned_expressions() {
    let source = r#"
        data Packet [copy] { value: u32; }

        machine Packet::attached_ready(&self) -> bool { true }
        machine Packet::qualified_ready(value: Packet) -> bool { true }

        proposition attached(packet: Packet) = packet.attached_ready();
        proposition qualified(packet: Packet) = Packet::qualified_ready(packet);

        domain Packet::Ready
        requires
            self.attached_ready(),
            Packet::qualified_ready(self);

        machine Packet::accept(&self)
        requires
            self.attached_ready(),
            Packet::qualified_ready(self)
        { }
    "#;
    let checked = checked_program(source);
    let selected = checked
        .authored_declaration_selections()
        .iter()
        .filter_map(|selection| match selection.target() {
            AuthoredDeclarationSelectionTarget::Resolved(target)
                if selection.kind() == AuthoredDeclarationSelectionKind::Call =>
            {
                Some(target.selected_symbol())
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    for state_name in ["attached_ready", "qualified_ready"] {
        let state = checked
            .machines()
            .iter()
            .flat_map(|machine| checked.machine_states(machine))
            .find(|state| state.name.as_str() == state_name)
            .expect("declared nominal call target");
        assert_eq!(
            selected
                .iter()
                .filter(|selected| **selected == state.symbol)
                .count(),
            3,
            "domain, proposition, and contract calls must select {state_name} exactly"
        );
    }
    assert!(checked.authored_declaration_selections().all_finalized());
}

#[test]
fn path_qualified_call_custody_rejects_a_same_named_target_from_another_owner() {
    let source = r#"
        data Packet [copy] { value: u32; }
        data Decoy [copy] { value: u32; }

        machine Decoy::is_ready(value: Packet) -> bool { true }

        domain Packet::Ready requires Packet::is_ready(self);
    "#;
    let diagnostic = checked_program_result(source)
        .expect_err("a same-named state under another nominal owner must not be selected");

    assert!(
        diagnostic.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("authored Call declaration selection")
                && diagnostic.message.contains("remained unresolved")
        }),
        "diagnostics={diagnostic:#?}"
    );
}

#[test]
fn unrelated_nominal_operator_does_not_capture_primitive_comparison() {
    let source = r#"
        data Quantity { value: i32; }
        domain Quantity::Ordered;
        operator < Quantity::Ordered::less(left: Quantity, right: Quantity) -> bool;
        machine bounded(value: u64) -> bool { value < 256 }
    "#;
    let typed = typed_program(source);
    let comparison = typed
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            matches!(node, typed_trees::expression::ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::Less)
            .then_some(handle)
        })
        .expect("primitive comparison");
    assert!(
        crate::authored_selections::typed_operator_authored_selection_candidates(
            &typed, comparison
        )
        .is_empty()
    );
    assert!(
        crate::authored_selections::typed_operator_has_no_authored_selection(&typed, comparison),
        "an unrelated same-spelled nominal operator cannot acquire a primitive occurrence"
    );
}

#[test]
fn endpoint_operator_candidates_retain_explicit_literal_carriers() {
    for (operator, endpoint, expected_candidates) in [
        (
            "operator % Quantity::remainder(left: Quantity, right: Quantity) -> Quantity;",
            "18446744073709551615u64 % 512u64",
            0,
        ),
        (
            "operator % u64::custom(left: u64, right: u64) -> u64;",
            "7u64 % 3u64",
            1,
        ),
        (
            "operator % u64::custom(left: u64, right: f64) -> u64;",
            "7u64 % 3",
            1,
        ),
        (
            "operator % Quantity::remainder(left: Quantity, right: Quantity) -> Quantity;",
            "18446744073709551616u64 % 18446744073709551616u64",
            1,
        ),
    ] {
        let source = format!(
            "data Quantity {{ value: u64; }}\n{operator}\nmachine bounded(value: u64[0..={endpoint}]) {{}}"
        );
        let typed = typed_program(&source);
        let expression = typed
            .expression_table
            .iter_expressions()
            .find_map(|(expression, node)| {
                matches!(node, typed_trees::expression::ExpressionNode::Binary(binary)
                    if binary.operator == typed_trees::expression::BinaryOperator::Modulo)
                .then_some(expression)
            })
            .expect("authored endpoint modulo");
        assert_eq!(
            crate::authored_selections::typed_operator_authored_selection_candidates(
                &typed, expression
            )
            .len(),
            expected_candidates,
            "{source}"
        );
        assert_eq!(
            crate::authored_selections::typed_operator_has_no_authored_selection(
                &typed, expression
            ),
            expected_candidates == 0,
            "{source}"
        );
        if expected_candidates == 0 {
            let checked = lower_typed_trees(typed, &CheckingRequest::settled())
                .expect("check primitive endpoint");
            assert!(checked.authored_declaration_selections().all_finalized());
        }
    }
}

#[test]
fn successful_checking_finalizes_inferred_field_members_and_primitive_operators() {
    let source = r#"
        data Build { freestanding: bool; }

        machine Build::configure(&mut self) {
            self.freestanding = false;
            let unchanged: bool = self.freestanding == false;
        }
    "#;
    let checked = checked_program(source);
    let selections = checked.authored_declaration_selections();

    assert!(selections.iter().any(|selection| {
        selection.kind() == AuthoredDeclarationSelectionKind::MemberAccess
            && matches!(
                selection.target(),
                AuthoredDeclarationSelectionTarget::Resolved(_)
            )
    }));
    assert!(selections.iter().any(|selection| {
        selection.kind() == AuthoredDeclarationSelectionKind::Operator
            && selection.target()
                == AuthoredDeclarationSelectionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                )
    }));
    assert!(selections.all_finalized(), "selections={selections:#?}");
}

#[test]
fn data_where_collection_measures_finalize_as_intrinsics() {
    let source = r#"
        data Buffer
        where
            used == storage.capacity,
        {
            used: u64;
            storage: [u8; 8];
        }
        data Packet
        where
            length == payload.len,
        {
            length: u64;
            payload: &[u8];
        }
    "#;
    let checked = checked_program(source);
    let selections = checked.authored_declaration_selections();

    for (member_name, intrinsic) in [
        (
            "capacity",
            AuthoredDeclarationSelectionIntrinsic::CollectionCapacity,
        ),
        (
            "len",
            AuthoredDeclarationSelectionIntrinsic::CollectionLength,
        ),
    ] {
        assert!(
            selections.iter().any(|selection| {
                selection.kind() == AuthoredDeclarationSelectionKind::MemberAccess
                    && selection.target()
                        == AuthoredDeclarationSelectionTarget::Intrinsic(intrinsic)
            }),
            "member `{member_name}` selections={selections:#?}"
        );
    }
    assert!(selections.all_finalized(), "selections={selections:#?}");
}

#[test]
fn successful_checking_finalizes_index_and_range_operator_occurrences() {
    let source = r#"
        proposition selected(value: i32);
        proposition window(values: &[i32]);
        machine inspect(values: [i32; 2])
        requires
            selected(values[0]),
            window(values[0..1])
        { }
    "#;
    let checked = checked_program(source);
    let operators = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Operator)
        .collect::<Vec<_>>();

    assert_eq!(operators.len(), 2, "selections={operators:#?}");
    assert!(operators.iter().all(|selection| {
        selection.target()
            == AuthoredDeclarationSelectionTarget::Intrinsic(
                AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
            )
    }));
    let indexed = checked
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, typed_trees::expression::ExpressionNode::Indexed(_))
                .then_some(expression)
        })
        .expect("checked program retains indexed expression");
    assert!(
        crate::authored_selections::typed_operator_has_no_authored_selection(&checked, indexed)
    );
    assert!(checked.authored_declaration_selections().all_finalized());
}

#[test]
fn successful_checking_retains_inferred_generic_call_conformance() {
    let source = r#"
        trait Marker { }
        data Good { }
        GoodMarker: Good satisfies Marker;

        machine accepts<T>(value: T) -> bool
        where T satisfies Marker
        {
            true
        }

        machine caller(value: Good) -> bool { accepts(value) }
    "#;
    let (resolved, typed) = typed_program_with_resolution(source);
    let selected = resolved
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "GoodMarker")
        })
        .expect("GoodMarker conformance")
        .symbol;
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");

    assert!(
        checked
            .authored_declaration_selections()
            .iter()
            .any(|selection| {
                selection.kind() == AuthoredDeclarationSelectionKind::Conformance
                    && matches!(
                        selection.target(),
                        AuthoredDeclarationSelectionTarget::Resolved(target)
                            if target.selected_symbol() == selected
                    )
            }),
        "selections={:#?}; specializations={:#?}",
        checked.authored_declaration_selections(),
        checked.machine_specializations,
    );
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| specialization.inferred_conformance_arguments == [selected])
        .expect("specialization retains its exact inferred conformance");
    assert!(
        specialization.conformance_arguments.is_empty(),
        "inferred conformance must not impersonate an explicit evidence argument"
    );
    assert!(checked.authored_declaration_selections().all_finalized());
}

#[test]
fn successful_checking_finalizes_attached_calls_through_parameter_fields() {
    let source = r#"
        domain [u8]::Path requires no_nul(self);
        data BuildSource {}
        data Build { source: BuildSource; }
        machine BuildSource::resolve<'path>(
            &self,
            relative: &'path [u8] in Path
        ) -> &'path [u8] in Path {
            relative
        }
        machine build(builder: &mut Build) {
            let resolved: &[u8] in Path = builder.source.resolve("input.txt");
        }
    "#;
    let checked = checked_program(source);
    assert!(
        checked
            .authored_declaration_selections()
            .iter()
            .any(|selection| matches!(
                selection.target(),
                AuthoredDeclarationSelectionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::ByteSequencePredicate(
                        language_semantics::byte_predicates::ByteSequencePredicate::NoNul,
                    )
                )
            ))
    );
    assert!(
        checked.authored_declaration_selections().all_finalized(),
        "selections={:#?}",
        checked.authored_declaration_selections()
    );
}

#[test]
fn successful_checking_binds_boundary_calls_through_parameter_fields() {
    let source = r#"
        boundary trait FilesystemHost {
            machine open(&self, path: &[u8], flags: i32) -> i32
            reaches FilesystemHost;
        }
        data Build<'s> { filesystem: &'s mut FilesystemHost; }
        machine build(builder: &mut Build)
        reaches FilesystemHost
        {
            let descriptor: i32 = builder.filesystem.open("input.txt", 0);
        }
    "#;
    let checked = checked_program(source);
    let call = checked
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "open" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("boundary call");
    let requirement = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "FilesystemHost")
        .and_then(|definition| checked.trait_machine_signatures(definition).first())
        .expect("filesystem requirement");
    assert_eq!(call.target_symbol, requirement.symbol);
}

#[test]
fn successful_checking_canonicalizes_local_selections_across_specializations() {
    let source = r#"
        data Light [copy] { weight: i32; }
        data Main { light: Light; number: i32; }
        machine Main::pick<T [copy]>(&self, value: &T) -> i32 {
            let selected: i32 = 7;
            transition { _ -> selected }
        }
        machine Main::use_both(&mut self) {
            let from_light: i32 = self.pick(&self.light);
            let from_number: i32 = self.pick(&self.number);
        }
    "#;
    let checked = checked_program(source);
    assert!(
        checked.authored_declaration_selections().all_finalized(),
        "selections={:#?}",
        checked.authored_declaration_selections()
    );
}

#[test]
fn public_conformance_rejects_private_header_declarations() {
    let source = r#"
        trait Shape {}
        data Circle {}
        pub CircleShape: Circle satisfies Shape;
    "#;
    let diagnostics = checked_program_result(source).expect_err("private header must reject");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("public conformance `CircleShape` exposes private data `Circle`"));
    assert!(rendered.contains("public conformance `CircleShape` exposes private trait `Shape`"));
}

#[test]
fn undeclared_contract_view_calls_finalize_as_proof_view_intrinsics() {
    let source = r#"
        machine carries(items: &[u64], before: &[u64])
        requires Bag(items) == Bag(before)
        ensures Bag(items) == Bag(before)
        {}
    "#;
    let typed = typed_program(source);
    let checked =
        lower_typed_trees(typed, &CheckingRequest::settled()).expect("admitted proof view checks");
    let proof_view_selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            selection.target()
                == AuthoredDeclarationSelectionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::ProofView,
                )
        })
        .count();
    assert_eq!(
        proof_view_selections,
        4,
        "each authored Bag call occurrence finalizes as a proof view: {:#?}",
        checked.authored_declaration_selections()
    );
    assert!(checked.authored_declaration_selections().all_finalized());
}

/// A managed-package quotient program: `TOTAL_DIRECT_DEFINE` from the
/// validation bridge tests beside the entry `omega --check` selects. Lowered
/// with a package identity so the proof-only bridge's hermetic identity rule
/// holds, and unmodified otherwise: every typed termination summary is
/// `NoGuarantee`, as on the compiler route.
fn managed_quotient_define_program() -> typed_trees::TypedTrees {
    const CORE_RELATION: &str = include_str!("../../../../../../source/library/core/relation.omg");
    const SOURCE: &str = r#"
use omega::language::core::relation;

data Representative {
    case Zero;
    case Next(previous: Representative);
}

proposition equivalent(a: Representative, b: Representative) = a == b;

machine equivalent_reflexive(a: Representative)
ensures a == a
{
}

machine equivalent_symmetric(a: Representative, b: Representative)
requires a == b
ensures b == a
{
}

machine equivalent_transitive(
    a: Representative,
    b: Representative,
    c: Representative
)
requires
    a == b
    b == c
ensures a == c
{
}

RepresentativeEquivalence: satisfies Equivalence<Representative, equivalent> {
    Reflexive::reflexive = equivalent_reflexive;
    Symmetric::symmetric = equivalent_symmetric;
    Transitive::transitive = equivalent_transitive;
}

data EquivalenceClass = Representative % equivalent
where equivalent satisfies
    Equivalence<Representative, equivalent>
    as RepresentativeEquivalence;

machine representative(value: Representative) -> Representative {
    value
}

machine representative_respects(left: Representative, right: Representative)
requires equivalent(left, right)
ensures equivalent(representative(left), representative(right))
{
}

machine admitted(value: EquivalenceClass) -> EquivalenceClass {
    Quotient::define<representative, representative_respects>(value)
}

data Main {
}

machine Main::main(&mut self) {
}
"#;
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x73; 32])
        .expect("nonzero package identity");
    let mut sources = source::SourceMap::default();
    let core_source_id = sources
        .add_with_metadata(
            std::path::PathBuf::from("source/library/core/relation.omg"),
            CORE_RELATION.to_owned(),
            std::path::PathBuf::from("source/library/core"),
            None,
            source::SourceOrigin::Toolchain,
        )
        .source_id;
    let source_id = sources
        .add_with_metadata(
            std::path::PathBuf::from("managed/quotient/main.omg"),
            SOURCE.to_owned(),
            std::path::PathBuf::from("managed/quotient"),
            Some(package),
            source::SourceOrigin::User,
        )
        .source_id;
    typed_program_from_source_map(
        sources,
        &[(core_source_id, CORE_RELATION), (source_id, SOURCE)],
    )
}

#[test]
fn a_sealed_quotient_request_call_resolves_as_a_proof_only_intrinsic() {
    let typed = managed_quotient_define_program();
    let request_span = typed
        .authored_declaration_selections()
        .iter()
        .find(|selection| {
            selection.kind() == AuthoredDeclarationSelectionKind::Call
                && selection.target()
                    == AuthoredDeclarationSelectionTarget::LateBound(
                        AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    )
        })
        .map(|selection| selection.source_span())
        .expect("the sealed request is a late-bound call selection");

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("the checked route admits the managed direct define after checked termination");
    let selections = checked.authored_declaration_selections();
    let request = selections
        .iter()
        .find(|selection| selection.source_span() == request_span)
        .expect("the request selection is retained");
    assert_eq!(
        request.target(),
        AuthoredDeclarationSelectionTarget::Intrinsic(
            AuthoredDeclarationSelectionIntrinsic::QuotientDefine
        )
    );
    assert!(selections.all_finalized());
    // Proof-only: the flow ledger keeps the request's occurrence (every call
    // expression has one) but it names no executable callee, so no checked
    // call target, value plan or Unit operation can arise from it.
    let request_expression = checked
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| {
            matches!(
                expression,
                typed_trees::expression::ExpressionNode::Call(call) if call.quotient_operation.is_some()
            )
            .then_some(handle)
        })
        .expect("the request expression is retained");
    for (_, state) in checked.facts.flow.control.states.iter() {
        for call in checked.facts.flow.control.calls.span_or_empty(state.calls) {
            let site = crate::semantic::calls::find_call_site(
                &checked.typed,
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            );
            if matches!(
                site,
                Some(crate::semantic::calls::CallSite::Expression { expression, .. })
                    if expression == request_expression
            ) {
                assert!(
                    !call.target_symbol.is_valid() && !call.receiver_symbol.is_valid(),
                    "the sealed request must not acquire an executable call target"
                );
            }
        }
    }
}

#[test]
fn builtin_arithmetic_on_a_min_call_finalizes_beside_a_declared_spelling() {
    // A declared `operator -` for an unrelated type makes every `-` a
    // late-bound authored selection. `min(x, 30) - 1` must still finalize as
    // the builtin operator: the builtin `min` call carries its operands'
    // carrier, and the literal is its anonymous peer.
    for expression in [
        "min(self.mid, 30) - 1",
        "max(self.mid, 0) - 1",
        "min(max(self.mid, 0), 30) - 1",
        "1 - min(self.mid, 30)",
        "min(self.mid, 30) + 1",
    ] {
        let source = format!(
            r#"
            data Quantity {{ value: i32; }}
            domain Quantity::Additive
            requires
                self.value >= 0;
            operator - Quantity::Additive::take(left: Quantity, right: Quantity) -> Quantity;

            data Main {{ hi: i32; mid: i32 [0..=10]; }}
            machine Main::main(&mut self) {{
                self.mid = 5;
                self.hi = {expression};
            }}
            "#
        );
        let unresolved = crate::tests::front_end::checked_program_result(&source)
            .err()
            .into_iter()
            .flatten()
            .filter(|diagnostic| diagnostic.message.contains("remained unresolved"))
            .collect::<Vec<_>>();
        assert!(unresolved.is_empty(), "{expression}: {unresolved:#?}");
    }
}
