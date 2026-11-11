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
    BlockId, EdgeId, IdTable, MachineId, ObligationId, OperationId, StructuralTypeId,
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

/// The result of elaborating one entry machine and every machine it calls.
#[derive(Debug, Clone)]
pub struct Elaboration {
    /// Unsealed: no proof fields and no evidence. Its only declarations are
    /// the record types its machines name.
    pub module: TerminalModule,
    /// One `MachineTables` per machine. `summaries` stays empty; see the
    /// README, "Findings".
    pub tables: CheckTables,
    pub sources: IdTable<MachineId, MachineSources>,
    /// The typed-tree machine each `MachineId` elaborates. The entry is 1.
    pub machines: IdTable<MachineId, SymbolHandle>,
    /// The `requires` clauses of each data type the module declares, by the
    /// type's identity and in authored order, as `RecordType::requires`
    /// states them. They ride here because Terminal Psi's type declaration
    /// has no slot for them: the current pipeline proves them on checked
    /// trees and its Terminal module carries none. A `DataDomain` obligation
    /// names its clause by position among these.
    pub domains: IdTable<StructuralTypeId, Vec<semantic_vocabulary::Proposition>>,
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
            | Self::Completion(name) => name,
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

/// Elaborates `entry` and the closure of machines it calls, in discovery
/// order: the entry is `MachineId` 1. Any refusal refuses the whole closure,
/// because a module missing a callee would be a silent drop.
pub fn elaborate(typed: &TypedTrees, entry: SymbolHandle) -> Checked<Elaboration> {
    let mut program = read::Program::new(typed);
    let entry = program.machine(entry)?;
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
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
    let mut elaboration = lower_bodies(entry, &bodies, &program.signatures, &program.records)?;
    elaboration.machines = program.symbols;
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
    let outcomes = outcome::outcomes(bodies, signatures)?;
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
        let lowered = lower::machine(id, body, signatures, &outcomes, &declared.records)?;
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
    })
}

#[cfg(test)]
mod tests;
