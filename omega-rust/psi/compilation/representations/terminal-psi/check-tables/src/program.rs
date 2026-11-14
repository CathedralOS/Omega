//! Complete unsealed Terminal program and its retained source correspondence.
//! Source handles are opaque publication/diagnostic custody, never proof facts.

use crate::CheckTables;
use crate::names::MachineNames;
use semantic_vocabulary::{
    BlockId, EdgeId, IdTable, MachineId, ObligationId, OperationId, StructuralTypeId,
};
use symbols::SymbolHandle;
use terminal_psi::TerminalModule;

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
    pub fn of(span: source::SourceSpan) -> Self {
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
    /// The member of the call component each loop head stands for, in
    /// block order (`lower`, `component`). The lowered form does not say
    /// which machine's body a head begins; a diagnostic about an arrival
    /// names it as the callee.
    pub heads: Vec<(BlockId, MachineId)>,
    /// The block each `value as +D` of a domain with predicates continues
    /// in (`lower`, `prove_membership`), with the domain and predicates the
    /// cast owes as written. The lowered form holds only the block's
    /// invariant; a diagnostic about the arrival names the domain.
    pub memberships: Vec<(BlockId, String)>,
    /// The statements lowering left out because the call before them never
    /// returns, one row per such call (`lower`, `body`). The lowered form
    /// holds nothing of them, so the advisory warning about them is read
    /// from here (`Elaboration::unreachable_statements`).
    pub unreachable: Vec<Unreachable>,
    /// Each authored transition arm that only some arrivals take, one row
    /// per arm (`lower`, `statement`). Whether an arm can be arrived at is
    /// the checker's question; the lowered form does not say which blocks
    /// begin an authored arm, so the advisory warning about an arm no
    /// arrival can take is read from here.
    pub branches: Vec<Branch>,
}

/// One authored arm of a transition and where control decides to take it.
/// `wiki/spec/language/state_contracts.md`, "Unreachable branches": "issue an
/// unreachable-branch warning when an authored branch contradicts its
/// machine's entry contract or established source facts independent of
/// specialization. The diagnostic identifies the branch condition and the
/// conflicting premises". `block` is the arm's first block, which the edge
/// of `decided_in`'s conditional alone enters, so the facts known at its
/// entry are the facts before the branch with its condition added; `arm`
/// is the authored arm and `condition` the guard that decides it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Branch {
    pub block: BlockId,
    pub decided_in: BlockId,
    pub arm: SourceOrigin,
    pub condition: SourceOrigin,
}

/// Executable statements written after a call that never returns.
/// `wiki/spec/language/state_contracts.md`, "Statements after a call that
/// never returns": "the call has no normal successor. Executable statements
/// written after it on that path are unreachable, not inherently erroneous".
/// `statement` is the first of them and `call` the call before it: "Issue
/// one advisory warning at the first unreachable executable statement, with
/// a note locating the call that prevents continuation, not one per
/// following statement."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unreachable {
    pub statement: SourceOrigin,
    pub call: SourceOrigin,
}

/// One requirement a chosen provider supplies and the machine of the module
/// that realizes it. `provider` and `requirement` are declarations of the
/// judged source, compared by identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuppliedRequirement {
    pub provider: SymbolHandle,
    pub requirement: SymbolHandle,
    pub machine: MachineId,
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
    /// One checking-table group per machine, in the module's identity order.
    pub tables: CheckTables,
    pub sources: IdTable<MachineId, MachineSources>,
    /// The boundary requirement each `BoundaryMachineId` of the module
    /// lowers, as `machines` holds the machine of each `MachineId`. Empty
    /// for raw bodies. Publication reads it to tell which boundary calls
    /// are operations of a toolchain Build protocol.
    pub boundaries: IdTable<semantic_vocabulary::BoundaryMachineId, SymbolHandle>,
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
    /// The machine of this module each chosen provider gives each
    /// requirement of the slot it supplies (`Compilation::supplies`), in
    /// supply order. One module holds the providers chosen for every
    /// target, so two rows may name one requirement under different
    /// providers; a consumer follows a requirement into the machine of the
    /// provider it settled on, never into "the" provider of the requirement.
    /// Empty outside `elaborate`.
    pub supplied: Vec<SuppliedRequirement>,
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

/// Symbolic root of a data invariant, substituted before observing storage.
pub fn self_place() -> Option<semantic_vocabulary::PlaceId> {
    semantic_vocabulary::PlaceId::new(1)
}

impl Elaboration {
    /// The unreachable statements this program's authored machines hold,
    /// each with its machine, in machine order.
    ///
    /// A machine whose declaration says it was generated to close a static
    /// application is left out: what is unreachable there may be so only
    /// for that application's arguments (state_contracts.md:
    /// "Unreachability arises only through generic specialization or
    /// compile-time selection: accepted; no warning solely for that
    /// specialization"). A generic machine whose statements are unreachable
    /// as written says so in its own check form, where its parameters are
    /// rigid and a requirement is called through its interface contract.
    pub fn unreachable_statements(&self) -> impl Iterator<Item = (MachineId, Unreachable)> + '_ {
        self.authored_sources().flat_map(|(machine, sources)| {
            sources.unreachable.iter().map(move |row| (machine, *row))
        })
    }

    /// The authored transition arms of this program's authored machines,
    /// each with its machine, in machine order. Generated machines are left
    /// out for the reason `unreachable_statements` gives: "Do not warn
    /// merely because ordinary generic specialization or compile-time
    /// selection removes an alternative; a generic branch that already
    /// contradicts its own contract is not exempt" (state_contracts.md,
    /// "Unreachable branches"), and the generic machine's own check form is
    /// where it is judged as written.
    pub fn authored_branches(&self) -> impl Iterator<Item = (MachineId, Branch)> + '_ {
        self.authored_sources()
            .flat_map(|(machine, sources)| sources.branches.iter().map(move |row| (machine, *row)))
    }

    /// The source rows of every machine that is not generated to close a
    /// static application.
    fn authored_sources(&self) -> impl Iterator<Item = (MachineId, &MachineSources)> + '_ {
        self.sources.iter().filter(|(machine, _)| {
            !self.module.machines.iter().any(|declared| {
                declared.id == *machine
                    && declared
                        .declaration
                        .as_ref()
                        .is_some_and(|declaration| declaration.specialization.is_some())
            })
        })
    }
}
