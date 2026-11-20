use super::*;

use source_text::SourceId;

const SOURCE: SourceId = SourceId(0);

fn parse(text: &str) -> SyntaxTrees {
    let tokens = crate::frontend_fixture::Lexer::new(text)
        .tokenize()
        .unwrap();
    crate::frontend_fixture::parse_syntax_trees_with_id(SOURCE, &tokens).unwrap()
}

/// The stage `stage` of `text`: how many stages the entry has, how many
/// statements this one keeps, and how many nested machines.
fn stage(text: &str, stage: usize) -> Result<(usize, usize, usize), String> {
    let mut syntax = parse(text);
    let name = text.find("machine build").unwrap() + "machine ".len();
    let selected_entry = SourceSpan::new(
        SOURCE,
        Span {
            start: name,
            end: name + "build".len(),
        },
    );
    let selected = select_build_stage(&mut syntax, selected_entry, stage).map_err(|refusals| {
        refusals
            .into_iter()
            .map(|refusal| refusal.message)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let (_, machine, state) = entry(&syntax, selected_entry).unwrap();
    Ok((
        selected.program_ends.len() + 1,
        syntax.items.state(state).statements.len(),
        machine.nested_machines.len(),
    ))
}

#[test]
fn a_stage_uses_the_selected_declaration_occurrence_in_its_file() {
    let text = "machine build(builder: @mut Build) {}\nmachine build(builder: @mut Build) suspends { suspend Build::prepare_dependency(builder, registration); }";
    let mut syntax = parse(text);
    let name = text.rfind("machine build").unwrap() + "machine ".len();
    let occurrence = SourceSpan::new(
        SOURCE,
        Span {
            start: name,
            end: name + 5,
        },
    );
    let stage = select_build_stage(&mut syntax, occurrence, 0).unwrap();
    let [site] = stage.preparations() else {
        panic!("selected one preparation")
    };
    assert_eq!(&text[site.start..site.end], "prepare_dependency");
    let mut syntax = parse(text);
    assert!(select_build_stage(&mut syntax, SourceSpan::default(), 0).is_err());
}

const STAGED: &str = "
machine build(builder: @mut Build) suspends {
    Build::application(builder, \"app\");
    let first: DependencyRegistration = Build::build_dependency(builder, one());
    suspend Build::prepare_dependency(builder, first);
    let second: DependencyRegistration = Build::build_dependency(builder, two());
    suspend Build::prepare_dependency(builder, second);
    let seven: i32 = kit\\v();
    transition seven == 7 {
        true -> done(builder),
        _ -> wrong()
    }
    machine done(builder: @mut Build) { }
    machine wrong() { }
}
";

/// Each top-level wait ends a stage. The nested machines and the closing
/// dispatch stay with the last one: its `let` and the dispatch statement that owns both arms.
#[test]
fn an_entry_is_divided_at_each_top_level_preparation() {
    assert_eq!(stage(STAGED, 0), Ok((3, 3, 0)));
    assert_eq!(stage(STAGED, 1), Ok((3, 2, 0)));
    assert_eq!(stage(STAGED, 2), Ok((3, 2, 2)));
    assert!(stage(STAGED, 3).is_err());
}

/// The twins: an entry with no wait, and one whose only wait is inside a
/// nested machine, are one stage and are left whole.
#[test]
fn an_entry_with_no_top_level_preparation_is_one_stage() {
    let plain = "
machine build(builder: @mut Build) {
    Build::application(builder, \"app\");
    let seven: i32 = 7;
}
";
    assert_eq!(stage(plain, 0), Ok((1, 2, 0)));
    assert!(stage(plain, 1).is_err());
    let nested = "
machine build(builder: @mut Build) suspends {
    let registered: DependencyRegistration = Build::build_dependency(builder, one());
    suspend wait(builder, registered);
    machine wait(builder: @mut Build, registered: DependencyRegistration) suspends {
        suspend Build::prepare_dependency(builder, registered);
    }
}
";
    assert_eq!(stage(nested, 0), Ok((1, 2, 1)));
}

/// A read, projection or loan after a wait belongs to the frame that
/// established the local. The ordinary checker sees both sides of its use.
#[test]
fn a_local_used_after_preparation_stays_in_the_same_checked_program() {
    for used in ["before", "before.alias", "pick(@before)"] {
        let text = format!(
            "
machine build(builder: @mut Build) suspends {{
    let before: DependencyDescription = one();
    let registered: DependencyRegistration = Build::build_dependency(builder, two());
    suspend Build::prepare_dependency(builder, registered);
    let after: i32 = {used};
}}
"
        );
        assert_eq!(stage(&text, 0), Ok((1, 4, 0)));
    }
}

#[test]
fn the_next_program_restores_the_home_and_its_loan_before_the_first_authored_use() {
    let text = r#"
machine build(builder: @mut Build) suspends {
    let amount: i32 = 7;
    let held: @i32 = @amount;
    let registered: DependencyRegistration = Build::build_dependency(builder, request());
    suspend Build::prepare_dependency(builder, registered);
    let saved: i32 = $held;
    builder.tests.disabled = saved == 7;
    kit\configure(builder);
}
"#;
    assert_eq!(stage(text, 0), Ok((3, 5, 0)));
    assert_eq!(stage(text, 1), Ok((3, 5, 0)));
}

#[test]
fn preparation_preserves_the_lexical_scope_of_earlier_locals() {
    for declaration in [
        "let value: i32 = 2;",
        "let result: i32 = match 2 { value -> 7 };",
        "transition 2 { value -> done() }",
    ] {
        let text = format!(
            r#"
machine build(builder: @mut Build) suspends {{
    let value: i32 = 1;
    let registered: DependencyRegistration = Build::build_dependency(builder, one());
    suspend Build::prepare_dependency(builder, registered);
    {declaration}
    machine done() {{}}
}}
"#
        );
        let refusal = stage(&text, 0).unwrap_err();
        assert!(
            refusal.contains("binding `value` conflicts with a local still visible"),
            "{refusal}"
        );
    }
    let disjoint = r#"
machine build(builder: @mut Build) suspends {
    let first: i32 = match 1 { item -> 7 };
    let registered: DependencyRegistration = Build::build_dependency(builder, one());
    suspend Build::prepare_dependency(builder, registered);
    let second: i32 = match 2 { item -> 7 };
}
"#;
    assert_eq!(stage(disjoint, 1), Ok((2, 1, 0)));
}

/// The entry's nested machines are checked with the last stage, so an
/// earlier stage cannot name one.
#[test]
fn a_stage_before_the_last_cannot_name_a_nested_machine() {
    let text = "
machine build(builder: @mut Build) suspends {
    helper(builder);
    let registered: DependencyRegistration = Build::build_dependency(builder, one());
    suspend Build::prepare_dependency(builder, registered);
    helper(builder);
    machine helper(builder: @mut Build) { }
}
";
    let refusal = stage(text, 0).unwrap_err();
    assert!(
        refusal.starts_with("not supported yet:") && refusal.contains("`helper`"),
        "{refusal}"
    );
    assert_eq!(
        refusal.lines().count(),
        1,
        "the call after the wait is fine"
    );
}

#[test]
fn each_wait_saves_only_the_homes_needed_by_its_successor() {
    let source = r#"
machine build(builder: @mut Build) suspends {
    let first: i32 = 3;
    let held: @i32 = @first;
    let one: DependencyRegistration = Build::build_dependency(builder, request());
    suspend Build::prepare_dependency(builder, one);
    builder.tests.disabled = $held == 3;
    let second: i32 = 7;
    let two: DependencyRegistration = Build::build_dependency(builder, request());
    suspend Build::prepare_dependency(builder, two);
    builder.tests.disabled = second == 7;
}
"#;
    let name = source.find("build(").unwrap();
    let entry = SourceSpan::new(SOURCE, Span::new(name, name + "build".len()));
    let select = |index| select_build_stage(&mut parse(source), entry, index).unwrap();
    let first_wait = select(0);
    let second_wait = select(first_wait.next_index().unwrap());
    let finished = select(second_wait.next_index().unwrap());
    let [first, second] = *first_wait.frame_homes() else {
        panic!("the two locals own the shared entry parameters");
    };
    assert_eq!(&source[first.start..first.end], "first");
    assert_eq!(&source[second.start..second.end], "second");
    assert_eq!(first_wait.frame_homes(), second_wait.frame_homes());
    assert!(first_wait.saves_frame_home(first));
    assert!(!first_wait.saves_frame_home(second));
    assert!(!second_wait.saves_frame_home(first));
    assert!(second_wait.saves_frame_home(second));
    assert!(!finished.saves_frame_home(first));
    assert!(!finished.saves_frame_home(second));
}
