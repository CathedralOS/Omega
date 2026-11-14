#![forbid(unsafe_code)]

/*
Structure-only elaboration: frozen typed trees (Psi 04 output) become an
unsealed Terminal Psi module and the check tables that describe it, for route
one's scalar subset and plain records by structure
(`wiki/drafts/designs/check_on_lowered_form.md`,
`wiki/drafts/designs/places_foundation_lane.md`). It proves nothing and reads
no checker fact. Every obligation the lowered form implies is
minted with its identity and left open; a later checker discharges it.

It runs in two steps, each one route per construct:

- `read` resolves a machine's typed-tree nodes into a closed body (`body`):
  every name becomes a parameter or local slot by its symbol, every type a
  `ScalarType` by its builtin symbol slot or a record type by its data
  declaration, every field a position in that declaration, every callee a
  `MachineId`.
  Its matches over typed-tree node kinds are exhaustive and every kind outside
  the subset is refused as `Unsupported`, naming the kind. Nothing is keyed on
  a spelling: where the typed trees offer only a spelling, the reader refuses.
- `lower` turns that body into blocks, typed values, operations, edges and
  invocations, pushing each row into `MachineTables` as it mints the identity,
  so the module and the tables name the same values, edges and obligations.

Scalar locals are SSA values: a binding or an assignment names the value it
holds from then on, and a write never changes a value. In this subset every
transition arm completes the machine or leaves it with no successor (a crash,
or a call of a machine with no normal completion), so the only joins are the
short-circuit operators, whose join block takes the result as a block
parameter.

A record value is storage. A record formal, and every operation that
produces a record (a literal, a whole copy, a call), is a root place; a
member path is a projection of one, interned on first use; and every
establishment and store is logged in the forward write log as what that one
operation stored (`lower/places.rs`). A data local declared without an
initializer holds its type's zero value; a scalar local declared that way is
reserved storage with no content until a store reaches it. A data type's
`requires` clauses are read with the contract clause reader and owed, as
`DataDomain` obligations, by every operation that forms or changes a value
of the type. A machine with no record has no structural place, and
its place, root and write tables stay empty.

Gaps found against the contract are listed in the crate README, "Findings".
*/

mod body;
mod canonical;
mod components;
mod declare;
mod lower;
mod names;
mod outcome;
mod policy;
mod proof_scope;
mod read;

pub use body::{
    Axis, Body, Borrowed, CaseType, Ceiling, CrashCeiling, DomainSet, Expression, ExpressionKind,
    Policy, Qualification, RecordField, RecordType, Signature, Statement, StorageShape, Strict,
    Target, Termination, ValueType, self_place,
};
pub use names::{Binder, ClauseText, MachineNames, Piece};
pub use outcome::Outcome;

use semantic_vocabulary::{
    BlockId, EdgeId, IdTable, MachineId, ObligationId, OperationId, PackageKeyIdentity,
    StructuralTypeId,
};
use symbols::SymbolHandle;
use terminal_check_tables::{CheckTables, Checked, Construct, Refusal, RefusalReason};
use terminal_psi::TerminalModule;
use typed_trees::TypedTrees;

/// Where one lowered construct came from. A construct that lowering made with
/// no authored counterpart says so and why; it never borrows a neighbouring
/// construct's position (`wiki/drafts/designs/stage_trust.md`: "Pointing back
/// to source never guesses").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOrigin {
    Authored(source::SourceSpan),
    Unpositioned(Unpositioned),
}

/// Why a lowered construct has no source position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unpositioned {
    /// The authored typed-tree node carries an empty span.
    SpanAbsent,
    /// The machine's entry block.
    MachineEntry,
    /// The block that evaluates the right operand of `&&` or `||`.
    ShortCircuitRight,
    /// The join block of `&&` or `||` and the jump into it.
    ShortCircuitJoin,
    /// The block a guarded arm falls through to when its guard is false.
    DispatchContinuation,
    /// The join block of a value-position `match` and the jumps into it.
    MatchJoin,
    /// The head block of one member of a call component lowered as a loop.
    LoopHead,
    /// The Unit completion after a unit machine's last statement.
    ImplicitReturn,
    /// The block a call continues in after the edge that retires the homes
    /// of the record arguments it consumed.
    ArgumentRetirement,
    /// The one call a conformance witness makes of the implementation.
    ConformanceWitness,
}

impl SourceOrigin {
    pub(crate) fn of(span: source::SourceSpan) -> Self {
        if span.span.start < span.span.end {
            Self::Authored(span)
        } else {
            Self::Unpositioned(Unpositioned::SpanAbsent)
        }
    }
}

/// One machine's source correspondence: one row per lowered identity, indexed
/// by that identity and minted in lockstep with it.
#[derive(Debug, Clone, Default)]
pub struct MachineSources {
    pub blocks: IdTable<BlockId, SourceOrigin>,
    pub operations: IdTable<OperationId, SourceOrigin>,
    pub edges: IdTable<EdgeId, SourceOrigin>,
    pub obligations: IdTable<ObligationId, SourceOrigin>,
    /// The Build selection each boundary call of the selection protocol
    /// stands for, in emission order: the operation and the designated
    /// operands the typed trees hold for it. The reference is no operand of
    /// the operation; publication hands it to the Build host beside the
    /// artifact.
    pub designated: Vec<(OperationId, body::DesignatedOperands)>,
    /// The member of the call component each loop head stands for, in
    /// block order (`lower`, `component`). The lowered form does not say
    /// which machine's body a head begins; a diagnostic about an arrival
    /// names it as the callee.
    pub heads: Vec<(BlockId, MachineId)>,
}

/// What one compilation asks to have judged. This is request information: the
/// typed trees also hold declarations of other packages and do not say which
/// package the request checks (`wiki/spec/build/validation_subjects.md`: "The subject
/// set is decided by the request, not by where source lives").
#[derive(Debug, Clone, Copy)]
pub struct Compilation<'a> {
    /// Exact source declarations whose canonical metadata this product needs.
    /// This nominates descriptors, not executable roots or outbound service
    /// demand, and grants no compatibility or installation authority.
    pub declaration_metadata: &'a [SymbolHandle],
    /// The Build's bound program entries. The first is the module's entry.
    pub entries: &'a [SymbolHandle],
    /// Full checks explicitly requested in addition to the entry/selection
    /// closure. `Some(package)` selects that package; `None` selects standalone
    /// user source. An empty slice requests only the concrete root closure.
    pub subjects: &'a [Option<PackageKeyIdentity>],
    /// Which of a subject's two checked contexts this compilation judges
    /// (`wiki/spec/build/scoped_execution.md`, "Two checked contexts"). A
    /// package's Build sources and Product sources are separate programs: a
    /// Build activation's subject declarations are the ones read under the
    /// Build dependency scope, and its entry must come from that context.
    pub purpose: source::DependencyScope,
    /// The declarations the Build's selections name (selected providers and
    /// what each provides, layout-scope entries, chosen representations),
    /// whichever package declares them. The check claims each with its
    /// closure: "the obligations of the Build's selections".
    pub selected: &'a [SymbolHandle],
    /// One visible-continuation shape obligation per executed Build binding.
    /// Deduplicating machine roots must not discard distinct slot contracts.
    pub bindings: &'a [typed_trees::callable_signature_shapes::BoundCallableShape],
}

/// The result of elaborating everything one compilation is responsible for:
/// its entries, what its Build selected, every declaration of its subject,
/// and every machine those call.
#[derive(Debug, Clone)]
pub struct Elaboration {
    /// Unsealed: no proof fields and no evidence. Its only type declarations
    /// are the record types its machines name and the subject declares. In a
    /// compilation's module (`elaborate`) each machine also states the
    /// declaration identity of the typed-tree machine it lowers (`declare`).
    pub module: TerminalModule,
    /// One `MachineTables` per machine. `summaries` stays empty; see the
    /// README, "Findings".
    pub tables: CheckTables,
    pub sources: IdTable<MachineId, MachineSources>,
    /// The boundary requirement each `BoundaryMachineId` of the module
    /// lowers, as `machines` holds the machine of each `MachineId`. Empty
    /// for raw bodies. Publication reads it to tell which boundary calls
    /// are operations of a toolchain Build protocol.
    pub boundaries: IdTable<semantic_vocabulary::BoundaryMachineId, SymbolHandle>,
    /// Nominal storage and field identities from this same typed-tree owner.
    /// Empty for raw bodies, which have no source declaration/application.
    pub source_records: Vec<typed_trees::SourceRecordCorrespondence>,
    /// What diagnostics call each machine's constructs, read from the typed
    /// trees (`names`). Empty for bodies lowered without typed trees
    /// (`lower_bodies`): a message then names identities only.
    pub names: IdTable<MachineId, MachineNames>,
    /// The typed-tree machine each `MachineId` elaborates. The first entry
    /// is 1.
    pub machines: IdTable<MachineId, SymbolHandle>,
    /// Private checked helpers in the complete verification subject whose
    /// input clause was omitted. Every inferred condition must
    /// become a requirement at every invocation before the module is judged.
    /// Written `requires true` is complete even though it leaves no clause.
    /// Entries, selected providers and abstract requirements never appear here.
    pub private_inputs: Vec<MachineId>,
    /// The private subject machines whose callers in this compilation may
    /// use what the body returns beyond its written `ensures`
    /// (`read::Program::local_result_bodies`; state_contracts.md, "Local
    /// call summaries"). The checker derives each summary and proves it on
    /// every normal return before any caller reads it. Empty outside
    /// `elaborate`.
    pub local_results: Vec<MachineId>,
    /// The `requires` clauses of each data type the module declares, by the
    /// type's identity and in authored order, as `RecordType::requires`
    /// states them: the checker's restatement of each type declaration's
    /// `StructuralTypeDeclaration::requires` in `module`, which the
    /// discharge refuses when the two differ. A `DataDomain` obligation
    /// names its clause by position among these.
    pub domains: IdTable<StructuralTypeId, Vec<semantic_vocabulary::Proposition>>,
    /// The machines that stand for a requirement instead of a body: the
    /// rigid machine parameters of a generic check form (`lower_generic`).
    /// Each has its formals, result and contract and no block. Empty for
    /// every concrete module; a module that names one is never sealed,
    /// encoded or run (generics.md: "Generic bodies use the abstract
    /// requirement envelope").
    pub requirements: Vec<MachineId>,
    /// Every call through an implementation parameter that a static
    /// application closed, with the member it selects.
    pub selections: Vec<Selected>,
    /// The requirements that are another generic machine's own contract, at
    /// the type arguments one call in this generic check form's entry
    /// applies it at (`same<T>(left, right)` inside `compare<T>`). A
    /// specialization of the entry calls the machine stage 04 generated
    /// from that machine for the same arguments, whose contract is this one
    /// at them, so no refinement proof stands between the two. Each is
    /// also in `requirements`. Empty for every concrete module.
    pub applications: Vec<MachineId>,
    /// How many of each machine's scalar formals are value parameters
    /// (`const Units: u64`) left rigid by a generic check form: they are the
    /// machine's last scalar formals, in declaration order, and no caller
    /// passes them. Zero for every machine of a concrete module, and empty
    /// for bodies lowered without typed trees.
    pub value_parameters: IdTable<MachineId, usize>,
    /// The literals a requirement's value parameters are bound to in the
    /// machine that stands for it, in declaration order: a requirement read
    /// at the static arguments of a call (`Step::apply<7>(x)`), in a generic
    /// check form or as a conformance witness. Empty for every other
    /// machine, and for bodies lowered without typed trees.
    pub value_arguments: IdTable<MachineId, Vec<semantic_vocabulary::ScalarTerm>>,
    /// The subject's generic machines. Each is a declaration the compilation
    /// must judge, and none is a machine of this module: a generic body is
    /// checked once as written, with its parameters rigid, or once per
    /// alternative of a finite family it lists (`elaborate_generic_forms`).
    /// The machines stage 04 generated from them for the program's own
    /// applications are closed machines of this module. Empty outside
    /// `elaborate`.
    pub generic_templates: Vec<SymbolHandle>,
    /// The operation each implementation the subject declares selects for
    /// each requirement of its trait. Each is a declaration the compilation
    /// must judge, and none is a machine of this module: that the operation
    /// refines the requirement is a statement of its own
    /// (`elaborate_witness`). Empty outside `elaborate`.
    pub conformance_witnesses: Vec<ConformanceWitness>,
    /// The machines of a compilation's module that were given no declaration
    /// identity, each with the reason (`declare`). A module that lists one is
    /// judged like any other and is never published. Empty outside
    /// `elaborate`, whose forms are not programs.
    pub undeclared: Vec<(MachineId, &'static str)>,
}

/// One call through an implementation parameter in a machine stage 04
/// generated for a static application: which machine holds the call, the
/// parameter and the requirement it was written against, and the member the
/// application selected for it. All identities; nothing here is a spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selected {
    pub caller: MachineId,
    pub parameter: SymbolHandle,
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub implementation: MachineId,
    /// The literals the call binds the requirement's value parameters to
    /// (`Selected::apply<7>(x)`), in declaration order, read from the
    /// call's own static arguments; empty when it has none.
    pub value_arguments: Vec<semantic_vocabulary::ScalarTerm>,
}

/// What the subset does not serve, named for the refusal. Closed: a new
/// refusal names one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unserved {
    /// A machine declaration feature outside the subset (generics, reach,
    /// attached data, a missing body, several states, ...).
    Machine(&'static str),
    Parameter(&'static str),
    /// A type that is not a builtin integer, `bool` or a plain record.
    Type(&'static str),
    Contract(&'static str),
    Statement(&'static str),
    Expression(&'static str),
    Call(&'static str),
    Transition(&'static str),
    /// Operand or result types that do not agree.
    Typing(&'static str),
    /// A name or type the typed trees hold only as a spelling.
    Spelling(&'static str),
    /// A completion rule of the machine body (a value machine that can fall
    /// off its last statement, a statement after an unconditional arm).
    Completion(&'static str),
    /// A declaration the check claims and the route does not judge: a
    /// subject declaration or a Build selection of any kind other than a
    /// machine or a plain data type, or one it cannot place in a package.
    Declaration(&'static str),
}

impl Unserved {
    pub fn family(self) -> &'static str {
        match self {
            Self::Machine(_) => "machine",
            Self::Parameter(_) => "parameter",
            Self::Type(_) => "type",
            Self::Contract(_) => "contract",
            Self::Statement(_) => "statement",
            Self::Expression(_) => "expression",
            Self::Call(_) => "call",
            Self::Transition(_) => "transition",
            Self::Typing(_) => "typing",
            Self::Spelling(_) => "spelling",
            Self::Completion(_) => "completion",
            Self::Declaration(_) => "declaration",
        }
    }

    pub fn construct(self) -> &'static str {
        match self {
            Self::Machine(name)
            | Self::Parameter(name)
            | Self::Type(name)
            | Self::Contract(name)
            | Self::Statement(name)
            | Self::Expression(name)
            | Self::Call(name)
            | Self::Transition(name)
            | Self::Typing(name)
            | Self::Spelling(name)
            | Self::Completion(name)
            | Self::Declaration(name) => name,
        }
    }
}

/// An `Unsupported` refusal naming the construct. `detail` starts with
/// `<family> <construct>` so a report can count refusals by construct; that
/// is diagnostic text only and nothing in the compiler reads it back.
pub(crate) fn refuse(machine: MachineId, concerns: Construct, unserved: Unserved) -> Refusal {
    Refusal {
        reason: RefusalReason::Unsupported,
        machine,
        concerns,
        detail: format!("{} {}", unserved.family(), unserved.construct()),
    }
}

/// Elaborates everything `compilation` is responsible for: its entries, what
/// the Build's selections name, every declaration of its subject whether or
/// not anything uses it, and the machines those call. The first entry is
/// `MachineId` 1; the other entries, the selected machines, the subject's
/// machines in declaration order and then callees follow in discovery order.
/// Any refusal refuses the whole compilation, because a module missing a
/// machine the check claims would be a silent drop.
pub fn elaborate(typed: &TypedTrees, compilation: Compilation<'_>) -> Checked<Elaboration> {
    elaborate_for(
        typed,
        compilation,
        terminal_codec::TerminalPublication::Portable,
    )
}

/// Fixes publication custody before any structural identity is minted. An
/// invocation is judged with its actual activation scope, never rewritten
/// from a portable module after acceptance.
pub fn elaborate_for(
    typed: &TypedTrees,
    compilation: Compilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Checked<Elaboration> {
    let mut program = read::Program::new(typed, false);
    program.publication = publication;
    program.declaration_metadata = compilation.declaration_metadata.to_vec();
    let first = MachineId::new(1).expect("1 is nonzero");
    let Some((entry, others)) = compilation.entries.split_first() else {
        // A Terminal module names one entry machine, so a compilation that
        // binds none has no module to elaborate into.
        return Err(refuse(
            first,
            Construct::Machine,
            Unserved::Machine("no bound entry"),
        ));
    };
    program.build_entry_context(first, compilation.purpose, compilation.entries)?;
    let entry = program.machine(*entry)?;
    program.fixed_inputs.push(entry);
    for other in others {
        let other = program.machine(*other)?;
        program.fixed_inputs.push(other);
    }
    program.selected_declarations(entry, compilation.selected)?;
    program.subject_declarations(entry, compilation.subjects, compilation.purpose)?;
    let mut elaboration = finish_reading(program, entry, IdTable::default(), Some(typed))?;
    proof_scope::declare(typed, &mut elaboration)?;
    Ok(elaboration)
}

/// Elaborates one machine and the closure of machines it calls, in discovery
/// order: `entry` is `MachineId` 1. This is not what a compilation claims
/// (`elaborate`): it is the closure of one machine, for a measurement or for
/// carrying a generic body's proofs into one of its specializations. Any
/// refusal refuses the whole closure.
pub fn elaborate_closure(typed: &TypedTrees, entry: SymbolHandle) -> Checked<Elaboration> {
    elaborate_form(typed, entry, false)
}

/// Elaborates the generic check form of `template`, a machine whose generic
/// parameters are implementation parameters and value parameters. Each stays
/// rigid. A call through an implementation parameter names the requirement,
/// which is read for its contract and has no body (`lower_generic`). A value
/// parameter (`const Units: u64`) is a scalar formal after the authored
/// ones, with no definition: what is known of it is what the machine's
/// `requires` say (`Elaboration::value_parameters`). The result is checked
/// once, against the requirements alone, and is never sealed, encoded or
/// run.
pub fn elaborate_generic(typed: &TypedTrees, template: SymbolHandle) -> Checked<Elaboration> {
    elaborate_form(typed, template, true)
}

/// Elaborates a generic machine over a finite type family once per
/// alternative it lists, each with its type parameter fixed to that
/// alternative's type, in the order the family lists them. generics.md,
/// "Static type equality": "Checking the body once per listed alternative,
/// each with `T` fixed to that type, satisfies this rule for a finite
/// family; the list is closed, so those checks cover every admissible use."
/// Each is an ordinary closed check of the body: a branch on `T == u32` is
/// decided there and the other arm is never read. A machine stage 04
/// generated for one alternative is that alternative's body, so its
/// certificates carry into it (`lowered_discharge::reverify_specialization`).
/// Refused unless `template` lists a finite family (`family_alternatives`).
pub fn elaborate_alternatives(
    typed: &TypedTrees,
    template: SymbolHandle,
) -> Checked<Vec<Elaboration>> {
    let first = MachineId::new(1).expect("1 is nonzero");
    let alternatives = read::Program::new(typed, false).family_alternatives(first, template)?;
    alternatives
        .into_iter()
        .map(|arguments| {
            let mut program = read::Program::new(typed, false);
            program.alternative = Some(arguments);
            let entry = program.machine(template)?;
            finish_reading(program, entry, IdTable::default(), None)
        })
        .collect()
}

/// The check forms of the generic machines `templates`, in order: for each,
/// the machine checked once as written with its parameters rigid
/// (`elaborate_generic`), or, when it lists a finite type family, once per
/// alternative (`elaborate_alternatives`). Refused when either form refuses
/// one of them; a refusal names the template's own form.
pub fn elaborate_generic_forms(
    typed: &TypedTrees,
    templates: &[SymbolHandle],
) -> Checked<Vec<Elaboration>> {
    let mut forms = Vec::new();
    for template in templates {
        match elaborate_generic(typed, *template) {
            Ok(form) => forms.push(form),
            Err(rigid) => match elaborate_alternatives(typed, *template) {
                Ok(alternatives) => forms.extend(alternatives),
                Err(_) => return Err(rigid),
            },
        }
    }
    Ok(forms)
}

/// One row of an implementation: the machine it selects for a requirement of
/// its trait. conformances.md, "Binding a machine to a requirement": the
/// machine "refines the complete requirement: parameters, results,
/// preconditions, guarantees", and "A matching signature is insufficient."
/// So each row a subject declares is an obligation of its own, whether or
/// not the program selects the implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConformanceWitness {
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub implementation: SymbolHandle,
}

/// The declaration a machine was generated from, when stage 04 generated it
/// to close a static application: the generic machine it specializes.
pub fn template_of(typed: &TypedTrees, machine: SymbolHandle) -> Option<SymbolHandle> {
    let states = typed
        .machine_states
        .span(typed.machine_by_symbol(machine)?.states)?;
    let [state] = states else {
        return None;
    };
    let declared = typed.symbols.get(state.symbol).generated_from;
    if !declared.is_valid() {
        return None;
    }
    Some(match typed.symbols.get(declared).kind {
        symbols::SymbolKind::State => typed.symbols.get(declared).parent,
        _ => declared,
    })
}

/// Elaborates the conformance witness of `implementation` for `requirement`:
/// machine 1 has the requirement's formals, result and contract, and its
/// body is one call of the implementation with those formals, returning its
/// result. Checking it is checking that the implementation refines the
/// requirement (machines.md, "Machines and contract refinement"): its call
/// requirements are the implementation's `requires` under the requirement's,
/// and its guarantees the requirement's `ensures` under the implementation's.
/// It is a statement to prove, not a program: nothing calls or runs it.
///
/// `arguments` are the literals of the requirement's value parameters, in
/// declaration order (`Selected::value_arguments`): the requirement's
/// contract is read at them, and `implementation` is then the machine stage
/// 04 generated for the member at the same arguments. Empty for a
/// requirement with no value parameter.
pub fn elaborate_witness(
    typed: &TypedTrees,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
    arguments: &[semantic_vocabulary::ScalarTerm],
    implementation: SymbolHandle,
) -> Checked<Elaboration> {
    let mut program = read::Program::new(typed, false);
    let entry = program.witnessed(declaring_trait, requirement, arguments)?;
    let callee = program.machine(implementation)?;
    elaborate_witness_call(program, entry, callee)
}

/// The exact declaration supplying a callable's published contract. An applied
/// signature is already instantiated by the typed-tree application owner; this
/// view neither discovers a selection nor invents application arguments.
#[derive(Clone, Copy)]
pub enum CallableDeclaration<'a> {
    Signature(&'a typed_trees::signature::StateSignature),
    Machine(SymbolHandle),
}

/// The logical refinement obligation between two published callable contracts.
/// The witness has the required entry condition and one call to the supplied
/// contract. Its normal exits owe the required guarantees. The supplied body is
/// not read: its published contract is an abstraction, whose implementation is
/// checked independently. Type, lifetime and operational formation remain
/// separate obligations of the caller of this operation.
pub fn elaborate_callable_witness<'a>(
    typed: &'a TypedTrees,
    required: &'a typed_trees::signature::StateSignature,
    supplied: CallableDeclaration<'a>,
) -> Checked<Elaboration> {
    let mut program = read::Program::new(typed, false);
    let entry = program.witnessed_signature(required, &[])?;
    let callee = program.callable_contract(supplied)?;
    elaborate_witness_call(program, entry, callee)
}

fn elaborate_witness_call(
    program: read::Program<'_>,
    entry: MachineId,
    callee: MachineId,
) -> Checked<Elaboration> {
    let required =
        terminal_check_tables::row(&program.signatures, entry, entry, Construct::Machine)?;
    let supplied =
        terminal_check_tables::row(&program.signatures, callee, entry, Construct::Machine)?;
    if required.parameters != supplied.parameters || required.result != supplied.result {
        return Err(refuse(
            entry,
            Construct::Machine,
            Unserved::Typing("implementation signature differs from the requirement's"),
        ));
    }
    let source = SourceOrigin::Unpositioned(Unpositioned::ConformanceWitness);
    let call = Expression {
        kind: ExpressionKind::Call {
            callee: callee.into(),
            arguments: required
                .parameters
                .iter()
                .enumerate()
                .map(|(slot, value_type)| Expression {
                    kind: ExpressionKind::Slot(slot),
                    result: Some(*value_type),
                    source,
                })
                .collect(),
            designated: None,
        },
        result: required.result,
        source,
    };
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    bodies.push(Body {
        slots: required.parameters.clone(),
        statements: vec![match required.result {
            Some(_) => Statement::Arm {
                guard: None,
                target: Target::Value(call),
                source,
            },
            None => Statement::Evaluate(call),
        }],
    });
    finish_reading(program, entry, bodies, None)
}

fn elaborate_form(typed: &TypedTrees, entry: SymbolHandle, rigid: bool) -> Checked<Elaboration> {
    let mut program = read::Program::new(typed, rigid);
    let entry = program.machine(entry)?;
    finish_reading(program, entry, IdTable::default(), None)
}

/// Reads every body not yet in `bodies`, callees as they are discovered,
/// and lowers the closure. `declared` is the typed trees when the closure is a
/// compilation's program, whose machines are then given their declaration
/// identities (`declare`).
fn finish_reading(
    mut program: read::Program<'_>,
    entry: MachineId,
    mut bodies: IdTable<MachineId, Body>,
    declared: Option<&TypedTrees>,
) -> Checked<Elaboration> {
    // `program.machine` appends callees while bodies are read, so the bound
    // is re-read on every step.
    while bodies.len() < program.symbols.len() {
        let next = MachineId::new(bodies.len() as u64 + 1).ok_or_else(|| {
            refuse(
                entry,
                Construct::Machine,
                Unserved::Machine("identity overflow"),
            )
        })?;
        let body = program.body(next)?;
        bodies.push(body);
    }
    let private_inputs = program.private_input_contracts(entry)?;
    let local_results = program.local_result_bodies(entry)?;
    program.begin_declaration_storage(entry, bodies.len())?;
    let declarations = program.declaration_metadata.clone();
    let calling_abis = program.prepare_calling_abis(entry, &declarations)?;
    let mut elaboration = lower_generic_for(
        entry,
        &bodies,
        body::Signatures {
            machines: &program.signatures,
            boundaries: &program.boundary_signatures,
            completions: &program.boundary_completions,
        },
        &program.records,
        &program.requirements,
        program.publication,
        Some(&program),
    )?;
    calling_abis.publish(&mut elaboration.module)?;
    if let Some(typed) = declared {
        declare::declare(
            typed,
            &program.symbols,
            &program.signatures,
            &mut elaboration,
        );
    }
    elaboration.source_records = program.source_record_correspondence(entry)?;
    elaboration.machines = program.symbols;
    elaboration.boundaries = program.boundary_symbols;
    elaboration.private_inputs = private_inputs;
    elaboration.local_results = local_results;
    elaboration.names = program.names;
    elaboration.selections = program.selections;
    elaboration.applications = program.applications;
    elaboration.value_parameters = program.value_parameters;
    elaboration.value_arguments = program.value_arguments;
    elaboration.generic_templates = program.generic_templates;
    elaboration.conformance_witnesses = program.conformance_witnesses;
    Ok(elaboration)
}

/// Lowers closed bodies that were already read; `entry` is machine 1.
/// `elaborate` reads typed trees and then calls this; a checker's tests call
/// it directly to state programs. `records` holds every record type the
/// bodies and signatures name, a nested record before its owner; a scalar
/// program passes an empty table. A call cycle is lowered as a loop when
/// `components` serves it and refused by name otherwise. `machines` is left empty: these bodies have no typed-tree
/// machine.
pub fn lower_bodies(
    entry: MachineId,
    bodies: &IdTable<MachineId, Body>,
    signatures: &IdTable<MachineId, Signature>,
    records: &IdTable<StructuralTypeId, RecordType>,
) -> Checked<Elaboration> {
    lower_generic(entry, bodies, signatures, records, &[])
}

/// `lower_bodies` for the generic check form: a generic body lowered once
/// with each of its machine parameters left rigid. A machine listed in
/// `requirements` is such a parameter. It has a signature, which is the
/// requirement's contract, and no body: its row in `bodies` holds its
/// formals as slots and no statement, and it lowers to a machine with no
/// block. A call of it is an ordinary direct call, so the caller's
/// obligations and knowledge are stated against the requirement's contract
/// by the same route as any callee's.
///
/// A requirement that leaves `crashes` to its selected member (generics.md,
/// "Requirements and conformance evidence": "omitted `crashes` defers to its
/// published routes") publishes no route here, and the dependency has no
/// row: the generic form then decides nothing about crashes through that
/// call. Each specialization is a concrete module, where the selected
/// member's own routes are read and a `crashes none` they break is refused.
pub fn lower_generic(
    entry: MachineId,
    bodies: &IdTable<MachineId, Body>,
    signatures: &IdTable<MachineId, Signature>,
    records: &IdTable<StructuralTypeId, RecordType>,
    requirements: &[MachineId],
) -> Checked<Elaboration> {
    lower_generic_for(
        entry,
        bodies,
        body::Signatures {
            machines: signatures,
            boundaries: &IdTable::default(),
            completions: &IdTable::default(),
        },
        records,
        requirements,
        terminal_codec::TerminalPublication::Portable,
        None,
    )
}

fn lower_generic_for(
    entry: MachineId,
    bodies: &IdTable<MachineId, Body>,
    signatures: body::Signatures<'_>,
    records: &IdTable<StructuralTypeId, RecordType>,
    requirements: &[MachineId],
    publication: terminal_codec::TerminalPublication,
    declarations: Option<&read::Program<'_>>,
) -> Checked<Elaboration> {
    for requirement in requirements {
        let body = terminal_check_tables::row(bodies, *requirement, entry, Construct::Machine)?;
        if !body.statements.is_empty() {
            return Err(refuse(
                *requirement,
                Construct::Machine,
                Unserved::Machine("requirement with a body"),
            ));
        }
    }
    let components = components::components(bodies, signatures.machines)?;
    let outcomes = outcome::outcomes(bodies, signatures, requirements)?;
    let mut module = TerminalModule::for_entry(entry);
    // What running the module's entry may reach is what the entry publishes.
    module.root_service_reach.concrete =
        terminal_check_tables::row(&outcomes, entry, entry, Construct::Machine)?
            .service_reach
            .clone();
    // Terminal Psi declares each record type once for the module, with the
    // interned identity its declaration form requires. A module with no
    // record declares none and keeps the empty identity table.
    let identities = match publication {
        terminal_codec::TerminalPublication::Portable => {
            terminal_psi::TerminalIdentityTableBuilder::default()
        }
        terminal_codec::TerminalPublication::Invocation => {
            terminal_psi::TerminalIdentityTableBuilder::for_invocation()
        }
    };
    let declared = lower::places::declare(entry, records, bodies, signatures, identities)?;
    module.identities = declared.identities;
    module.structural_types = declared.structural_types;
    module.native_layouts = declared.native_layouts;
    if let Some(source) = declarations {
        source.publish_services(entry, &mut module)?;
        source.publish_boundaries(entry, signatures, &outcomes, &declared.records, &mut module)?;
    } else if !signatures.boundaries.is_empty() {
        return Err(refuse(
            entry,
            Construct::Machine,
            Unserved::Declaration("boundary contracts have no exact source owner"),
        ));
    }
    let mut tables = CheckTables::default();
    let mut sources = IdTable::default();
    for position in 1..=bodies.len() as u64 {
        let id = MachineId::new(position).ok_or_else(|| {
            refuse(
                entry,
                Construct::Machine,
                Unserved::Machine("identity overflow"),
            )
        })?;
        let component = terminal_check_tables::row(&components, id, id, Construct::Machine)?;
        let lowered = lower::machine(
            id,
            bodies,
            component,
            signatures,
            &outcomes,
            &declared.records,
            requirements.contains(&id),
        )?;
        module.machines.push(lowered.machine);
        // Machines are lowered in identity order and each machine's heads in
        // block order, which is the order the module's roster requires.
        module.scalar_block_invariants.extend(lowered.invariants);
        tables.machines.push(lowered.tables);
        sources.push(lowered.sources);
    }
    for machine in &mut module.machines {
        let plan = terminal_semantics::plain_affine::plan(machine, &module.structural_types)
            .map_err(|error| {
                let (reason, detail) = match error {
                    terminal_semantics::plain_affine::Error::Unsupported(detail) => {
                        (terminal_check_tables::RefusalReason::Unsupported, detail)
                    }
                    terminal_semantics::plain_affine::Error::Malformed(detail) => (
                        terminal_check_tables::RefusalReason::ForeignIdentity,
                        detail,
                    ),
                };
                terminal_check_tables::Refusal {
                    reason,
                    machine: machine.id,
                    concerns: Construct::Machine,
                    detail: detail.to_owned(),
                }
            })?;
        // Planning records disposal; it does not establish ownership. The
        // checker reconstructs unavailable uses and the complete edge schedule.
        for block in &mut machine.blocks {
            use terminal_psi::Terminator;
            let append = |edge, actions: &mut Vec<terminal_psi::TerminalAffineCleanupAction>| {
                actions.retain(|action| !plan.affine_roots.contains(action.place()));
                if let Ok(position) = plan.cleanup.binary_search_by_key(&edge, |(edge, _)| *edge) {
                    actions.extend(plan.cleanup[position].1.iter().cloned());
                }
            };
            match &mut block.terminator {
                Terminator::Jump {
                    edge,
                    cleanup_actions,
                    ..
                }
                | Terminator::Return {
                    edge,
                    cleanup_actions,
                    ..
                }
                | Terminator::ReturnStructural {
                    edge,
                    cleanup_actions,
                    ..
                }
                | Terminator::ReturnUnit {
                    edge,
                    cleanup_actions,
                } => append(*edge, cleanup_actions),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    append(when_true.edge, &mut when_true.cleanup_actions);
                    append(when_false.edge, &mut when_false.cleanup_actions);
                }
                Terminator::StructuralCase { cases, .. } => {
                    for case in cases {
                        append(case.edge, &mut case.cleanup_actions);
                    }
                }
                Terminator::Crash { .. }
                | Terminator::InvokeNoNormal { .. }
                | Terminator::ExternalCompletion { .. } => {}
            }
        }
    }
    // This is the emitted behavior before proving or publication. Keeping its
    // inventory here lets product consumers retain the exact judged module.
    module.unoptimized_behavior = module
        .machines
        .iter()
        .map(terminal_psi::UnoptimizedMachineBehavior::capture)
        .collect();
    let mut domains = IdTable::default();
    for (_, record) in records.iter() {
        domains.push(record.requires.clone());
    }
    Ok(Elaboration {
        module,
        tables,
        sources,
        boundaries: IdTable::default(),
        source_records: Vec::new(),
        names: IdTable::default(),
        machines: IdTable::default(),
        private_inputs: Vec::new(),
        local_results: Vec::new(),
        domains,
        requirements: requirements.to_vec(),
        selections: Vec::new(),
        applications: Vec::new(),
        value_parameters: IdTable::default(),
        value_arguments: IdTable::default(),
        generic_templates: Vec::new(),
        conformance_witnesses: Vec::new(),
        undeclared: Vec::new(),
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod publication_scope_tests {
    use super::*;

    #[test]
    fn invocation_scope_is_minted_before_lowering_even_without_records() {
        let entry = MachineId::new(1).unwrap();
        let mut bodies = IdTable::default();
        bodies.push(Body {
            slots: Vec::new(),
            statements: Vec::new(),
        });
        let mut signatures = IdTable::default();
        signatures.push(Signature {
            parameters: Vec::new(),
            erased: Vec::new(),
            borrowed: Vec::new(),
            result: None,
            requires: Vec::new(),
            ensures: Vec::new(),
            reaches: crate::Ceiling::Inferred,
            invokes: crate::Ceiling::Inferred,
            crashes: CrashCeiling::Authored(Vec::new()),
            termination: Termination::Silent,
        });
        let lower = |publication| {
            lower_generic_for(
                entry,
                &bodies,
                body::Signatures {
                    machines: &signatures,
                    boundaries: &IdTable::default(),
                    completions: &IdTable::default(),
                },
                &IdTable::default(),
                &[],
                publication,
                None,
            )
            .unwrap()
        };
        let first = lower(terminal_codec::TerminalPublication::Invocation);
        let second = lower(terminal_codec::TerminalPublication::Invocation);
        let portable = lower(terminal_codec::TerminalPublication::Portable);
        assert!(portable.module.identities.invocation_scope().is_none());
        let first_scope = first.module.identities.invocation_scope().unwrap();
        let second_scope = second.module.identities.invocation_scope().unwrap();
        assert_ne!(first_scope, second_scope);
        let first_code = terminal_codec::InvocationTerminalCode::new(first.module).unwrap();
        let second_code = terminal_codec::InvocationTerminalCode::new(second.module).unwrap();
        assert_ne!(
            first_code.view().custody_identity(),
            second_code.view().custody_identity()
        );
    }
}
