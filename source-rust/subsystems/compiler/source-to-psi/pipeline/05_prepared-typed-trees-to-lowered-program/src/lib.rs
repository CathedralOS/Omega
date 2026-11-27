#![forbid(unsafe_code)]

// Crate-wide aliases for sibling stages and the shared test fixtures.
use source_declarations::{declaration_selection, source_selections};
#[cfg(test)]
use source_symbol_table::source_inputs;
use source_symbol_table::{source_symbols, source_tables};
use typed_trees::source_state;

/*
We lower prepared typed trees into an unsealed Terminal Psi module and its
check tables. Each operation that owes a proof records the obligation at
the point that created it. The lowered-form checker discharges those
obligations before publication.

It runs in two steps, each one route per construct:

- `read` resolves a machine's typed-tree nodes into a closed body (`body`):
  local names use their resolved symbols, fields use their declared
  positions, and direct callees use `MachineId`. The readers retain
  the types, contracts and storage paths needed by lowering. An
  unsupported form reports the construct at its source location.
- `lower` turns that body into blocks, typed values, operations, edges and
  invocations, pushing each row into `MachineTables` as it mints the identity,
  so the module and the tables name the same values, edges and obligations.

Scalar locals use SSA values. An assignment changes which value the binding
names. Paths that continue at a shared block pass the values that reach it.

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
mod callables;
mod canonical;
mod compilation;
mod components;
mod declare;
mod lower;
mod mathematics;
mod policy;
mod publication;
mod qualifications;
mod read;
mod references;
mod sum_disposal;

#[cfg(any(test, feature = "test-support"))]
pub use canonical::substituted_routes as substituted_call_routes;
pub use compilation::{lower_for_publication, source_roster};
pub use declare::provider_selections;
pub use lowered_source_publication::LoweredSourcePublication;
pub use prepared_source::PreparedTypedTrees;
use publication::PreparedProgram;
pub use publication::{
    DesignatedOperands, RequirementInvocation, RequirementReview, RequirementReviewMetadata,
    RequirementService, SourceCorrespondence, SourceFormCorrespondence, SourceOrigin,
    selection_questions,
};

/// What every elaboration of one typed program reads in common. A
/// compilation, or a public entry point, prepares it once and lends it to
/// the closure, each conformance witness and each generic form it reads, so
/// none of them derives it again.
type SharedReading = std::sync::Arc<SharedProgram>;
fn shared_reading(typed: &TypedTrees) -> SharedReading {
    std::sync::Arc::new(SharedProgram {
        captures: crate::source_state::NamedConformanceCaptureIndex::new(typed),
        exact_toolchain_sources: Default::default(),
        calling_catalogs: Default::default(),
        declaration_references: Default::default(),
    })
}

pub(crate) struct SharedProgram {
    pub captures: crate::source_state::NamedConformanceCaptureIndex,
    exact_toolchain_sources:
        std::sync::OnceLock<Result<Vec<(source_text::SourceId, [u8; 32])>, String>>,
    /// The service catalog that supplies boundary calling ABIs, one slot per
    /// dependency purpose (`Product`, `Build`). The catalog depends only on
    /// the typed program and that purpose, so the first reading that asks for
    /// boundary metadata prepares it and every later reading of the same
    /// program and purpose borrows the same one.
    calling_catalogs: [std::sync::OnceLock<CallingCatalog>; 2],
    /// Which declarations refer to each symbol. The closure and every
    /// conformance witness check their private helpers' input boundary
    /// against the same program (`Program::private_input_contracts`), so the
    /// first reading that asks walks the program and the rest borrow it.
    declaration_references: std::sync::OnceLock<declare::references::DeclarationReferences>,
}

type CallingCatalog = Result<
    std::sync::Arc<crate::source_state::service_declarations::PreparedServiceCatalog>,
    String,
>;

impl SharedProgram {
    pub(crate) fn exact_toolchain_sources(
        &self,
        typed: &TypedTrees,
    ) -> Result<&[(source_text::SourceId, [u8; 32])], &str> {
        self.exact_toolchain_sources
            .get_or_init(|| {
                source_admission::source_identity::toolchain_source_identities(
                    typed.symbols.source_files(),
                )
            })
            .as_deref()
            .map_err(String::as_str)
    }

    pub(crate) fn calling_catalog(
        &self,
        typed: &TypedTrees,
        purpose: source_provenance::DependencyScope,
    ) -> &CallingCatalog {
        let slot = match purpose {
            source_provenance::DependencyScope::Product => 0,
            source_provenance::DependencyScope::Build => 1,
        };
        self.calling_catalogs[slot].get_or_init(|| {
            let context = crate::source_state::service_declarations::provider_declarations::ProviderPlanningContext::new(
                typed, purpose,
            );
            // This catalog supplies source signatures, not selection
            // authority. The coordinator authenticates the nominated handles
            // against its admitted source registry before constructing the
            // compilation.
            let sources = self.exact_toolchain_sources(typed).map_err(str::to_owned)?;
            Ok(std::sync::Arc::new(
                crate::source_state::service_declarations::PreparedServiceCatalog::prepare(
                    context, sources,
                ),
            ))
        })
    }

    pub(crate) fn declaration_references(
        &self,
        typed: &TypedTrees,
    ) -> &declare::references::DeclarationReferences {
        self.declaration_references
            .get_or_init(|| declare::references::DeclarationReferences::new(typed))
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

pub use body::outcome::Outcome;
#[cfg(any(test, feature = "test-support"))]
pub use body::{
    ArmGuard, Axis, Body, Borrowed, BorrowedReferent, CaseSelector, CaseType, Ceiling,
    CrashCeiling, DomainInstance, DomainSet, ErasedField, Expression, ExpressionKind, Policy,
    Qualification, Ranking, RecordField, RecordType, Selector, Signature, Statement, StorageShape,
    Strict, Target, Termination, ValueType, self_place,
};
#[cfg(not(any(test, feature = "test-support")))]
pub use body::{
    ArmGuard, Body, Borrowed, Ceiling, DomainSet, Expression, ExpressionKind, RecordType,
    Signature, Statement, StorageShape, Target, Termination, ValueType,
};
pub use callables::formations::check_bound_callable_shapes;
#[cfg(any(test, feature = "test-support"))]
pub use callables::headers::{CallableCrashPolicy, CallableHeader};
#[cfg(any(test, feature = "test-support"))]
pub use callables::static_contracts::{
    StaticCallableEnvelope, StaticConformanceQuestion, StaticImplementation,
    StaticProviderCandidate, StaticRequirementSchema, StaticSchemaQuestion, StaticServiceCeiling,
};
#[cfg(not(any(test, feature = "test-support")))]
pub use compilation::prepare_compilation;
#[cfg(any(test, feature = "test-support"))]
pub use compilation::{LoweredCompilationWithSource, prepare_compilation};
#[cfg(not(any(test, feature = "test-support")))]
pub use declare::external_providers::ExternalProviderCandidate;
#[cfg(any(test, feature = "test-support"))]
pub use declare::external_providers::{ExternalProviderCandidate, ProviderContractHeader};
pub use publication::reach::ServiceReach;
pub use read::calling::{CallingPolicySelections, ProgramEntryContractSelection};
// Installed logical calls share source preparation with ordinary refinement;
// their canonical witnesses are judged by the next stage and never published.
pub use terminal_check_tables::compilation::{
    CallableArrivalWitness, CallableInputOrigin, NominalPremiseOrigin,
};
pub use terminal_check_tables::names::{Binder, ClauseText, MachineNames, Piece};

use crate::source_state::TypedTrees;
use arena::IdTable;
use semantic_vocabulary::{MachineId, PackageKeyIdentity, StructuralTypeId};
use symbols::SymbolHandle;
pub use terminal_check_tables::contracts::RequirementCeilings;
use terminal_check_tables::{CheckTables, Checked, Construct, Refusal, RefusalReason};
use terminal_psi::TerminalModule;

pub use lowered_source_publication::Compilation;

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
pub use crate::source_state::trait_definition::Supply;

pub use terminal_check_tables::program::{
    Branch, ConformanceWitness, Elaboration, MachineSources, PredicateSource, Selected,
    SuppliedRequirement, Unpositioned, Unreachable,
};

/// What the subset does not serve, named for the refusal. Closed: a new
/// refusal names one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unserved {
    /// A machine declaration the reader cannot lower in the requested context.
    Machine(&'static str),
    Parameter(&'static str),
    /// A type or type requirement that lowering cannot represent here.
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
        site: terminal_check_tables::RefusalSite::Machine(machine, concerns),
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
        site: terminal_check_tables::RefusalSite::Machine(machine, Construct::Machine),
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
#[cfg(any(test, feature = "test-support"))]
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
#[cfg(any(test, feature = "test-support"))]
pub fn elaborate_for(
    typed: &TypedTrees,
    compilation: Compilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Checked<Elaboration> {
    // The first refusal read is the one a reading that stops there meets;
    // `prepare_compilation` reports the rest beside it.
    let mut prepared =
        elaborate_source_for(typed, compilation, publication, shared_reading(typed), &[])
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
    domain_schemas: &[prepared_source::DomainSchema],
) -> Result<PreparedProgram<'source>, Vec<Refusal>> {
    let shared_for_warm = captures.clone();
    let mut program = read::Program::with_captures(typed, false, compilation.purpose, captures);
    program.local_summary_scope = Some(read::LocalSummaryScope::Compilation(
        compilation.subjects.to_vec(),
    ));
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
    let parallel_machines = worker_budget::current_workers() > 1;
    let mut read_all_bodies = || -> Result<_, Vec<Refusal>> {
        let entry = if let Some((entry, others)) = compilation.entries.split_first() {
            let entry = program.machine(*entry).map_err(alone)?;
            program
                .fixed_contracts
                .push(read::FixedContract::Interface(entry));
            for other in others {
                match program.machine(*other) {
                    Ok(other) => program
                        .fixed_contracts
                        .push(read::FixedContract::Interface(other)),
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
            .subject_declarations(
                first,
                compilation.subjects,
                compilation.purpose,
                domain_schemas,
            )
            .map_err(alone)?;
        let bodies = read_bodies(&mut program, first).map_err(alone)?;
        Ok((entry, bodies))
    };
    let (entry, bodies) = if parallel_machines
        && compilation.purpose == source_provenance::DependencyScope::Product
        && !compilation.declaration_metadata.is_empty()
    {
        let (_, res) = rayon::join(
            || {
                if !compilation.subjects.is_empty() {
                    rayon::join(
                        || {
                            let _ = shared_for_warm.calling_catalog(typed, compilation.purpose);
                        },
                        || {
                            let _ = shared_for_warm.declaration_references(typed);
                        },
                    );
                } else {
                    let _ = shared_for_warm.calling_catalog(typed, compilation.purpose);
                }
            },
            read_all_bodies,
        );
        res?
    } else {
        read_all_bodies()?
    };
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
        parallel_machines,
        BodyScope::All,
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
        // A row whose signature was refused after it reserved the row has
        // no body to read. Its refusal is already kept.
        if !program.row_is_read(next) {
            if program.collected.is_none() {
                return Err(refuse(
                    diagnostic,
                    Construct::Machine,
                    Unserved::Machine("a body whose signature was not read"),
                ));
            }
            complete = false;
            continue;
        }
        // A generic mathematical machine's body was read as terms with its
        // contract (`read::templates`); nothing of it is lowered.
        if program
            .template_bodies
            .iter()
            .any(|(template, _)| *template == next)
        {
            if complete {
                bodies.push(Body {
                    slots: Vec::new(),
                    statements: Vec::new(),
                });
            }
            continue;
        }
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
#[cfg(any(test, feature = "test-support"))]
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
#[cfg(any(test, feature = "test-support"))]
fn declaration_context(
    typed: &TypedTrees,
    declaration: SymbolHandle,
) -> source_provenance::DependencyScope {
    typed
        .symbols
        .symbol_provenance_source_span(declaration)
        .and_then(|span| typed.symbols.source_file(span))
        .map(|source| source.dependency_scope)
        .unwrap_or(source_provenance::DependencyScope::Product)
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
#[cfg(any(test, feature = "test-support"))]
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
/// certificates carry into it (`crate::checking::reverify_specialization`).
/// Refused unless `template` lists a finite family (`family_alternatives`).
#[cfg(any(test, feature = "test-support"))]
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
    purpose: source_provenance::DependencyScope,
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
            finish_reading(
                program,
                Some(entry),
                IdTable::default(),
                None,
                None,
                None,
                worker_budget::current_workers() > 1,
            )
        })
        .collect()
}

/// The check forms of the generic machines `templates`, in order: for each,
/// the machine checked once with opaque parameters when its bounds suffice,
/// otherwise once per member of its complete static type partition. An open
/// classifier includes an opaque remainder beyond its closed cases. Refused
/// when a required alternative refuses; its diagnostic names that form.
#[cfg(any(test, feature = "test-support"))]
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
                worker_budget::current_workers() > 1,
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
    purpose: source_provenance::DependencyScope,
    templates: &[SymbolHandle],
    captures: SharedReading,
    publication: terminal_codec::TerminalPublication,
    parallel_machines: bool,
) -> Checked<Vec<PreparedProgram<'source>>> {
    let mut forms = Vec::new();
    for template in templates {
        // A template requirement (own lifetimes, or sized by its own `const`
        // binders) is checked once here; programs carry only its uses.
        if let Some((_, requirement)) = typed.trait_machine_signature(*template) {
            let mut program = read::Program::with_captures(typed, true, purpose, captures.clone());
            program.publication = publication;
            program.generic_requirement(requirement)?;
            forms.push(finish_reading(
                program,
                None,
                IdTable::default(),
                None,
                None,
                None,
                parallel_machines,
            )?);
            continue;
        }
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
                parallel_machines,
            )?);
            continue;
        }
        if let Some(domain) = typed.domain_definition(*template) {
            let mut program = read::Program::with_captures(typed, true, purpose, captures.clone());
            program.publication = publication;
            program.form_domain_predicate(domain)?;
            forms.push(finish_reading(
                program,
                None,
                IdTable::default(),
                None,
                None,
                None,
                parallel_machines,
            )?);
            continue;
        }
        if let Some(declared) = typed.unique_data_definition_by_symbol(*template) {
            let mut program = read::Program::with_captures(typed, true, purpose, captures.clone());
            program.publication = publication;
            program.generic_data(declared)?;
            forms.push(finish_reading(
                program,
                None,
                IdTable::default(),
                None,
                None,
                None,
                parallel_machines,
            )?);
            continue;
        }
        if let Some(alias) = typed
            .type_aliases()
            .iter()
            .find(|alias| alias.symbol == *template)
        {
            let mut program = read::Program::with_captures(typed, true, purpose, captures.clone());
            program.publication = publication;
            program.generic_type_alias(alias)?;
            forms.push(finish_reading(
                program,
                None,
                IdTable::default(),
                None,
                None,
                None,
                parallel_machines,
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
            parallel_machines,
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
    machine: &crate::source_state::machine::Machine,
) -> bool {
    machine.has_specialization_parameters()
        || typed
            .enclosing_source_conformance(machine.symbol)
            .is_some_and(|owner| {
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

#[derive(Clone, PartialEq)]
pub(crate) struct CalleeSummary {
    pub envelope: publication::reach::SourceEnvelope,
    pub crash_routes: Vec<terminal_psi::CrashRouteBucket>,
    pub completes_normally: bool,
}

#[derive(Default)]
pub(crate) struct CalleeSummaries(arena::HandleKeyMap<SymbolHandle, Option<CalleeSummary>>);

impl CalleeSummaries {
    pub(crate) fn insert(&mut self, symbol: SymbolHandle, summary: CalleeSummary) {
        match self.0.get_mut(&symbol) {
            Some(slot @ Some(_)) => {
                if slot.as_ref() != Some(&summary) {
                    *slot = None;
                }
            }
            Some(None) => {}
            None => {
                self.0.insert(symbol, Some(summary));
            }
        }
    }

    pub(crate) fn merge(&mut self, other: Self) {
        for (symbol, entry) in other.0 {
            match entry {
                Some(summary) => self.insert(symbol, summary),
                None => {
                    self.0.insert(symbol, None);
                }
            }
        }
    }

    pub(crate) fn get(&self, symbol: SymbolHandle) -> Option<&CalleeSummary> {
        self.0.get(&symbol).and_then(Option::as_ref)
    }
}

struct WitnessInputs<'scope> {
    captures: SharedReading,
    purpose: source_provenance::DependencyScope,
    outcome_subjects: Option<&'scope [Option<PackageKeyIdentity>]>,
    callee_summaries: Option<&'scope CalleeSummaries>,
    parallel_machines: bool,
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
#[cfg(any(test, feature = "test-support"))]
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
            callee_summaries: None,
            parallel_machines: worker_budget::current_workers() > 1,
        },
    )
    .and_then(PreparedProgram::into_check_form)
}

/// The empty reading a conformance witness of `implementation` starts from.
fn witness_program<'source>(
    typed: &'source TypedTrees,
    implementation: SymbolHandle,
    inputs: &WitnessInputs<'_>,
) -> read::Program<'source> {
    let generic = typed
        .machine_by_symbol(implementation)
        .is_some_and(|machine| is_generic_template(typed, machine));
    let mut program =
        read::Program::with_captures(typed, generic, inputs.purpose, inputs.captures.clone());
    program.local_summary_scope = inputs
        .outcome_subjects
        .map(|subjects| read::LocalSummaryScope::Witness(subjects.to_vec()));
    program
}

/// Whether a call selecting `implementation` under the header `selected`
/// proves the same statement as its conformance row read without a header,
/// so the row's witness also decides the selection.
///
/// `selected` changes a witness in two ways (`read::Program::witnessed`):
/// it can refuse, when it names another declaration or disagrees with the
/// row's own header, and it supplies the contract when the implementation
/// owns no row of a trait with parameters. This reads only that header.
/// Both refusals are judged here, and a contract the header supplied is
/// never treated as the row's.
fn selected_witness_is_declared<'source>(
    typed: &'source TypedTrees,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
    arguments: &[semantic_vocabulary::ScalarTerm],
    implementation: SymbolHandle,
    selected: &'source crate::source_state::signature::StateSignature,
    inputs: WitnessInputs<'_>,
) -> bool {
    witness_program(typed, implementation, &inputs)
        .witnessed(
            declaring_trait,
            requirement,
            arguments,
            implementation,
            Some(selected),
        )
        .is_ok_and(|(_, source)| source == read::WitnessRequirement::Declared)
}

fn prepare_witness<'source>(
    typed: &'source TypedTrees,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
    arguments: &[semantic_vocabulary::ScalarTerm],
    implementation: SymbolHandle,
    expected: Option<&'source crate::source_state::signature::StateSignature>,
    inputs: WitnessInputs<'_>,
) -> Checked<PreparedProgram<'source>> {
    let parallel_machines = inputs.parallel_machines;
    let summaries = inputs.callee_summaries;
    let mut program = witness_program(typed, implementation, &inputs);
    let (entry, _) = program.witnessed(
        declaring_trait,
        requirement,
        arguments,
        implementation,
        expected,
    )?;
    let callee = program.implementation(implementation, declaring_trait, requirement)?;
    let bodies = witness_call(&mut program, entry, callee)?;
    let scope = match summaries {
        Some(summaries) => BodyScope::UpTo {
            last_body: callee,
            summaries,
        },
        None => BodyScope::All,
    };
    let mut prepared = finish_reading_into(
        program,
        Some(entry),
        bodies,
        None,
        None,
        None,
        &mut Vec::new(),
        parallel_machines,
        scope,
    )?;
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
#[cfg(any(test, feature = "test-support"))]
pub enum CallableDeclaration<'a> {
    Signature(&'a crate::source_state::signature::StateSignature),
    Machine(SymbolHandle),
}

/// Prepare the logical refinement witness without reading the supplied body.
#[cfg(any(test, feature = "test-support"))]
pub fn elaborate_callable_witness<'a>(
    typed: &'a TypedTrees,
    required: &'a crate::source_state::signature::StateSignature,
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
    let mut supplied =
        terminal_check_tables::row(&program.signatures, callee, entry, Construct::Machine)?.clone();
    program.apply_forwarded_array_lengths(entry, callee, &required, &mut supplied);
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
            static_reach_arguments: Vec::new(),
            proof_arguments: Vec::new(),
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
                guard: ArmGuard::Always,
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
#[cfg(any(test, feature = "test-support"))]
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
    let source_selection_questions = capture_source_questions(&mut program, entry)?;
    let services = program.service_map(entry)?;
    let components = prepare_call_components(
        &bodies,
        &mut program.signatures,
        &program.theorem_parameters,
    )?;
    let mut witness = lower_program(
        Some(entry),
        &bodies,
        &components,
        body::Signatures {
            inferred_inputs: None,
            theorems: &program.theorem_parameters,
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
    witness.source.selection_questions = Some(
        publication::selection_questions::bind_invocation_declarations(
            source_selection_questions,
            program.typed,
            &mut witness.program,
            &witness.source.records,
        )
        .map_err(|reason| refuse(entry, Construct::Machine, Unserved::Declaration(reason)))?,
    );
    Ok(witness)
}

fn capture_source_questions(
    program: &mut read::Program<'_>,
    entry: MachineId,
) -> Checked<publication::selection_questions::RawSelectionQuestions> {
    program.completed_operators.finish();
    publication::selection_questions::capture_with_bodies(
        program.typed,
        &program.admitted_declarations,
        &program.admitted_bodies,
        &program.completed_operators,
        &program.admitted_signature_views,
        &program.admitted_signature_view_owners,
        &program.generic_templates,
    )
    .map_err(|error| Refusal {
        source_span: error.source_span(),
        site: terminal_check_tables::RefusalSite::Machine(entry, Construct::Machine),
        reason: RefusalReason::Unsupported,
        detail: error.to_string(),
    })
}

#[cfg(any(test, feature = "test-support"))]
fn elaborate_form(
    typed: &TypedTrees,
    purpose: source_provenance::DependencyScope,
    entry: SymbolHandle,
    rigid: bool,
    captures: SharedReading,
    publication: terminal_codec::TerminalPublication,
) -> Checked<Elaboration> {
    prepare_form(
        typed,
        purpose,
        entry,
        rigid,
        captures,
        publication,
        worker_budget::current_workers() > 1,
    )
    .and_then(PreparedProgram::into_check_form)
}

fn prepare_form<'source>(
    typed: &'source TypedTrees,
    purpose: source_provenance::DependencyScope,
    entry: SymbolHandle,
    rigid: bool,
    captures: SharedReading,
    publication: terminal_codec::TerminalPublication,
    parallel_machines: bool,
) -> Checked<PreparedProgram<'source>> {
    let mut program = read::Program::with_captures(typed, rigid, purpose, captures);
    program.publication = publication;
    let entry = program.machine(entry)?;
    finish_reading(
        program,
        Some(entry),
        IdTable::default(),
        None,
        None,
        None,
        parallel_machines,
    )
}

/// Reads every body not yet in `bodies`, callees as they are discovered,
/// and lowers the closure. `declared` is the typed trees when the closure is a
/// compilation's program, whose machines are then given their declaration
/// identities (`declare`). This owner publishes their proof scopes once after
/// the declarations are formed.
#[derive(Clone, Copy)]
enum BodyScope<'a> {
    All,
    UpTo {
        last_body: MachineId,
        summaries: &'a CalleeSummaries,
    },
}

fn finish_reading<'source>(
    program: read::Program<'source>,
    entry: Option<MachineId>,
    bodies: IdTable<MachineId, Body>,
    declared: Option<&'source TypedTrees>,
    calling: Option<CallingPolicySelections<'_>>,
    providers: Option<declare::provider_selections::ProviderSelections<'_>>,
    parallel_machines: bool,
) -> Checked<PreparedProgram<'source>> {
    finish_reading_into(
        program,
        entry,
        bodies,
        declared,
        calling,
        providers,
        &mut Vec::new(),
        parallel_machines,
        BodyScope::All,
    )
}

/// `finish_reading`, putting each machine refused after the first one, where
/// each machine is refused on its own, into `further`.
#[allow(clippy::too_many_arguments)]
fn finish_reading_into<'source>(
    mut program: read::Program<'source>,
    entry: Option<MachineId>,
    mut bodies: IdTable<MachineId, Body>,
    declared: Option<&'source TypedTrees>,
    calling: Option<CallingPolicySelections<'_>>,
    providers: Option<declare::provider_selections::ProviderSelections<'_>>,
    further: &mut Vec<Refusal>,
    parallel_machines: bool,
    body_scope: BodyScope<'_>,
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
            let stubbed = if bodies.get(next).is_none() {
                let stubbed = match body_scope {
                    BodyScope::All => false,
                    BodyScope::UpTo {
                        last_body,
                        summaries,
                    } if next > last_body => program.stub_witness_callee(next, summaries)?,
                    BodyScope::UpTo { .. } => false,
                };
                if stubbed {
                    let slots = terminal_check_tables::row(
                        &program.signatures,
                        next,
                        next,
                        Construct::Machine,
                    )?
                    .parameters
                    .clone();
                    bodies.push(Body {
                        slots,
                        statements: Vec::new(),
                    });
                    true
                } else {
                    bodies.push(program.body(next)?);
                    false
                }
            } else {
                false
            };
            binder_reach.push(if stubbed {
                Vec::new()
            } else {
                program.application_binders(next)?
            });
        }
        let components = prepare_call_components(
            &bodies,
            &mut program.signatures,
            &program.theorem_parameters,
        )?;
        program.finish_operational_envelopes(&bodies, &components)?;
        program.prepare_service_bindings(diagnostic)?;
        let more = program.prepare_calling_policies(diagnostic, calling, &mut policies)?;
        if !more {
            break components;
        }
    };
    let private_inputs = program.private_input_contracts(entry, diagnostic)?;
    program.finish_operational_envelopes(&bodies, &components)?;
    program.finish_declarations(diagnostic)?;
    program.prepare_external_providers(diagnostic)?;
    program.requirements.sort_unstable();
    program.requirements.dedup();
    for requirement in program.requirements.clone() {
        program.retain_requirement_schema(requirement)?;
    }
    program.begin_declaration_storage(diagnostic, bodies.len())?;
    let declarations = program.declaration_metadata.clone();
    let calling_abis = program.prepare_calling_abis(diagnostic, &declarations)?;
    let descriptions = (!policies.requests.is_empty())
        .then(|| program.prepare_calling_storage(diagnostic, &policies))
        .transpose()?;
    let opaque_carriers = program.prepare_opaque_carriers(diagnostic)?;
    program.prepare_domain_predicates(diagnostic)?;
    program.finish_cleanup_selections()?;
    // Header types and clauses may name services and parent contracts.
    // Build the service list and check all parents after those reads finish.
    let services = program.service_map(diagnostic)?;
    program.require_complete_boundary_parent_contracts(diagnostic, &policies)?;
    let policies = (!policies.requests.is_empty()).then_some(policies);
    let source_selection_questions = capture_source_questions(&mut program, diagnostic)?;
    let PreparedProgram {
        program: mut elaboration,
        outcomes,
        mut source,
        ..
    } = lower_program_with_parallel(
        entry,
        &bodies,
        &components,
        body::Signatures {
            inferred_inputs: Some(&private_inputs),
            theorems: &program.theorem_parameters,
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
        parallel_machines,
    )?;
    elaboration.machines = program.symbols.clone();
    for schema in &elaboration.static_schemas {
        for declaration in &schema.requirements {
            let Some(instance) = &declaration.instance else {
                continue;
            };
            let requirement = instance.machine;
            let ceiling = elaboration
                .requirement_ceilings
                .get_mut(requirement)
                .ok_or_else(|| {
                    refuse(
                        requirement,
                        Construct::Machine,
                        Unserved::Contract("abstract requirement has no exact ceiling row"),
                    )
                })?;
            if ceiling
                .envelope
                .replace(declaration.envelope.clone())
                .is_some()
            {
                return Err(refuse(
                    requirement,
                    Construct::Machine,
                    Unserved::Contract("abstract requirement envelope was already retained"),
                ));
            }
        }
    }

    let mut checking_envelopes = IdTable::default();
    for (id, envelope) in program.checking_envelopes.iter() {
        checking_envelopes.push(
            envelope
                .as_ref()
                .map(|row| row.publish(id, &services))
                .transpose()?,
        );
    }

    elaboration.boundaries = program.boundaries.rows.map(|_, source| source.source);
    source.boundary_signatures = program
        .boundaries
        .rows
        .map(|_, row| row.signature.static_signature());
    source.selection_questions = Some(
        publication::selection_questions::bind_invocation_declarations(
            source_selection_questions,
            program.typed,
            &mut elaboration,
            &source.records,
        )
        .map_err(|reason| {
            refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Declaration(reason),
            )
        })?,
    );
    publication::reach::publish(entry, &services, &mut elaboration)?;
    calling_abis.publish(&mut elaboration.module)?;
    program.publish_record_layouts(diagnostic, &mut elaboration.module)?;
    let declaration_error = |reason| {
        refuse(
            diagnostic,
            Construct::Machine,
            Unserved::Declaration(reason),
        )
    };
    elaboration.static_reach_parameters =
        program.static_reach_contexts(&mut elaboration.module, &source.calls)?;
    elaboration
        .static_reach_applications
        .sort_unstable_by_key(|row| (row.caller, row.site));
    // Real typed bodies retain their actual declarations in check forms too.
    // A synthetic frame has no such declaration and is recorded as undeclared.
    program.publish_theorem_parameters(&mut elaboration.module)?;
    let prepared_applications = declare::declare(
        program.typed,
        &program,
        &binder_reach,
        &checking_envelopes,
        &mut elaboration,
        parallel_machines,
    )
    .map_err(|error| error.into_refusal(diagnostic, &program))?;
    let mut requirements = elaboration.requirements.clone();
    requirements.sort_unstable();
    for slot in &mut elaboration.module.execution.callables.machine_envelopes {
        // An abstract header retains its static bounds, never an executable
        // declaration or a synthetic checked body.
        if slot
            .as_ref()
            .is_some_and(|row| requirements.binary_search(&row.machine).is_ok())
        {
            *slot = None;
        }
    }
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
        for slot in &mut elaboration.module.execution.callables.machine_envelopes {
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
    qualifications::publish_access_plans(diagnostic, &program, &mut elaboration.module)?;
    elaboration.nominal_types = program.nominal_type_inputs();
    elaboration.nominal_calls =
        program.publish_nominal_calls(&source.calls, &mut elaboration.module)?;
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
    references::results::publish_requirements(
        &mut elaboration.module,
        &program,
        &program.requirements,
    )?;
    references::results::publish(&mut elaboration, &program.selections, parallel_machines)
        .map_err(|refusal| {
            let span = refusal.source_span.or_else(|| {
                program
                    .names
                    .get(refusal.site.in_machine()?.0)
                    .and_then(|names| names.declared)
            });
            refusal.with_source_span(span)
        })?;
    finish_storage(diagnostic, &mut elaboration, further, parallel_machines)?;
    elaboration.generic_storage = program.generic_storage_bindings(diagnostic)?;
    program.publish_opaque_carriers(
        diagnostic,
        opaque_carriers,
        descriptions
            .iter()
            .flat_map(|rows| rows.signatures.iter().map(|row| &row.storage)),
        &mut elaboration.module,
    )?;
    // A call can forward a loan. Its published result and storage contracts
    // must exist before a crossing can name the borrower's live backing.
    lower::suspension::publish_crossings(&mut elaboration.module).map_err(declaration_error)?;
    source.review_requirements = program.review_requirements(diagnostic, &elaboration.module);
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
        source_envelopes: program.callables.envelopes,
        has_type_alternative: program.alternative.is_some(),
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
#[cfg(any(test, feature = "test-support"))]
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
            inferred_inputs: None,
            theorems: &[],
            declarations: &IdTable::default(),
            declaration_aliases: None,
            machines: signatures,
            boundaries: &declare::boundaries::BoundaryNamespace::default(),
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
    let components = prepare_call_components(bodies, &mut machine_signatures, signatures.theorems)?;
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
        &declare::boundaries::BoundaryNamespace::default(),
        &publication::reach::ServiceMap::default(),
        &declare::external_providers::ExternalProviderNamespace::default(),
        &callables::static_contracts::StaticContracts::default(),
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
    theorems: &[terminal_psi::TheoremParameter],
) -> Checked<components::Components> {
    for (machine, body) in bodies.iter() {
        lower::operational::positions(machine, body)?;
    }
    components::components(bodies, signatures, theorems)
}

fn lower_program<'source>(
    entry: Option<MachineId>,
    bodies: &IdTable<MachineId, Body>,
    components: &components::Components,
    signatures: body::Signatures<'_>,
    records: &IdTable<StructuralTypeId, RecordType>,
    requirements: &[MachineId],
    callables: &callables::CallableNamespace,
    boundaries: &declare::boundaries::BoundaryNamespace,
    services: &publication::reach::ServiceMap,
    external_providers: &declare::external_providers::ExternalProviderNamespace,
    static_contracts: &callables::static_contracts::StaticContracts,
    publication: terminal_codec::TerminalPublication,
    typed_source: Option<&'source TypedTrees>,
    declarations: Option<&read::Program<'source>>,
) -> Checked<PreparedProgram<'source>> {
    lower_program_with_parallel(
        entry,
        bodies,
        components,
        signatures,
        records,
        requirements,
        callables,
        boundaries,
        services,
        external_providers,
        static_contracts,
        publication,
        typed_source,
        declarations,
        worker_budget::current_workers() > 1,
    )
}

#[allow(clippy::too_many_arguments)]
fn lower_program_with_parallel<'source>(
    entry: Option<MachineId>,
    bodies: &IdTable<MachineId, Body>,
    components: &components::Components,
    signatures: body::Signatures<'_>,
    records: &IdTable<StructuralTypeId, RecordType>,
    requirements: &[MachineId],
    callables: &callables::CallableNamespace,
    boundaries: &declare::boundaries::BoundaryNamespace,
    services: &publication::reach::ServiceMap,
    external_providers: &declare::external_providers::ExternalProviderNamespace,
    static_contracts: &callables::static_contracts::StaticContracts,
    publication: terminal_codec::TerminalPublication,
    typed_source: Option<&'source TypedTrees>,
    declarations: Option<&read::Program<'source>>,
    parallel: bool,
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
        signatures.machines.map(|_, signature| RequirementCeilings {
            reaches: signature.reaches.clone(),
            invokes: signature.invokes.clone(),
            envelope: None,
            suspension: signature.operational.suspension,
            blocking: signature.operational.blocking,
            // The signature reader refuses unsupported progress premises before
            // lowering. A missing promise remains NoGuarantee, never a body proof.
            termination: match signature.termination {
                Termination::Silent => terminal_psi::TerminalCallableTermination::NotGuaranteed,
                Termination::Promised => terminal_psi::TerminalCallableTermination::Terminates {
                    premises: Vec::new(),
                },
            },
        })
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
        terminal_codec::TerminalPublication::Invocation
        | terminal_codec::TerminalPublication::LayoutPolicyInvocation => {
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
    let outcomes = body::outcome::outcomes(
        bodies,
        signatures,
        requirements,
        declarations.map_or(&[][..], |source| {
            source.non_returning_requirements.as_slice()
        }),
        callables,
        local_summaries
            .as_ref()
            .map(|local| local.outcomes.as_slice()),
        &declared.records.inhabitation,
        components,
    )?;
    // Declaration checking publishes no executable arrival or root reach.
    if let Some(entry) = entry {
        module.interfaces.root_service_reach.concrete =
            terminal_check_tables::row(&outcomes, entry, entry, Construct::Machine)?
                .service_reach
                .clone();
    }
    module.identities = declared.identities;
    module.types.structural_types = declared.structural_types;
    module.layouts.native_layouts = declared.native_layouts;
    let mut source_records = Vec::new();
    if let Some(program) = declarations {
        source_records = program.source_record_correspondence(diagnostic)?;
        program.publish_source_declarations(diagnostic, &source_records, &mut module)?;
    }
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
                .execution
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
    let mut source = SourceFormCorrespondence {
        records: source_records,
        ..SourceFormCorrespondence::default()
    };
    let loop_rows = std::sync::Mutex::new(terminal_check_tables::LoopCheckingRows::default());
    let mut static_reach_applications = Vec::new();
    let decl_contracts_len = module.execution.declaration_contracts.len();
    let lower_one = |position: usize| -> Checked<lower::Lowered> {
        let id = MachineId::new(position as u64).ok_or_else(|| {
            refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Machine("identity overflow"),
            )
        })?;
        let component = components.invocation_members(id)?;
        let contract = u64::try_from(decl_contracts_len)
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
        let mut lowered = lower::machine_with_read_only_qualifications(
            id,
            contract,
            bodies,
            component,
            components.has_separate_entry(id),
            signatures,
            &outcomes,
            &declared.records,
            &module.types.structural_types,
            callables,
            boundaries,
            requirements.contains(&id),
            &qualifications,
        )?;
        if let Some(typed) = typed_source {
            lower::operational::retain_callee_spans(typed, &mut lowered);
        }
        if let Some(bounds) =
            declarations.and_then(|source| source.declared_operational_ceilings(id))
        {
            lowered.machine.declared_suspension = bounds.suspension;
            lowered.machine.declared_blocking = bounds.blocking;
        }
        // A family member keeps its own entry, edges, roots and proof obligations.
        // Share equal machine-local rows before retaining the completed body.
        if component.len() > 1 {
            loop_rows
                .lock()
                .expect("loop row storage poisoned")
                .retain(&mut lowered.tables);
        }
        Ok(lowered)
    };
    let lowered_machines: Vec<Checked<lower::Lowered>> = if parallel && bodies.len() >= 2 {
        use rayon::prelude::*;
        (0..bodies.len())
            .into_par_iter()
            .with_max_len(1)
            .map(|index| lower_one(index + 1))
            .collect()
    } else {
        (1..=bodies.len()).map(lower_one).collect()
    };
    drop(loop_rows);
    for lowered in lowered_machines {
        let mut lowered = lowered?;
        lower::apply_machine_qualifications(&mut lowered, &mut qualifications)?;
        module.machines.push(lowered.machine);
        // Machines are lowered in identity order and each machine's heads in
        // block order, which is the order the module's roster requires.
        module
            .proofs
            .scalar_block_invariants
            .extend(lowered.invariants);
        module
            .proofs
            .theorem_applications
            .extend(lowered.theorem_applications);
        tables.machines.push(lowered.tables);
        sources.push(lowered.sources);
        source.designated.push(lowered.designated);
        module
            .proofs
            .theorem_citations
            .extend(lowered.theorem_citations);
        source.calls.extend(lowered.source_calls);
        static_reach_applications.extend(lowered.static_reach_applications);
        source.constructions.extend(lowered.source_constructions);
        source.folded_calls.extend(lowered.source_folded_calls);
    }
    if let Some(source) = declarations {
        let proof_scope = std::mem::take(&mut module.scalars.scalar_applications.proof_scope);
        let conditions = std::mem::take(&mut module.scalars.scalar_applications.conditions);
        module.scalars.scalar_applications =
            source.scalar_applications.published().map_err(|reason| {
                refuse(
                    MachineId::new(1).expect("nonzero identity"),
                    Construct::Machine,
                    Unserved::Declaration(reason),
                )
            })?;
        module.scalars.scalar_applications.proof_scope = proof_scope;
        module.scalars.scalar_applications.conditions = conditions;
    }
    module.identities = std::mem::take(&mut qualifications.identities).build();
    module.types.scalar_qualifications = qualifications.catalog.clone();
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
        &outcomes,
        &declared.records,
        boundaries,
        &tables,
        services,
        &mut qualifications,
        &mut module,
        declarations,
    )?;
    module.execution.callables.declaration_envelopes = declaration_envelopes;
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
        declarations,
        &external_provider_candidates,
    )?;
    module.types.scalar_qualifications = qualifications.catalog;
    module.types.structural_domains = qualifications.structural;
    qualifications
        .structural_changes
        .sort_unstable_by_key(|change| (change.machine, change.edge, change.argument_ordinal));
    module.types.structural_qualification_changes = qualifications.structural_changes;
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
        source_envelopes: IdTable::default(),
        has_type_alternative: false,
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
            domain_sources: qualifications.domain_sources,
            machines: IdTable::default(),
            private_inputs: Vec::new(),
            local_results: local_summaries.map_or_else(Vec::new, |local| local.result_contracts),
            requirements: requirements.to_vec(),
            requirement_ceilings,
            selections: Vec::new(),
            applications: Vec::new(),
            value_parameters: IdTable::default(),
            generic_binders: Vec::new(),
            generic_storage: Vec::new(),
            nominal_calls: Vec::new(),
            nominal_types: Vec::new(),
            value_arguments: IdTable::default(),
            generic_templates: Vec::new(),
            conformance_witnesses: Vec::new(),
            supplied: Vec::new(),
            provider_candidate_intents: Vec::new(),
            undeclared: Vec::new(),
            check_form_envelopes: Vec::new(),
            static_reach_parameters: Vec::new(),
            static_reach_applications,
            static_reach_summaries: Vec::new(),
            generic_callable_declarations: Vec::new(),
            reach: ServiceReach::default(),
            external_provider_candidates,
            static_schemas,
            static_conformances,
            predicate_sources: Vec::new(),
        },
    };
    if declarations.is_none() {
        references::results::publish(&mut prepared.program, &[], parallel)?;
        finish_storage(diagnostic, &mut prepared.program, &mut Vec::new(), parallel)?;
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
    parallel: bool,
) -> Checked<()> {
    let mut unsupported: Vec<Refusal> = Vec::new();
    let names = &elaboration.names;
    let module = &mut elaboration.module;
    let tables = &mut elaboration.tables;
    let sources = &mut elaboration.sources;
    sum_disposal::expand_unmatched_sums(module, tables, sources)?;
    references::ends::install(module, tables, sources, further, parallel)?;
    references::default_obligations::install(module, tables, sources, parallel)?;
    sum_disposal::expand_unmatched_sums(module, tables, sources)?;
    let sources = &*sources;
    // Each machine's storage plan comes from its own ownership walk over the
    // module; the walks run on the invocation's workers (`plan_then_apply`).
    // Applying a plan changes only its machine's blocks, and a walk reads the
    // blocks of no machine other than those in its descriptor graph
    // (`Plan::graphed_machines`). A refused walk names no graph, so it counts
    // as reading every machine.
    lower::machine_order::plan_then_apply_with(
        module,
        parallel,
        |module, summaries, machine| storage_plan(module, summaries, machine, names, sources),
        |plan| match plan {
            Ok(Ok(plan)) => Some(plan.graphed_machines.as_slice()),
            _ => None,
        },
        |machine, plan| match plan? {
            Ok(plan) => install_storage_plan(machine, &plan),
            // An unsupported walk is reported beside the first one and
            // the next machine is planned; any other refusal ends the loop.
            Err(refused) if refused.reason == RefusalReason::Unsupported => {
                unsupported.push(refused);
                Ok(false)
            }
            Err(refused) => Err(refused),
        },
    )?;
    if !unsupported.is_empty() {
        let first = unsupported.remove(0);
        further.append(&mut unsupported);
        return Err(first);
    }
    lower::publish_write_frames_with(module, tables, parallel)?;
    publication::claims::publish(diagnostic, module, parallel)?;
    // This is the emitted behavior before proving or publication. Keeping its
    // inventory here lets product consumers retain the exact judged module.
    module.execution.unoptimized_behavior = module
        .machines
        .iter()
        .map(terminal_psi::UnoptimizedMachineBehavior::capture)
        .collect();
    Ok(())
}

/// The storage plan of `machine`: `Ok(Err(_))` when its ownership walk
/// refuses, which `finish_storage` may report beside other refusals, and
/// `Err(_)` when the stores it plans cannot be validated, which ends storage.
fn storage_plan(
    module: &terminal_psi::TerminalModule,
    summaries: &terminal_semantics::plain_affine::AffineSummaries<'_>,
    machine: &terminal_psi::TerminalMachine,
    names: &IdTable<MachineId, MachineNames>,
    sources: &IdTable<MachineId, MachineSources>,
) -> Checked<Checked<terminal_semantics::plain_affine::Plan>> {
    let walks =
        terminal_semantics::plain_affine::MachineAffine::new(module, machine.id, Some(summaries));
    let plan = walks.and_then(|walks| walks.plan()).map_err(|error| {
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
        // A linear value lost on an edge with no written position (the
        // end of a body) is reported where the value was made: the call
        // whose result `_ =` drops, or the construction a local holds.
        let lost_at = match error {
            terminal_semantics::plain_affine::Error::Unconsumed { place, .. } => {
                origin.and_then(|origin| origin.span()).or_else(|| {
                    let (producer, _) = machine
                        .structural_place(place)?
                        .kind
                        .as_operation_result()?;
                    sources.get(machine.id)?.operations.get(producer)?.span()
                })
            }
            _ => None,
        };
        let (reason, detail) = match error {
            terminal_semantics::plain_affine::Error::Unavailable(unavailable) => (
                terminal_check_tables::RefusalReason::Unestablished,
                unavailable.message(),
            ),
            terminal_semantics::plain_affine::Error::Unconsumed { at_join, .. } => (
                terminal_check_tables::RefusalReason::Violation,
                terminal_semantics::plain_affine::Error::unconsumed_message(at_join),
            ),
            terminal_semantics::plain_affine::Error::MissingActiveCase { .. } => (
                terminal_check_tables::RefusalReason::Unsupported,
                terminal_semantics::plain_affine::Error::missing_active_case_message(),
            ),
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
            site: terminal_check_tables::RefusalSite::Machine(machine.id, concerns),
            source_span: lost_at,
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
        Err(refused) => return Ok(Err(refused)),
    };
    terminal_semantics::validate_structural_store_values(
        machine,
        &module.types.structural_types,
        &plan.stores,
    )
    .map_err(|error| match error {
        terminal_semantics::StructuralStoreValueError::SourceDeclaration(place) => Refusal {
            reason: RefusalReason::ForeignIdentity,
            site: terminal_check_tables::RefusalSite::Machine(machine.id, Construct::Machine),
            source_span: None,
            detail: format!("replacement input {place:?} has no unique source declaration"),
        },
        terminal_semantics::StructuralStoreValueError::CannotDiscard {
            operation,
            multiplicity,
        } => {
            let linear = multiplicity == terminal_psi::StructuralMultiplicity::Linear;
            Refusal {
                reason: if linear { RefusalReason::Violation } else { RefusalReason::Unsupported },
                site: terminal_check_tables::RefusalSite::Machine(machine.id, Construct::Operation(operation)),
                source_span: sources.get(machine.id)
                    .and_then(|source| source.operations.get(operation))
                    .and_then(|origin| origin.span()),
                detail: if linear {
                    "whole-value replacement cannot implicitly discard a linear value"
                } else {
                    "replacement input does not have the checked discardability required by this store"
                }.into(),
            }
        },
    })?;
    Ok(Ok(plan))
}

/// Install the store dispositions and cleanup `plan` reconstructed.
/// Returns `Ok(true)` iff `machine` was mutated.
fn install_storage_plan(
    machine: &mut terminal_psi::TerminalMachine,
    plan: &terminal_semantics::plain_affine::Plan,
) -> Checked<bool> {
    let has_scheduled_roots = plan.scheduled_roots != arena::IdSet::new();
    if plan.stores.is_empty() && plan.cleanup.is_empty() && !has_scheduled_roots {
        return Ok(false);
    }
    // Receiving storage may be intact or exactly vacated by a preceding
    // move. Install the mode reconstructed from the final ownership
    // frontier; source syntax alone cannot distinguish them. The checker
    // independently reconstructs these modes and the disposal schedule.
    let update_stores = !plan.stores.is_empty();
    let update_rosters = has_scheduled_roots || !plan.cleanup.is_empty();
    let mut changed = false;
    for block in &mut machine.blocks {
        if update_stores {
            for operation in &mut block.operations {
                if let terminal_psi::OperationKind::StoreStructuralField { disposition, .. } =
                    &mut operation.kind
                    && let Ok(position) = plan
                        .stores
                        .binary_search_by_key(&operation.id, |(operation, _)| *operation)
                    && *disposition != plan.stores[position].1
                {
                    *disposition = plan.stores[position].1;
                    changed = true;
                }
            }
        }
        if update_rosters {
            for (edge, actions) in block.terminator.cleanup_rosters_mut() {
                if has_scheduled_roots && !actions.is_empty() {
                    let before = actions.len();
                    actions.retain(|action| !plan.scheduled_roots.contains(action.place()));
                    if actions.len() != before {
                        changed = true;
                    }
                }
                if let Ok(position) = plan.cleanup.binary_search_by_key(&edge, |(edge, _)| *edge)
                    && !plan.cleanup[position].1.is_empty()
                {
                    actions.extend(plan.cleanup[position].1.iter().cloned());
                    changed = true;
                }
            }
        }
    }
    Ok(changed)
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
            membership_ensures: Vec::new(),
            content_ensures: Vec::new(),
            reaches: crate::Ceiling::Inferred,
            invokes: crate::Ceiling::Inferred,
            crashes: CrashCeiling::Authored(Vec::new()),
            termination: Termination::Silent,
            ranking: None,
        });
        let lower = |publication| {
            let mut signatures = signatures.clone();
            let components = prepare_call_components(&bodies, &mut signatures, &[]).unwrap();
            lower_program(
                Some(entry),
                &bodies,
                &components,
                body::Signatures {
                    inferred_inputs: None,
                    theorems: &[],
                    declarations: &IdTable::default(),
                    declaration_aliases: None,
                    machines: &signatures,
                    boundaries: &declare::boundaries::BoundaryNamespace::default(),
                },
                &IdTable::default(),
                &[],
                &callables::CallableNamespace::default(),
                &declare::boundaries::BoundaryNamespace::default(),
                &publication::reach::ServiceMap::default(),
                &declare::external_providers::ExternalProviderNamespace::default(),
                &callables::static_contracts::StaticContracts::default(),
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
