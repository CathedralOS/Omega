//! Lowering must read the field typing selected for each place of a
//! positional pattern, and nothing else. These tests keep the source fixed,
//! change one selection as a faulty typer would, and require the lowered
//! read to change with it.
//!
//! The spec rule is "A positional pattern binds fields by position; binding
//! names need not match declaration names" (`wiki/spec/language/patterns.md`,
//! "Data and case decomposition"). The program and what typing hands over
//! are described and asserted in
//! `03_symbol-resolved-trees-to-typed-trees/tests/positional_patterns.rs`.
//!
//! `PreparedTypedTrees` cannot be built or edited outside stage 04. Each
//! altered product here is made by taking the prepared trees back with
//! `into_inner`, changing one fact, and running stage 04 again, so lowering
//! still receives a product only stage 04 could issue.

#[path = "../../03_symbol-resolved-trees-to-typed-trees/tests/support/positional_fields.rs"]
mod program;

use program::{FieldRead, MACHINES, MAIN, PACKAGE, field, field_reads, typed};
use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::representations::source_state::TypedTrees;
use source_compiler::representations::source_state::expression::{ExpressionNode, MemberSelection};
use source_compiler::test_support::lowering::{self, Elaboration};
use source_compiler::test_support::preparation::{self, PreparedTypedTrees};
use source_compiler::test_support::{
    SourcePreparationRequest, TerminalSourceCompilation, prepare_source_for_test,
};
use source_provenance::DependencyScope;
use symbols::SymbolHandle;
use terminal_check_tables::Refusal;
use terminal_check_tables::compilation::TerminalCompilation;
use terminal_psi::{OperationKind, OperationResult, StructuralPathSegment, Terminator};

struct Prepared {
    program: PreparedTypedTrees,
    entries: Vec<SymbolHandle>,
    subjects: [Option<PackageKeyIdentity>; 1],
}

impl Prepared {
    fn compilation(&self) -> TerminalSourceCompilation<'_> {
        compilation(&self.entries, &self.subjects)
    }
}

fn compilation<'a>(
    entries: &'a [SymbolHandle],
    subjects: &'a [Option<PackageKeyIdentity>],
) -> TerminalSourceCompilation<'a> {
    TerminalSourceCompilation {
        declaration_metadata: &[],
        entries,
        subjects,
        purpose: DependencyScope::Product,
        selected: &[],
        bindings: &[],
        calling: None,
        providers: None,
        supplies: &[],
    }
}

/// The program after the lexer, parser, resolver, typer and stage 04, with
/// its four machines as the request's entries.
fn prepare() -> Prepared {
    let (_, typed) = typed(MAIN).unwrap();
    let entries: Vec<_> = MACHINES
        .iter()
        .map(|name| typed.realized_machine_named(name).unwrap().symbol)
        .collect();
    let subjects = [Some(PackageKeyIdentity::from_digest(PACKAGE).unwrap())];
    let frozen = prepare_source_for_test(SourcePreparationRequest {
        program: typed,
        accounting: Default::default(),
        admission: Default::default(),
        compilation: compilation(&entries, &subjects),
    })
    .unwrap_or_else(|failure| panic!("preparation refused: {:?}", failure.diagnostics));
    Prepared {
        program: frozen.program,
        entries,
        subjects,
    }
}

/// Stage 05's lowered program for `program` under the fixture's request, or
/// its refusal.
fn lower(fixture: &Prepared, program: &PreparedTypedTrees) -> Result<Elaboration, Refusal> {
    let lowered = lowering::prepare_compilation(
        program,
        fixture.compilation(),
        terminal_codec::TerminalPublication::Portable,
    )
    .unwrap();
    let TerminalCompilation::Prepared(lowered) = lowered.program else {
        panic!("the program binds no callable shape that could fail to form")
    };
    lowered.closure
}

/// The field each machine returns, by the field's declared name, for
/// `by_place`, `with_rest`, `in_arm` and `by_name` in that order. Every
/// machine must read exactly one scalar from its `Pair` parameter, through a
/// path of one field step, and return that scalar.
fn returned_fields(lowered: &Elaboration, trees: &TypedTrees) -> [String; 4] {
    MACHINES.map(|name| {
        let machine = lowered
            .module
            .machines
            .iter()
            .find(|machine| {
                trees
                    .symbols
                    .name(*lowered.machines.get(machine.id).unwrap())
                    == name
            })
            .unwrap_or_else(|| panic!("stage 05 lowered no machine named `{name}`"));
        let mut reads = machine.operations().filter_map(|operation| {
            let OperationKind::PrimitiveScalarRead { path, .. } = &operation.kind else {
                return None;
            };
            let [StructuralPathSegment::Field(read)] = path.as_slice() else {
                panic!("`{name}` reads one field of its parameter: {path:?}")
            };
            let OperationResult::Scalar(result) = &operation.result else {
                panic!("`{name}` reads a scalar: {operation:?}")
            };
            Some((*read, result.id))
        });
        let (read, value) = reads
            .next()
            .unwrap_or_else(|| panic!("`{name}` reads a field"));
        assert!(reads.next().is_none(), "`{name}` reads one field");
        let returned = machine
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator {
                Terminator::Return { value, .. } => Some(*value),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(returned, [value], "`{name}` returns the field it read");
        lowered
            .module
            .identities
            .text(read)
            .unwrap_or_else(|| panic!("`{name}` reads an unnamed field"))
            .to_owned()
    })
}

/// Stage 04's product for the same trees after `change`.
fn prepared_with(
    prepared: &PreparedTypedTrees,
    change: impl FnOnce(&mut TypedTrees),
) -> PreparedTypedTrees {
    let values = prepared.values().to_vec();
    let mut trees = prepared.clone().into_inner();
    change(&mut trees);
    preparation::prepare_typed_trees(trees, &values, &[]).unwrap()
}

/// Give one read another field's symbol and leave its name, its receiver
/// and every pattern roster as typing wrote them.
fn select(trees: &mut TypedTrees, read: &FieldRead, field: SymbolHandle) {
    let ExpressionNode::Member(member) = trees.expression_table.expression_mut(read.expression)
    else {
        panic!("a field read is a member")
    };
    member.selection = MemberSelection::Field(field);
}

#[test]
fn each_binding_is_lowered_to_a_read_of_the_field_typing_selected() {
    let fixture = prepare();
    let prepared = &fixture.program;
    let lowered = lower(&fixture, prepared).unwrap();
    let [by_place, with_rest, in_arm, by_name] = returned_fields(&lowered, prepared.trees());
    // Place 0 is `zeta` although `by_place` calls its binding `alpha`, and
    // a positional read and a named read of one field are the same read.
    assert_eq!(
        [by_place, with_rest, in_arm, by_name],
        ["zeta", "zeta", "alpha", "alpha"]
    );
}

#[test]
fn changing_only_the_selected_field_changes_the_field_that_is_read() {
    let fixture = prepare();
    let prepared = &fixture.program;
    let trees = prepared.trees();
    let (zeta, alpha) = (field(trees, "zeta"), field(trees, "alpha"));
    let reads = field_reads(trees);
    let [by_place, with_rest, in_arm, by_name] = reads.as_slice() else {
        panic!("one read per machine: {reads:?}")
    };
    let original = returned_fields(&lower(&fixture, prepared).unwrap(), trees);

    // Control: stage 04 run again with nothing changed lowers to the same
    // reads, so a later difference comes from the one change. (The product
    // itself differs: stage 04 writes each pattern's resolved roster as new
    // rows every time it runs.)
    let unchanged = prepared_with(prepared, |_| {});
    assert_eq!(
        returned_fields(&lower(&fixture, &unchanged).unwrap(), unchanged.trees()),
        original
    );

    // Each read in turn gets the other field. Only that machine's result
    // moves, and it moves to the field a correct read of the other field
    // produces.
    for (position, (read, other)) in [
        (by_place, alpha),
        (with_rest, alpha),
        (in_arm, zeta),
        (by_name, zeta),
    ]
    .into_iter()
    .enumerate()
    {
        let altered = prepared_with(prepared, |trees| select(trees, read, other));
        let lowered = lower(&fixture, &altered).unwrap();
        let mut expected = original.clone();
        expected[position] = original[if other == alpha { 3 } else { 0 }].clone();
        assert_eq!(
            returned_fields(&lowered, altered.trees()),
            expected,
            "after giving the read of `{}` in machine {position} the other field",
            read.member
        );
    }
}
