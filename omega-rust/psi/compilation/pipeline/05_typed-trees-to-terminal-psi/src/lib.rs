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
mod callable_formations;
mod compilation;
mod publication;
use publication::PreparedProgram;
pub use publication::{DesignatedOperands, SourceCorrespondence};
mod canonical;
mod components;
mod declare;
mod lower;

mod outcome;
mod policy;
mod proof_scope;
mod read;

pub use body::{
    Axis, Body, Borrowed, CaseType, Ceiling, CrashCeiling, DomainSet, Expression, ExpressionKind,
    Policy, Qualification, RecordField, RecordType, Selector, Signature, Statement, StorageShape,
    Strict, Target, Termination, ValueType, self_place,
};
pub use callable_formations::check_bound_callable_shapes;
pub use compilation::{PreparedSource, prepare_compilation};
pub use outcome::Outcome;
pub use terminal_check_tables::names::{Binder, ClauseText, MachineNames, Piece};

use semantic_vocabulary::{IdTable, MachineId, PackageKeyIdentity, StructuralTypeId};
use symbols::SymbolHandle;
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
    Branch, ConformanceWitness, Elaboration, MachineSources, Selected, SourceOrigin,
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

/// A `Violation` refusal: the program breaks a rule of the language at `at`,
/// and `rule` states the rule and how. The route publishes it as a rejection
/// of the program. Only for a form the specification makes an error in every
/// program: one the reader merely cannot judge is `refuse`, because a
/// rejection must never be said of a valid program.
pub(crate) fn reject(machine: MachineId, at: SourceOrigin, rule: impl Into<String>) -> Refusal {
    Refusal {
        reason: RefusalReason::Violation(match at {
            SourceOrigin::Authored(span) => Some(span),
            SourceOrigin::Unpositioned(_) => None,
        }),
        machine,
        concerns: Construct::Machine,
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
    elaborate_source_for(typed, compilation, publication).map(|prepared| prepared.program)
}

fn elaborate_source_for(
    typed: &TypedTrees,
    compilation: Compilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Checked<PreparedProgram> {
    let mut program = read::Program::new(typed, false);
    program.publication = publication;
    program.declaration_metadata = compilation.declaration_metadata.to_vec();
    let first = MachineId::new(1).expect("1 is nonzero");
    program.build_entry_context(first, compilation.purpose, compilation.entries)?;
    let entry = if let Some((entry, others)) = compilation.entries.split_first() {
        let entry = program.machine(*entry)?;
        program.fixed_inputs.push(entry);
        for other in others {
            let other = program.machine(*other)?;
            program.fixed_inputs.push(other);
        }
        Some(entry)
    } else {
        // A source check still claims every subject declaration. It creates
        // a declaration module, not an executable entry inferred from a name
        // or from whichever subject machine happens to be read first.
        None
    };
    program.selected_declarations(first, compilation.selected)?;
    program.supplied_slots(first, compilation.supplies)?;
    program.subject_declarations(first, compilation.subjects, compilation.purpose)?;
    let mut elaboration = finish_reading(program, entry, IdTable::default(), Some(typed))?;
    proof_scope::declare(typed, &mut elaboration.program)?;
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
/// certificates carry into it (`terminal_psi_to_terminal_psi::reverify_specialization`).
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
            finish_reading(program, Some(entry), IdTable::default(), None)
                .map(|prepared| prepared.program)
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
    let entry = program.witnessed(declaring_trait, requirement, arguments, implementation)?;
    let callee = program.implementation(implementation, declaring_trait, requirement)?;
    elaborate_witness_call(program, entry, callee)
}

fn elaborate_witness_call(
    mut program: read::Program<'_>,
    entry: MachineId,
    callee: MachineId,
) -> Checked<Elaboration> {
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
    finish_reading(program, Some(entry), bodies, None).map(|prepared| prepared.program)
}

fn elaborate_form(typed: &TypedTrees, entry: SymbolHandle, rigid: bool) -> Checked<Elaboration> {
    let mut program = read::Program::new(typed, rigid);
    let entry = program.machine(entry)?;
    finish_reading(program, Some(entry), IdTable::default(), None).map(|prepared| prepared.program)
}

/// Reads every body not yet in `bodies`, callees as they are discovered,
/// and lowers the closure. `declared` is the typed trees when the closure is a
/// compilation's program, whose machines are then given their declaration
/// identities (`declare`).
fn finish_reading(
    mut program: read::Program<'_>,
    entry: Option<MachineId>,
    mut bodies: IdTable<MachineId, Body>,
    declared: Option<&TypedTrees>,
) -> Checked<PreparedProgram> {
    // Refusals need a coordinate even for an empty declaration module. This
    // diagnostic anchor is never installed as an entry or an input boundary.
    let diagnostic = entry.unwrap_or_else(|| MachineId::new(1).expect("1 is nonzero"));
    // `program.machine` appends callees while bodies are read, so the bound
    // is re-read on every step.
    while bodies.len() < program.symbols.len() {
        let next = MachineId::new(bodies.len() as u64 + 1).ok_or_else(|| {
            refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Machine("identity overflow"),
            )
        })?;
        let body = program.body(next)?;
        bodies.push(body);
    }
    let private_inputs = program.private_input_contracts(entry, diagnostic)?;
    let local_results = program.local_result_bodies(entry)?;
    program.begin_declaration_storage(diagnostic, bodies.len())?;
    let declarations = program.declaration_metadata.clone();
    let calling_abis = program.prepare_calling_abis(diagnostic, &declarations)?;
    let PreparedProgram {
        program: mut elaboration,
        mut source,
    } = lower_generic_for(
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
            &program.declaration_symbols,
            &program.declaration_signatures,
            &mut elaboration,
        )
        .and_then(|()| {
            declare::provider_candidates(
                typed,
                &program.symbols,
                &program.boundary_symbols,
                &program.supplied,
                &mut elaboration.module,
            )
        })
        .map_err(|reason| {
            refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Declaration(reason),
            )
        })?;
    }
    source.records = program.source_record_correspondence(diagnostic)?;
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
    elaboration.supplied = program.supplied;
    Ok(PreparedProgram {
        program: elaboration,
        source,
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
    lower_generic_for(
        Some(entry),
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
    .map(|prepared| prepared.program)
}

fn lower_generic_for(
    entry: Option<MachineId>,
    bodies: &IdTable<MachineId, Body>,
    signatures: body::Signatures<'_>,
    records: &IdTable<StructuralTypeId, RecordType>,
    requirements: &[MachineId],
    publication: terminal_codec::TerminalPublication,
    declarations: Option<&read::Program<'_>>,
) -> Checked<PreparedProgram> {
    let diagnostic = entry.unwrap_or_else(|| MachineId::new(1).expect("1 is nonzero"));
    for requirement in requirements {
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
    let components = components::components(bodies, signatures.machines)?;
    let outcomes = outcome::outcomes(bodies, signatures, requirements)?;
    let mut module = match entry {
        Some(entry) => TerminalModule::for_entry(entry),
        None => TerminalModule::declarations(),
    };
    // Declaration checking publishes no executable arrival or root reach.
    if let Some(entry) = entry {
        module.root_service_reach.concrete =
            terminal_check_tables::row(&outcomes, entry, entry, Construct::Machine)?
                .service_reach
                .clone();
    }
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
    let declared = lower::places::declare(diagnostic, records, bodies, signatures, identities)?;
    module.identities = declared.identities;
    module.structural_types = declared.structural_types;
    module.native_layouts = declared.native_layouts;
    if let Some(source) = declarations {
        source.publish_services(diagnostic, &mut module)?;
        source.publish_boundaries(
            diagnostic,
            signatures,
            &outcomes,
            &declared.records,
            &mut module,
        )?;
    } else if !signatures.boundaries.is_empty() {
        return Err(refuse(
            diagnostic,
            Construct::Machine,
            Unserved::Declaration("boundary contracts have no exact source owner"),
        ));
    }
    let mut tables = CheckTables::default();
    let mut sources = IdTable::default();
    if let Some(source) = declarations {
        for (id, signature) in source.declaration_signatures.iter() {
            module
                .declaration_contracts
                .push(lower::declaration_contract(
                    diagnostic,
                    id,
                    signature,
                    signatures,
                    &outcomes,
                    &declared.records,
                )?);
        }
    }
    let mut source = SourceCorrespondence::default();
    for position in 1..=bodies.len() as u64 {
        let id = MachineId::new(position).ok_or_else(|| {
            refuse(
                diagnostic,
                Construct::Machine,
                Unserved::Machine("identity overflow"),
            )
        })?;
        let component = terminal_check_tables::row(&components, id, id, Construct::Machine)?;
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
            requirements.contains(&id),
        )?;
        module.machines.push(lowered.machine);
        // Machines are lowered in identity order and each machine's heads in
        // block order, which is the order the module's roster requires.
        module.scalar_block_invariants.extend(lowered.invariants);
        tables.machines.push(lowered.tables);
        sources.push(lowered.sources);
        source.designated.push(lowered.designated);
    }
    for machine in &mut module.machines {
        let plan = terminal_semantics::plain_affine::plan(machine, &module.structural_types)
            .map_err(|error| {
                let (concerns, operation, origin) = match error.site() {
                    Some(terminal_semantics::plain_affine::Site::Operation(at)) => (
                        Construct::Operation(at),
                        machine
                            .operations()
                            .find(|operation| operation.id == at)
                            .map(|operation| {
                                terminal_semantics::OperationSemanticTag::for_operation(
                                    &operation.kind,
                                )
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
                    detail: match (operation, origin) {
                        (Some(operation), Some(SourceOrigin::Authored(span))) => format!(
                            "while checking {operation} at source {} bytes {}..{}: {detail}",
                            span.source_id.0, span.span.start, span.span.end,
                        ),
                        (Some(operation), _) => format!("while checking {operation}: {detail}"),
                        (None, _) => detail.to_owned(),
                    },
                }
            })?;
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
    Ok(PreparedProgram {
        source,
        program: Elaboration {
            module,
            tables,
            sources,
            boundaries: IdTable::default(),
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
            supplied: Vec::new(),
            undeclared: Vec::new(),
        },
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
                Some(entry),
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
