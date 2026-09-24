fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    typed_trees_to_checked_trees::lower_typed_trees(
        typed(source)?,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
}

fn typed(source: &str) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(
            std::path::PathBuf::from("generic_copy_getter.omg"),
            source.to_owned(),
        )
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let syntax = syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
        syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest::new(syntax),
    )?;
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
        },
    )?;
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])
}

#[test]
fn method_copy_requirement_allows_borrowed_generic_field_read() {
    let checked = check(
        "data Counter<T> { value: T; }
        machine Counter::current<T [copy]>(&self) -> T { self.value }",
    )
    .expect("the getter uses its own declared copy requirement");
    let owner = &checked.data_definitions()[0];
    let method = &checked.machines()[0];
    assert_eq!(
        checked.data_type_parameters(owner)[0].bounds.multiplicity,
        language_semantics::Multiplicity::Affine
    );
    assert_eq!(
        checked.machine_type_parameters(method)[0]
            .bounds
            .multiplicity,
        language_semantics::Multiplicity::Unrestricted
    );
    assert_ne!(
        checked.data_type_parameters(owner)[0].symbol,
        checked.machine_type_parameters(method)[0].symbol
    );
}

#[test]
fn method_without_copy_requirement_cannot_move_borrowed_generic_field() {
    assert!(
        check(
            "data Counter<T> { value: T; }
        machine Counter::current<T>(&self) -> T { self.value }"
        )
        .is_err()
    );
}

#[test]
fn selected_method_checks_unused_copy_requirement() {
    let source = "data Item { value: u32; }
        data Counter<T> { value: T; }
        machine Counter::count<T [copy]>(&self) -> u32 { 7 }
        machine observe(counter: &Counter<Item>) -> u32 { counter.count() }";
    let diagnostics =
        check(source).expect_err("specialization cannot erase an unused method requirement");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not satisfy its authored type bounds")),
        "{diagnostics:?}"
    );
}

#[test]
fn uncalled_method_requirement_does_not_constrain_container() {
    check(
        "data Item { value: u32; }
        data Counter<T> { value: T; }
        machine Counter::count<T [copy]>(&self) -> u32 { 7 }
        data Main { counter: Counter<Item>; }
        machine Main::main(&self) -> u32 { 0 }",
    )
    .expect("an uncalled method does not strengthen the data declaration");
}

#[test]
fn uncalled_generated_method_does_not_select_its_constrained_callee() {
    check(
        "data Item { value: u32; }
        data Counter<T> { value: T; }
        machine Counter::first<T [copy]>(&self) -> u32 { self.second() }
        machine Counter::second<T [copy]>(&self) -> u32 { 7 }
        machine observe(counter: &Counter<Item>) -> u32 { 0 }",
    )
    .expect("unused generated bodies cannot demand another method's requirements");
}

#[test]
fn method_copy_requirement_projects_nested_fields_and_elements() {
    for (declarations, field, expression) in [
        ("", "value: T;", "value"),
        (
            "data Inner<T> { value: T; }",
            "inner: Inner<T>;",
            "self.inner.value",
        ),
        ("", "values: [T; 2];", "self.values[0]"),
    ] {
        check(&format!(
            "{declarations} data Counter<T> {{ {field} }}
            machine Counter::current<T [copy]>(&self) -> T {{ {expression} }}"
        ))
        .unwrap_or_else(|errors| panic!("{expression}: {errors:?}"));
    }
}

#[test]
fn owner_copy_requirement_survives_method_attachment() {
    check(
        "data Counter<T [copy]> { value: T; }
        machine Counter::current<T>(&self) -> T { self.value }",
    )
    .expect("the receiver's existing owner requirement remains available");
}

#[test]
fn concrete_getter_calls_retain_distinct_owner_applications() {
    check(
        "data Counter<T> { value: T; }
        machine Counter::current<T [copy]>(&self) -> T { self.value }
        data Main { number: Counter<u32>; selected: Counter<bool>; }
        machine Main::main(&self) -> u32 {
            let observed_number: u32 = self.number.current();
            let observed_selection: bool = self.selected.current();
            transition observed_selection { true -> (observed_number) false -> (0) }
        }",
    )
    .expect("two concrete applications retain their own method argument types");
}

#[test]
fn receiverless_attachment_cannot_inherit_owner_copy_permission() {
    assert!(
        check(
            "data Counter<T [copy]> { value: T; }
        machine Counter::repeat<T>(input: T) -> T {
            let saved: T = input;
            input
        }"
        )
        .is_err(),
        "an attachment name alone supplies no receiver premise"
    );
}
