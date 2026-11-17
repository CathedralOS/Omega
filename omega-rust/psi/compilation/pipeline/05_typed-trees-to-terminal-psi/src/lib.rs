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
mod boundaries;
mod callable_formations;
mod callable_headers;
mod callables;
mod calling_publication;
mod compilation;
mod publication;
use publication::PreparedProgram;
pub use publication::{
    DesignatedOperands, SourceCorrespondence, SourceFormCorrespondence, SourceOrigin,
};
mod canonical;
pub use canonical::substituted_routes as substituted_call_routes;
mod components;
mod declare;
mod external_providers;
mod inhabitation;
mod lower;

mod operational;
mod outcome;
mod policy;
mod proof_scope;
pub mod provider_selections;
mod qualifications;
mod reach;
mod read;
mod reference_ends;
mod reference_results;
pub mod selection_questions;
mod static_contracts;
mod suspension;

/// What every elaboration of one typed program reads in common. A
/// compilation, or a public entry point, prepares it once and lends it to
/// the closure, each conformance witness and each generic form it reads, so
/// none of them derives it again.
type SharedReading = std::sync::Arc<SharedProgram>;
fn shared_reading(typed: &TypedTrees) -> SharedReading {
    std::sync::Arc::new(SharedProgram {
        captures: typed_trees::NamedConformanceCaptureIndex::new(typed),
        calling_catalogs: Default::default(),
    })
}

pub(crate) struct SharedProgram {
    pub captures: typed_trees::NamedConformanceCaptureIndex,
    /// The service catalog that supplies boundary calling ABIs, one slot per
    /// dependency purpose (`Product`, `Build`). The catalog depends only on
    /// the typed program and that purpose, so the first reading that asks for
    /// boundary metadata prepares it and every later reading of the same
    /// program and purpose borrows the same one.
    calling_catalogs: [std::sync::OnceLock<CallingCatalog>; 2],
}

type CallingCatalog =
    Result<std::sync::Arc<typed_trees::service_declarations::PreparedServiceCatalog>, String>;

impl SharedProgram {
    pub(crate) fn calling_catalog(
        &self,
        typed: &TypedTrees,
        purpose: source::DependencyScope,
    ) -> &CallingCatalog {
        let slot = match purpose {
            source::DependencyScope::Product => 0,
            source::DependencyScope::Build => 1,
        };
        self.calling_catalogs[slot].get_or_init(|| {
            let context = typed_trees::service_declarations::provider_declarations::ProviderPlanningContext::new(
                typed, purpose,
            );
            // This catalog supplies source signatures, not selection
            // authority. The coordinator authenticates the nominated handles
            // against its admitted source registry before constructing the
            // compilation.
            let sources =
                source::source_identity::toolchain_source_identities(typed.symbols.source_files())?;
            Ok(std::sync::Arc::new(
                typed_trees::service_declarations::PreparedServiceCatalog::prepare(
                    context, &sources,
                ),
            ))
        })
    }

    /// How many purposes have prepared a calling catalog, for tests that pin
    /// one preparation per program and purpose.
    #[cfg(test)]
    pub(crate) fn prepared_calling_catalogs(&self) -> usize {
        self.calling_catalogs
            .iter()
            .filter(|slot| slot.get().is_some())
            .count()
    }
}

pub use body::{
    Axis, Body, Borrowed, BorrowedReferent, CaseType, Ceiling, CrashCeiling, DomainInstance,
    DomainSet, Expression, ExpressionKind, Policy, Qualification, RecordField, RecordType,
    Selector, Signature, Statement, StorageShape, Strict, Target, Termination, ValueType,
    self_place,
};
pub use callable_formations::check_bound_callable_shapes;
pub use callable_headers::{CallableCrashPolicy, CallableHeader};
pub use compilation::{PreparedSource, prepare_compilation};
pub use external_providers::{ExternalProviderCandidate, ProviderContractHeader};
pub use outcome::Outcome;
pub use reach::ServiceReach;
pub use read::calling::{CallingPolicySelections, ProgramEntryContractSelection};
pub use static_contracts::{
    StaticCallableEnvelope, StaticConformanceQuestion, StaticImplementation,
    StaticProviderCandidate, StaticRequirementSchema, StaticSchemaQuestion, StaticServiceCeiling,
};
// Installed logical calls share source preparation with ordinary refinement;
// their canonical witnesses are judged by the next stage and never published.
pub use terminal_check_tables::compilation::{
    CallableArrivalWitness, CallableInputOrigin, NominalPremiseOrigin,
};
pub use terminal_check_tables::names::{Binder, ClauseText, MachineNames, Piece};

use semantic_vocabulary::{IdTable, MachineId, PackageKeyIdentity, StructuralTypeId};
use symbols::SymbolHandle;
pub use terminal_check_tables::contracts::RequirementCeilings;
use terminal_check_tables::{CheckTables, Checked, Construct, Refusal, RefusalReason};
use terminal_psi::TerminalModule;
use typed_trees::TypedTrees;

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
    /// An empty slice checks the declared subjects and selections in a
    /// declaration module with no executable entry.
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
    /// Exact source applications selected for calling policies and entry ABI
    /// publication. They are consumed before body closure, never by a later
    /// target stage reopening typed source.
    pub calling: Option<CallingPolicySelections<'a>>,
    /// Build/default precedence has selected these exact source providers.
    /// Their schemas and choice coordinates are published in the module.
    pub providers: Option<provider_selections::ProviderSelections<'a>>,
    /// The implementations chosen for nominal requirements of this compilation:
    /// the Build's own selections, and the supply settlement chose for each
    /// requirement the compilation reaches. The check claims each provider's
    /// machines for the slot as roots and owes, for each, the proof that the
    /// machine refines the requirement it supplies (`Supply`).
    pub supplies: &'a [Supply],
}

/// One chosen implementation of a nominal slot. `slot` is the declaration
/// the selection names: a trait, which the provider must cover requirement
/// by requirement, or one exact trait requirement. Ordinary trait bindings
/// grant no boundary operation, service identity or native authority.
/// `provider` is the named implementation or the data type whose machines
/// state `satisfies` for the slot's requirements. Both are declarations of
/// the judged source, compared by identity and never by spelling.
///
/// provider_selection.md: "The assembled record must satisfy the slot's
/// complete schema, operation contracts and evidence constraints", and
/// "Checked providers must fit that row". So each supplied requirement must
/// be given exactly one machine by the provider, that machine and everything
/// it calls is judged like any other root, and the machine must refine the
/// requirement: signature, contracts, service reach, callback entries and
/// crash routes (`Elaboration::conformance_witnesses`).
pub use typed_trees::trait_definition::Supply;

pub use terminal_check_tables::program::{
    Branch, ConformanceWitness, Elaboration, MachineSources, PredicateSource, Selected,
    SuppliedRequirement, Unpositioned, Unreachable,
};

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
    /// A completion or control-flow form the route cannot judge yet.
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
        source_span: None,
        detail: format!("{} {}", unserved.family(), unserved.construct()),
    }
}

/// A `Violation` refusal: the program breaks a rule of the language at `at`,
/// and `rule` states the rule and how. The route publishes it as a rejection
/// of the program. Only for a form the specification makes an error in every
/// program: one the reader merely cannot judge is `refuse`, because a
/// rejection must never be said of a valid program.
pub(crate) fn reject(machine: MachineId, at: SourceOrigin, rule: impl Into<String>) -> Refusal {
    Refusal {
        reason: RefusalReason::Violation,
        machine,
        concerns: Construct::Machine,
        source_span: at.span(),
        detail: rule.into(),
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
    // The first refusal read is the one a reading that stops there meets;
    // `prepare_compilation` reports the rest beside it.
    let mut prepared = elaborate_source_for(typed, compilation, publication, shared_reading(typed))
        .map_err(|mut refused| refused.swap_remove(0))?;
    publication::finalize_source(
        typed,
        &mut prepared.program,
        &prepared.selected_calls,
        prepared.pending_publication.take(),
    )?;
    Ok(prepared.program)
}

fn elaborate_source_for<'source>(
    typed: &'source TypedTrees,
    compilation: Compilation<'_>,
    publication: terminal_codec::TerminalPublication,
    captures: SharedReading,
) -> Result<PreparedProgram<'source>, Vec<Refusal>> {
    let mut program = read::Program::with_captures(typed, false, compilation.purpose, captures);
    program.publication = publication;
    program.declaration_metadata = compilation.declaration_metadata.to_vec();
    let first = MachineId::new(1).expect("1 is nonzero");
    let alone = |refused: Refusal| vec![refused];
    program
        .build_entry_context(first, compilation.purpose, compilation.entries)
        .map_err(alone)?;
    // Every claimed declaration and every body is read even after one is
    // refused, so one compilation names every machine the checker cannot
    // read yet (`read::Program::keep`). The first entry is the exception:
    // it is machine 1, the frame every other refusal is reported in.
    program.collected = Some(Vec::new());
    let entry = if let Some((entry, others)) = compilation.entries.split_first() {
        let entry = program.machine(*entry).map_err(alone)?;
        program.fixed_inputs.push(entry);
        for other in others {
            match program.machine(*other) {
                Ok(other) => program.fixed_inputs.push(other),
                Err(refused) => program.keep(Err(refused)).map_err(alone)?,
            }
        }
        Some(entry)
    } else {
        // A source check still claims every subject declaration. It creates
        // a declaration module, not an executable entry inferred from a name
        // or from whichever subject machine happens to be read first.
        None
    };
    for selected in compilation.selected {
        let read = program.selected_declarations(first, std::slice::from_ref(selected));
        program.keep(read).map_err(alone)?;
    }
    for supply in compilation.supplies {
        let read = program.supplied_slots(first, std::slice::from_ref(supply));
        program.keep(read).map_err(alone)?;
    }
    program
        .subject_declarations(first, compilation.subjects, compilation.purpose)
        .map_err(alone)?;
    let bodies = read_bodies(&mut program, first).map_err(alone)?;
    // Past reading, each step works over the whole closure, so the first
    // refusal there ends it, except the storage plan of each machine
    // (`finish_storage`).
    let refused = program.collected.take().unwrap_or_default();
    if !refused.is_empty() {
        return Err(refused);
    }
    let mut further = Vec::new();
    let mut elaboration = finish_reading_into(
        program,
        entry,
        bodies,
        Some(typed),
        compilation.calling,
        compilation.providers,
        &mut further,
    )
    .map_err(|first| {
        let mut refused = vec![first];
        refused.append(&mut further);
        refused
    })?;
    elaboration.selected_calls =
        compilation::prepare_selected_signatures(typed, &elaboration.program, &elaboration.source)
            .map_err(alone)?;
    Ok(elaboration)
}

/// Reads the body of every machine discovered so far, and of each callee
/// those bodies discover. A refused body is recorded (`read::Program::keep`)
/// and the next is read; the table then holds only the bodies before the
/// first refused one, and is used only when nothing was refused.
fn read_bodies(
    program: &mut read::Program<'_>,
    diagnostic: MachineId,
) -> Checked<IdTable<MachineId, Body>> {
    let mut bodies = IdTable::default();
    let mut complete = true;
    let mut position = 0;
    // `program.body` appends callees while bodies are read, so the bound is
    // re-read on every step.
    loop {
        program.resolve_scalar_applications(diagnostic)?;
        if position == program.symbols.len() {
            break;
        }
        position += 1;
        let next = MachineId::new(position as u64).ok_or_else(|| {
            refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Machine("identity overflow"),
            )
        })?;
        match program.body(next) {
            Ok(body) if complete => {
                bodies.push(body);
            }
            Ok(_) => {}
            Err(refused) => {
                program.keep(Err(refused))?;
                complete = false;
            }
        }
    }
    Ok(bodies)
}

/// Elaborates one machine and the closure of machines it calls, in discovery
/// order: `entry` is `MachineId` 1. This is not what a compilation claims
/// (`elaborate`): it is the closure of one machine, for a measurement or for
/// carrying a generic body's proofs into one of its specializations. Any
/// refusal refuses the whole closure.
pub fn elaborate_closure(typed: &TypedTrees, entry: SymbolHandle) -> Checked<Elaboration> {
    elaborate_form(
        typed,
        declaration_context(typed, entry),
        entry,
        false,
        shared_reading(typed),
        terminal_codec::TerminalPublication::Portable,
    )
}

/// Single-declaration APIs use that declaration's checked source context.
/// Source-free representation fixtures retain the Product interpretation.
fn declaration_context(typed: &TypedTrees, declaration: SymbolHandle) -> source::DependencyScope {
    typed
        .symbols
        .symbol_provenance_source_span(declaration)
        .and_then(|span| typed.symbols.source_file(span))
        .map(|source| source.dependency_scope)
        .unwrap_or(source::DependencyScope::Product)
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
    elaborate_form(
        typed,
        declaration_context(typed, template),
        template,
        true,
        shared_reading(typed),
        terminal_codec::TerminalPublication::Portable,
    )
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
/// certificates carry into it (`terminal_psi_to_terminal_psi::reverify_specialization`).
/// Refused unless `template` lists a finite family (`family_alternatives`).
pub fn elaborate_alternatives(
    typed: &TypedTrees,
    template: SymbolHandle,
) -> Checked<Vec<Elaboration>> {
    prepare_alternatives(
        typed,
        declaration_context(typed, template),
        template,
        shared_reading(typed),
        terminal_codec::TerminalPublication::Portable,
        None,
    )
    .and_then(|forms| {
        forms
            .into_iter()
            .map(PreparedProgram::into_check_form)
            .collect()
    })
}

fn prepare_alternatives<'source>(
    typed: &'source TypedTrees,
    purpose: source::DependencyScope,
    template: SymbolHandle,
    captures: SharedReading,
    publication: terminal_codec::TerminalPublication,
    opaque_refusal: Option<Refusal>,
) -> Checked<Vec<PreparedProgram<'source>>> {
    let first = MachineId::new(1).expect("1 is nonzero");
    let alternatives = read::Program::with_captures(typed, false, purpose, captures.clone())
        .family_alternatives(first, template)
        // Preserve the opaque refusal only when no complete partition exists.
        // Once its coverage is known, a failing member reports its own cause.
        .map_err(|refusal| opaque_refusal.unwrap_or(refusal))?;
    alternatives
        .into_iter()
        .map(|arguments| {
            let mut program = read::Program::with_captures(typed, false, purpose, captures.clone());
            program.publication = publication;
            program.alternative = Some(arguments);
            let entry = program.machine(template)?;
            finish_reading(program, Some(entry), IdTable::default(), None, None, None)
        })
        .collect()
}

/// The check forms of the generic machines `templates`, in order: for each,
/// the machine checked once with opaque parameters when its bounds suffice,
/// otherwise once per member of its complete static type partition. An open
/// classifier includes an opaque remainder beyond its closed cases. Refused
/// when a required alternative refuses; its diagnostic names that form.
pub fn elaborate_generic_forms(
    typed: &TypedTrees,
    templates: &[SymbolHandle],
) -> Checked<Vec<Elaboration>> {
    let captures = shared_reading(typed);
    let mut forms = Vec::new();
    for template in templates {
        forms.extend(
            prepare_generic_forms(
                typed,
                declaration_context(typed, *template),
                &[*template],
                captures.clone(),
                terminal_codec::TerminalPublication::Portable,
            )?
            .into_iter()
            .map(PreparedProgram::into_check_form)
            .collect::<Checked<Vec<_>>>()?,
        );
    }
    Ok(forms)
}

fn prepare_generic_forms<'source>(
    typed: &'source TypedTrees,
    purpose: source::DependencyScope,
    templates: &[SymbolHandle],
    captures: SharedReading,
    publication: terminal_codec::TerminalPublication,
) -> Checked<Vec<PreparedProgram<'source>>> {
    let mut forms = Vec::new();
    for template in templates {
        if let Some(declared) = typed.trait_definition_by_symbol(*template) {
            let mut program = read::Program::with_captures(typed, true, purpose, captures.clone());
            program.publication = publication;
            program.generic_trait(declared)?;
            forms.push(finish_reading(
                program,
                None,
                IdTable::default(),
                None,
                None,
                None,
            )?);
            continue;
        }
        match prepare_form(
            typed,
            purpose,
            *template,
            true,
            captures.clone(),
            publication,
        ) {
            Ok(form) => forms.push(form),
            Err(rigid) => forms.extend(prepare_alternatives(
                typed,
                purpose,
                *template,
                captures.clone(),
                publication,
                Some(rigid),
            )?),
        }
    }
    Ok(forms)
}

/// Source forms include an owning map's independent telescope even when
/// its member declares no additional parameters of its own.
pub(crate) fn is_generic_template(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> bool {
    machine.has_specialization_parameters()
        || typed
            .owned_conformance_realization(machine.symbol)
            .is_some_and(|(owner, _)| {
                !owner.type_parameters.is_empty() || !owner.lifetime_parameters.is_empty()
            })
}

/// The declaration a machine was generated from, when stage 04 generated it
/// to close a static application: the generic machine it specializes.
pub fn template_of(typed: &TypedTrees, machine: SymbolHandle) -> Option<SymbolHandle> {
    typed
        .executable_application_template(machine)
        .ok()
        .flatten()
}

struct WitnessInputs<'scope> {
    captures: SharedReading,
    purpose: source::DependencyScope,
    outcome_subjects: Option<&'scope [Option<PackageKeyIdentity>]>,
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
    prepare_witness(
        typed,
        declaring_trait,
        requirement,
        arguments,
        implementation,
        None,
        WitnessInputs {
            captures: shared_reading(typed),
            purpose: declaration_context(typed, implementation),
            outcome_subjects: None,
        },
    )
    .and_then(PreparedProgram::into_check_form)
}

fn prepare_witness<'source>(
    typed: &'source TypedTrees,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
    arguments: &[semantic_vocabulary::ScalarTerm],
    implementation: SymbolHandle,
    expected: Option<&'source typed_trees::signature::StateSignature>,
    inputs: WitnessInputs<'_>,
) -> Checked<PreparedProgram<'source>> {
    let generic = typed
        .machine_by_symbol(implementation)
        .is_some_and(|machine| is_generic_template(typed, machine));
    let mut program = read::Program::with_captures(typed, generic, inputs.purpose, inputs.captures);
    program.outcome_subjects = inputs.outcome_subjects.map(|subjects| subjects.to_vec());
    let entry = program.witnessed(
        declaring_trait,
        requirement,
        arguments,
        implementation,
        expected,
    )?;
    let callee = program.implementation(implementation, declaring_trait, requirement)?;
    let bodies = witness_call(&mut program, entry, callee)?;
    let mut prepared = finish_reading(program, Some(entry), bodies, None, None, None)?;
    if typed
        .trait_machine_signature(requirement)
        .is_some_and(|(owner, signature)| {
            owner.is_boundary && signature.service_reach_is_installation_bound
        })
    {
        prepared.program.installation_bound_refinement = Some(entry);
    }
    Ok(prepared)
}

/// The declaration supplying a callable's published contract. An applied
/// signature is already instantiated by the typed-tree application owner.
#[derive(Clone, Copy)]
pub enum CallableDeclaration<'a> {
    Signature(&'a typed_trees::signature::StateSignature),
    Machine(SymbolHandle),
}

/// Prepare the logical refinement witness without reading the supplied body.
pub fn elaborate_callable_witness<'a>(
    typed: &'a TypedTrees,
    required: &'a typed_trees::signature::StateSignature,
    supplied: CallableDeclaration<'a>,
) -> Checked<Elaboration> {
    let mut program = read::Program::with_captures(
        typed,
        false,
        declaration_context(typed, required.symbol),
        shared_reading(typed),
    );
    let entry = program.witnessed_signature(required, &[], None)?;
    let callee = program.callable_contract(supplied)?;
    let bodies = witness_call(&mut program, entry, callee)?;
    finish_logical_witness(program, entry, bodies).and_then(PreparedProgram::into_check_form)
}

/// Form the same positional call obligation for a body conformance or a
/// purely logical callable contract. Each entrance retains its actual body
/// ownership and publication rules when it finishes this checking form.
fn witness_call(
    program: &mut read::Program<'_>,
    entry: MachineId,
    callee: MachineId,
) -> Checked<IdTable<MachineId, Body>> {
    let required =
        terminal_check_tables::row(&program.signatures, entry, entry, Construct::Machine)?.clone();
    let supplied =
        terminal_check_tables::row(&program.signatures, callee, entry, Construct::Machine)?.clone();
    let supplied_machine = program
        .symbols
        .get(callee)
        .copied()
        .unwrap_or_else(SymbolHandle::invalid);
    // The implementation's lifetimes are its own; it refines the
    // requirement when its signature is the requirement's once they are
    // instantiated, as the witness's call instantiates them.
    if !program.same_signature(entry, &required, &supplied, supplied_machine)? {
        return Err(refuse(
            entry,
            Construct::Machine,
            Unserved::Typing("implementation signature differs from the requirement's"),
        ));
    }
    let source = SourceOrigin::Unpositioned(Unpositioned::ConformanceWitness);
    let call = Expression {
        call_context: Some(terminal_psi::CallAcknowledgement::generated()),
        kind: ExpressionKind::Call {
            callee: callee.into(),
            arguments: required
                .parameters
                .iter()
                .enumerate()
                .map(|(slot, value_type)| {
                    let place = Expression {
                        call_context: None,
                        kind: ExpressionKind::Slot(slot),
                        result: Some(*value_type),
                        source,
                    };
                    // The witness forwards each formal's original passing
                    // mode. A borrowed formal denotes incoming storage;
                    // forwarding its value alone would read a scalar or move
                    // its carrier instead of reborrowing that storage.
                    match required.borrowed(slot) {
                        Some(borrowed) => Expression {
                            call_context: None,
                            kind: ExpressionKind::Borrow {
                                access: borrowed.access,
                                place: Box::new(place),
                                home: borrowed.home.clone(),
                            },
                            result: Some(*value_type),
                            source,
                        },
                        None => place,
                    }
                })
                .collect(),
            designated: None,
            required_contract: None,
            static_reach_binding: None,
        },
        result: required.result,
        source,
    };
    let mut bodies: IdTable<MachineId, Body> = IdTable::default();
    bodies.push(callable_refinement_body(&required, call));
    Ok(bodies)
}

/// A callable refines its requirement by making this call under the
/// requirement's inputs and proving its normal completion afterward. Both
/// ordinary and projected witnesses use the canonical call/return operations;
/// the callee's guarantees become available only after that call completes.
pub(crate) fn callable_refinement_body(required: &body::Signature, call: Expression) -> Body {
    let source = call.source;
    Body {
        slots: required.parameters.clone(),
        statements: vec![match required.result {
            Some(_) => Statement::Arm {
                guard: None,
                target: Target::Value(call),
                source,
            },
            None => Statement::Evaluate {
                expression: call,
                discards_result: false,
            },
        }],
    }
}

/// A logical witness has abstract dependencies and no executable closure. Read
/// no dependency body and publish no inferred operational promise for it.
fn finish_logical_witness<'source>(
    mut program: read::Program<'source>,
    entry: MachineId,
    mut bodies: IdTable<MachineId, Body>,
) -> Checked<PreparedProgram<'source>> {
    while bodies.len() < program.signatures.len() {
        let id = MachineId::new(bodies.len() as u64 + 1).ok_or_else(|| {
            refuse(
                entry,
                Construct::Machine,
                Unserved::Machine("identity overflow"),
            )
        })?;
        let signature =
            terminal_check_tables::row(&program.signatures, id, entry, Construct::Machine)?;
        bodies.push(Body {
            slots: signature.parameters.clone(),
            statements: Vec::new(),
        });
        if !program.requirements.contains(&id) {
            program.requirements.push(id);
        }
    }
    program.prepare_domain_predicates(entry)?;
    program.prepare_envelope_policies()?;
    let services = program.service_map(entry)?;
    let source_selection_questions = capture_source_questions(&mut program, entry)?;
    let components = prepare_call_components(&bodies, &mut program.signatures)?;
    let mut witness = lower_program(
        Some(entry),
        &bodies,
        &components,
        body::Signatures {
            machines: &program.signatures,
            boundaries: &program.boundaries,
            declarations: &program.declaration_signatures,
            declaration_aliases: None,
        },
        &program.records,
        &program.requirements,
        &program.callables,
        &program.boundaries,
        &services,
        &program.external_providers,
        &program.static_contracts,
        program.publication,
        Some(program.typed),
        Some(&program),
    )?;
    // Abstract ceilings still name real applied services. Retain their exact
    // declarations without deriving executable reach from the synthetic body.
    witness.program.reach.names = services.publish(entry, &mut witness.program.module)?;
    witness.source.records = program.source_record_correspondence(entry)?;
    witness.program.machines = program.symbols;
    witness.program.names = program.names;
    witness.program.value_parameters = program.value_parameters;
    witness.program.value_arguments = program.value_arguments;
    witness.program.applications = program.applications;
    witness.program.selections = program.selections;
    witness.source.selection_questions = Some(source_selection_questions);
    Ok(witness)
}

fn capture_source_questions(
    program: &mut read::Program<'_>,
    entry: MachineId,
) -> Checked<selection_questions::RawSelectionQuestions> {
    program.completed_operators.finish();
    selection_questions::capture_with_bodies(
        program.typed,
        &program.admitted_declarations,
        &program.admitted_bodies,
        &program.completed_operators,
        &program.admitted_signature_views,
        &program.generic_templates,
    )
    .map_err(|error| Refusal {
        source_span: None,
        machine: entry,
        concerns: Construct::Machine,
        reason: RefusalReason::Unsupported,
        detail: error.to_string(),
    })
}

fn elaborate_form(
    typed: &TypedTrees,
    purpose: source::DependencyScope,
    entry: SymbolHandle,
    rigid: bool,
    captures: SharedReading,
    publication: terminal_codec::TerminalPublication,
) -> Checked<Elaboration> {
    prepare_form(typed, purpose, entry, rigid, captures, publication)
        .and_then(PreparedProgram::into_check_form)
}

fn prepare_form<'source>(
    typed: &'source TypedTrees,
    purpose: source::DependencyScope,
    entry: SymbolHandle,
    rigid: bool,
    captures: SharedReading,
    publication: terminal_codec::TerminalPublication,
) -> Checked<PreparedProgram<'source>> {
    let mut program = read::Program::with_captures(typed, rigid, purpose, captures);
    program.publication = publication;
    let entry = program.machine(entry)?;
    finish_reading(program, Some(entry), IdTable::default(), None, None, None)
}

/// Reads every body not yet in `bodies`, callees as they are discovered,
/// and lowers the closure. `declared` is the typed trees when the closure is a
/// compilation's program, whose machines are then given their declaration
/// identities (`declare`). This owner publishes their proof scopes once after
/// the declarations are formed.
fn finish_reading<'source>(
    program: read::Program<'source>,
    entry: Option<MachineId>,
    bodies: IdTable<MachineId, Body>,
    declared: Option<&'source TypedTrees>,
    calling: Option<CallingPolicySelections<'_>>,
    providers: Option<provider_selections::ProviderSelections<'_>>,
) -> Checked<PreparedProgram<'source>> {
    finish_reading_into(
        program,
        entry,
        bodies,
        declared,
        calling,
        providers,
        &mut Vec::new(),
    )
}

/// `finish_reading`, putting each machine refused after the first one, where
/// each machine is refused on its own, into `further`.
fn finish_reading_into<'source>(
    mut program: read::Program<'source>,
    entry: Option<MachineId>,
    mut bodies: IdTable<MachineId, Body>,
    declared: Option<&'source TypedTrees>,
    calling: Option<CallingPolicySelections<'_>>,
    providers: Option<provider_selections::ProviderSelections<'_>>,
    further: &mut Vec<Refusal>,
) -> Checked<PreparedProgram<'source>> {
    // Refusals need a coordinate even for an empty declaration module. This
    // diagnostic anchor is never installed as an entry or an input boundary.
    let diagnostic = entry.unwrap_or_else(|| MachineId::new(1).expect("1 is nonzero"));
    let selected_providers = providers
        .map(|selected| program.prepare_provider_selections(diagnostic, selected))
        .transpose()?;
    let mut policies = read::calling::SourceCallingPolicies::default();
    let mut binder_reach = IdTable::default();
    let components = loop {
        program.prepare_calling_policies(diagnostic, calling, &mut policies)?;
        let mut predicates_prepared = false;
        loop {
            program.resolve_scalar_applications(diagnostic)?;
            program.finish_cleanup_selections()?;
            if binder_reach.len() == program.symbols.len() {
                program.prepare_calling_policy_roots(diagnostic)?;
                if binder_reach.len() != program.symbols.len() {
                    continue;
                }
                if predicates_prepared {
                    break;
                }
                program.prepare_domain_predicates(diagnostic)?;
                predicates_prepared = true;
                continue;
            }
            let next = MachineId::new(binder_reach.len() as u64 + 1).ok_or_else(|| {
                refuse(
                    diagnostic,
                    Construct::Machine,
                    Unserved::Machine("identity overflow"),
                )
            })?;
            if bodies.get(next).is_none() {
                bodies.push(program.body(next)?);
            }
            binder_reach.push(program.application_binders(next)?);
        }
        let components = prepare_call_components(&bodies, &mut program.signatures)?;
        program.finish_operational_envelopes(&bodies, &components)?;
        program.prepare_service_bindings(diagnostic)?;
        if !program.prepare_calling_policies(diagnostic, calling, &mut policies)? {
            break components;
        }
    };
    let private_inputs = program.private_input_contracts(entry, diagnostic)?;
    program.finish_operational_envelopes(&bodies, &components)?;
    program.finish_declarations(diagnostic)?;
    program.prepare_external_providers(diagnostic)?;
    let services = program.service_map(diagnostic)?;
    program.require_complete_boundary_parent_contracts(diagnostic, &policies)?;
    let policies = (!policies.requests.is_empty()).then_some(policies);
    program.begin_declaration_storage(diagnostic, bodies.len())?;
    let declarations = program.declaration_metadata.clone();
    let calling_abis = program.prepare_calling_abis(diagnostic, &declarations)?;
    let descriptions = policies
        .as_ref()
        .map(|policies| program.prepare_calling_storage(diagnostic, policies))
        .transpose()?;
    program.prepare_domain_predicates(diagnostic)?;
    program.finish_cleanup_selections()?;
    let source_selection_questions = capture_source_questions(&mut program, diagnostic)?;
    let PreparedProgram {
        program: mut elaboration,
        outcomes,
        mut source,
        ..
    } = lower_program(
        entry,
        &bodies,
        &components,
        body::Signatures {
            machines: &program.signatures,
            boundaries: &program.boundaries,
            declarations: &program.declaration_signatures,
            declaration_aliases: Some(&program.declaration_aliases),
        },
        &program.records,
        &program.requirements,
        &program.callables,
        &program.boundaries,
        &services,
        &program.external_providers,
        &program.static_contracts,
        program.publication,
        Some(program.typed),
        Some(&program),
    )?;
    elaboration.machines = program.symbols.clone();
    elaboration.boundaries = program.boundaries.rows.map(|_, source| source.source);
    source.selection_questions = Some(source_selection_questions);
    crate::reach::publish(entry, &services, &mut elaboration)?;
    calling_abis.publish(&mut elaboration.module)?;
    program.publish_record_layouts(diagnostic, &mut elaboration.module)?;
    let declaration_error = |reason| {
        refuse(
            diagnostic,
            Construct::Machine,
            Unserved::Declaration(reason),
        )
    };
    // Real typed bodies retain their actual declarations in check forms too.
    // A synthetic frame has no such declaration and is recorded as undeclared.
    let prepared_applications = declare::declare(
        program.typed,
        &program,
        &outcomes,
        &binder_reach,
        &mut elaboration,
    )
    .map_err(declaration_error)?;
    if let Some(typed) = declared {
        elaboration.provider_candidate_intents = declare::provider_candidates(
            typed,
            &program.symbols,
            &elaboration.boundaries,
            &program.supplied,
            &elaboration.static_conformances,
            &mut elaboration.module,
        )
        .map_err(declaration_error)?;
    } else {
        if !elaboration.check_form_envelopes.is_empty() {
            return Err(declaration_error(
                "check-form envelopes were already retained",
            ));
        }
        elaboration
            .undeclared
            .sort_unstable_by_key(|(machine, _)| *machine);
        let mut requirements = elaboration.requirements.clone();
        requirements.sort_unstable();
        for slot in &mut elaboration.module.callables.machine_envelopes {
            // A bodyless requirement retains its exact abstract ceiling in
            // requirement_ceilings, not a named executable declaration.
            if slot
                .as_ref()
                .is_some_and(|row| requirements.binary_search(&row.machine).is_ok())
            {
                *slot = None;
                continue;
            }
            if slot.as_ref().is_some_and(|row| {
                elaboration
                    .undeclared
                    .binary_search_by_key(&row.machine, |(machine, _)| *machine)
                    .is_ok()
            }) && let Some(row) = slot.take()
            {
                elaboration.check_form_envelopes.push(row);
            }
        }
    }
    suspension::publish_crossings(&mut elaboration.module).map_err(declaration_error)?;
    qualifications::publish_access_plans(diagnostic, &program, &mut elaboration.module)?;
    source.records = program.source_record_correspondence(diagnostic)?;
    program.publish_source_declarations(diagnostic, &source.records, &mut elaboration.module)?;
    source.readiness = program.publish_readiness(diagnostic, &mut elaboration.module)?;
    for selected in &mut program.selections {
        if let Some(required) = selected.required_contract {
            selected.required_contract =
                Some(*program.declaration_aliases.get(required).ok_or_else(|| {
                    refuse(
                        diagnostic,
                        Construct::Machine,
                        Unserved::Declaration("selected witness lost its public header coordinate"),
                    )
                })?);
        }
    }
    reference_results::publish_requirements(
        &mut elaboration.module,
        &program,
        &program.requirements,
    )?;
    reference_results::publish(&mut elaboration, &program.selections)?;
    finish_storage(diagnostic, &mut elaboration, further)?;
    elaboration.machines = program.symbols;
    elaboration.private_inputs = private_inputs;
    elaboration.names = program.names;
    elaboration.selections = program.selections;
    elaboration.applications = program.applications;
    elaboration.value_parameters = program.value_parameters;
    elaboration.generic_binders = program.generic_binders;
    elaboration.value_arguments = program.value_arguments;
    elaboration.generic_templates = program.generic_templates;
    elaboration.conformance_witnesses = program.conformance_witnesses;
    let calling = match (policies, descriptions) {
        (Some(policies), Some(descriptions)) => Some((policies, descriptions)),
        (None, None) => None,
        _ => {
            return Err(read::services::unavailable(
                diagnostic,
                "prepared calling policy lost its canonical storage descriptions",
            ));
        }
    };
    // Calling records are publication output, and only the program form
    // (`declared`) publishes. A check form still prepared and validated each
    // policy its bodies reach (`require_complete_boundary_parent_contracts`
    // above) because its verdict depends on those contracts; the program form
    // that reaches the same boundaries prepares and publishes their records
    // itself. A check form therefore carries none forward.
    let calling = calling.filter(|_| declared.is_some());
    let pending_publication = (calling.is_some() || selected_providers.is_some()).then_some(
        publication::PendingSourcePublication {
            source: program.typed,
            context: diagnostic,
            calling,
            providers: selected_providers,
        },
    );
    elaboration.supplied = program.supplied;
    Ok(PreparedProgram {
        program: elaboration,
        outcomes,
        source,
        selected_calls: Vec::new(),
        prepared_applications,
        pending_publication,
    })
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
    // This model-level entrance borrows its caller's headers. Source preparation
    // above already owns and completes the same signature table in place.
    lower_generic_for(
        Some(entry),
        bodies,
        body::Signatures {
            declarations: &IdTable::default(),
            declaration_aliases: None,
            machines: signatures,
            boundaries: &boundaries::BoundaryNamespace::default(),
        },
        records,
        requirements,
        terminal_codec::TerminalPublication::Portable,
        None,
    )
    .and_then(PreparedProgram::into_check_form)
}

fn lower_generic_for<'source>(
    entry: Option<MachineId>,
    bodies: &IdTable<MachineId, Body>,
    signatures: body::Signatures<'_>,
    records: &IdTable<StructuralTypeId, RecordType>,
    requirements: &[MachineId],
    publication: terminal_codec::TerminalPublication,
    source: Option<&'source read::Program<'source>>,
) -> Checked<PreparedProgram<'source>> {
    let mut machine_signatures = signatures.machines.clone();
    let components = prepare_call_components(bodies, &mut machine_signatures)?;
    let signatures = body::Signatures {
        machines: &machine_signatures,
        ..signatures
    };
    lower_program(
        entry,
        bodies,
        &components,
        signatures,
        records,
        requirements,
        &callables::CallableNamespace::default(),
        &boundaries::BoundaryNamespace::default(),
        &reach::ServiceMap::default(),
        &external_providers::ExternalProviderNamespace::default(),
        &static_contracts::StaticContracts::default(),
        publication,
        source.map(|source| source.typed),
        source,
    )
}

/// Validate call positions and prepare the graph once for this body closure.
/// Progress inference and lowering borrow the same component/ranking data.
fn prepare_call_components(
    bodies: &IdTable<MachineId, Body>,
    signatures: &mut IdTable<MachineId, Signature>,
) -> Checked<components::Components> {
    for (machine, body) in bodies.iter() {
        operational::positions(machine, body)?;
    }
    components::components(bodies, signatures)
}

fn lower_program<'source>(
    entry: Option<MachineId>,
    bodies: &IdTable<MachineId, Body>,
    components: &components::Components,
    signatures: body::Signatures<'_>,
    records: &IdTable<StructuralTypeId, RecordType>,
    requirements: &[MachineId],
    callables: &callables::CallableNamespace,
    boundaries: &boundaries::BoundaryNamespace,
    services: &reach::ServiceMap,
    external_providers: &external_providers::ExternalProviderNamespace,
    static_contracts: &static_contracts::StaticContracts,
    publication: terminal_codec::TerminalPublication,
    typed_source: Option<&'source TypedTrees>,
    declarations: Option<&read::Program<'source>>,
) -> Checked<PreparedProgram<'source>> {
    let diagnostic = entry.unwrap_or_else(|| MachineId::new(1).expect("1 is nonzero"));
    let mut required_ids = requirements.to_vec();
    required_ids.sort_unstable();
    if required_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(refuse(
            diagnostic,
            Construct::Machine,
            Unserved::Machine("abstract requirements repeat a declaration identity"),
        ));
    }
    for requirement in requirements {
        signatures.get((*requirement).into(), diagnostic)?;
        let body =
            terminal_check_tables::row(bodies, *requirement, diagnostic, Construct::Machine)?;
        if !body.statements.is_empty() {
            return Err(refuse(
                *requirement,
                Construct::Machine,
                Unserved::Machine("requirement with a body"),
            ));
        }
    }
    let requirement_ceilings = if requirements.is_empty() {
        IdTable::default()
    } else {
        if !signatures.machines.keys().eq(bodies.keys()) {
            return Err(refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Machine(
                    "abstract signature clauses do not cover the exact body namespace",
                ),
            ));
        }
        if let Some(source) = declarations {
            source.abstract_requirement_ceilings(services)?
        } else {
            signatures.machines.map(|_, signature| RequirementCeilings {
                reaches: signature.reaches.clone(),
                invokes: match &signature.invokes {
                    crate::Ceiling::Authored(row) => Some(
                        row.iter()
                            .copied()
                            .map(terminal_psi::TerminalCallableInvocation::Service)
                            .collect(),
                    ),
                    crate::Ceiling::Inferred => None,
                },
                suspension: signature.operational.suspension,
                blocking: signature.operational.blocking,
                termination: match signature.termination {
                    crate::body::Termination::Silent => {
                        terminal_psi::TerminalCallableTermination::NotGuaranteed
                    }
                    crate::body::Termination::Promised => {
                        terminal_psi::TerminalCallableTermination::Terminates {
                            premises: Vec::new(),
                        }
                    }
                },
            })
        }
    };
    // One source owner fixes the local inference boundary. Result clauses
    // retain their private publication restriction; local non-return does not
    // become a public clause and can use every eligible body in that unit.
    let local_summaries = declarations
        .map(|source| source.local_summary_bodies(entry))
        .transpose()?;
    let mut module = match entry {
        Some(entry) => TerminalModule::for_entry(entry),
        None => TerminalModule::declarations(),
    };
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
    let declared = lower::places::declare(
        diagnostic,
        records,
        bodies,
        signatures
            .machines
            .values()
            .chain(signatures.declarations.values())
            .chain(
                callables
                    .contracts
                    .values()
                    .map(|contract| &contract.signature),
            )
            .chain(boundaries.rows.values().map(|boundary| &boundary.signature))
            .chain(
                external_providers
                    .rows
                    .iter()
                    .map(|provider| &provider.signature),
            )
            .chain(
                static_contracts
                    .conformances
                    .iter()
                    .map(|question| &question.signature),
            ),
        declarations.map_or(&[][..], |source| source.byte_domains.as_slice()),
        typed_source,
        identities,
    )?;
    let outcomes = outcome::outcomes(
        bodies,
        signatures,
        requirements,
        callables,
        local_summaries
            .as_ref()
            .map(|local| local.outcomes.as_slice()),
        &declared.records.inhabitation,
        components,
    )?;
    // Declaration checking publishes no executable arrival or root reach.
    if let Some(entry) = entry {
        module.root_service_reach.concrete =
            terminal_check_tables::row(&outcomes, entry, entry, Construct::Machine)?
                .service_reach
                .clone();
    }
    module.identities = declared.identities;
    module.structural_types = declared.structural_types;
    module.native_layouts = declared.native_layouts;
    let mut tables = CheckTables::default();
    let mut qualifications = qualifications::Qualifications::declare(
        diagnostic,
        declarations,
        std::mem::take(&mut module.identities),
        declared.structural_domains,
    )?;
    for (_, record) in records.iter() {
        for field in record
            .fields()
            .iter()
            .chain(record.cases().iter().flat_map(|case| &case.fields))
        {
            if let ValueType::Scalar(scalar_type, qualifier) = field.value_type {
                let set = qualifications.scalar(diagnostic, scalar_type, qualifier)?;
                if !set.is_empty() {
                    qualifications
                        .catalog
                        .fields
                        .push(terminal_psi::ScalarFieldQualification {
                            field: field.id,
                            value_type: semantic_vocabulary::QualifiedScalarType {
                                scalar_type,
                                qualifications: set,
                            },
                        });
                }
            }
        }
    }
    qualifications
        .catalog
        .fields
        .sort_unstable_by_key(|field| field.field);
    for (value_type, structural_type) in declared.records.primitive_types() {
        if let ValueType::Scalar(scalar_type, qualifier) = value_type {
            let set = qualifications.scalar(diagnostic, scalar_type, qualifier)?;
            if !set.is_empty() {
                qualifications.catalog.primitives.push(
                    terminal_psi::ScalarPrimitiveQualification {
                        structural_type,
                        value_type: semantic_vocabulary::QualifiedScalarType {
                            scalar_type,
                            qualifications: set,
                        },
                    },
                );
            }
        }
    }
    qualifications
        .catalog
        .primitives
        .sort_unstable_by_key(|row| row.structural_type);
    let mut sources = IdTable::default();
    let mut declaration_envelopes = Vec::new();
    if let Some(source) = declarations {
        for (id, original) in source.declaration_representatives.iter() {
            let envelope = source.declaration_envelopes.get(*original).ok_or_else(|| {
                refuse(
                    diagnostic,
                    Construct::Machine,
                    Unserved::Declaration(
                        "public contract lost its completed declaration envelope",
                    ),
                )
            })?;
            declaration_envelopes.push(terminal_psi::TerminalDeclarationEnvelope {
                contract: id,
                envelope: services.envelope(diagnostic, envelope)?,
            });
            let signature = source
                .declaration_signatures
                .get(*original)
                .ok_or_else(|| {
                    refuse(
                        diagnostic,
                        Construct::Machine,
                        Unserved::Declaration("public contract lost its representative signature"),
                    )
                })?;
            module
                .declaration_contracts
                .push(lower::declaration_contract(
                    diagnostic,
                    id,
                    signature,
                    source
                        .declaration_selections
                        .get(*original)
                        .and_then(|selected| selected.as_ref().map(|(_, body, _, _, _)| *body)),
                    signatures,
                    &outcomes,
                    &declared.records,
                    callables,
                    &mut qualifications,
                )?);
        }
    }
    let mut source = SourceFormCorrespondence::default();
    for position in 1..=bodies.len() as u64 {
        let id = MachineId::new(position).ok_or_else(|| {
            refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Machine("identity overflow"),
            )
        })?;
        let component = components.members(id)?;
        let contract = u64::try_from(module.declaration_contracts.len())
            .ok()
            .and_then(|count| count.checked_add(id.get()))
            .and_then(semantic_vocabulary::ContractId::new)
            .ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Machine("contract identity overflow"),
                )
            })?;
        let lowered = lower::machine(
            id,
            contract,
            bodies,
            component,
            signatures,
            &outcomes,
            &declared.records,
            callables,
            boundaries,
            requirements.contains(&id),
            &mut qualifications,
        )?;
        module.machines.push(lowered.machine);
        // Machines are lowered in identity order and each machine's heads in
        // block order, which is the order the module's roster requires.
        module.scalar_block_invariants.extend(lowered.invariants);
        tables.machines.push(lowered.tables);
        sources.push(lowered.sources);
        source.designated.push(lowered.designated);
        source.calls.extend(lowered.source_calls);
        source.constructions.extend(lowered.source_constructions);
        source.folded_calls.extend(lowered.source_folded_calls);
    }
    if let Some(source) = declarations {
        let proof_scope = std::mem::take(&mut module.scalar_applications.proof_scope);
        let conditions = std::mem::take(&mut module.scalar_applications.conditions);
        module.scalar_applications = source.scalar_applications.published().map_err(|reason| {
            refuse(
                MachineId::new(1).expect("nonzero identity"),
                Construct::Machine,
                Unserved::Declaration(reason),
            )
        })?;
        module.scalar_applications.proof_scope = proof_scope;
        module.scalar_applications.conditions = conditions;
    }
    module.identities = std::mem::take(&mut qualifications.identities).build();
    module.scalar_qualifications = qualifications.catalog.clone();
    boundaries.publish(
        diagnostic,
        signatures.machines,
        &outcomes,
        &declared.records,
        callables,
        services,
        &mut qualifications,
        &mut module,
        declarations,
    )?;
    callables.publish(
        diagnostic,
        signatures.machines,
        &required_ids,
        &outcomes,
        &declared.records,
        boundaries,
        &tables,
        services,
        &mut qualifications,
        &mut module,
    )?;
    module.callables.declaration_envelopes = declaration_envelopes;
    let external_provider_candidates = external_providers.publish(
        diagnostic,
        signatures.machines,
        &outcomes,
        &declared.records,
        callables,
        boundaries,
        services,
        &mut qualifications,
        &mut module,
        typed_source,
    )?;
    let mut domains = IdTable::default();
    let (static_schemas, static_conformances) = static_contracts.publish(
        diagnostic,
        signatures.machines,
        &outcomes,
        &declared.records,
        callables,
        boundaries,
        services,
        &mut qualifications,
        &mut module,
        typed_source,
        &external_provider_candidates,
    )?;
    module.scalar_qualifications = qualifications.catalog;
    module.structural_domains = qualifications.structural;
    qualifications
        .structural_changes
        .sort_unstable_by_key(|change| (change.machine, change.edge, change.argument_ordinal));
    module.structural_qualification_changes = qualifications.structural_changes;
    for (_, record) in records.iter() {
        domains.push(record.requires.clone());
    }
    // Formation questions come from the completed module, including unread
    // structural headers. This emits questions only; the actual static-binder
    // roster arrives with source preparation and the checker admits it before
    // deciding these ordinary entry goals.
    let widths = terminal_check_tables::obligations::integer_width_parameters(&module)
        .map_err(|(machine, detail)| refuse(machine, Construct::Machine, Unserved::Type(detail)))?;
    for (machine, (owner, parameters)) in module.machines.iter().zip(widths) {
        if parameters.is_empty() {
            continue;
        }
        if owner != machine.id || !machine.blocks.iter().any(|block| block.id == machine.entry) {
            return Err(refuse(
                owner,
                Construct::Machine,
                Unserved::Type("integer width formation without an owning entry"),
            ));
        }
        let minted = tables.machines.get_mut(owner).ok_or_else(|| {
            refuse(
                owner,
                Construct::Machine,
                Unserved::Type("integer width formation without machine tables"),
            )
        })?;
        let source = sources.get_mut(owner).ok_or_else(|| {
            refuse(
                owner,
                Construct::Machine,
                Unserved::Type("integer width formation without source coordinates"),
            )
        })?;
        for parameter in parameters {
            let obligation =
                minted
                    .obligations
                    .push(terminal_check_tables::obligations::ObligationRow {
                    kind:
                        terminal_check_tables::obligations::ObligationKind::IntegerWidthPositive {
                            parameter,
                        },
                    created_at: terminal_check_tables::ProgramPoint::BlockEntry(machine.entry),
                });
            if source
                .obligations
                .push(SourceOrigin::Unpositioned(Unpositioned::MachineEntry).into())
                != obligation
            {
                return Err(refuse(
                    owner,
                    Construct::Machine,
                    Unserved::Type("integer width question/source identity mismatch"),
                ));
            }
        }
    }
    let mut prepared = PreparedProgram {
        outcomes,
        source,
        selected_calls: Vec::new(),
        prepared_applications: Vec::new(),
        pending_publication: None,
        program: Elaboration {
            installation_bound_refinement: None,
            module,
            tables,
            sources,
            boundaries: IdTable::default(),
            names: IdTable::default(),
            machines: IdTable::default(),
            private_inputs: Vec::new(),
            local_results: local_summaries.map_or_else(Vec::new, |local| local.result_contracts),
            domains,
            requirements: requirements.to_vec(),
            requirement_ceilings,
            selections: Vec::new(),
            applications: Vec::new(),
            value_parameters: IdTable::default(),
            generic_binders: Vec::new(),
            value_arguments: IdTable::default(),
            generic_templates: Vec::new(),
            conformance_witnesses: Vec::new(),
            supplied: Vec::new(),
            provider_candidate_intents: Vec::new(),
            undeclared: Vec::new(),
            check_form_envelopes: Vec::new(),
            reach: ServiceReach::default(),
            external_provider_candidates,
            static_schemas,
            static_conformances,
            predicate_sources: Vec::new(),
        },
    };
    if declarations.is_none() {
        reference_results::publish(&mut prepared.program, &[])?;
        finish_storage(diagnostic, &mut prepared.program, &mut Vec::new())?;
    }
    Ok(prepared)
}

/// `further` receives every machine whose storage plan is refused as not
/// supported after the first, which is the refusal returned; the plan of
/// each later machine is still made, so one compilation names them all.
fn finish_storage(
    diagnostic: MachineId,
    elaboration: &mut Elaboration,
    further: &mut Vec<Refusal>,
) -> Checked<()> {
    let mut unsupported: Vec<Refusal> = Vec::new();
    let names = &elaboration.names;
    let module = &mut elaboration.module;
    let tables = &mut elaboration.tables;
    let sources = &mut elaboration.sources;
    reference_ends::install(module, tables, sources, further)?;
    for ordinal in 0..module.machines.len() {
        let machine = &module.machines[ordinal];
        let plan = terminal_semantics::plain_affine::plan_in(module, machine.id).map_err(|error| {
            let (concerns, operation, origin) = match error.site() {
                Some(terminal_semantics::plain_affine::Site::Operation(at)) => (
                    Construct::Operation(at),
                    machine
                        .operations()
                        .find(|operation| operation.id == at)
                        .map(|operation| {
                            terminal_semantics::OperationSemanticTag::for_operation(&operation.kind)
                                .name()
                        }),
                    sources
                        .get(machine.id)
                        .and_then(|source| source.operations.get(at))
                        .copied(),
                ),
                Some(terminal_semantics::plain_affine::Site::Edge(at)) => (
                    Construct::Edge(at),
                    Some("control-flow edge"),
                    sources
                        .get(machine.id)
                        .and_then(|source| source.edges.get(at))
                        .copied(),
                ),
                None => (Construct::Machine, None, None),
            };
            let (reason, detail) = match error {
                terminal_semantics::plain_affine::Error::Unsupported { detail, .. } => {
                    (terminal_check_tables::RefusalReason::Unsupported, detail)
                }
                terminal_semantics::plain_affine::Error::Malformed { detail, .. } => (
                    terminal_check_tables::RefusalReason::ForeignIdentity,
                    detail,
                ),
            };
            terminal_check_tables::Refusal {
                reason,
                machine: machine.id,
                concerns,
                source_span: None,
                detail: {
                    let detail = match (operation, origin.and_then(|origin| origin.span())) {
                        (Some(operation), Some(span)) => format!(
                            "while checking {operation} at source {} bytes {}..{}: {detail}",
                            span.source_id.0, span.span.start, span.span.end,
                        ),
                        (Some(operation), _) => format!("while checking {operation}: {detail}"),
                        (None, _) => detail.to_owned(),
                    };
                    match names.get(machine.id) {
                        Some(name) => {
                            format!("while checking storage in `{}`: {detail}", name.machine)
                        }
                        None => detail,
                    }
                },
            }
        });
        let plan = match plan {
            Ok(plan) => plan,
            Err(refused) if refused.reason == RefusalReason::Unsupported => {
                unsupported.push(refused);
                continue;
            }
            Err(refused) => return Err(refused),
        };
        let machine = &mut module.machines[ordinal];
        // Receiving storage may be intact or exactly vacated by a preceding
        // move. Install the mode reconstructed from the final ownership
        // frontier; source syntax alone cannot distinguish them. The checker
        // independently reconstructs these modes and the disposal schedule.
        for block in &mut machine.blocks {
            for operation in &mut block.operations {
                if let terminal_psi::OperationKind::StoreStructuralField { disposition, .. } =
                    &mut operation.kind
                    && let Ok(position) = plan
                        .stores
                        .binary_search_by_key(&operation.id, |(operation, _)| *operation)
                {
                    *disposition = plan.stores[position].1;
                }
            }
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
                | Terminator::EliminateEmpty { .. }
                | Terminator::ExternalCompletion { .. } => {}
            }
        }
    }
    if !unsupported.is_empty() {
        let first = unsupported.remove(0);
        further.append(&mut unsupported);
        return Err(first);
    }
    lower::publish_write_frames(diagnostic, module, tables)?;
    // This is the emitted behavior before proving or publication. Keeping its
    // inventory here lets product consumers retain the exact judged module.
    module.unoptimized_behavior = module
        .machines
        .iter()
        .map(terminal_psi::UnoptimizedMachineBehavior::capture)
        .collect();
    Ok(())
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
            operational: Default::default(),
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
            ranking: None,
        });
        let lower = |publication| {
            let mut signatures = signatures.clone();
            let components = prepare_call_components(&bodies, &mut signatures).unwrap();
            lower_program(
                Some(entry),
                &bodies,
                &components,
                body::Signatures {
                    declarations: &IdTable::default(),
                    declaration_aliases: None,
                    machines: &signatures,
                    boundaries: &boundaries::BoundaryNamespace::default(),
                },
                &IdTable::default(),
                &[],
                &callables::CallableNamespace::default(),
                &boundaries::BoundaryNamespace::default(),
                &reach::ServiceMap::default(),
                &external_providers::ExternalProviderNamespace::default(),
                &static_contracts::StaticContracts::default(),
                publication,
                None,
                None,
            )
            .unwrap()
            .program
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
