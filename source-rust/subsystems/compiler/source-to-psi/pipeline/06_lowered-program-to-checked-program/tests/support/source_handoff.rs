//! Runs a small program written in a test through every stage up to the
//! lowered-form checker, so a test can read what stage 05 produced for it,
//! change one fact of that product, and hand the result to the real checker.
//!
//! `Program::of` runs the files through the lexer, parser, resolver, typer
//! and stage 04 as the source driver does, with every machine of the package
//! as an entry except a generic one, which a call instantiates instead.
//! `Program::lowered` is stage 05's output. `Program::judged` clones that
//! output, applies the caller's change and returns stage 06's verdict;
//! `|_| {}` is the control, the verdict on the program as stage 05 lowered
//! it.
//!
//! Stage 05's own tests read the same product: `Program::prepared_with`
//! changes one fact of stage 04's output, and `Program::lower_prepared`
//! lowers it again and names its machines and operations by what the
//! author wrote.
#![allow(dead_code)]

#[path = "../../../../../../../../tests/support/frontend.rs"]
mod frontend;

use semantic_vocabulary::{MachineId, OperationId, PackageKeyIdentity, ValueId};
use source_compiler::representations::source_inputs::SourceMap;
use source_compiler::representations::source_state::TypedTrees;
use source_compiler::representations::source_state::expression::{
    ExpressionHandle, ExpressionNode,
};
use source_compiler::test_support::checking::{
    CompilationRefusal, RouteVerdict, judge_compilation,
};
use source_compiler::test_support::lowering::{self, Elaboration};
use source_compiler::test_support::preparation::{self, PreparedTypedTrees};
use source_compiler::test_support::{
    SourcePreparationRequest, TerminalSourceCompilation, prepare_source_for_test, resolution,
    typing,
};
use source_provenance::DependencyScope;
use source_text::{SourceId, SourceSpan};
use symbols::SymbolHandle;
use terminal_check_tables::compilation::{PreparedCompilation, TerminalCompilation};
use terminal_check_tables::program::SourceOrigin;
use terminal_check_tables::{MachineTables, Refusal, RefusalReason};
use terminal_psi::{Operation, OperationKind, OperationResult, TerminalMachine, Terminator};

pub const PACKAGE: [u8; 32] = [0x4d; 32];

/// Where a stage before the checker stopped: the message and the source
/// text it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarlierRefusal {
    pub stage: &'static str,
    pub message: String,
    pub at: String,
}

pub struct Program {
    pub prepared: PreparedTypedTrees,
    /// Stage 05's output, as stage 06 receives it.
    pub lowered: PreparedCompilation,
    files: Vec<(SourceId, String)>,
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

fn written_in(files: &[(SourceId, String)], span: Option<SourceSpan>) -> String {
    span.and_then(|span| {
        let (_, text) = files.iter().find(|(id, _)| *id == span.source_id)?;
        text.get(span.span.start..span.span.end).map(str::to_owned)
    })
    .unwrap_or_default()
}

impl Program {
    /// Run `main` as `product/main.omg` through stage 05. Every stage must
    /// accept it.
    pub fn of(main: &str) -> Self {
        Self::try_of(&[("product/main.omg", main)]).unwrap_or_else(|refusal| panic!("{refusal:?}"))
    }

    /// Run the files of one package through stage 05, or say which stage
    /// refused them, with what message, at what text.
    pub fn try_of(files: &[(&str, &str)]) -> Result<Self, EarlierRefusal> {
        let package = PackageKeyIdentity::from_digest(PACKAGE).unwrap();
        let mut sources = SourceMap::default();
        let mut syntax = None;
        let mut read = Vec::new();
        for (path, text) in files {
            let id = sources
                .add_checked_instance(
                    (*path).into(),
                    *text,
                    "product".into(),
                    Some(package),
                    source_admission::SourceOrigin::User,
                    source_admission::SourceResolutionStratum::Base,
                    DependencyScope::Product,
                )
                .source_id;
            read.push((id, (*text).to_owned()));
            let tokens = frontend::Lexer::new(text).tokenize().unwrap();
            match &mut syntax {
                Some(syntax) => {
                    frontend::parse(syntax, id, &tokens)
                        .unwrap_or_else(|error| panic!("{path} does not parse: {error:?}"));
                }
                None => {
                    syntax = Some(
                        frontend::parse_syntax_trees_with_id(id, &tokens)
                            .unwrap_or_else(|error| panic!("{path} does not parse: {error:?}")),
                    )
                }
            }
        }
        let mut request = resolution::ResolutionRequest::new(syntax.expect("at least one file"));
        request.sources = Some(std::sync::Arc::new(sources));
        let resolved = resolution::resolve(request).map_err(|refusal| EarlierRefusal {
            stage: "resolution",
            message: format!("{refusal:?}"),
            at: String::new(),
        })?;
        let typed = typing::lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| {
            EarlierRefusal {
                stage: "typing",
                at: written_in(&read, diagnostic.source_span),
                message: diagnostic.message,
            }
        })?;
        let entries: Vec<_> = typed
            .machines()
            .iter()
            .filter(|machine| machine.target.is_none() && !machine.has_specialization_parameters())
            .map(|machine| machine.symbol)
            .collect();
        let subjects = [Some(package)];
        let frozen = prepare_source_for_test(SourcePreparationRequest {
            program: typed,
            accounting: Default::default(),
            admission: Default::default(),
            compilation: compilation(&entries, &subjects),
        })
        .map_err(|failure| {
            let diagnostic = failure
                .diagnostics
                .first()
                .expect("a refusal carries a diagnostic");
            EarlierRefusal {
                stage: "preparation",
                at: written_in(&read, diagnostic.source_span),
                message: diagnostic.message.clone(),
            }
        })?;
        let lowered = lowering::prepare_compilation(
            &frozen.program,
            compilation(&entries, &subjects),
            terminal_codec::TerminalPublication::Portable,
        )
        .map_err(|diagnostics| {
            let diagnostic = diagnostics.first().expect("a refusal carries a diagnostic");
            EarlierRefusal {
                stage: "lowering",
                at: written_in(&read, diagnostic.source_span),
                message: diagnostic.message.clone(),
            }
        })?;
        let TerminalCompilation::Prepared(lowered) = lowered.program else {
            panic!("the program binds no callable shape that could fail to form")
        };
        if let Err(refusal) = &lowered.closure {
            return Err(EarlierRefusal {
                stage: "lowering",
                message: format!("{:?}: {}", refusal.reason, refusal.detail),
                at: written_in(&read, refusal.source_span),
            });
        }
        Ok(Self {
            prepared: frozen.program,
            lowered,
            files: read,
        })
    }

    pub fn trees(&self) -> &TypedTrees {
        self.prepared.trees()
    }

    /// The lowered program stage 06 receives.
    pub fn elaboration(&self) -> &Elaboration {
        self.lowered.closure.as_ref().unwrap()
    }

    /// The position in `module.machines` of the machine whose path from the
    /// package root is `path`, such as `main` or `east\pick`.
    pub fn position(&self, path: &str) -> usize {
        let lowered = self.elaboration();
        let found: Vec<_> = lowered
            .module
            .machines
            .iter()
            .enumerate()
            .filter(|(_, machine)| {
                lowered
                    .machines
                    .get(machine.id)
                    .is_some_and(|symbol| machine_path(self.trees(), *symbol) == path)
            })
            .map(|(position, _)| position)
            .collect();
        let [position] = found.as_slice() else {
            panic!(
                "expected one lowered machine at `{path}`, found {}",
                found.len()
            )
        };
        *position
    }

    pub fn machine(&self, path: &str) -> &TerminalMachine {
        &self.elaboration().module.machines[self.position(path)]
    }

    pub fn tables(&self, path: &str) -> &MachineTables {
        self.elaboration()
            .tables
            .machines
            .get(self.machine(path).id)
            .unwrap()
    }

    /// The source path of the machine stage 05 gave `id`.
    pub fn machine_named(&self, id: MachineId) -> String {
        machine_path(self.trees(), *self.elaboration().machines.get(id).unwrap())
    }

    /// The source text stage 05 recorded for an operation of `machine`, or
    /// the empty text for an operation with no authored counterpart.
    pub fn written(&self, machine: &TerminalMachine, operation: OperationId) -> String {
        let Some(SourceOrigin::Authored(span)) = self
            .elaboration()
            .sources
            .get(machine.id)
            .and_then(|sources| sources.operations.get(operation))
        else {
            return String::new();
        };
        written_in(&self.files, Some(*span))
    }

    /// The text a span covers.
    pub fn text(&self, span: SourceSpan) -> String {
        written_in(&self.files, Some(span))
    }

    /// Stage 06's verdict on stage 05's output after `change`.
    /// The checker judges the program first, then each generic machine's own
    /// form; the program is accepted only when every one of them is.
    pub fn judged(&self, change: impl FnOnce(&mut Elaboration)) -> Verdict {
        let mut altered = self.lowered.clone();
        change(altered.closure.as_mut().unwrap());
        let mut verdicts = judge_compilation(TerminalCompilation::Prepared(altered))
            .into_iter()
            .map(verdict);
        let program = verdicts.next().expect("the program has a verdict");
        match program {
            Verdict::Accepted => verdicts
                .find(|form| !matches!(form, Verdict::Accepted))
                .unwrap_or(Verdict::Accepted),
            other => other,
        }
    }

    /// Stage 06's verdict after `change` to the machine at `path`.
    pub fn judged_in(&self, path: &str, change: impl FnOnce(&mut TerminalMachine)) -> Verdict {
        let position = self.position(path);
        self.judged(|lowered| change(&mut lowered.module.machines[position]))
    }
}

fn verdict(verdict: Result<RouteVerdict, CompilationRefusal>) -> Verdict {
    match verdict {
        Ok(RouteVerdict::Accepted(_)) => Verdict::Accepted,
        Ok(RouteVerdict::Rejected {
            violations,
            unproved,
            unestablished,
            conflicts,
            exceeded,
            formations,
        }) => Verdict::Rejected(Rejection {
            violations: violations
                .iter()
                .map(|violation| violation.statement.clone())
                .collect(),
            unproved: unproved
                .iter()
                .map(|owed| format!("{:?}", owed.kind))
                .collect(),
            unestablished: unestablished
                .iter()
                .map(|used| used.detail.clone())
                .collect(),
            conflicts: conflicts
                .iter()
                .map(|loan| loan.statement.clone())
                .collect(),
            exceeded: exceeded.iter().map(|call| call.statement.clone()).collect(),
            formations: formations.len(),
        }),
        // A goal outside the prover's forms leaves the program unjudged at
        // that obligation; it is not accepted.
        Ok(RouteVerdict::Incomplete(unproved)) => Verdict::Rejected(Rejection {
            violations: Vec::new(),
            unproved: unproved.iter().map(|owed| owed.statement.clone()).collect(),
            unestablished: Vec::new(),
            conflicts: Vec::new(),
            exceeded: Vec::new(),
            formations: 0,
        }),
        Ok(RouteVerdict::FallBack(refusal)) => Verdict::Unserved(refusal),
        Err(CompilationRefusal::Elaboration(refusal)) => Verdict::Refused(refusal),
        other => panic!("{other:?}"),
    }
}

/// A machine's path from its package root, with `\` between modules.
pub fn machine_path(trees: &TypedTrees, symbol: SymbolHandle) -> String {
    let path = trees.symbols.display_path(symbol, "\\");
    path.strip_prefix("product\\").unwrap_or(&path).to_owned()
}

/// What the checker found wrong with a program it understood.
#[derive(Debug)]
pub struct Rejection {
    /// The statement of each language rule the program breaks.
    pub violations: Vec<String>,
    /// The kind of each obligation left unproved.
    pub unproved: Vec<String>,
    /// Each use of storage that holds no value on some path.
    pub unestablished: Vec<String>,
    /// Each loan that may not be formed where it is.
    pub conflicts: Vec<String>,
    /// Each call outside its caller's declared reach.
    pub exceeded: Vec<String>,
    /// How many bound calling shapes failed to form.
    pub formations: usize,
}

#[derive(Debug)]
pub enum Verdict {
    Accepted,
    /// The checker judged the program and found it wrong.
    Rejected(Rejection),
    /// The lowered program disagrees with itself: the checker's answer to a
    /// product no correct stage 05 issues.
    Refused(Refusal),
    /// The checker does not judge this form and accepts nothing in it.
    Unserved(Refusal),
}

impl Verdict {
    #[track_caller]
    pub fn accepted(self) {
        assert!(matches!(self, Self::Accepted), "{self:?}");
    }

    /// The refusal of a lowered program that disagrees with itself.
    #[track_caller]
    pub fn refused(self) -> Refusal {
        match self {
            Self::Refused(refusal) => {
                assert_eq!(
                    refusal.reason,
                    RefusalReason::ForeignIdentity,
                    "{refusal:?}"
                );
                refusal
            }
            other => panic!("the changed program must be refused: {other:?}"),
        }
    }

    #[track_caller]
    pub fn rejected(self) -> Rejection {
        match self {
            Self::Rejected(rejection) => rejection,
            other => panic!("the program must be rejected: {other:?}"),
        }
    }
}

/// The one operation of `machine` that `select` picks.
#[track_caller]
pub fn the_operation(
    machine: &TerminalMachine,
    select: impl Fn(&OperationKind) -> bool,
) -> &Operation {
    let mut found = machine
        .operations()
        .filter(|operation| select(&operation.kind));
    let operation = found.next().expect("the machine has the operation");
    assert!(found.next().is_none(), "the machine has one such operation");
    operation
}

/// The operation `id` of `machine`, to change.
#[track_caller]
pub fn operation_mut(machine: &mut TerminalMachine, id: OperationId) -> &mut Operation {
    machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.id == id)
        .expect("the machine has the operation")
}

impl Program {
    /// Stage 05's lowered program for the program as stage 04 issued it.
    pub fn lower(&self) -> Lowered<'_> {
        self.lower_prepared(&self.prepared)
            .unwrap_or_else(|refusal| panic!("stage 05 refused the program: {refusal:?}"))
    }

    /// Stage 05's lowered program for `prepared`, with every machine of the
    /// package as an entry, or its refusal.
    pub fn lower_prepared<'a>(
        &'a self,
        prepared: &'a PreparedTypedTrees,
    ) -> Result<Lowered<'a>, Refusal> {
        let package = PackageKeyIdentity::from_digest(PACKAGE).unwrap();
        let entries: Vec<_> = prepared
            .trees()
            .machines()
            .iter()
            .filter(|machine| machine.target.is_none() && !machine.has_specialization_parameters())
            .map(|machine| machine.symbol)
            .collect();
        let subjects = [Some(package)];
        let lowered = lowering::prepare_compilation(
            prepared,
            compilation(&entries, &subjects),
            terminal_codec::TerminalPublication::Portable,
        )
        .unwrap();
        let TerminalCompilation::Prepared(lowered) = lowered.program else {
            panic!("the program binds no callable shape that could fail to form")
        };
        Ok(Lowered {
            elaboration: lowered.closure?,
            trees: prepared.trees(),
            files: &self.files,
        })
    }

    /// Stage 04's product for the same trees after `change`.
    /// `PreparedTypedTrees` cannot be built or edited outside stage 04, so
    /// the trees are taken back with `into_inner`, changed, and prepared
    /// again; stage 05 still receives a product only stage 04 could issue.
    pub fn prepared_with(&self, change: impl FnOnce(&mut TypedTrees)) -> PreparedTypedTrees {
        let values = self.prepared.values().to_vec();
        let mut trees = self.prepared.clone().into_inner();
        change(&mut trees);
        preparation::prepare_typed_trees(trees, &values, &[]).unwrap()
    }

    /// The source text an expression stands for.
    pub fn written_expression(&self, expression: ExpressionHandle) -> String {
        self.text(self.trees().expression_table.source_span(expression))
    }

    /// The one expression accepted by `wanted` whose source text is
    /// `written`.
    pub fn expression(
        &self,
        written: &str,
        wanted: impl Fn(&ExpressionNode) -> bool,
    ) -> ExpressionHandle {
        let found: Vec<_> = self
            .trees()
            .expression_table
            .iter_expressions()
            .filter(|(handle, node)| wanted(node) && self.written_expression(*handle) == written)
            .map(|(handle, _)| handle)
            .collect();
        let [expression] = found.as_slice() else {
            panic!(
                "expected one such expression written `{written}`, found {}",
                found.len()
            )
        };
        *expression
    }

    /// The symbol of the machine whose path from the package root is `path`.
    pub fn machine_symbol(&self, path: &str) -> SymbolHandle {
        let trees = self.trees();
        let found: Vec<_> = trees
            .machines()
            .iter()
            .filter(|machine| {
                machine.target.is_none() && machine_path(trees, machine.symbol) == path
            })
            .map(|machine| machine.symbol)
            .collect();
        let [symbol] = found.as_slice() else {
            panic!("expected one machine at `{path}`, found {}", found.len())
        };
        *symbol
    }
}

/// Stage 05's product for one version of the program, with what a test needs
/// to name its machines and operations by what the author wrote.
pub struct Lowered<'a> {
    pub elaboration: Elaboration,
    trees: &'a TypedTrees,
    files: &'a [(SourceId, String)],
}

impl Lowered<'_> {
    /// The identity stage 05 gave the machine at `path`.
    pub fn machine_id(&self, path: &str) -> MachineId {
        self.machine(path).id
    }

    /// The lowered machine at `path`, such as `main` or `east\pick`.
    pub fn machine(&self, path: &str) -> &TerminalMachine {
        let found: Vec<_> = self
            .elaboration
            .module
            .machines
            .iter()
            .filter(|machine| {
                self.elaboration
                    .machines
                    .get(machine.id)
                    .is_some_and(|symbol| machine_path(self.trees, *symbol) == path)
            })
            .collect();
        let [machine] = found.as_slice() else {
            panic!(
                "expected one lowered machine at `{path}`, found {}",
                found.len()
            )
        };
        machine
    }

    /// The source text stage 05 recorded for an operation of `machine`, or
    /// the empty text for an operation with no authored counterpart.
    pub fn written(&self, machine: &TerminalMachine, operation: OperationId) -> &str {
        let Some(SourceOrigin::Authored(span)) = self
            .elaboration
            .sources
            .get(machine.id)
            .and_then(|sources| sources.operations.get(operation))
        else {
            return "";
        };
        self.files
            .iter()
            .find(|(id, _)| *id == span.source_id)
            .and_then(|(_, text)| text.get(span.span.start..span.span.end))
            .unwrap_or_default()
    }

    /// Every operation of the machine at `path`, in block order, each with
    /// the source text recorded for it.
    pub fn operations(&self, path: &str) -> Vec<(&str, &Operation)> {
        let machine = self.machine(path);
        machine
            .operations()
            .map(|operation| (self.written(machine, operation.id), operation))
            .collect()
    }
}

/// The scalar an operation produces.
pub fn scalar(operation: &Operation) -> ValueId {
    let OperationResult::Scalar(result) = &operation.result else {
        panic!("the operation produces no scalar: {operation:?}")
    };
    result.id
}

/// The one scalar `machine` returns: it must have exactly one scalar return.
pub fn returned(machine: &TerminalMachine) -> ValueId {
    let returns: Vec<_> = machine
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Terminator::Return { value, .. } => Some(*value),
            _ => None,
        })
        .collect();
    let [value] = returns.as_slice() else {
        panic!("the machine returns one scalar once: {returns:?}")
    };
    *value
}
