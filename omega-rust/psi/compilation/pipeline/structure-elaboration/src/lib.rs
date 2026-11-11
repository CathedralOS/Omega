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
mod lower;
mod outcome;
mod read;

pub use body::{
    Body, CaseType, CrashCeiling, Expression, ExpressionKind, RecordField, RecordType, Signature,
    Statement, Strict, Target, ValueType, self_place,
};
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
}

/// What one compilation asks to have judged. This is request information: the
/// typed trees also hold declarations of other packages and do not say which
/// package the request checks (`wiki/spec/build/validation_subjects.md`: "The subject
/// set is decided by the request, not by where source lives").
#[derive(Debug, Clone, Copy)]
pub struct Compilation<'a> {
    /// The Build's bound program entries. The first is the module's entry.
    pub entries: &'a [SymbolHandle],
    /// The package the request checks. `None` for a source that belongs to
    /// no admitted package; such a source is its own subject.
    pub subject: Option<PackageKeyIdentity>,
    /// The declarations the Build's selections name (selected providers and
    /// what each provides, layout-scope entries, chosen representations),
    /// whichever package declares them. The check claims each with its
    /// closure: "the obligations of the Build's selections".
    pub selected: &'a [SymbolHandle],
}

/// The result of elaborating everything one compilation is responsible for:
/// its entries, what its Build selected, every declaration of its subject,
/// and every machine those call.
#[derive(Debug, Clone)]
pub struct Elaboration {
    /// Unsealed: no proof fields and no evidence. Its only declarations are
    /// the record types its machines name and the subject declares.
    pub module: TerminalModule,
    /// One `MachineTables` per machine. `summaries` stays empty; see the
    /// README, "Findings".
    pub tables: CheckTables,
    pub sources: IdTable<MachineId, MachineSources>,
    /// The typed-tree machine each `MachineId` elaborates. The first entry
    /// is 1.
    pub machines: IdTable<MachineId, SymbolHandle>,
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
}

/// One call through an implementation parameter in a machine stage 04
/// generated for a static application: which machine holds the call, the
/// parameter and the requirement it was written against, and the member the
/// application selected for it. All identities; nothing here is a spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selected {
    pub caller: MachineId,
    pub parameter: SymbolHandle,
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
    pub implementation: MachineId,
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
    let mut program = read::Program::new(typed, false);
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
    let entry = program.machine(*entry)?;
    for other in others {
        program.machine(*other)?;
    }
    program.selected_declarations(entry, compilation.selected)?;
    program.subject_declarations(entry, compilation.subject)?;
    finish_reading(program, entry, IdTable::default())
}

/// Elaborates one machine and the closure of machines it calls, in discovery
/// order: `entry` is `MachineId` 1. This is not what a compilation claims
/// (`elaborate`): it is the closure of one machine, for a measurement or for
/// carrying a generic body's proofs into one of its specializations. Any
/// refusal refuses the whole closure.
pub fn elaborate_closure(typed: &TypedTrees, entry: SymbolHandle) -> Checked<Elaboration> {
    elaborate_form(typed, entry, false)
}

/// Elaborates the generic check form of `template`, a machine whose only
/// generic parameters are implementation parameters. Each parameter stays
/// rigid: a call through one names the requirement, which is read for its
/// contract and has no body (`lower_generic`). The result is checked once,
/// against the requirements alone, and is never sealed, encoded or run.
pub fn elaborate_generic(typed: &TypedTrees, template: SymbolHandle) -> Checked<Elaboration> {
    elaborate_form(typed, template, true)
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
pub fn elaborate_witness(
    typed: &TypedTrees,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
    implementation: SymbolHandle,
) -> Checked<Elaboration> {
    let mut program = read::Program::new(typed, false);
    let entry = program.witnessed(declaring_trait, requirement)?;
    let callee = program.machine(implementation)?;
    program.forwards(entry, callee);
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
            callee,
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
    finish_reading(program, entry, bodies)
}

fn elaborate_form(typed: &TypedTrees, entry: SymbolHandle, rigid: bool) -> Checked<Elaboration> {
    let mut program = read::Program::new(typed, rigid);
    let entry = program.machine(entry)?;
    finish_reading(program, entry, IdTable::default())
}

/// Reads every body not yet in `bodies`, callees as they are discovered,
/// and lowers the closure.
fn finish_reading(
    mut program: read::Program<'_>,
    entry: MachineId,
    mut bodies: IdTable<MachineId, Body>,
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
    program.refuse_recursion()?;
    let mut elaboration = lower_generic(
        entry,
        &bodies,
        &program.signatures,
        &program.records,
        &program.requirements,
    )?;
    elaboration.machines = program.symbols;
    elaboration.selections = program.selections;
    Ok(elaboration)
}

/// Lowers closed bodies that were already read; `entry` is machine 1.
/// `elaborate` reads typed trees and then calls this; a checker's tests call
/// it directly to state programs. `records` holds every record type the
/// bodies and signatures name, a nested record before its owner; a scalar
/// program passes an empty table. The reader refuses recursion first; a
/// cycle passed here directly is refused by the outcome pass, which walks
/// callees first. `machines` is left empty: these bodies have no typed-tree
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
    let outcomes = outcome::outcomes(bodies, signatures, requirements)?;
    let mut module = TerminalModule::for_entry(entry);
    // Terminal Psi declares each record type once for the module, with the
    // interned identity its declaration form requires. A module with no
    // record declares none and keeps the empty identity table.
    let declared = lower::places::declare(entry, records, bodies)?;
    if !declared.structural_types.is_empty() {
        module.identities = declared.identities;
        module.structural_types = declared.structural_types;
        module.native_layouts = declared.native_layouts;
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
        let body = terminal_check_tables::row(bodies, id, id, Construct::Machine)?;
        let lowered = lower::machine(
            id,
            body,
            signatures,
            &outcomes,
            &declared.records,
            requirements.contains(&id),
        )?;
        module.machines.push(lowered.machine);
        tables.machines.push(lowered.tables);
        sources.push(lowered.sources);
    }
    let mut domains = IdTable::default();
    for (_, record) in records.iter() {
        domains.push(record.requires.clone());
    }
    Ok(Elaboration {
        module,
        tables,
        sources,
        machines: IdTable::default(),
        domains,
        requirements: requirements.to_vec(),
        selections: Vec::new(),
    })
}

#[cfg(test)]
mod tests;
