//! One program for testing what stage 05 hands stage 06 about a match on a
//! sum. `Left` and `Right` are unrelated sums that spell their cases alike:
//! each declares `Small(value: ...)` first and `Nothing` second, and only the
//! payload type differs, `u8` in `Left` and `bool` in `Right`. `choose`
//! matches a `Left`, `other` matches a `Right`, `plain` matches a Boolean and
//! names no sum, and `lend` matches a borrowed `Left` and lends the payload
//! of `Left::Small` to `look`.
//!
//! A case's name, its position in its sum and its field's name are the same
//! in both sums, so none of them says which sum a match arm selects from.
//! A compiler that paired an arm with "the case named `Small`" or "the first
//! case" could hand `choose` the Boolean payload of `Right::Small` where its
//! arm returns a `u8`.
#![allow(dead_code)]

#[path = "../../../../../../../../tests/support/frontend.rs"]
mod frontend;

use semantic_vocabulary::{OperationId, PackageKeyIdentity, StructuralTypeId};
use source_compiler::representations::source_inputs::SourceMap;
use source_compiler::test_support::lowering::{self, Elaboration};
use source_compiler::test_support::preparation::PreparedTypedTrees;
use source_compiler::test_support::{
    SourcePreparationRequest, TerminalSourceCompilation, prepare_source_for_test, resolution,
    typing,
};
use source_provenance::DependencyScope;
use symbols::SymbolHandle;
use terminal_check_tables::compilation::{PreparedCompilation, TerminalCompilation};
use terminal_psi::{
    Block, StructuralArgument, StructuralCaseDeclaration, StructuralCaseSuccessorEdge,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    Terminator,
};

pub const MAIN: &str = r"data Left [copy] {
    case Small(value: u8);
    case Nothing;
}
data Right [copy] {
    case Small(value: bool);
    case Nothing;
}
machine choose(subject: Left) -> u8 {
    match subject {
        Left::Small { value } -> value,
        Left::Nothing -> 23u8,
    }
}
machine other(subject: Right) -> u8 {
    match subject {
        Right::Small { value } -> match value { true -> 17u8, _ -> 19u8, },
        Right::Nothing -> 23u8,
    }
}
machine plain(x: bool) -> u8 {
    match x { true -> 17u8, _ -> 23u8, }
}
machine look(value: @u8) -> u8 { 1u8 }
machine lend(held: @Left) -> u8 {
    match held {
        Left::Small { value as _ } -> look(@held.value),
        Left::Nothing -> 23u8,
    }
}";

/// The machines the request names as entries. `look` is lowered because
/// `lend` calls it.
pub const ENTRIES: [&str; 4] = ["choose", "other", "plain", "lend"];

pub struct Prepared {
    pub program: PreparedTypedTrees,
    entries: Vec<SymbolHandle>,
    subjects: [Option<PackageKeyIdentity>; 1],
}

impl Prepared {
    /// The request both stages receive: the four entries, and every
    /// declaration of the package as a subject.
    pub fn compilation(&self) -> TerminalSourceCompilation<'_> {
        compilation(&self.entries, &self.subjects)
    }

    /// Stage 05's complete output for the program, which is stage 06's whole
    /// input. Stage 05 must lower the program without a refusal.
    pub fn lower(&self) -> PreparedCompilation {
        let lowered = lowering::prepare_compilation(
            &self.program,
            self.compilation(),
            terminal_codec::TerminalPublication::Portable,
        )
        .unwrap();
        let TerminalCompilation::Prepared(lowered) = lowered.program else {
            panic!("the program binds no callable shape that could fail to form")
        };
        assert!(lowered.closure.is_ok(), "{:?}", lowered.closure);
        lowered
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

/// Run the program through the lexer, parser, resolver, typer and stage 04.
pub fn prepare() -> Prepared {
    let package = PackageKeyIdentity::from_digest([0x53; 32]).unwrap();
    let mut sources = SourceMap::default();
    let id = sources
        .add_checked_instance(
            "product/main.omg".into(),
            MAIN,
            "product".into(),
            Some(package),
            source_admission::SourceOrigin::User,
            source_admission::SourceResolutionStratum::Base,
            DependencyScope::Product,
        )
        .source_id;
    let tokens = frontend::Lexer::new(MAIN).tokenize().unwrap();
    let syntax = frontend::parse_syntax_trees_with_id(id, &tokens).unwrap();
    let mut request = resolution::ResolutionRequest::new(syntax);
    request.sources = Some(std::sync::Arc::new(sources));
    let resolved = resolution::resolve(request).unwrap();
    let typed = typing::lower_symbol_resolved_trees(&resolved).unwrap();
    let entries: Vec<_> = ENTRIES
        .iter()
        .map(|name| typed.realized_machine_named(name).unwrap().symbol)
        .collect();
    let subjects = [Some(package)];
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

/// The position in `lowered.module.machines` of the machine the source
/// names `name`.
pub fn machine_position(prepared: &Prepared, lowered: &Elaboration, name: &str) -> usize {
    let symbols = &prepared.program.trees().symbols;
    lowered
        .module
        .machines
        .iter()
        .position(|machine| symbols.name(*lowered.machines.get(machine.id).unwrap()) == name)
        .unwrap_or_else(|| panic!("stage 05 lowered no machine named `{name}`"))
}

/// One sum as stage 05 declared it: the type and its cases in declared order.
#[derive(Debug, Clone)]
pub struct Sum {
    pub id: StructuralTypeId,
    pub cases: Vec<StructuralCaseDeclaration>,
}

/// The sum the source names `name`.
pub fn sum(lowered: &Elaboration, name: &str) -> Sum {
    let named = format!(r"path\({name}\)");
    let mut found = lowered.module.types.structural_types.iter().filter(
        |declared: &&StructuralTypeDeclaration| {
            matches!(declared.shape, StructuralTypeShape::Sum { .. })
                && lowered
                    .module
                    .identities
                    .text(declared.identity)
                    .is_some_and(|identity| identity.contains(&named))
        },
    );
    let declared = found
        .next()
        .unwrap_or_else(|| panic!("stage 05 declared no sum named `{name}`"));
    assert!(found.next().is_none(), "two sums are named `{name}`");
    Sum {
        id: declared.id,
        cases: declared.shape.cases().to_vec(),
    }
}

/// The one dispatch on a sum in `machine`: its block, the declared type of
/// the place it reads, and its edges. The match subject is a whole formal,
/// so the dispatch names that formal with an empty path.
pub fn dispatch(
    machine: &TerminalMachine,
) -> (&Block, StructuralTypeId, &[StructuralCaseSuccessorEdge]) {
    let mut dispatches = machine.blocks.iter().filter_map(|block| {
        let Terminator::StructuralCase {
            source,
            path,
            cases,
        } = &block.terminator
        else {
            return None;
        };
        assert!(path.is_empty(), "the subject is a whole formal: {path:?}");
        let subject = machine
            .structural_place_type(*source)
            .expect("the dispatch names a declared place");
        Some((block, subject, cases.as_slice()))
    });
    let found = dispatches.next().expect("the machine dispatches on a sum");
    assert!(dispatches.next().is_none(), "the machine dispatches once");
    found
}

/// The edges of the one dispatch on a sum in `machine`, to change.
pub fn dispatch_edges_mut(machine: &mut TerminalMachine) -> &mut Vec<StructuralCaseSuccessorEdge> {
    machine
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            Terminator::StructuralCase { cases, .. } => Some(cases),
            _ => None,
        })
        .expect("the machine dispatches on a sum")
}

fn names_a_case(argument: &StructuralArgument) -> bool {
    argument
        .path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
}

/// The one place in `machine` where a call is handed storage inside a case
/// payload: the call and the argument that names the payload.
pub fn payload_loan(machine: &TerminalMachine) -> (OperationId, &StructuralArgument) {
    let mut loans = machine.operations().flat_map(|operation| {
        operation
            .kind
            .structural_arguments()
            .iter()
            .filter(|argument| names_a_case(argument))
            .map(move |argument| (operation.id, argument))
    });
    let found = loans.next().expect("the machine lends a case payload");
    assert!(loans.next().is_none(), "the machine lends one payload");
    found
}

/// The argument `payload_loan` finds, to change.
pub fn payload_loan_mut(machine: &mut TerminalMachine) -> &mut StructuralArgument {
    machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .filter_map(|operation| operation.kind.structural_arguments_mut())
        .flatten()
        .find(|argument| names_a_case(argument))
        .expect("the machine lends a case payload")
}
