//! Readiness probe: which Psi output forms do the published examples emit?
//!
//! The exhaustive matches make a new Psi variant or package record visible to
//! the report. Emission shows that a form occurs in a lowered product. It does
//! not show that the form carries the right meaning. Property tests and their
//! registered receipts make that claim. `tests/spec_corpus.rs` owns each
//! example's verdict; this probe only counts what the products contain.
#[path = "support/preparation.rs"]
mod preparation;

use std::collections::{BTreeMap, BTreeSet};

use omega_lowering::{
    BlockTerminator, CallTarget, CleanupAction, ContractClause, LoweredMachine, LoweredPackage, MemberRealization, OperationKind, PlaceProjection, PredicateStep,
    ProjectedPlace, ReturnEdge, StructuralStoreDisposition, SuccessorEdge, TypeShape, ValueOperand, lower_package,
};
use omega_parser::parse_fragment;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::resolve_source_package;
use spec_corpus::parser_examples::{fragment_kind, parser_source};

trait Form {
    fn form(&self) -> &'static str;
}

macro_rules! families {
    ($($family:ident => [$($variant:ident),+ $(,)?]),+ $(,)?) => {
        const FORMS: &[&str] = &[$($(concat!(stringify!($family), "::", stringify!($variant))),+),+];
        $(impl Form for $family {
            fn form(&self) -> &'static str {
                match self {
                    $($family::$variant { .. } => concat!(stringify!($family), "::", stringify!($variant))),+
                }
            }
        })+
    };
}

families! {
    OperationKind => [
        BeginAssembly, AssemblyInstruction, Constant, Boolean, ExactNumericCast, TrappingInteger, Integer, Float, Call, ApplyTheorem,
        ConstructCallableDescriptor, RefineCallableDescriptor, ByteSequenceLength, ByteSequenceRead, BorrowPlace, ScalarBitRecast,
        MutableScalarBitRecast, BorrowedMove,
        StructuralStore, ElementViewLength, ElementViewRead, ElementViewWrite, EstablishZeroedStructural, EstablishCase, EstablishRecord,
        CopyStructural, MoveStructural, DecomposeRecord, StructuralScalarFieldStore, ProjectedScalarStore,
        StructuralCaseMembership, ProjectedScalarRead, PrimitiveScalarRead, StructuralLeafCopy, EstablishScalarArray,
        EstablishStructuralArray, EstablishPrimitiveLocal,
    ],
    BlockTerminator => [Return, Conditional, Jump, TerminalCall, ExternalExit],
    ReturnEdge => [Unit, Scalar, Structural],
    CleanupAction => [DiscardRoot, DiscardResidual, Invoke, RetireHome],
    StructuralStoreDisposition => [FillVacancy, ReplaceUnrestricted, Dispose],
    ValueOperand => [Scalar, CopyWhole, MoveWhole, Borrow],
    PlaceProjection => [Referent, Case, Field, FixedIndex, ViewElement],
    CallTarget => [Machine, Member, Runtime],
    MemberRealization => [Boundary, Conformance],
    PredicateStep => [
        Formal, MachineApplication, CallbackApplication, CallbackGuarantee, TypesEqual, ValueIndex, Result, SelfValue, Field, CasePayload, FixedIndex, FixedArrayRead,
        IsCase, InDomain,
        FixedArrayLength, ByteSequenceLength, ElementViewLength, Boolean, Integer, IeeeFloat, IeeeComparison, MathematicalTerm,
        HypothesisParameter, HypothesisGuarantee, Apply,
    ],
    TypeShape => [
        PrimitiveScalar, QualifiedScalar, Nominal, QualifiedNominal, SharedQualifiedScalarReference, SharedByteView, SharedElementView, MutableElementView,
        ScalarReference, NominalReference, IndexedSharedReference, Callable, FixedArray,
    ],
}

/// Records and optional facts that are not enum variants. `Tally::package`
/// destructures the package without `..`, so a new record fails to compile here.
const RECORDS: &[&str] = &[
    "Package::target_closure",
    "Package::data",
    "Package::aliases",
    "Package::domains",
    "Package::domain_templates",
    "Package::domain_aliases",
    "Package::mathematics",
    "Package::theorems",
    "Package::data_templates",
    "Package::traits",
    "Package::conformances",
    "Package::member_calls",
    "Package::callable_contracts",
    "Package::machine_templates",
    "Package::machines",
    "Machine::operator",
    "Machine::static_argument",
    "Contract::requires",
    "Contract::ensures",
    "Contract::crash_route",
    "Contract::ranking",
    "Body::suspension_site",
    "Body::call_acknowledgement",
    "Edge::selected_case",
    "Edge::requires_adjacency",
    "Edge::scalar_qualification_change",
];

#[derive(Default)]
struct Tally(BTreeMap<&'static str, usize>);

impl Tally {
    fn add(&mut self, form: &'static str, count: usize) {
        if count > 0 {
            *self.0.entry(form).or_insert(0) += count;
        }
    }

    fn form(&mut self, value: &impl Form) {
        self.add(value.form(), 1);
    }

    fn package(&mut self, package: &LoweredPackage) {
        let LoweredPackage {
            target_closure,
            types,
            data,
            aliases,
            domains,
            domain_templates,
            domain_aliases,
            mathematics,
            theorems,
            data_templates,
            traits,
            conformances,
            member_calls,
            callable_contracts,
            machine_templates,
            machines,
        } = package;
        self.add("Package::target_closure", usize::from(target_closure.is_some()));
        self.add("Package::data", data.len());
        self.add("Package::aliases", aliases.len());
        self.add("Package::domains", domains.len());
        self.add("Package::domain_templates", domain_templates.len());
        for template in domain_templates {
            self.clauses(&template.requires);
        }
        self.add("Package::domain_aliases", domain_aliases.len());
        self.add("Package::mathematics", mathematics.len());
        self.add("Package::theorems", theorems.len());
        self.add("Package::data_templates", data_templates.len());
        self.add("Package::traits", traits.len());
        self.add("Package::conformances", conformances.len());
        self.add("Package::member_calls", member_calls.len());
        self.add("Package::callable_contracts", callable_contracts.len());
        self.add("Package::machine_templates", machine_templates.len());
        self.add("Package::machines", machines.len());
        for (_, shape) in types.rows() {
            self.form(shape);
        }
        for schema in data {
            self.clauses(&schema.requires);
        }
        for domain in domains {
            self.clauses(&domain.requires);
        }
        for template in machine_templates {
            self.clauses(&template.signature.requires);
            self.clauses(&template.signature.ensures);
            self.clauses(template.signature.crash_routes.iter().filter_map(|route| route.guard.as_ref()));
        }
        for declaration in traits {
            for requirement in &declaration.requirements {
                self.clauses(&requirement.requires);
                self.clauses(&requirement.ensures);
                self.clauses(requirement.crash_routes.iter().filter_map(|route| route.guard.as_ref()));
            }
        }
        for call in member_calls {
            self.form(&call.target);
        }
        for machine in machines {
            self.machine(machine);
        }
    }

    fn machine(&mut self, machine: &LoweredMachine) {
        let contract = &machine.contract;
        self.add("Machine::operator", usize::from(machine.operator.is_some()));
        self.add("Machine::static_argument", machine.arguments.len());
        self.add("Contract::requires", contract.requires.len());
        self.add("Contract::ensures", contract.ensures.len());
        self.add("Contract::crash_route", contract.crash_routes.len());
        self.add("Contract::ranking", usize::from(contract.ranking.is_some()));
        self.clauses(&contract.requires);
        self.clauses(&contract.ensures);
        self.clauses(contract.crash_routes.iter().filter_map(|route| route.guard.as_ref()));
        let body = &machine.body;
        self.add("Body::suspension_site", body.suspension_sites.len());
        self.add("Body::call_acknowledgement", body.call_acknowledgements.len());
        for operation in &body.operations {
            self.operation(&operation.kind);
        }
        for block in &body.blocks {
            self.terminator(&block.terminator);
        }
    }

    fn clauses<'a>(&mut self, clauses: impl IntoIterator<Item = &'a ContractClause>) {
        for clause in clauses {
            for step in &clause.predicate {
                self.form(step);
            }
        }
    }

    fn place(&mut self, place: &ProjectedPlace) {
        for step in &place.path {
            self.form(step);
        }
    }

    fn operand(&mut self, operand: &ValueOperand) {
        self.form(operand);
        if let ValueOperand::Borrow { place, .. } = operand {
            self.place(place);
        }
    }

    fn cleanup(&mut self, actions: &[CleanupAction]) {
        for action in actions {
            self.form(action);
            if let CleanupAction::DiscardResidual { place, .. } | CleanupAction::Invoke { place, .. } = action {
                self.place(place);
            }
        }
    }

    fn operation(&mut self, kind: &OperationKind) {
        self.form(kind);
        match kind {
            OperationKind::Call { callee, arguments, .. } => {
                self.form(callee);
                for argument in arguments {
                    self.operand(argument);
                }
            }
            OperationKind::StructuralStore { destination, disposition, .. } => {
                self.place(destination);
                self.form(disposition);
                if let StructuralStoreDisposition::Dispose(actions) = disposition {
                    self.cleanup(actions);
                }
            }
            OperationKind::EstablishCase { fields, .. } | OperationKind::EstablishRecord { fields } => {
                for field in fields {
                    self.operand(&field.operand);
                }
            }
            OperationKind::BorrowPlace { source, .. }
            | OperationKind::BorrowedMove { source }
            | OperationKind::CopyStructural { source }
            | OperationKind::MoveStructural { source }
            | OperationKind::DecomposeRecord { source, .. }
            | OperationKind::StructuralCaseMembership { source, .. }
            | OperationKind::ProjectedScalarRead { source } => self.place(source),
            OperationKind::StructuralScalarFieldStore { destination, .. } | OperationKind::ProjectedScalarStore { destination, .. } => self.place(destination),
            _ => {}
        }
    }

    fn edge(&mut self, edge: &SuccessorEdge) {
        if let Some(selection) = &edge.selected_case {
            self.add("Edge::selected_case", 1);
            self.place(&selection.source);
        }
        self.add("Edge::requires_adjacency", usize::from(edge.requires_adjacency));
        self.add("Edge::scalar_qualification_change", edge.scalar_qualification_changes.len());
        for argument in &edge.structural_arguments {
            self.operand(argument);
        }
        self.cleanup(&edge.cleanup);
    }

    fn terminator(&mut self, terminator: &BlockTerminator) {
        self.form(terminator);
        match terminator {
            BlockTerminator::Return { result, cleanup, .. } => {
                self.form(result);
                self.cleanup(cleanup);
            }
            BlockTerminator::Conditional { when_true, when_false, .. } => {
                self.edge(when_true);
                self.edge(when_false);
            }
            BlockTerminator::Jump(edge) => self.edge(edge),
            BlockTerminator::TerminalCall { callee, arguments, cleanup, .. } => {
                self.form(callee);
                for argument in arguments {
                    self.operand(argument);
                }
                self.cleanup(cleanup);
            }
            BlockTerminator::ExternalExit(exit) => {
                for argument in &exit.arguments {
                    self.operand(argument);
                }
            }
        }
    }
}

enum Outcome {
    Produced(BTreeMap<&'static str, usize>),
    Stopped(&'static str),
}

/// The same package path as the corpus verdict, without its expectations.
fn measure(example: &spec_corpus::SpecExample) -> Outcome {
    let label = example.get_label();
    let text = parser_source(&example.source_text);
    let Ok(tokens) = omega_lexer::lex(text.as_bytes()) else {
        return Outcome::Stopped("L0");
    };
    let Ok(tree) = parse_fragment(text.as_bytes(), &tokens, fragment_kind(&label)) else {
        return Outcome::Stopped("L1");
    };
    let files = [StageSource {
        package: "example",
        source: ParsedSource {
            logical_path: "example.omg",
            text: &text,
            tokens: &tokens,
            tree: &tree,
        },
    }];
    let Ok(package) = resolve_source_package(&files) else {
        return Outcome::Stopped("L2");
    };
    let prepared = match preparation::prepare(&package) {
        Ok(prepared) => prepared,
        Err((stage, _)) => return Outcome::Stopped(stage),
    };
    match lower_package(&prepared) {
        Ok(lowered) => {
            let mut tally = Tally::default();
            tally.package(&lowered);
            Outcome::Produced(tally.0)
        }
        Err(_) => Outcome::Stopped("L5"),
    }
}

#[test]
fn published_examples_expose_their_psi_output_forms() {
    let inventory: Vec<&'static str> = FORMS.iter().chain(RECORDS).copied().collect();
    let unique: BTreeSet<&'static str> = inventory.iter().copied().collect();
    assert_eq!(unique.len(), inventory.len(), "each output form has one name");
    if std::env::var_os("OMEGA_LAYER_REPORT").is_none() {
        return;
    }
    let examples = spec_corpus::published_examples().unwrap();
    println!("OMEGA_L5_OUTPUT_INVENTORY\t{}", inventory.join(","));
    println!("OMEGA_L5_OUTPUT_COUNT\t{}", examples.len());
    for example in &examples {
        let label = example.get_label();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| measure(example)));
        let (status, detail) = match outcome {
            Ok(Outcome::Produced(counts)) => {
                for form in counts.keys() {
                    assert!(unique.contains(form), "{label}: {form} is missing from the output inventory");
                }
                let pairs = counts
                    .iter()
                    .map(|(form, count)| format!("{form}={count}"))
                    .collect::<Vec<_>>();
                ("produced", pairs.join(","))
            }
            Ok(Outcome::Stopped(stage)) => ("stopped", stage.to_owned()),
            Err(_) => ("panicked", "-".to_owned()),
        };
        println!("OMEGA_L5_OUTPUT\t{label}\t{status}\t{detail}");
    }
}
