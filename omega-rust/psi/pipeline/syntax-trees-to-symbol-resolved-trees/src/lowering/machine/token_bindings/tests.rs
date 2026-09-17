//! Token bindings survive resolution; bindings without a semantic home in their
//! operand tuple and duplicate owner-local shapes reject.

use crate::{ResolutionRequest, resolve};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::operator_spelling::OperatorSpelling;
use tokens_to_syntax_trees::parse_syntax_trees;

fn resolve_source(source: &str) -> Result<SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    resolve(ResolutionRequest::new(&syntax))
}

fn machine_spellings(program: &SymbolResolvedTrees) -> Vec<(String, Option<OperatorSpelling>)> {
    program
        .machines
        .iter()
        .map(|machine| (machine.name.to_string(), machine.spelling))
        .collect()
}

#[test]
fn token_bearing_machines_retain_their_spelling_and_named_machines_have_none() {
    let program = resolve_source(
        "data Vec2 { x: u64; y: u64; }
         pub machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 { left }
         machine [] Vec2::at(items: Vec2, index: u64) -> u64 { index }
         boundary machine == Vec2::equal(left: Vec2, right: Vec2) -> bool;
         machine - subtract(left: Vec2, right: u64) -> u64 { right }
         machine Vec2::length(items: Vec2) -> u64 { 0u64 }
         machine ordinary(value: u64) -> u64 { value }",
    )
    .expect("token-bearing machines resolve like named machines");
    assert_eq!(
        machine_spellings(&program),
        [
            ("Vec2::add".to_owned(), Some(OperatorSpelling::Add)),
            ("Vec2::at".to_owned(), Some(OperatorSpelling::Index)),
            ("subtract".to_owned(), Some(OperatorSpelling::Subtract)),
            ("Vec2::length".to_owned(), None),
            ("ordinary".to_owned(), None),
        ]
    );
    // The owner check compares settled identities: the attached owner and the
    // operand types carry symbols, never only their spelling.
    let add = program.machines.iter().next().expect("attached machine");
    assert!(add.attached_data_symbol.is_valid());
    let entry = program.machine_state(program.machine_state_handles(add.states)[0]);
    for parameter in program.state_parameters(entry.parameters) {
        let symbol_resolved_trees::types::TypeReference::Named { symbol, .. } =
            &parameter.type_reference
        else {
            panic!("named operand type");
        };
        assert!(symbol.is_valid(), "{}", parameter.name);
    }
    // A bodyless token-bearing `boundary machine` is the required operator
    // slot itself, not a machine: it lowers to the boundary operator
    // declaration every provider route is keyed on.
    let equal = program
        .operators
        .iter()
        .next()
        .expect("boundary operator slot");
    assert!(equal.is_boundary);
    assert_eq!(equal.spelling, Some(OperatorSpelling::Equal));
    assert_eq!(
        program
            .operator_path_members(equal.name)
            .iter()
            .map(|member| member.as_str().to_owned())
            .collect::<Vec<_>>(),
        ["Vec2", "equal"]
    );
}

#[test]
fn same_owner_token_and_operand_shape_rejects_at_the_second_declaration() {
    let source = "data Vec2 { x: u64; y: u64; }
         machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 { left }
         machine + Vec2::plus(left: Vec2, right: Vec2) -> Vec2 { right }";
    let diagnostics = resolve_source(source).expect_err("duplicate binding rejects");
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("one diagnostic per duplicate: {diagnostics:?}");
    };
    assert!(
        diagnostic.message.contains(
            "`Vec2::plus` binds the fixed operator token `+` already bound by `Vec2::add`"
        ),
        "{}",
        diagnostic.message
    );
    let span = diagnostic
        .source_span
        .expect("reported at the second declaration");
    assert_eq!(
        &source[span.span.start..span.span.end],
        "Vec2::plus",
        "the diagnostic points at the second declaration's name"
    );
}

#[test]
fn free_machines_in_one_scope_share_the_owner_check() {
    let diagnostics = resolve_source(
        "data Vec2 { x: u64; y: u64; }
         machine + add(left: Vec2, right: Vec2) -> u64 { 0u64 }
         machine + plus(left: Vec2, right: Vec2) -> u64 { 1u64 }",
    )
    .expect_err("root-scope free machines share one owner");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("`plus` binds the fixed operator token `+` already bound by `add`"),
        "{}",
        diagnostics[0].message
    );
}

#[test]
fn attached_binding_whose_operands_omit_its_home_rejects_at_the_declaration() {
    let source = "data Wrapped { value: u8; }
         machine + Wrapped::add(left: u8, right: u8) -> u64 { 0u64 }";
    let diagnostics = resolve_source(source).expect_err("foreign family injection rejects");
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("one diagnostic: {diagnostics:?}");
    };
    assert!(
        diagnostic.message.contains(
            "`Wrapped::add` binds the fixed operator token `+` but no operand names its \
             semantic home `Wrapped`"
        ),
        "{}",
        diagnostic.message
    );
    let span = diagnostic.source_span.expect("reported at the declaration");
    assert_eq!(&source[span.span.start..span.span.end], "Wrapped::add");
}

#[test]
fn free_binding_over_bare_primitives_rejects_at_the_declaration() {
    let diagnostics = resolve_source("machine + add(left: u8, right: u8) -> u64 { 0u64 }")
        .expect_err("a primitive family has no declaration-owned home");
    let [diagnostic] = diagnostics.as_slice() else {
        panic!("one diagnostic: {diagnostics:?}");
    };
    assert!(
        diagnostic.message.contains(
            "`add` binds the fixed operator token `+` over operands that name no declared \
             type or domain"
        ),
        "{}",
        diagnostic.message
    );
}

#[test]
fn a_home_anywhere_in_the_operand_tuple_participates() {
    let program = resolve_source(
        "data Wrapped { value: u8; }
         data Pair<T> { first: T; second: T; }
         domain u8::Level;
         machine + Wrapped::scale(factor: u64, value: &Wrapped) -> u64 { factor }
         machine * Wrapped::spread(items: [Wrapped], factor: u64) -> u64 { factor }
         machine - Wrapped::inside(pair: Pair<Wrapped>, factor: u64) -> u64 { factor }
         machine + add(left: u8 in Level, right: u8) -> u64 { 0u64 }
         machine - subtract(left: &Wrapped, right: u64) -> u64 { right }",
    )
    .expect("a reference, element, generic argument, or domain constraint names the home");
    assert_eq!(program.machines.len(), 5);
}

#[test]
fn distinct_operand_shapes_may_share_a_token() {
    let program = resolve_source(
        "data Vec2 { x: u64; y: u64; }
         machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 { left }
         machine + Vec2::scale(left: Vec2, factor: u64) -> Vec2 { left }
         machine + Vec2::add_borrowed(left: &Vec2, right: &Vec2) -> u64 { 0u64 }
         machine + Vec2::add_mutable(left: &mut Vec2, right: &Vec2) -> u64 { 0u64 }",
    )
    .expect("overloads by operand shape are not duplicates");
    assert_eq!(program.machines.len(), 4);
}

#[test]
fn distinct_tokens_and_distinct_owners_are_not_duplicates() {
    let program = resolve_source(
        "data Vec2 { x: u64; y: u64; }
         data Vec3 { x: u64; y: u64; z: u64; }
         machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 { left }
         machine - Vec2::subtract(left: Vec2, right: Vec2) -> Vec2 { left }
         machine + Vec3::add(left: Vec3, right: Vec3) -> Vec3 { left }
         machine + add(left: Vec2, right: Vec2) -> Vec2 { left }",
    )
    .expect("a different token, owner, or free scope is a separate binding");
    assert_eq!(program.machines.len(), 4);
}

#[test]
fn a_named_sibling_never_collides_with_a_token_binding() {
    let program = resolve_source(
        "data Vec2 { x: u64; y: u64; }
         machine + Vec2::add(left: Vec2, right: Vec2) -> Vec2 { left }
         machine Vec2::plus(left: Vec2, right: Vec2) -> Vec2 { right }",
    )
    .expect("a tokenless machine binds no token");
    assert_eq!(
        machine_spellings(&program),
        [
            ("Vec2::add".to_owned(), Some(OperatorSpelling::Add)),
            ("Vec2::plus".to_owned(), None),
        ]
    );
}

#[test]
fn domain_attached_binding_homes_in_the_carrier_and_marks_the_domain_semantic() {
    let program = resolve_source(
        "data Quantity { value: i32; }
         domain Quantity::Additive requires self.value >= 0;
         domain Quantity::Plain requires self.value >= 0;
         machine + Quantity::Additive::add(left: Quantity, right: Quantity) -> Quantity { left }",
    )
    .expect("a domain-attached binding whose operands name the carrier resolves");
    let roles = program
        .domain_definitions
        .iter()
        .map(|domain| {
            (
                domain.name.to_string(),
                domain.semantic_roles.denotation_dimension.is_some(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        roles,
        [
            ("Quantity::Additive".to_owned(), true),
            ("Quantity::Plain".to_owned(), false),
        ],
        "only the domain owning a token binding gains the denotation role"
    );

    let diagnostics = resolve_source(
        "data Quantity { value: i32; }
         data Other { value: i32; }
         domain Quantity::Additive requires self.value >= 0;
         machine + Quantity::Additive::add(left: Other, right: Other) -> Other { left }",
    )
    .expect_err("operands must name the home domain's carrier");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "`Quantity::Additive::add` binds the fixed operator token `+` but no operand names the carrier of its home domain `Quantity::Additive`"
        )),
        "{diagnostics:?}"
    );

    let diagnostics = resolve_source(
        "data Quantity { value: i32; }
         domain Quantity::Additive requires self.value >= 0;
         machine + Quantity::Additive::add(left: Quantity, right: Quantity) -> Quantity { left }
         machine + Quantity::Additive::plus(left: Quantity, right: Quantity) -> Quantity { right }",
    )
    .expect_err("the domain is the owner for the duplicate-shape check");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "`Quantity::Additive::plus` binds the fixed operator token `+` already bound by `Quantity::Additive::add`"
        )),
        "{diagnostics:?}"
    );
}

#[test]
fn generic_domain_binding_homes_through_qualified_operands_and_index_arguments_shape() {
    // `domain<T, const U: Unit> T::Quantity<U>` classifies an open carrier, so
    // an operand participates by carrying the domain; distinct index
    // arguments are distinct operand shapes under the one owner.
    let source = "data Unit [copy] { scale: i32; }
         data Units {}
         const Units::METER: Unit = Unit { scale: 1 };
         const Units::KILOMETER: Unit = Unit { scale: 1000 };
         domain<T, const U: Unit> T::Quantity<U>;
         machine + Quantity::add_meters(
             left: f64 in Quantity<Units::METER>,
             right: f64 in Quantity<Units::METER>
         ) -> f64 in Quantity<Units::METER> { left }
         machine + Quantity::add_kilometers(
             left: f64 in Quantity<Units::KILOMETER>,
             right: f64 in Quantity<Units::KILOMETER>
         ) -> f64 in Quantity<Units::KILOMETER> { left }";
    resolve_source(source).expect("qualified operands home in the generic domain");

    let diagnostics = resolve_source(
        "data Unit [copy] { scale: i32; }
         domain<T, const U: Unit> T::Quantity<U>;
         machine + Quantity::add(left: f64, right: f64) -> f64 { left }",
    )
    .expect_err("bare carrier operands name no home of a generic domain");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("no operand names the carrier of its home domain `Quantity`")),
        "{diagnostics:?}"
    );

    let diagnostics = resolve_source(&source.replace(
        "Quantity<Units::KILOMETER>,\n             right: f64 in Quantity<Units::KILOMETER>",
        "Quantity<Units::METER>,\n             right: f64 in Quantity<Units::METER>",
    ))
    .expect_err("the same index arguments repeat the first binding's shape");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "`Quantity::add_kilometers` binds the fixed operator token `+` already bound by `Quantity::add_meters`"
        )),
        "{diagnostics:?}"
    );
}

#[test]
fn token_bearing_boundary_signature_is_the_operator_slot_of_the_operator_spelling() {
    let operator_form = "data CheckedMath {}
         boundary operator - CheckedMath::select_left(left: i32, right: i32) -> i32
         requires left >= right;
         data Provider {}
         machine Provider::select_left_impl(left: i32, right: i32) -> i32
         satisfies CheckedMath::select_left
         { transition { _ -> left } }";
    let machine_form = operator_form.replace(
        "boundary operator - CheckedMath::select_left",
        "boundary machine - CheckedMath::select_left",
    );
    let operator_program = resolve_source(operator_form).expect("operator spelling resolves");
    let machine_program = resolve_source(&machine_form).expect("machine spelling resolves");
    let mut operator_snapshot = operator_program.snapshot();
    let mut machine_snapshot = machine_program.snapshot();
    // The machine head has no operator token count; everything else about
    // the slot, its realization, and the rest of the program is identical.
    for snapshot in [&mut operator_snapshot, &mut machine_snapshot] {
        for operator in &mut snapshot.roots.operators {
            operator.token_count = 0;
        }
    }
    assert_eq!(operator_snapshot, machine_snapshot);
    assert_eq!(machine_program.operators.iter().count(), 1);
    assert_eq!(machine_program.machines.iter().count(), 1);
    let slot = machine_program.operators.iter().next().expect("slot");
    assert!(slot.is_boundary);
    assert_eq!(slot.spelling, Some(OperatorSpelling::Subtract));
    assert_eq!(
        machine_program
            .operator_path_members(slot.name)
            .iter()
            .map(|member| member.as_str().to_owned())
            .collect::<Vec<_>>(),
        ["CheckedMath", "select_left"]
    );

    let diagnostics = resolve_source(
        "data CheckedMath {}
         boundary trait Host {}
         boundary machine - CheckedMath::select_left(left: i32, right: i32) -> i32
         reaches Host;",
    )
    .expect_err("machine-only clauses do not belong to a token-bearing boundary signature");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "`CheckedMath::select_left` is a token-bearing boundary signature and declares only its signature and contracts; `reaches` belongs to a named `boundary requirement`"
        )),
        "{diagnostics:?}"
    );
}

#[test]
fn bare_bodyless_signatures_are_catalog_primitives_or_reject() {
    // No source map: nothing is the sealed toolchain source, so the catalog
    // spelling is refused by custody and an ordinary name by the body rule.
    let diagnostics = resolve_source(
        "data FloatMeaning { value: u64; }
         machine Float::meaning32(value: f32) -> FloatMeaning;",
    )
    .expect_err("a lookalike outside the sealed toolchain source grants no primitive");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "`Float::meaning32` names a compiler primitive, but only the sealed toolchain declaration supplies it; merely naming a declaration `Float::meaning32` grants no primitive"
        )),
        "{diagnostics:?}"
    );
    let diagnostics = resolve_source(
        "data Plain { value: u64; }
         machine Plain::describe(value: u64) -> Plain;",
    )
    .expect_err("a bodyless nonboundary machine outside the catalog needs a body");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "`Plain::describe` has no body and is neither a boundary signature nor a compiler-catalog primitive"
        )),
        "{diagnostics:?}"
    );
}
