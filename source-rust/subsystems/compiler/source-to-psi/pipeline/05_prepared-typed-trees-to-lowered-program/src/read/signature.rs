use crate::read::refuse_at;
/*
Turns the mathematical declarations this reader has read into one kernel
signature (`terminal_check_tables::mathematics`).

Each entry refers only to earlier ones. Data becomes the declarations
`PlainData` gives it: its canonical trees, one definition per case and an
induction principle. A machine with a result becomes a definition: a body
that dispatches on one formal is that induction, whose branch for a case
binds the case's fields, and a call of the machine itself at such a field is
the recursion's result for it. A machine with no result states one law per
`ensures` equality. Its body is not a term: the calls it makes are recorded as
citations, a call of the law itself at a field of the dispatched case as the
induction hypothesis, and stage 06 builds the proof from them.

A machine that returns a value and states an equality is both: a definition,
and a law whose statement may name the machine's own result. Its citations
are every call, anywhere in the value it returns, of a machine that states a
law. A machine declared inside another that states nothing is read in place
where it is called, as a step of its parent's value or of its parent's
argument.

A record with no cases whose fields are all mathematical data is the product
of its fields' types. A record written from its fields is a tuple, a field is
a projection, in a body and in a clause alike, and a `let` in a body is the
term of its value. A clause states a field as the field's declaration applied
to a value, so the value may be a formal, a call or another field.

A `bool` formal or result is one entry equal to the kernel's `Two`, with
`false` and `true` its two values. A clause that uses a machine with a `bool`
result as a proposition states that its result is `true`.

A law's `requires` equalities are its premises. Its statement takes a proof
of each after its formals, and stage 06 proves each wherever the law is
cited.

A cited law is read before the law that cites it, and a law that reaches
itself through its citations is refused, so no statement can justify itself.
*/

use arena::IdTable;
use mathematical_kernel::{
    Declaration, GenericPlainData, GenericPlainField, IndexedCase, IndexedData, IndexedField,
    Level, PlainData, PlainField, Sort, Term, TermHandle, instantiate, shift,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ClauseCarrier, ClauseIdentity, ClauseOperand, ClauseValue,
    IntegerMathTerm, IntegerValue, MachineId, Proposition, ScalarTerm, ScalarType,
    StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use symbols::SymbolHandle;
use terminal_check_tables::mathematics::{
    Citation, DeclaredLaw, Evidence, FamilyLawCase, FunctionCases, IntegerPositions, Law, LawCase,
    LawProof, LawSplit, MathematicalSignature, OperandLicense, SignatureEntry, SignatureEntryKind,
    SplitSubject, operand_law,
};
use terminal_check_tables::{Checked, Construct, Refusal};

use super::Program;
use super::families::{EvidenceUse, FamilyRecord, FamilyTerm, FamilyTheorem};
use super::proof_stratum::TemplateField;
use super::quotients::{LiftArgument, LiftedOperation};
use crate::Unserved;
use crate::body::{
    AppliedHead, ArmGuard, Body, CallTarget, CaseSelector, Expression, ExpressionKind, Statement,
    Target, ValueType,
};

/// The trait requirement a mathematical machine satisfies, read at the
/// trait's actual type arguments.
pub(crate) struct Satisfaction {
    machine: MachineId,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
    scope: Vec<ValueType>,
    /// The name after `as`, when the clause has one. Machines that satisfy
    /// one trait at the same type arguments under different names are
    /// different conformances: `combine as Nand` and `combine as And` are
    /// two operations, and a law proved for one says nothing of the other.
    group: Option<String>,
    ensures: Vec<Proposition>,
}

impl Program<'_> {
    /// Reads the requirement named by the machine's `satisfies` clause under
    /// that clause's type arguments. The requirement's equalities apply the
    /// trait's operations; the signature resolves each to its satisfier.
    pub(crate) fn satisfaction(&mut self, id: MachineId) -> Checked<Option<Satisfaction>> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        let symbol = *self
            .symbols
            .get(id)
            .ok_or_else(|| unserved("mathematical machine lost its declaration"))?;
        let (machine, _) = self.declaration(id, symbol)?;
        let mut edges = typed.machine_trait_conformances(machine).into_iter();
        let Some(edge) = edges.next() else {
            return Ok(None);
        };
        if edges.next().is_some() {
            return Err(unserved(
                "mathematical machine satisfying several requirements",
            ));
        }
        if edge.external_binding.is_some()
            || edge.via_expression.is_valid()
            || !edge.trait_lifetime_arguments.is_empty()
        {
            return Err(unserved("`satisfies` clause with a binding or lifetimes"));
        }
        let Some(crate::source_state::machine::SatisfiedDeclaration::Trait {
            definition,
            requirement,
        }) = crate::source_state::machine::resolve_satisfied_declaration(typed, machine, edge)
        else {
            return Err(unserved(
                "`satisfies` clause that names no requirement of a trait",
            ));
        };
        let application =
            crate::source_state::trait_definition::AppliedTraitScope::from_machine_conformance(
                typed, machine, edge,
            )
            .ok_or_else(|| unserved("`satisfies` clause lost its trait application"))?;
        // The requirement is read as the trait wrote it, with each type
        // parameter standing for this application's argument.
        let scope = self.applied_scope(id, &application)?;
        let required = self
            .static_requirement_signature(id, requirement, scope.clone(), false, false)
            .map_err(|refusal| {
                self.read_failure_at(refusal, symbol, "the requirement it satisfies")
            })?;
        let own = self
            .signatures
            .get(id)
            .ok_or_else(|| unserved("mathematical machine lost its signature"))?;
        if own.result.is_some()
            && (!required.ensures.is_empty() || required.has_structural_guarantees())
        {
            return Err(self.read_failure_at(
                unserved("operation requirement with a guarantee"),
                symbol,
                "the requirement it satisfies",
            ));
        }
        if required.parameters != own.parameters
            || required.result != own.result
            || !required.requires.is_empty()
        {
            return Err(self.read_failure_at(
                unserved("member signature differs from its requirement's"),
                symbol,
                "the requirement it satisfies",
            ));
        }
        Ok(Some(Satisfaction {
            machine: id,
            declaring_trait: definition.symbol,
            requirement: self.requirement_template(requirement.symbol),
            scope: scope.values.into_iter().map(|(_, value)| value).collect(),
            group: edge.alias.as_ref().map(|alias| alias.as_str().to_owned()),
            ensures: required.ensures,
        }))
    }

    fn requirement_template(&self, requirement: SymbolHandle) -> SymbolHandle {
        self.typed
            .closed_requirement_application(requirement)
            .map_or(requirement, |row| row.template)
    }

    /// The signature of every mathematical machine read so far.
    pub(crate) fn mathematical_signature(
        &self,
        bodies: &IdTable<MachineId, Body>,
        satisfactions: &[Satisfaction],
    ) -> Result<MathematicalSignature, Vec<Refusal>> {
        let mut machines = IdTable::default();
        let mut functions = IdTable::default();
        for _ in self.symbols.iter() {
            machines.push(Read::Unread);
            functions.push(Read::Unread);
        }
        let mut converter = Converter {
            program: self,
            bodies,
            satisfactions,
            signature: MathematicalSignature::default(),
            data: Vec::new(),
            boolean: None,
            machines,
            functions,
            fueled: Vec::new(),
            result_as: None,
            callable_scope: Vec::new(),
            generic_premises: Vec::new(),
            held: Vec::new(),
            stating_premises: false,
            integers: None,
            families: Vec::new(),
            generic: Vec::new(),
            declaring: Vec::new(),
        };
        let mut refused = Vec::new();
        let diagnostic = MachineId::new(1).expect("1 is nonzero");
        // The kernel numbers its quotient declarations from zero, so they
        // come before everything else.
        if !self.quotients.is_empty() {
            for declaration in mathematical_kernel::quotient_scheme(&mut converter.signature.arena)
            {
                converter.push(
                    declaration,
                    SignatureEntryKind::Scheme,
                    "quotient".to_owned(),
                );
            }
        }
        // Data indexed by values needs an integer vocabulary for the indices.
        if !self.families.is_empty() {
            if let Err(refusal) = converter.declare_integers(diagnostic) {
                return Err(vec![refusal]);
            }
            for family in &self.families {
                if let Err(refusal) = converter.indexed_family(diagnostic, family) {
                    refused.push(self.read_failure_at(
                        refusal,
                        family.declaration,
                        "the mathematical declaration",
                    ));
                }
            }
        }
        for &record in &self.self_containing {
            // Data indexed by values was declared above.
            if self.families.iter().any(|family| family.record == record) {
                continue;
            }
            if let Err(refusal) = converter.data(diagnostic, record) {
                let declaration = self
                    .record_sources
                    .get(record)
                    .map_or_else(SymbolHandle::invalid, |source| source.declaration);
                refused.push(self.read_failure_at(
                    refusal,
                    declaration,
                    "the mathematical declaration",
                ));
            }
        }
        // A quotient is declared with the laws of its relation, before any
        // machine that states something of it.
        for quotient in &self.quotients {
            if let Err(refusal) = converter.data(diagnostic, quotient.record) {
                refused.push(self.read_failure_at(
                    refusal,
                    quotient.declaration,
                    "the mathematical declaration",
                ));
            }
        }
        // A law whose requirement states associativity or commutativity of
        // an operation is read before the other machines, and then one whose
        // requirement states distributivity of one operation over another. A
        // proof may only rest on statements that come before its own, so this
        // is what lets any other law regroup, reorder and expand under these
        // wherever the source declares them, and lets a distributivity law
        // regroup and reorder under the first ones.
        //
        // One type's laws may lean on another's: a pair of numbers adds by
        // adding its fields, so its laws need the numbers' laws first. The
        // conformances are taken in the order of the data they are about,
        // and data is declared after the data its fields hold.
        let mut ordered: Vec<(u32, &Satisfaction)> = satisfactions
            .iter()
            .map(|satisfaction| {
                let about = satisfaction
                    .scope
                    .iter()
                    .filter_map(|argument| match argument {
                        ValueType::Record(record, ..) => {
                            converter.data(satisfaction.machine, *record).ok()
                        }
                        _ => None,
                    })
                    .max();
                // A conformance about no readable data is refused where its
                // machines are declared; it sorts last here.
                (about.unwrap_or(u32::MAX), satisfaction)
            })
            .collect();
        ordered.sort_by_key(|(about, _)| *about);
        let mut first = Vec::new();
        let mut distributive: Vec<(u32, MachineId)> = Vec::new();
        let mut before = None;
        for (about, satisfaction) in ordered {
            // The distributivity laws of the conformances before this one.
            if before.is_some_and(|before| before != about) {
                for (_, id) in distributive.drain(..) {
                    let read = converter.declare(id);
                    first.push(id);
                    if let Err(refusal) = read {
                        let symbol = self
                            .symbols
                            .get(id)
                            .copied()
                            .unwrap_or_else(SymbolHandle::invalid);
                        refused.push(self.read_failure_at(
                            refusal,
                            symbol,
                            "the mathematical declaration",
                        ));
                    }
                }
            }
            before = Some(about);
            let id = satisfaction.machine;
            let read = match converter.requires_operand_law(satisfaction) {
                Ok(None) => continue,
                Ok(Some(terminal_check_tables::mathematics::OperandLaw::Distributes {
                    ..
                })) => {
                    distributive.push((about, id));
                    continue;
                }
                Ok(Some(_)) => converter.declare(id),
                Err(refusal) => {
                    if let Some(state @ Read::Unread) = converter.machines.get_mut(id) {
                        *state = Read::Refused;
                    }
                    Err(refusal)
                }
            };
            first.push(id);
            if let Err(refusal) = read {
                let symbol = self
                    .symbols
                    .get(id)
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                refused.push(self.read_failure_at(refusal, symbol, "the mathematical declaration"));
            }
        }
        for (_, id) in distributive {
            let read = converter.declare(id);
            first.push(id);
            if let Err(refusal) = read {
                let symbol = self
                    .symbols
                    .get(id)
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                refused.push(self.read_failure_at(refusal, symbol, "the mathematical declaration"));
            }
        }
        for (id, symbol) in self.symbols.iter() {
            if first.contains(&id) {
                continue;
            }
            let read = converter.declare(id).map_err(|refusal| {
                self.read_failure_at(refusal, *symbol, "the mathematical declaration")
            });
            if let Err(refusal) = read {
                refused.push(refusal);
            }
        }
        // What the machines that run build of indexed data.
        for used in &self.evidence_uses {
            if let Err(refusal) = converter.evidence_laws(diagnostic, used) {
                refused.push(self.read_failure_at(refusal, used.machine, "the evidence"));
            }
        }
        // A machine that states no law means something only where a law
        // calls it. One that nothing calls would be left unread.
        for (id, symbol) in self.symbols.iter() {
            if matches!(
                converter.machines.get(id),
                Some(Read::Cites { called: false })
            ) {
                refused.push(self.read_failure_at(
                    unserved(
                        id,
                        "mathematical machine that states no law and that no law calls",
                    ),
                    *symbol,
                    "the mathematical declaration",
                ));
            }
        }
        if !refused.is_empty() {
            return Err(refused);
        }
        // The license is the declared conformance: the law and the
        // operations it speaks of satisfy requirements of one trait at the
        // same type arguments.
        let mut licenses = Vec::new();
        for satisfaction in satisfactions {
            let Some(Read::Laws(laws)) = converter.machines.get(satisfaction.machine) else {
                continue;
            };
            for &law in laws {
                let statement = converter.signature.laws[law as usize].statement;
                let stated = converter.signature.declarations[statement as usize].ty;
                let Some((operation, law)) = operand_law(&converter.signature.arena, stated) else {
                    continue;
                };
                let conforms = |operation: u32| {
                    satisfactions.iter().any(|other| {
                        other.declaring_trait == satisfaction.declaring_trait
                            && other.scope == satisfaction.scope
                            && other.group == satisfaction.group
                            && matches!(
                                converter.functions.get(other.machine),
                                Some(Read::Function(entry)) if *entry == operation
                            )
                    })
                };
                use terminal_check_tables::mathematics::OperandLaw;
                let over = match law {
                    OperandLaw::Distributes { over, .. } => Some(over),
                    OperandLaw::Commutes | OperandLaw::Associates { .. } => None,
                };
                if conforms(operation) && over.is_none_or(conforms) {
                    licenses.push(OperandLicense {
                        operation,
                        statement,
                        law,
                    });
                }
            }
        }
        licenses.sort_by_key(|license| license.statement);
        converter.signature.licenses = licenses;
        Ok(converter.signature)
    }
}

#[derive(Clone)]
enum Read {
    Unread,
    /// On the path being read: reaching it again is a cycle.
    Reading,
    Function(u32),
    /// The laws the machine states, by their index.
    Laws(Vec<u32>),
    /// A machine that states no law. Its body is read where a law calls it,
    /// with its formals standing for that call's arguments.
    Cites {
        called: bool,
    },
    /// An operation on a quotient, judged (`lifted`). A term that applies
    /// it reads its definition into `functions` (`lifted_function`).
    Lifted,
    Refused,
}

/// What a name of the body stands for, and whether it is a field of the
/// dispatched case: only such a term may carry a recursive call.
#[derive(Clone, Copy)]
struct Bound {
    term: TermHandle,
    field: Option<u32>,
    /// With `field`, a field that holds a function to the data applied to
    /// this argument: the child there, whose recursion is the field's
    /// hypothesis at the argument.
    applied: Option<TermHandle>,
}

struct Scope<'s> {
    /// What each formal stands for, by position.
    slots: &'s [Option<Bound>],
    /// The formal each slot of the body names: a dispatch reads its subject
    /// through a local bound to it.
    aliases: &'s [usize],
    /// The dispatched formal, the case being read and its fields.
    case: Option<(usize, usize, &'s [TermHandle])>,
    /// The machine whose body this is and, for each field of the case, the
    /// recursion's result for it.
    recursion: Option<Recursion<'s>>,
    /// How many fields the enclosing cases of a law bind: its terms are
    /// stated under the law's formals and then those fields.
    depth: u32,
    /// Whether `case` is the case of the recursion or induction. A case
    /// split inside one is not: the law at one of its fields is no
    /// hypothesis.
    induction: bool,
    /// Set where a body is read for its value.
    value: Option<&'s Valuing<'s>>,
}

/// A definition by cases read as the kernel's recursion: a call of the
/// machine, or of another machine of its group, at a field of the case is the
/// recursion's result at that field.
#[derive(Clone, Copy)]
struct Recursion<'s> {
    machine: MachineId,
    /// The dispatched formal.
    subject: usize,
    /// For each field of the case, the recursion's result there, if the
    /// field holds the data or another member of its block.
    results: &'s [Option<TermHandle>],
    /// For a definition by mutual recursion over a block of data: each
    /// machine defined with it, with the member it dispatches on. Empty
    /// otherwise.
    group: &'s [(MachineId, u32)],
    /// For each field of the case, the member of the block it holds.
    members: &'s [Option<u32>],
}

impl Recursion<'_> {
    /// Whether a call of `callee` is the recursion, and, for a mutual group,
    /// the member it recurses on.
    fn calls(&self, callee: MachineId) -> Option<Option<u32>> {
        if self.group.is_empty() {
            return (callee == self.machine).then_some(None);
        }
        self.group
            .iter()
            .find(|(machine, _)| *machine == callee)
            .map(|(_, member)| Some(*member))
    }
}

/// How many variables `scope` binds inside the formals of the machine
/// being read, which a machine parameter, bound outside them, is past: the
/// fields of the enclosing cases of a law. A body read for its value has
/// its machine parameters in place (`by_cases` binds them in each branch).
fn callable_shift(scope: &Scope<'_>) -> u32 {
    if scope.value.is_some() {
        0
    } else {
        scope.depth
    }
}

/// What reading a body for its value needs beyond its names.
#[derive(Clone, Copy)]
struct Valuing<'s> {
    /// The machine being defined.
    owner: MachineId,
    /// The type of each variable in scope, outermost first. Each is closed.
    /// A dispatch is a definition of its own over its subject and all of
    /// these, so it needs them.
    context: &'s [TermHandle],
    /// The closed type of the value.
    result: TermHandle,
    /// How the machine being defined calls itself, when it recurses on fuel.
    fuel: Option<Fuel>,
    /// The machine's own premises, as terms of this scope.
    premises: &'s [TermHandle],
    /// For each enclosing dispatch, the equation between its subject and
    /// the case being read, as terms of this scope. A dispatch on a computed
    /// value such as `saturating_sub(b, a)` replaces the value by the case,
    /// so this is the only place the branch keeps what it knows. A call
    /// here owes its premises and descent under these as well.
    facts: &'s [TermHandle],
    /// What the bodies read so far have cited.
    cited: &'s [Citation],
}

/// A machine defined by recursion on fuel calls itself as the definition
/// with one unit less: `step` applied to the call's arguments. With no fuel
/// left there is no such definition and the call is `default`.
#[derive(Clone, Copy)]
struct Fuel {
    machine: MachineId,
    step: Option<TermHandle>,
    default: TermHandle,
    /// Set in the one reading of the body that owes each recursive call's
    /// descent: the body is read once per case of the fuel, and the
    /// obligation is the same in each.
    descent: Option<Descent>,
}

/// What a recursive call at a computed natural owes: its ranked argument
/// `next` strictly below the caller's ranked formal `ranked` in natural
/// order, spelled as core spells it, `saturating_sub(Succ(next), ranked) =
/// Zero` (`wiki/spec/language/termination.md`, "Computed recursive arguments
/// in proofs").
#[derive(Clone, Copy)]
struct Descent {
    /// The position of the ranked formal among the machine's formals.
    measure: usize,
    /// The caller's ranked formal, as a term of the body's scope.
    ranked: TermHandle,
    /// The entry of the ranked formal's data.
    nat: u32,
}

/// How an operation on a quotient passes its formals to the operation on
/// representatives (`Converter::lift_shape`).
struct LiftShape {
    /// The operation on representatives.
    operation: MachineId,
    /// The type and entry of each formal of the operation on classes.
    formals: Vec<(TermHandle, u32)>,
    /// The type and entry of each parameter of the operation on
    /// representatives.
    parameters: Vec<(TermHandle, u32)>,
    /// For each formal, the parameter it is passed to, when the operation
    /// passes each formal to exactly one parameter as it is.
    receives: Option<Vec<usize>>,
    /// For each parameter, the carrier and relation of the quotient its
    /// formal is a class of, when it is one. Only a formal passed as it is
    /// may be a class.
    classes: Vec<Option<(u32, u32)>>,
    /// The result type of the operation on classes, and its entry.
    result: TermHandle,
    result_entry: u32,
    /// The quotient the result is a class of, when it is one.
    result_class: Option<(u32, u32)>,
    /// The entry of the operation on representatives' definition.
    defined: u32,
}

/// The machine a definition is being read for.
struct Defining<'d> {
    id: MachineId,
    symbol: SymbolHandle,
    name: &'d str,
    formals: &'d [(TermHandle, u32)],
    /// The type of each machine parameter of a generic machine, bound
    /// before its formals.
    callables: &'d [TermHandle],
    result: TermHandle,
    premises: &'d [TermHandle],
}

struct Converter<'p, 't> {
    program: &'p Program<'t>,
    bodies: &'p IdTable<MachineId, Body>,
    satisfactions: &'p [Satisfaction],
    signature: MathematicalSignature,
    /// Each data record with its entry, in the order they were declared.
    data: Vec<(StructuralTypeId, u32)>,
    /// The entry of `bool`, once a machine has a Boolean formal or result.
    boolean: Option<u32>,
    /// Each machine as a law, or as a machine that only cites.
    machines: IdTable<MachineId, Read>,
    /// Each machine with a result as a definition. A machine that returns a
    /// value and states a law has a row in both tables.
    functions: IdTable<MachineId, Read>,
    /// Each machine defined by recursion on fuel: the entry of the
    /// definition over the fuel, the entry of the fuel's data and the
    /// formal the machine takes its fuel from.
    fueled: Vec<(MachineId, u32, u32, u32)>,
    /// While a law is read for its induction on fuel: the machine and the
    /// term its clauses' `result` stands for.
    result_as: Option<(MachineId, TermHandle)>,
    /// The machine parameters of the generic machine being read, each as
    /// the variable it is under that machine's formals.
    callable_scope: Vec<(SymbolHandle, TermHandle)>,
    /// The premises of each generic machine read so far, as terms under
    /// its machine parameters and formals: what a call of it owes.
    generic_premises: Vec<(MachineId, Vec<TermHandle>)>,
    /// The premises of the machine being read that are read so far, as
    /// terms under its formals: what a clause of it may rest a call on.
    held: Vec<TermHandle>,
    /// Set while a called machine's premises are stated at a call's
    /// arguments: what those premises themselves call was owed where that
    /// machine was declared.
    stating_premises: bool,
    /// The integer vocabulary, declared when the program has data indexed
    /// by values.
    integers: Option<proof_admission::SourceIntegerTheory>,
    /// Each data declaration indexed by values, as it was declared.
    families: Vec<FamilyDeclared>,
    /// Each generic data declaration declared so far: its symbol, the entry
    /// of its inductive former and its fields.
    generic: Vec<(SymbolHandle, u32, GenericPlainData)>,
    /// The data records being declared, outermost first, while their
    /// payload types are read.
    declaring: Vec<StructuralTypeId>,
}

/// Data indexed by values in the signature: where its family is, and what
/// each case requires and holds, under one binder per index and then one
/// per payload value.
#[derive(Clone)]
struct FamilyDeclared {
    record: StructuralTypeId,
    /// Its position in `MathematicalSignature::families`.
    family: u32,
    cases: Vec<FamilyCaseTerms>,
}

#[derive(Clone)]
struct FamilyCaseTerms {
    payload: u32,
    constraints: Vec<TermHandle>,
    /// For each child, the index it sits at, one term per index.
    children: Vec<Vec<TermHandle>>,
}

/// What the scalars of a clause over indexed data stand for: index `i` is
/// the value numbered `i + 1`, and a payload value is a field of the one
/// case in scope.
struct Integers<'e> {
    indices: &'e [TermHandle],
    payload: Option<(StructuralCaseId, &'e [(StructuralFieldId, TermHandle)])>,
}

fn unserved(id: MachineId, what: &'static str) -> Refusal {
    refuse_at(id, Unserved::Machine(what))
}

/// The slot each slot of a body stands for, following every local bound to
/// another slot, and the statements that remain.
fn spine(body: &Body) -> (Vec<usize>, Vec<&Statement>) {
    let mut aliases: Vec<usize> = (0..body.slots.len()).collect();
    let mut statements = Vec::with_capacity(body.statements.len());
    for statement in &body.statements {
        match statement {
            Statement::Bind { slot, value }
                if matches!(value.kind, ExpressionKind::Slot(source) if source < aliases.len())
                    && *slot < aliases.len() =>
            {
                let ExpressionKind::Slot(source) = value.kind else {
                    unreachable!("matched above")
                };
                aliases[*slot] = aliases[source];
            }
            other => statements.push(other),
        }
    }
    (aliases, statements)
}

fn index(id: MachineId, length: usize) -> Checked<u32> {
    u32::try_from(length).map_err(|_| unserved(id, "mathematical signature index overflow"))
}

impl<'p> Converter<'p, '_> {
    fn constant(&mut self, declaration: u32) -> TermHandle {
        self.signature.arena.insert(Term::Constant {
            declaration,
            levels: Vec::new(),
        })
    }

    fn variable(&mut self, index: u32) -> TermHandle {
        self.signature.arena.insert(Term::Variable(index))
    }

    fn name(&self, symbol: SymbolHandle) -> String {
        self.program.typed.symbols.name(symbol).to_owned()
    }

    fn push(&mut self, declaration: Declaration, kind: SignatureEntryKind, name: String) -> u32 {
        let position = self.signature.declarations.len() as u32;
        self.signature.declarations.push(declaration);
        self.signature.entries.push(SignatureEntry { kind, name });
        position
    }

    /// The entry of a data declaration that contains itself, declaring it and
    /// its cases the first time.
    fn data(&mut self, id: MachineId, record: StructuralTypeId) -> Checked<u32> {
        if let Some(&(_, entry)) = self.data.iter().find(|(known, _)| *known == record) {
            return Ok(entry);
        }
        let program = self.program;
        if let Some(quotient) = program
            .quotients
            .iter()
            .find(|quotient| quotient.record == record)
        {
            return self.quotient(id, quotient);
        }
        if program.self_containing.binary_search(&record).is_err() {
            return self.product(id, record);
        }
        let declared = program
            .records
            .get(record)
            .ok_or_else(|| unserved(id, "mathematical data lost its record"))?;
        let source = program
            .record_sources
            .get(record)
            .ok_or_else(|| unserved(id, "mathematical data lost its declaration"))?;
        // A payload that is itself data being declared further up is a
        // recursion through another declaration: a mutual block, which this
        // reader does not declare.
        if self.declaring.contains(&record) {
            return Err(unserved(
                id,
                "data that contains itself through another declaration",
            ));
        }
        let generic = source
            .application
            .as_ref()
            .is_some_and(|application| !application.substitutions.is_empty());
        if !generic {
            let block = program
                .typed
                .unique_data_definition_by_symbol(source.declaration)
                .map(|definition| program.mutual_block(definition))
                .unwrap_or_default();
            if block.len() > 1 {
                return self.mutual_data(id, record, &block);
            }
        }
        self.declaring.push(record);
        let read = if generic {
            self.generic_application(id, record)
        } else {
            self.plain_data(id, record)
        };
        self.declaring.pop();
        let (shape, declarations) = read?;
        let first = index(id, self.signature.declarations.len())?;
        index(id, self.signature.declarations.len() + declarations.len())?;
        let entry = PlainData::data_entry(first);
        let name = if source
            .application
            .as_ref()
            .is_some_and(|application| !application.substitutions.is_empty())
        {
            declared.identity.clone()
        } else {
            self.name(source.declaration)
        };
        for (offset, declaration) in declarations.into_iter().enumerate() {
            let position = first + offset as u32;
            let case = position
                .checked_sub(entry + 1)
                .and_then(|case| source.cases.get(case as usize).map(|found| (case, found.0)));
            let (kind, entry_name) = match case {
                _ if position == entry => (
                    SignatureEntryKind::Data {
                        declaration: source.declaration,
                        shape: shape.clone(),
                        type_identity: declared.identity.clone(),
                    },
                    name.clone(),
                ),
                Some((case, symbol)) => (
                    SignatureEntryKind::Constructor { data: entry, case },
                    format!("{name}::{}", self.name(symbol)),
                ),
                None => (SignatureEntryKind::Encoding { data: entry }, name.clone()),
            };
            self.push(declaration, kind, entry_name);
        }
        self.data.push((record, entry));
        Ok(entry)
    }

    /// Declares the mutual block of data that contains itself through the
    /// other declarations of `block` (`Program::mutual_block`), one native
    /// inductive block: each member's type, every member's constructors and
    /// each member's recursor. A field of a member is a recursive child of
    /// that member, and any other field a payload. Returns `record`'s entry.
    fn mutual_data(
        &mut self,
        id: MachineId,
        record: StructuralTypeId,
        block: &[SymbolHandle],
    ) -> Checked<u32> {
        let program = self.program;
        // Each member's record: the block's records are read together, since
        // a field of one names another.
        let mut records = Vec::with_capacity(block.len());
        for &symbol in block {
            let found = program.self_containing.iter().copied().find(|&known| {
                program.record_sources.get(known).is_some_and(|source| {
                    source.declaration == symbol
                        && source
                            .application
                            .as_ref()
                            .is_none_or(|application| application.substitutions.is_empty())
                })
            });
            records.push(found.ok_or_else(|| {
                unserved(id, "member of a mutual block of data that no type reads")
            })?);
        }
        if records.iter().any(|record| self.declaring.contains(record)) {
            return Err(unserved(
                id,
                "mutual block of data whose payload contains the block",
            ));
        }
        self.declaring.extend(records.iter().copied());
        let read = (|| {
            let mut members = Vec::with_capacity(records.len());
            for (own, &member) in records.iter().enumerate() {
                let declared = program
                    .records
                    .get(member)
                    .ok_or_else(|| unserved(id, "mathematical data lost its record"))?;
                let mut cases = Vec::with_capacity(declared.cases().len());
                for case in declared.cases() {
                    let mut fields = Vec::with_capacity(case.fields.len());
                    for field in &case.fields {
                        fields.push(self.data_field(id, field.value_type, &records, own)?);
                    }
                    cases.push(fields);
                }
                members.push(cases);
            }
            PlainData::block(members)
                .ok_or_else(|| unserved(id, "mutual block of data with a member without a case"))
        })();
        self.declaring
            .truncate(self.declaring.len() - records.len());
        let shapes = read?;
        let first = index(id, self.signature.declarations.len())?;
        let declarations = shapes[0].declarations(&mut self.signature.arena, first);
        index(id, self.signature.declarations.len() + declarations.len())?;
        let mut kinds = Vec::with_capacity(declarations.len());
        let mut names = Vec::with_capacity(records.len());
        for (member, &member_record) in records.iter().enumerate() {
            let declared = program
                .records
                .get(member_record)
                .ok_or_else(|| unserved(id, "mathematical data lost its record"))?;
            names.push(self.name(block[member]));
            kinds.push((
                SignatureEntryKind::Data {
                    declaration: block[member],
                    shape: shapes[member].clone(),
                    type_identity: declared.identity.clone(),
                },
                names[member].clone(),
            ));
        }
        for (member, &member_record) in records.iter().enumerate() {
            let source = program
                .record_sources
                .get(member_record)
                .ok_or_else(|| unserved(id, "mathematical data lost its declaration"))?;
            for (case, (symbol, _)) in source.cases.iter().enumerate() {
                kinds.push((
                    SignatureEntryKind::Constructor {
                        data: first + member as u32,
                        case: case as u32,
                    },
                    format!("{}::{}", names[member], self.name(*symbol)),
                ));
            }
        }
        for (member, name) in names.iter().enumerate() {
            kinds.push((
                SignatureEntryKind::Encoding {
                    data: first + member as u32,
                },
                name.clone(),
            ));
        }
        if kinds.len() != declarations.len() {
            return Err(unserved(
                id,
                "mutual block of data whose cases differ from its declarations",
            ));
        }
        for (declaration, (kind, name)) in declarations.into_iter().zip(kinds) {
            self.push(declaration, kind, name);
        }
        for (member, &member_record) in records.iter().enumerate() {
            self.data.push((member_record, first + member as u32));
        }
        let at = records
            .iter()
            .position(|&member| member == record)
            .expect("the record is a member of its own block");
        Ok(first + at as u32)
    }

    /// What a field of type `value_type` of member `own` of the block whose
    /// members' records are `records`, in order, holds: a member, a callable
    /// to a member, which holds a child for each argument, or a payload of
    /// its own kernel type. A callable whose parameter names the block is a
    /// negative occurrence and is refused, as is one whose result holds the
    /// block inside another type.
    fn data_field(
        &mut self,
        id: MachineId,
        value_type: ValueType,
        records: &[StructuralTypeId],
        own: usize,
    ) -> Checked<PlainField> {
        let member = |ty: ValueType| {
            records
                .iter()
                .position(|&other| ty == ValueType::record(other))
        };
        if let Some(at) = member(value_type) {
            return Ok(if at == own {
                PlainField::Recursive
            } else {
                PlainField::Member(at as u32)
            });
        }
        if let ValueType::Scalar(ScalarType::Callable(contract), _) = value_type
            && self.mentions_block(value_type, records, 0)
        {
            let signature = self
                .program
                .callables
                .contracts
                .get(contract)
                .map(|row| row.signature.clone())
                .ok_or_else(|| unserved(id, "stored callable lost its contract"))?;
            if signature
                .parameters
                .iter()
                .any(|&parameter| self.mentions_block(parameter, records, 0))
            {
                return Err(unserved(
                    id,
                    "data that contains itself as the input of a stored callable: a negative occurrence",
                ));
            }
            let Some(at) = signature.result.and_then(member) else {
                return Err(unserved(
                    id,
                    "data that contains itself inside the result of a stored callable",
                ));
            };
            let (&[parameter], true, true) = (
                signature.parameters.as_slice(),
                signature.erased.is_empty(),
                signature.borrowed.is_empty(),
            ) else {
                return Err(unserved(
                    id,
                    "data that contains itself through a stored callable of other than one plain parameter",
                ));
            };
            let argument = self.value_ty(id, parameter)?.0;
            return Ok(PlainField::Function {
                argument,
                member: at as u32,
            });
        }
        Ok(PlainField::Payload(self.value_ty(id, value_type)?.0))
    }

    /// Whether a type is a member of the block, or a callable that takes or
    /// returns one, however deeply.
    fn mentions_block(&self, ty: ValueType, records: &[StructuralTypeId], depth: u32) -> bool {
        if records
            .iter()
            .any(|&record| ty == ValueType::record(record))
        {
            return true;
        }
        let ValueType::Scalar(ScalarType::Callable(contract), _) = ty else {
            return false;
        };
        let Some(row) = self.program.callables.contracts.get(contract) else {
            return false;
        };
        depth < 16
            && row
                .signature
                .parameters
                .iter()
                .chain(row.signature.result.iter())
                .any(|&inner| self.mentions_block(inner, records, depth + 1))
    }

    /// The cases of plain data that contains itself, each field either the
    /// data itself or a payload of a type the signature declares, and its
    /// declarations from the next free entry on.
    fn plain_data(
        &mut self,
        id: MachineId,
        record: StructuralTypeId,
    ) -> Checked<(PlainData, Vec<Declaration>)> {
        let declared = self
            .program
            .records
            .get(record)
            .ok_or_else(|| unserved(id, "mathematical data lost its record"))?;
        let mut cases = Vec::with_capacity(declared.cases().len());
        for case in declared.cases() {
            let mut fields = Vec::with_capacity(case.fields.len());
            for field in &case.fields {
                fields.push(self.data_field(id, field.value_type, &[record], 0)?);
            }
            cases.push(fields);
        }
        let shape = PlainData::new(cases)
            .ok_or_else(|| unserved(id, "data that contains itself without a case"))?;
        let first = index(id, self.signature.declarations.len())?;
        let declarations = shape.declarations(&mut self.signature.arena, first);
        Ok((shape, declarations))
    }

    /// An application of generic data that contains itself (`Seq<u64>`): the
    /// generic declaration at the application's type arguments. The generic
    /// declaration is declared the first time, from its template, as one
    /// inductive block with a uniform parameter per type parameter; the
    /// application's entries are definitions equal to that block's at the
    /// arguments, from the next free entry on.
    fn generic_application(
        &mut self,
        id: MachineId,
        record: StructuralTypeId,
    ) -> Checked<(PlainData, Vec<Declaration>)> {
        let program = self.program;
        let declared = program
            .records
            .get(record)
            .ok_or_else(|| unserved(id, "mathematical data lost its record"))?;
        let source = program
            .record_sources
            .get(record)
            .ok_or_else(|| unserved(id, "mathematical data lost its declaration"))?;
        let (parameters, template) = program
            .generic_template_fields(source.declaration)
            .map_err(|what| unserved(id, what))?;
        if template.len() != declared.cases().len()
            || template
                .iter()
                .zip(declared.cases())
                .any(|(fields, case)| fields.len() != case.fields.len())
        {
            return Err(unserved(
                id,
                "application of generic data whose cases differ from its template",
            ));
        }
        // Each type argument, read from a field the template gives that
        // parameter's type; and each field whose type names no parameter,
        // the same at every application.
        let mut arguments = vec![None; parameters as usize];
        let mut closed = Vec::with_capacity(template.len());
        for (fields, case) in template.iter().zip(declared.cases()) {
            let mut row = Vec::with_capacity(fields.len());
            for (kind, field) in fields.iter().zip(&case.fields) {
                row.push(match *kind {
                    TemplateField::Recursive => {
                        if field.value_type != ValueType::record(record) {
                            return Err(unserved(
                                id,
                                "application of generic data whose recursive field is another type",
                            ));
                        }
                        GenericPlainField::Recursive
                    }
                    TemplateField::Parameter(position) => {
                        let ty = self.value_ty(id, field.value_type)?.0;
                        match arguments[position as usize] {
                            Some(known) if !self.signature.arena.structurally_equal(known, ty) => {
                                return Err(unserved(
                                    id,
                                    "application of generic data at two types for one parameter",
                                ));
                            }
                            _ => arguments[position as usize] = Some(ty),
                        }
                        GenericPlainField::Parameter(position)
                    }
                    TemplateField::Closed => {
                        GenericPlainField::Closed(self.value_ty(id, field.value_type)?.0)
                    }
                });
            }
            closed.push(row);
        }
        let arguments: Vec<TermHandle> =
            arguments
                .into_iter()
                .collect::<Option<_>>()
                .ok_or_else(|| {
                    unserved(
                        id,
                        "generic mathematical data with a parameter no field holds",
                    )
                })?;
        let generic = match self
            .generic
            .iter()
            .find(|(symbol, ..)| *symbol == source.declaration)
        {
            Some((_, entry, generic)) => {
                // A closed field names no parameter, so every application
                // reads it at the same type.
                let same =
                    GenericPlainData::new(parameters, closed).is_some_and(|read| &read == generic);
                if !same {
                    return Err(unserved(
                        id,
                        "application of generic data whose closed fields differ from the template's",
                    ));
                }
                (*entry, generic.clone())
            }
            None => {
                let generic = GenericPlainData::new(parameters, closed).ok_or_else(|| {
                    unserved(id, "generic data that contains itself without a case")
                })?;
                let first = index(id, self.signature.declarations.len())?;
                index(
                    id,
                    self.signature.declarations.len() + generic.width() as usize,
                )?;
                let declarations = generic.declarations(&mut self.signature.arena, first);
                let name = self.name(source.declaration);
                for (offset, declaration) in declarations.into_iter().enumerate() {
                    let kind = if offset == 0 {
                        SignatureEntryKind::GenericData {
                            declaration: source.declaration,
                        }
                    } else {
                        SignatureEntryKind::Encoding { data: first }
                    };
                    self.push(declaration, kind, name.clone());
                }
                self.generic
                    .push((source.declaration, first, generic.clone()));
                (first, generic)
            }
        };
        let (entry, generic) = generic;
        let recursor_type = self.signature.declarations[(entry + generic.width() - 1) as usize].ty;
        let first = index(id, self.signature.declarations.len())?;
        generic
            .instance(
                &mut self.signature.arena,
                entry,
                recursor_type,
                first,
                &arguments,
            )
            .ok_or_else(|| unserved(id, "application of generic data the kernel cannot state"))
    }

    /// The kernel's quotient of the data at entry `carrier` by the relation
    /// at entry `relation`, both at the lowest universe.
    fn family(&mut self, carrier: u32, relation: u32) -> mathematical_kernel::QuotientFamily {
        mathematical_kernel::QuotientFamily {
            levels: [Level::Constant(0), Level::Constant(0)],
            carrier: self.constant(carrier),
            relation: self.constant(relation),
        }
    }

    /// Declares a quotient of mathematical data: its relation as a
    /// definition over two values of the carrier, the quotient as the
    /// kernel's quotient of the two, and each law of the selected
    /// equivalence as the sealed statement its machine must have proved.
    fn quotient(
        &mut self,
        id: MachineId,
        quotient: &super::quotients::QuotientRecord,
    ) -> Checked<u32> {
        let carrier = self.data(id, quotient.carrier)?;
        let ty = self.constant(carrier);
        let open = [self.variable(1), self.variable(0)];
        let formula = self.equality(id, &quotient.formula, &open, None)?;
        let arena = &mut self.signature.arena;
        let universe = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let (mut related, mut relation_type) = (formula, universe);
        for _ in 0..2 {
            related = arena.insert(Term::Lambda {
                domain: ty,
                body: related,
            });
            relation_type = arena.insert(Term::Pi {
                domain: ty,
                codomain: relation_type,
            });
        }
        index(id, self.signature.declarations.len() + 2)?;
        let relation = self.push(
            Declaration::definition(0, relation_type, related),
            SignatureEntryKind::Function {
                machine: quotient.relation,
                cases: None,
            },
            quotient.relation_name.clone(),
        );
        let family = self.family(carrier, relation);
        let body = family.quotient(&mut self.signature.arena);
        let entry = self.push(
            Declaration::definition(0, universe, body),
            SignatureEntryKind::Quotient {
                declaration: quotient.declaration,
                carrier,
                relation,
                laws: [0; 4],
                type_identity: quotient.identity.clone(),
            },
            self.name(quotient.declaration),
        );
        self.data.push((quotient.record, entry));
        let laws = self.equivalence(id, quotient, carrier, relation)?;
        if let SignatureEntryKind::Quotient { laws: stored, .. } =
            &mut self.signature.entries[entry as usize].kind
        {
            *stored = laws;
        }
        Ok(entry)
    }

    /// The four laws of the `MereEquivalence` a quotient selects. Each row's
    /// machine must have proved exactly the sealed statement of its law
    /// about this relation: for all values of the carrier, `R a a`; from
    /// `R a b`, `R b a`; from `R a b` and `R b c`, `R a c`; and for all
    /// `a, b` and `p, q: R a b`, `Id (R a b) p q` (`IsMere(R(a, b))`).
    /// Returns `[propositional, reflexive, symmetric, transitive]` statement
    /// entry indices for effectivity.
    fn equivalence(
        &mut self,
        id: MachineId,
        quotient: &super::quotients::QuotientRecord,
        carrier: u32,
        relation: u32,
    ) -> Checked<[u32; 4]> {
        let program = self.program;
        let mut entries = [0u32; 4];
        for (slot, (name, count)) in [
            ("proofs_agree", 2u32),
            ("reflexive", 1),
            ("symmetric", 2),
            ("transitive", 3),
        ]
        .into_iter()
        .enumerate()
        {
            let row = quotient
                .rows
                .iter()
                .find(|(row, _)| row == name)
                .map(|(_, machine)| *machine)
                .ok_or_else(|| {
                    unserved(id, "quotient's equivalence lacks a law of its relation")
                })?;
            let (machine, _) = program
                .symbols
                .iter()
                .find(|(_, known)| **known == row)
                .ok_or_else(|| unserved(id, "quotient's equivalence law was not read"))?;
            let laws = self.laws(machine)?;
            let &[law] = laws.as_slice() else {
                return Err(unserved(
                    machine,
                    "row of a quotient's equivalence that states other than one law",
                ));
            };
            let ty = self.constant(carrier);
            let stated = &self.signature.laws[law as usize];
            if stated.formals.len() != count as usize
                || !stated
                    .formals
                    .iter()
                    .all(|&formal| self.signature.arena.structurally_equal(formal, ty))
            {
                return Err(unserved(
                    machine,
                    "row of a quotient's equivalence over other than values of its carrier",
                ));
            }
            entries[slot] = stated.statement;
            let related = self.constant(relation);
            let holds = |converter: &mut Self, left: u32, right: u32| {
                let left = converter.variable(count - 1 - left);
                let right = converter.variable(count - 1 - right);
                converter
                    .signature
                    .arena
                    .apply_arguments(related, &[left, right])
            };
            let (premises, goal) = match name {
                "proofs_agree" => {
                    let prop = holds(self, 0, 1);
                    let arena = &mut self.signature.arena;
                    let prop_1 = mathematical_kernel::shift(arena, prop, 0, 1);
                    let prop_2 = mathematical_kernel::shift(arena, prop, 0, 2);
                    let p = arena.insert(Term::Variable(1));
                    let q = arena.insert(Term::Variable(0));
                    let eq = arena.insert(Term::Id {
                        ty: prop_2,
                        left: p,
                        right: q,
                    });
                    let inner_pi = arena.insert(Term::Pi {
                        domain: prop_1,
                        codomain: eq,
                    });
                    let is_mere = arena.insert(Term::Pi {
                        domain: prop,
                        codomain: inner_pi,
                    });
                    (Vec::new(), is_mere)
                }
                "reflexive" => (Vec::new(), holds(self, 0, 0)),
                "symmetric" => (vec![holds(self, 0, 1)], holds(self, 1, 0)),
                _ => (
                    vec![holds(self, 0, 1), holds(self, 1, 2)],
                    holds(self, 0, 2),
                ),
            };
            let refusal = format!(
                "the proved equality is not the `{name}` law that the quotient `{}` needs of `{}`",
                self.name(quotient.declaration),
                quotient.relation_name,
            );
            self.signature.laws[law as usize].declared = Some(DeclaredLaw {
                premises,
                goal,
                refusal,
            });
        }
        Ok(entries)
    }

    /// Judges a machine whose body is one `Quotient::lift` or
    /// `Quotient::define`: an operation on a quotient given by an operation
    /// on its representatives.
    ///
    /// The operation has one answer per class only if it takes related
    /// representatives to related results, and it may be applied to a class
    /// only if what this machine requires gives what the operation requires
    /// of every representative. Each selected theorem must have proved
    /// exactly the statement that says so, which is recorded on its law for
    /// stage 06 to compare. Where `Quotient::lift` selects no transport
    /// theorem, and for `Quotient::define`, the second statement is owed
    /// here as a law of this machine, proved with no citation.
    ///
    /// Nothing is defined here. A term that applies the machine reads it as
    /// a definition (`lifted_function`).
    fn lifted(&mut self, id: MachineId, lifted: &LiftedOperation) -> Checked<()> {
        if self
            .begin(
                id,
                false,
                "quotient operation that reaches itself through its theorems",
            )?
            .is_some()
        {
            return Ok(());
        }
        let outer = (
            std::mem::take(&mut self.callable_scope),
            std::mem::take(&mut self.held),
        );
        let read = self.read_lifted(id, lifted);
        (self.callable_scope, self.held) = outer;
        self.finish(id, false, read.map(|()| ((), Read::Lifted)))
    }

    /// An operation on a quotient as a definition, so that a term may
    /// apply it: the kernel's `lift` of the operation on representatives,
    /// at the class the machine takes. The operation is judged first, so
    /// its congruence theorem's statement is declared and sealed before the
    /// definition cites it.
    ///
    /// `lift` gives a function out of one quotient into a set, from a
    /// function on representatives that takes related values to equal
    /// results. So the operation must take exactly one class, require
    /// nothing of its representatives (that needs the dependent
    /// eliminator), and return a class (the quotient is a set) or plain
    /// data (a set by Hedberg's theorem). Its congruence is then
    /// `respect`, through soundness when the result is a class.
    fn lifted_function(&mut self, id: MachineId, lifted: &LiftedOperation) -> Checked<u32> {
        self.lifted(id, lifted)?;
        match self.begin(
            id,
            true,
            "operation lifted to a quotient that applies itself",
        )? {
            None => {}
            Some(Read::Function(entry)) => return Ok(entry),
            Some(_) => return Err(unserved(id, "term that calls a machine with no result")),
        }
        let outer = (
            std::mem::take(&mut self.callable_scope),
            std::mem::take(&mut self.held),
        );
        let read = self.read_lifted_function(id, lifted);
        (self.callable_scope, self.held) = outer;
        self.finish(id, true, read.map(|entry| (entry, Read::Function(entry))))
    }

    fn read_lifted_function(&mut self, id: MachineId, lifted: &LiftedOperation) -> Checked<u32> {
        let program = self.program;
        let LiftShape {
            operation,
            formals,
            classes,
            result,
            result_entry,
            result_class,
            defined,
            ..
        } = self.lift_shape(id, lifted)?;
        let private = program
            .signatures
            .get(operation)
            .ok_or_else(|| unserved(id, "mathematical machine lost its signature"))?;
        if !private.requires.is_empty() {
            return Err(unserved(
                id,
                "term that applies an operation lifted to a quotient whose operation on \
                 representatives requires something of them",
            ));
        }
        // One class formal, passed to every parameter that takes a class.
        let mut taken = None;
        let mut paired = 0;
        for (parameter, class) in classes.iter().enumerate() {
            let Some(class) = *class else {
                continue;
            };
            paired += 1;
            let LiftArgument::Formal(formal) = lifted.arguments[parameter] else {
                return Err(unserved(id, "quotient operation computes a class"));
            };
            match taken {
                None => taken = Some((formal, class)),
                Some((known, _)) if known == formal => {}
                Some(_) => {
                    return Err(unserved(
                        id,
                        "term that applies an operation lifted to a quotient that takes other \
                         than one class",
                    ));
                }
            }
        }
        let Some((taken, (carrier, relation))) = taken else {
            return Err(unserved(
                id,
                "term that applies an operation lifted to a quotient that takes other than \
                 one class",
            ));
        };
        let family = self.family(carrier, relation);
        // The fibers of `lift` must be a set.
        let sets = match result_class {
            Some((carrier, relation)) => {
                let family = self.family(carrier, relation);
                family.law(
                    &mut self.signature.arena,
                    mathematical_kernel::QUOTIENT_SET,
                    Vec::new(),
                    &[],
                )
            }
            None => match &self.signature.entries[result_entry as usize].kind {
                SignatureEntryKind::Data { shape, .. } => {
                    let shape = shape.clone();
                    shape
                        .is_set(&mut self.signature.arena, result_entry)
                        .ok_or_else(|| {
                            unserved(
                                id,
                                "term that applies an operation lifted to a quotient whose result \
                                 is data with a payload",
                            )
                        })?
                }
                _ => {
                    return Err(unserved(
                        id,
                        "term that applies an operation lifted to a quotient whose result is \
                         neither a class nor data",
                    ));
                }
            },
        };
        // The congruence theorem: for two representatives of the class and
        // one value of each other argument, from related representatives
        // alone, related (or equal) results. Stage 06 checks that it states
        // exactly that (`seal`), and the kernel checks this definition's
        // use of it.
        let congruence = self.selected(id, lifted.congruence)?;
        let &[law] = self.laws(congruence)?.as_slice() else {
            return Err(unserved(
                congruence,
                "congruence theorem of a quotient operation that states other than one law",
            ));
        };
        let stated = &self.signature.laws[law as usize];
        if stated.premises.len() != paired {
            return Err(unserved(
                congruence,
                "congruence theorem of a quotient operation that requires more than related \
                 representatives",
            ));
        }
        let statement = stated.statement;

        // Under the machine's formals and `under` binders more: each
        // formal's value, and the representative `representative` of the
        // class formal.
        let count = formals.len() as u32;
        let at =
            |converter: &mut Self, under: u32, representative: u32| -> Checked<Vec<TermHandle>> {
                let values: Vec<_> = (0..count)
                    .map(|formal| converter.variable(count - 1 - formal + under))
                    .collect();
                let mut representatives = vec![None; formals.len()];
                representatives[taken as usize] = Some(converter.variable(representative));
                converter.lift_actuals(id, lifted, &classes, &values, &representatives)
            };
        let apply = |converter: &mut Self, arguments: &[TermHandle]| {
            let function = converter.constant(defined);
            converter
                .signature
                .arena
                .apply_arguments(function, arguments)
        };
        let carrier_type = self.constant(carrier);
        // `f = λ a. F(..)`, or the class of that when the result is one.
        let at_a = at(self, 1, 0)?;
        let mut value = apply(self, &at_a);
        if let Some((carrier, relation)) = result_class {
            let result_family = self.family(carrier, relation);
            value = result_family.project(&mut self.signature.arena, value);
        }
        let f = self.signature.arena.insert(Term::Lambda {
            domain: carrier_type,
            body: value,
        });
        // `respect = λ a b (r: R a b). ..`: the congruence at `a` and `b`
        // for each class, the other arguments as passed, and `r` for each
        // class.
        let (at_left, at_right) = (at(self, 3, 2)?, at(self, 3, 1)?);
        let mut cited = Vec::with_capacity(classes.len() + paired + paired);
        for (parameter, class) in classes.iter().enumerate() {
            cited.push(at_left[parameter]);
            if class.is_some() {
                cited.push(at_right[parameter]);
            }
        }
        for _ in 0..paired {
            cited.push(self.variable(0));
        }
        let theorem = self.constant(statement);
        let mut respected = self.signature.arena.apply_arguments(theorem, &cited);
        if let Some((carrier, relation)) = result_class {
            let (left, right) = (apply(self, &at_left), apply(self, &at_right));
            let result_family = self.family(carrier, relation);
            respected = result_family.sound(&mut self.signature.arena, left, right, respected);
        }
        let related = {
            let relation = self.constant(relation);
            let (a, b) = (self.variable(1), self.variable(0));
            self.signature.arena.apply_arguments(relation, &[a, b])
        };
        let mut respect = respected;
        for domain in [related, carrier_type, carrier_type] {
            respect = self.signature.arena.insert(Term::Lambda {
                domain,
                body: respect,
            });
        }
        let lift = family.law(
            &mut self.signature.arena,
            mathematical_kernel::QUOTIENT_LIFT,
            vec![Level::Constant(0)],
            &[result, sets, f, respect],
        );
        let class = self.variable(count - 1 - taken);
        let mut term = self.signature.arena.apply_arguments(lift, &[class]);
        let mut ty = result;
        for &(domain, _) in formals.iter().rev() {
            term = self
                .signature
                .arena
                .insert(Term::Lambda { domain, body: term });
            ty = self.signature.arena.insert(Term::Pi {
                domain,
                codomain: ty,
            });
        }
        let symbol = *program
            .symbols
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its declaration"))?;
        index(id, self.signature.declarations.len() + 1)?;
        let name = self.name(symbol);
        Ok(self.push(
            Declaration::definition(0, ty, term),
            SignatureEntryKind::Function {
                machine: symbol,
                cases: None,
            },
            name,
        ))
    }

    fn lifted_operation(&self, id: MachineId) -> Option<&'p LiftedOperation> {
        let program = self.program;
        program
            .lifted_operations
            .iter()
            .find(|(machine, _)| *machine == id)
            .map(|(_, lifted)| lifted)
    }

    /// The machine a quotient operation selects, by its symbol.
    fn selected(&self, id: MachineId, symbol: SymbolHandle) -> Checked<MachineId> {
        self.program
            .symbols
            .iter()
            .find(|(_, known)| **known == symbol)
            .map(|(machine, _)| machine)
            .ok_or_else(|| unserved(id, "quotient operation selects a machine that was not read"))
    }

    /// How an operation on a quotient passes its formals to the operation
    /// on representatives, and which of the types on either side are
    /// classes of which.
    fn lift_shape(&mut self, id: MachineId, lifted: &LiftedOperation) -> Checked<LiftShape> {
        let program = self.program;
        let operation = self.selected(id, lifted.representative)?;
        let lost = || unserved(id, "mathematical machine lost its signature");
        let public = program.signatures.get(id).ok_or_else(lost)?;
        let private = program.signatures.get(operation).ok_or_else(lost)?;
        if !public.ensures.is_empty() || public.has_structural_guarantees() {
            return Err(unserved(
                id,
                "quotient operation that states a guarantee of its own",
            ));
        }
        let formals = self.formals(id)?;
        let parameters = self.formals(operation)?;
        if lifted.arguments.len() != parameters.len() {
            return Err(unserved(
                id,
                "quotient operation with the wrong number of arguments",
            ));
        }
        // When each formal goes to exactly one parameter as it is, every
        // clause of either machine can be stated for the other: a transport
        // theorem and `Quotient::define` need that.
        let mut receives = vec![None; formals.len()];
        let mut bijective = true;
        for (parameter, argument) in lifted.arguments.iter().enumerate() {
            match argument {
                &LiftArgument::Formal(formal) => {
                    let received = receives.get_mut(formal as usize).ok_or_else(|| {
                        unserved(id, "quotient operation lost one of its formals")
                    })?;
                    bijective &= received.replace(parameter).is_none();
                }
                LiftArgument::Computed(_) => bijective = false,
            }
        }
        let receives = receives
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .filter(|_| bijective);
        if lifted.define
            && (receives.is_none()
                || lifted.arguments.iter().enumerate().any(|(parameter, argument)| {
                    !matches!(argument, &LiftArgument::Formal(formal) if formal == parameter as u32)
                }))
        {
            return Err(unserved(
                id,
                "`Quotient::define` that does not pass each formal in its place",
            ));
        }
        // What a type of this machine is to the type the operation has in
        // its place: the same type, or a quotient of it by this relation.
        let class = |converter: &Self, outer: u32, inner: u32| -> Checked<Option<(u32, u32)>> {
            if outer == inner {
                return Ok(None);
            }
            match converter.signature.entries[outer as usize].kind {
                SignatureEntryKind::Quotient {
                    carrier, relation, ..
                } if carrier == inner => Ok(Some((carrier, relation))),
                _ => Err(unserved(
                    id,
                    "quotient operation whose representative takes or returns another type",
                )),
            }
        };
        let mut classes = Vec::with_capacity(parameters.len());
        for (parameter, argument) in lifted.arguments.iter().enumerate() {
            let outer = match argument {
                &LiftArgument::Formal(formal) => {
                    formals
                        .get(formal as usize)
                        .ok_or_else(|| unserved(id, "quotient operation lost one of its formals"))?
                        .1
                }
                LiftArgument::Computed(value) => {
                    let value_type = value.result.ok_or_else(|| {
                        unserved(
                            id,
                            "computed argument of a quotient operation with no value",
                        )
                    })?;
                    let (_, entry) = self.ty(id, value_type)?;
                    if entry != parameters[parameter].1 {
                        return Err(unserved(
                            id,
                            "computed argument of a quotient operation that is no value of its \
                             parameter's type: a class is passed only as a formal",
                        ));
                    }
                    entry
                }
            };
            classes.push(class(self, outer, parameters[parameter].1)?);
        }
        let (result, result_entry) = self.ty(
            id,
            public
                .result
                .ok_or_else(|| unserved(id, "quotient operation without a result"))?,
        )?;
        let (_, value_entry) = self.ty(
            operation,
            private
                .result
                .ok_or_else(|| unserved(id, "quotient operation over a machine with no result"))?,
        )?;
        let result_class = class(self, result_entry, value_entry)?;
        let defined = self.function(operation)?;
        Ok(LiftShape {
            operation,
            formals,
            parameters,
            receives,
            classes,
            result,
            result_entry,
            result_class,
            defined,
        })
    }

    fn read_lifted(&mut self, id: MachineId, lifted: &LiftedOperation) -> Checked<()> {
        let program = self.program;
        let lost = || unserved(id, "mathematical machine lost its signature");
        let LiftShape {
            operation,
            formals,
            parameters,
            receives,
            classes,
            result,
            result_class,
            defined,
            ..
        } = self.lift_shape(id, lifted)?;
        let public = program.signatures.get(id).ok_or_else(lost)?;
        let private = program.signatures.get(operation).ok_or_else(lost)?;
        let name = self.name(lifted.representative);

        // The theorems are stated for two representatives of every class
        // and one value of every other argument, in the operation's order.
        let paired = classes.iter().any(Option::is_some);
        let count: u32 = classes
            .iter()
            .map(|class| if class.is_some() { 2 } else { 1 })
            .sum();
        let mut binders = Vec::with_capacity(count as usize);
        let (mut left, mut right) = (Vec::new(), Vec::new());
        for (class, &(ty, _)) in classes.iter().zip(&parameters) {
            let first = self.variable(count - 1 - binders.len() as u32);
            binders.push(ty);
            left.push(first);
            if class.is_some() {
                let second = self.variable(count - 1 - binders.len() as u32);
                binders.push(ty);
                right.push(second);
            } else {
                right.push(first);
            }
        }
        let sides: &[&[TermHandle]] = if paired { &[&left, &right] } else { &[&left] };
        // What the operation requires of each side's representatives.
        self.held.clear();
        let mut legal = Vec::new();
        for side in sides {
            for clause in &private.requires {
                let premise = self.equality(operation, clause, side, None)?;
                legal.push(premise);
                self.held.push(premise);
            }
        }
        // Congruence: related arguments, both calls legal, related results.
        let mut premises = Vec::new();
        for (parameter, class) in classes.iter().enumerate() {
            if let &Some((_, relation)) = class {
                let related = self.constant(relation);
                premises.push(
                    self.signature
                        .arena
                        .apply_arguments(related, &[left[parameter], right[parameter]]),
                );
            }
        }
        premises.extend(&legal);
        let function = self.constant(defined);
        let values: Vec<_> = [&left, &right]
            .into_iter()
            .map(|side| self.signature.arena.apply_arguments(function, side))
            .collect();
        let goal = match result_class {
            Some((_, relation)) => {
                let related = self.constant(relation);
                self.signature.arena.apply_arguments(related, &values)
            }
            None => self.signature.arena.insert(Term::Id {
                ty: result,
                left: values[0],
                right: values[1],
            }),
        };
        let own = self.name(*program.symbols.get(id).ok_or_else(lost)?);
        let congruence = self.selected(id, lifted.congruence)?;
        let &[law] = self.theorem(id, congruence)?.as_slice() else {
            return Err(unserved(
                congruence,
                "congruence theorem of a quotient operation that states other than one law",
            ));
        };
        self.seal(
            congruence,
            law,
            &binders,
            &premises,
            goal,
            format!(
                "the proved equality is not the congruence `{own}` needs: from related \
                 arguments and what `{name}` requires of each, related results of `{name}`"
            ),
        )?;

        // What this machine requires, stated of `values`, one term per
        // formal (a class for a class formal).
        let required = |converter: &mut Self, values: &[TermHandle]| -> Checked<Vec<TermHandle>> {
            let mut clauses = Vec::with_capacity(public.requires.len());
            for clause in &public.requires {
                let premise = converter.equality(id, clause, values, None)?;
                clauses.push(premise);
                converter.held.push(premise);
            }
            Ok(clauses)
        };
        if let Some(transport) = lifted.transport {
            // Transport: what this machine requires of both sides gives
            // each thing the operation requires, in the same order. It is
            // stated over the congruence's binders, so each formal must
            // reach exactly one of them.
            let Some(receives) = &receives else {
                return Err(unserved(
                    id,
                    "transport theorem of a quotient operation that computes, repeats or leaves \
                     out an argument: such a lift owes its transport itself",
                ));
            };
            self.held.clear();
            let mut premises = Vec::new();
            for side in sides {
                let mut values = Vec::with_capacity(receives.len());
                for &parameter in receives {
                    values.push(match classes[parameter] {
                        Some((carrier, relation)) => {
                            let family = self.family(carrier, relation);
                            family.project(&mut self.signature.arena, side[parameter])
                        }
                        None => side[parameter],
                    });
                }
                premises.extend(required(self, &values)?);
            }
            let transport = self.selected(id, transport)?;
            let laws = self.theorem(id, transport)?;
            if laws.len() != legal.len() {
                return Err(unserved(
                    transport,
                    "transport theorem of a quotient operation that does not state each thing its representative requires, for each side",
                ));
            }
            for (law, &goal) in laws.into_iter().zip(&legal) {
                self.seal(
                    transport,
                    law,
                    &binders,
                    &premises,
                    goal,
                    format!(
                        "the proved equality is not the transport `{own}` needs: from what \
                         `{own}` requires of each class, what `{name}` requires of its \
                         representative"
                    ),
                )?;
            }
            return Ok(());
        }
        // No transport theorem: the same, for one representative of each
        // class formal and every other formal as it is, owed here.
        // `Quotient::define` owes the converse too.
        let count = formals.len() as u32;
        let mut context = Vec::with_capacity(formals.len());
        let mut values = Vec::with_capacity(formals.len());
        let mut representatives = Vec::with_capacity(formals.len());
        for (formal, &(ty, entry)) in formals.iter().enumerate() {
            let variable = self.variable(count - 1 - formal as u32);
            match self.signature.entries[entry as usize].kind {
                SignatureEntryKind::Quotient {
                    carrier, relation, ..
                } => {
                    context.push(self.constant(carrier));
                    let family = self.family(carrier, relation);
                    values.push(family.project(&mut self.signature.arena, variable));
                    representatives.push(Some(variable));
                }
                _ => {
                    context.push(ty);
                    values.push(variable);
                    representatives.push(None);
                }
            }
        }
        self.held.clear();
        let asked = required(self, &values)?;
        let actuals = self.lift_actuals(id, lifted, &classes, &values, &representatives)?;
        let mut needed = Vec::with_capacity(private.requires.len());
        for clause in &private.requires {
            let premise = self.equality(operation, clause, &actuals, None)?;
            needed.push(premise);
            self.held.push(premise);
        }
        for &goal in &needed {
            self.owe(
                id,
                &context,
                &asked,
                goal,
                format!("what `{name}` requires of a representative"),
            )?;
        }
        if lifted.define {
            for &goal in &asked {
                self.owe(
                    id,
                    &context,
                    &needed,
                    goal,
                    format!("what `{own}` requires, from what `{name}` requires"),
                )?;
            }
        }
        Ok(())
    }

    /// The term each parameter of the operation on representatives
    /// receives: a formal's value as it is, or a class formal's
    /// representative where the parameter takes one, or a computed
    /// argument read over the formals' `values`.
    fn lift_actuals(
        &mut self,
        id: MachineId,
        lifted: &LiftedOperation,
        classes: &[Option<(u32, u32)>],
        values: &[TermHandle],
        representatives: &[Option<TermHandle>],
    ) -> Checked<Vec<TermHandle>> {
        let slots: Vec<_> = values
            .iter()
            .map(|&term| {
                Some(Bound {
                    term,
                    field: None,
                    applied: None,
                })
            })
            .collect();
        let aliases: Vec<usize> = (0..values.len()).collect();
        let scope = Scope {
            slots: &slots,
            aliases: &aliases,
            case: None,
            recursion: None,
            depth: 0,
            induction: false,
            value: None,
        };
        let mut actuals = Vec::with_capacity(lifted.arguments.len());
        for (parameter, argument) in lifted.arguments.iter().enumerate() {
            actuals.push(match argument {
                &LiftArgument::Formal(formal) => {
                    let formal = formal as usize;
                    match classes[parameter] {
                        Some(_) => {
                            representatives
                                .get(formal)
                                .copied()
                                .flatten()
                                .ok_or_else(|| {
                                    unserved(id, "quotient operation lost a class's representative")
                                })?
                        }
                        None => *values.get(formal).ok_or_else(|| {
                            unserved(id, "quotient operation lost one of its formals")
                        })?,
                    }
                }
                LiftArgument::Computed(value) => self.expression(id, value, &scope)?.term,
            });
        }
        Ok(actuals)
    }

    /// The laws of a theorem a quotient operation selects: a machine with
    /// no result.
    fn theorem(&mut self, id: MachineId, theorem: MachineId) -> Checked<Vec<u32>> {
        if self
            .program
            .signatures
            .get(theorem)
            .is_none_or(|signature| signature.result.is_some())
        {
            return Err(unserved(
                id,
                "quotient operation that selects a theorem with a result",
            ));
        }
        self.laws(theorem)
    }

    /// Records the statement law `law` of `machine` must have proved: for
    /// exactly `binders`, from `premises`, `goal`.
    fn seal(
        &mut self,
        machine: MachineId,
        law: u32,
        binders: &[TermHandle],
        premises: &[TermHandle],
        goal: TermHandle,
        refusal: String,
    ) -> Checked<()> {
        let stated = &self.signature.laws[law as usize];
        if stated.formals.len() != binders.len()
            || !stated
                .formals
                .iter()
                .zip(binders)
                .all(|(&formal, &binder)| self.signature.arena.structurally_equal(formal, binder))
        {
            return Err(unserved(
                machine,
                "theorem of a quotient operation over other than two representatives of each class and one value of each other argument",
            ));
        }
        let arena = &self.signature.arena;
        if stated.declared.as_ref().is_some_and(|known| {
            known.premises.len() != premises.len()
                || !arena.structurally_equal(known.goal, goal)
                || !known
                    .premises
                    .iter()
                    .zip(premises)
                    .all(|(&known, &premise)| arena.structurally_equal(known, premise))
        }) {
            return Err(unserved(
                machine,
                "theorem of a quotient operation that already owes another statement",
            ));
        }
        self.signature.laws[law as usize].declared = Some(DeclaredLaw {
            premises: premises.to_vec(),
            goal,
            refusal,
        });
        Ok(())
    }

    /// A law machine `id` owes without having written it: for all
    /// `context`, from `premises`, `goal`, proved with no citation.
    fn owe(
        &mut self,
        id: MachineId,
        context: &[TermHandle],
        premises: &[TermHandle],
        goal: TermHandle,
        text: String,
    ) -> Checked<()> {
        let program = self.program;
        let owner = *program
            .symbols
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its declaration"))?;
        let mut statement = self.implication(premises, goal);
        for &variable in context.iter().rev() {
            statement = self.signature.arena.insert(Term::Pi {
                domain: variable,
                codomain: statement,
            });
        }
        let law = index(id, self.signature.laws.len())?;
        index(id, self.signature.declarations.len() + 1)?;
        let owner_name = self.name(owner);
        let entry = self.push(
            Declaration::assumption(0, statement),
            SignatureEntryKind::Statement { law },
            owner_name.clone(),
        );
        self.signature.laws.push(Law {
            machine: owner,
            clause: 0,
            statement: entry,
            formals: context.to_vec(),
            premises: premises.to_vec(),
            goal,
            declared: None,
            proof: LawProof::Direct(LawCase::default()),
            formal_names: Vec::new(),
            machine_name: owner_name,
            text,
            source: program.names.get(id).and_then(|names| names.declared),
        });
        Ok(())
    }

    /// `premises` then `goal`, each premise stated under the ones before
    /// it. All are given under one context.
    fn implication(&mut self, premises: &[TermHandle], goal: TermHandle) -> TermHandle {
        let arena = &mut self.signature.arena;
        let mut statement = shift(arena, goal, 0, premises.len() as u32);
        for (position, &premise) in premises.iter().enumerate().rev() {
            let domain = shift(arena, premise, 0, position as u32);
            statement = arena.insert(Term::Pi {
                domain,
                codomain: statement,
            });
        }
        statement
    }

    /// One entry per declaration the kernel's builders added since the last
    /// entry: they push declarations themselves.
    fn sync(&mut self, name: &str) {
        while self.signature.entries.len() < self.signature.declarations.len() {
            self.signature.entries.push(SignatureEntry {
                kind: SignatureEntryKind::Scheme,
                name: name.to_owned(),
            });
        }
    }

    /// Declares the integer vocabulary, once: for the indices of indexed
    /// data, or for a fixed-width payload or formal, which is the integer it
    /// holds.
    fn declare_integers(&mut self, id: MachineId) -> Checked<u32> {
        if let Some(theory) = &self.integers {
            return Ok(theory.positions().vocabulary.carrier);
        }
        let theory = proof_admission::SourceIntegerTheory::new(
            &mut self.signature.arena,
            &mut self.signature.declarations,
        )
        .map_err(|_| unserved(id, "integer vocabulary the kernel cannot declare"))?;
        let positions = theory.positions();
        self.signature.integers = Some(IntegerPositions {
            carrier: positions.vocabulary.carrier,
            less_than: positions.vocabulary.less_than,
            less_or_equal: positions.vocabulary.less_or_equal,
            add: positions.vocabulary.add,
            subtract: positions.vocabulary.subtract,
            multiply: positions.vocabulary.multiply,
            numeral_zero: positions.numeral_zero,
            numeral_double: positions.numeral_double,
            numeral_odd: positions.numeral_odd,
            numeral_negate: positions.numeral_negate,
        });
        let carrier = positions.vocabulary.carrier;
        self.integers = Some(theory);
        self.sync("integer");
        Ok(carrier)
    }

    /// Runs `work` with the integer vocabulary and records what it declared.
    fn with_integers<T>(
        &mut self,
        id: MachineId,
        work: impl FnOnce(
            &mut proof_admission::SourceIntegerTheory,
            &mut mathematical_kernel::TermArena,
            &mut Vec<Declaration>,
        ) -> Result<T, proof_admission::BoundedDenotationError>,
    ) -> Checked<T> {
        let mut theory = self
            .integers
            .take()
            .ok_or_else(|| unserved(id, "integer term in a program without indexed data"))?;
        let done = work(
            &mut theory,
            &mut self.signature.arena,
            &mut self.signature.declarations,
        );
        self.integers = Some(theory);
        self.sync("integer");
        done.map_err(|_| unserved(id, "integer term the integer vocabulary cannot state"))
    }

    fn integer_literal(&mut self, id: MachineId, value: bignum::BigInt) -> Checked<TermHandle> {
        self.with_integers(id, |theory, arena, declarations| {
            theory.literal(arena, declarations, value)
        })
    }

    fn integer_vocabulary(
        &self,
        id: MachineId,
    ) -> Checked<proof_admission::SourceIntegerVocabulary> {
        self.integers
            .as_ref()
            .map(|theory| theory.vocabulary())
            .ok_or_else(|| unserved(id, "integer term in a program without indexed data"))
    }

    /// A scalar of a clause over indexed data as an integer: an index, a
    /// payload value of the case in scope, or a literal. A fixed-width
    /// value is the integer it holds, so only terms with no arithmetic of
    /// their own are read; arithmetic is written on `embed`.
    fn integer_scalar(
        &mut self,
        id: MachineId,
        term: &ScalarTerm,
        scope: &Integers<'_>,
    ) -> Checked<TermHandle> {
        match term {
            ScalarTerm::Value { id: value, .. } => scope
                .indices
                .get(value.get() as usize - 1)
                .copied()
                .ok_or_else(|| unserved(id, "clause over indexed data names another value")),
            ScalarTerm::IntegerField { root, path, .. }
                if Some(*root) == crate::body::self_place() =>
            {
                match (path.as_slice(), scope.payload) {
                    (
                        [
                            CanonicalStructuralPathSegment::Case(case),
                            CanonicalStructuralPathSegment::Field(field),
                        ],
                        Some((own, payload)),
                    ) if *case == own => payload
                        .iter()
                        .find(|(known, _)| known == field)
                        .map(|(_, term)| *term)
                        .ok_or_else(|| {
                            unserved(
                                id,
                                "clause over indexed data names a field that is no payload value",
                            )
                        }),
                    _ => Err(unserved(
                        id,
                        "clause over indexed data names a payload value of another case",
                    )),
                }
            }
            ScalarTerm::Integer { value, .. } => self.integer_literal(
                id,
                match *value {
                    IntegerValue::Signed(value) => bignum::BigInt::from_i128(value),
                    IntegerValue::Unsigned(value) => bignum::BigInt::from_u128(value),
                },
            ),
            _ => Err(unserved(
                id,
                "clause over indexed data with a term other than an index, a payload value or a literal",
            )),
        }
    }

    fn integer_term(
        &mut self,
        id: MachineId,
        term: &IntegerMathTerm,
        scope: &Integers<'_>,
    ) -> Checked<TermHandle> {
        let vocabulary = self.integer_vocabulary(id)?;
        let binary = |converter: &mut Self, operation: u32, left, right| {
            let left = converter.integer_term(id, left, scope)?;
            let right = converter.integer_term(id, right, scope)?;
            let operation = converter.constant(operation);
            Ok(converter
                .signature
                .arena
                .apply_arguments(operation, &[left, right]))
        };
        match term {
            IntegerMathTerm::IntegerLiteral(literal) => {
                let magnitude = bignum::BigInt::from_u128(literal.magnitude());
                let value = if literal.negative() {
                    magnitude.negate()
                } else {
                    magnitude
                };
                self.integer_literal(id, value)
            }
            IntegerMathTerm::MathValue { value, source_type } => self.integer_scalar(
                id,
                &ScalarTerm::Value {
                    id: *value,
                    scalar_type: semantic_vocabulary::ScalarType::Integer(*source_type),
                },
                scope,
            ),
            IntegerMathTerm::Embed(scalar) => self.integer_scalar(id, scalar, scope),
            IntegerMathTerm::Add(left, right) => binary(self, vocabulary.add, left, right),
            IntegerMathTerm::Subtract(left, right) => {
                binary(self, vocabulary.subtract, left, right)
            }
            IntegerMathTerm::Multiply(left, right) => {
                binary(self, vocabulary.multiply, left, right)
            }
            IntegerMathTerm::ShiftLeft { .. } => {
                Err(unserved(id, "clause over indexed data with a shift"))
            }
        }
    }

    /// A clause over indexed data as kernel propositions over the integer
    /// vocabulary, one per conjunct.
    fn integer_proposition(
        &mut self,
        id: MachineId,
        proposition: &Proposition,
        scope: &Integers<'_>,
        components: &mut Vec<TermHandle>,
    ) -> Checked<()> {
        let vocabulary = self.integer_vocabulary(id)?;
        let related = |converter: &mut Self, relation: u32, left, right| {
            let relation = converter.constant(relation);
            converter
                .signature
                .arena
                .apply_arguments(relation, &[left, right])
        };
        let equal = |converter: &mut Self, left, right| {
            let ty = converter.constant(vocabulary.carrier);
            converter
                .signature
                .arena
                .insert(Term::Id { ty, left, right })
        };
        let term = match proposition {
            Proposition::Conjunction(parts) => {
                for part in parts {
                    self.integer_proposition(id, part, scope, components)?;
                }
                return Ok(());
            }
            Proposition::Equal(left, right) => {
                let left = self.integer_scalar(id, left, scope)?;
                let right = self.integer_scalar(id, right, scope)?;
                equal(self, left, right)
            }
            Proposition::LessThan(left, right) => {
                let left = self.integer_scalar(id, left, scope)?;
                let right = self.integer_scalar(id, right, scope)?;
                related(self, vocabulary.less_than, left, right)
            }
            Proposition::LessOrEqual(left, right) => {
                let left = self.integer_scalar(id, left, scope)?;
                let right = self.integer_scalar(id, right, scope)?;
                related(self, vocabulary.less_or_equal, left, right)
            }
            Proposition::IntegerMathEqual(left, right) => {
                let left = self.integer_term(id, left, scope)?;
                let right = self.integer_term(id, right, scope)?;
                equal(self, left, right)
            }
            Proposition::IntegerMathLessThan(left, right) => {
                let left = self.integer_term(id, left, scope)?;
                let right = self.integer_term(id, right, scope)?;
                related(self, vocabulary.less_than, left, right)
            }
            Proposition::IntegerMathLessOrEqual(left, right) => {
                let left = self.integer_term(id, left, scope)?;
                let right = self.integer_term(id, right, scope)?;
                related(self, vocabulary.less_or_equal, left, right)
            }
            _ => {
                return Err(unserved(
                    id,
                    "clause over indexed data other than equalities and orders of integers",
                ));
            }
        };
        components.push(term);
        Ok(())
    }

    /// The propositions as one type: each paired with the rest.
    fn conjunction(&mut self, components: &[TermHandle]) -> Option<TermHandle> {
        self.signature.arena.product_type(components)
    }

    /// Declares data indexed by values as the kernel's indexed family: its
    /// indices and payload values are integers, each case's label carries a
    /// proof of what the declaration requires of that case, and each child
    /// sits at the index its field's type names.
    fn indexed_family(&mut self, id: MachineId, family: &FamilyRecord) -> Checked<()> {
        let vocabulary = self.integer_vocabulary(id)?;
        let integer = self.constant(vocabulary.carrier);
        let indices = family.indices.len() as u32;
        // What the declaration requires of each case: one disjunct per
        // case, naming it.
        let required: Vec<&[Proposition]> = match family.requires.as_slice() {
            [Proposition::Disjunction(cases)] => cases
                .iter()
                .map(|case| match case {
                    Proposition::Conjunction(parts) => Ok(parts.as_slice()),
                    _ => Err(unserved(
                        id,
                        "indexed data whose `requires` is not one clause per case",
                    )),
                })
                .collect::<Checked<_>>()?,
            _ => {
                return Err(unserved(
                    id,
                    "indexed data whose `requires` is not one clause per case",
                ));
            }
        };
        let mut cases = Vec::with_capacity(family.cases.len());
        let mut described = Vec::with_capacity(family.cases.len());
        for case in &family.cases {
            let payload = case.payload.len() as u32;
            let total = indices + payload;
            let index_terms: Vec<_> = (0..indices)
                .map(|index| self.variable(total - 1 - index))
                .collect();
            let payload_terms: Vec<_> = case
                .payload
                .iter()
                .enumerate()
                .map(|(position, value)| {
                    let field = value
                        .field
                        .ok_or_else(|| unserved(id, "payload value lost its field"))?;
                    Ok((field, self.variable(payload - 1 - position as u32)))
                })
                .collect::<Checked<_>>()?;
            let scope = Integers {
                indices: &index_terms,
                payload: Some((case.case, &payload_terms)),
            };
            let mut clauses = required.iter().filter(|parts| {
                parts.iter().any(|part| {
                    matches!(part, Proposition::StructuralCaseMembership { case: named, .. }
                        if *named == case.case)
                })
            });
            let (Some(parts), None) = (clauses.next(), clauses.next()) else {
                return Err(unserved(
                    id,
                    "indexed data whose `requires` is not one clause per case",
                ));
            };
            let mut constraints = Vec::new();
            for part in parts.iter() {
                if matches!(part, Proposition::StructuralCaseMembership { .. }) {
                    continue;
                }
                self.integer_proposition(id, part, &scope, &mut constraints)?;
            }
            let all = self.conjunction(&constraints).ok_or_else(|| {
                unserved(
                    id,
                    "case of indexed data that requires nothing of its payload",
                )
            })?;
            let mut children = Vec::with_capacity(case.children.len());
            let mut child_indices = Vec::with_capacity(case.children.len());
            for child in &case.children {
                let mut at = Vec::with_capacity(child.at.len());
                for term in &child.at {
                    at.push(self.family_term_kernel(id, term, &index_terms, &payload_terms)?);
                }
                let tuple = self.index_tuple(&at);
                child_indices.push(self.over(integer, total, tuple));
                children.push(at);
            }
            described.push(IndexedCase {
                payload: (0..payload)
                    .map(|position| IndexedField {
                        name: indices + position,
                        domain: integer,
                    })
                    .collect(),
                constraints: self.over(integer, total, all),
                child_indices,
            });
            cases.push(FamilyCaseTerms {
                payload,
                constraints,
                children,
            });
        }
        let declared = IndexedData {
            parameters: Vec::new(),
            fields: (0..indices)
                .map(|index| IndexedField {
                    name: index,
                    domain: integer,
                })
                .collect(),
            indices: (0..indices).collect(),
            cases: described,
            level_arity: 0,
        }
        .declare(&mut self.signature.arena, &mut self.signature.declarations)
        .map_err(|_| unserved(id, "indexed data the kernel's family builder refuses"))?;
        let name = self.name(family.declaration);
        self.sync(&name);
        let position = index(id, self.signature.families.len())?;
        self.signature.families.push(declared);
        self.families.push(FamilyDeclared {
            record: family.record,
            family: position,
            cases,
        });
        Ok(())
    }

    /// What each case literal a running machine writes owes: the case's
    /// constraints at the literal's index and payload, one law per
    /// constraint, from what the machine requires of its value parameters.
    fn evidence_laws(&mut self, id: MachineId, used: &EvidenceUse) -> Checked<()> {
        let vocabulary = self.integer_vocabulary(id)?;
        let integer = self.constant(vocabulary.carrier);
        let count = used.parameters.len() as u32;
        let open: Vec<_> = (0..count)
            .map(|parameter| self.variable(count - 1 - parameter))
            .collect();
        let mut premises = Vec::new();
        for proposition in &used.requires {
            // A requirement the integer vocabulary cannot state is no
            // hypothesis; nothing is assumed in its place.
            let mut components = Vec::new();
            let scope = Integers {
                indices: &open,
                payload: None,
            };
            if self
                .integer_proposition(id, proposition, &scope, &mut components)
                .is_ok()
            {
                premises.extend(components);
            }
        }
        for construction in &used.constructions {
            let declared = self
                .families
                .iter()
                .find(|declared| declared.record == construction.record)
                .ok_or_else(|| unserved(id, "evidence of indexed data that is not declared"))?
                .clone();
            let program = self.program;
            let source = program
                .families
                .iter()
                .find(|family| family.record == construction.record)
                .ok_or_else(|| unserved(id, "evidence of indexed data that is not declared"))?;
            let terms = declared
                .cases
                .get(construction.case)
                .ok_or_else(|| unserved(id, "literal of indexed data that names no case"))?;
            if construction.at.len() != source.indices.len()
                || construction.payload.len() != terms.payload as usize
            {
                return Err(unserved(
                    id,
                    "literal of indexed data without each value once",
                ));
            }
            let mut actuals =
                Vec::with_capacity(construction.at.len() + construction.payload.len());
            for term in construction.at.iter().chain(&construction.payload) {
                actuals.push(self.evidence_term_kernel(id, term, &open)?);
            }
            let case = format!(
                "{}::{}",
                self.name(source.declaration),
                source.cases[construction.case].name
            );
            for &constraint in &terms.constraints {
                let goal = instantiate(&mut self.signature.arena, constraint, &actuals);
                let mut statement = self.implication(&premises, goal);
                for _ in 0..count {
                    statement = self.signature.arena.insert(Term::Pi {
                        domain: integer,
                        codomain: statement,
                    });
                }
                let law = index(id, self.signature.laws.len())?;
                index(id, self.signature.declarations.len() + 1)?;
                let entry = self.push(
                    Declaration::assumption(0, statement),
                    SignatureEntryKind::Statement { law },
                    used.name.clone(),
                );
                self.signature.laws.push(Law {
                    machine: used.machine,
                    clause: 0,
                    statement: entry,
                    formals: vec![integer; count as usize],
                    premises: premises.clone(),
                    goal,
                    declared: None,
                    proof: LawProof::Integers,
                    formal_names: used
                        .parameters
                        .iter()
                        .map(|parameter| parameter.name.clone())
                        .collect(),
                    machine_name: used.name.clone(),
                    text: format!("what `{case}` requires of the value built here"),
                    source: construction.source,
                });
            }
        }
        for equality in &used.index_equalities {
            let left = self.evidence_term_kernel(id, &equality.left, &open)?;
            let right = self.evidence_term_kernel(id, &equality.right, &open)?;
            let goal = self.signature.arena.insert(Term::Id {
                ty: integer,
                left,
                right,
            });
            let mut statement = self.implication(&premises, goal);
            for _ in 0..count {
                statement = self.signature.arena.insert(Term::Pi {
                    domain: integer,
                    codomain: statement,
                });
            }
            let law = index(id, self.signature.laws.len())?;
            index(id, self.signature.declarations.len() + 1)?;
            let entry = self.push(
                Declaration::assumption(0, statement),
                SignatureEntryKind::Statement { law },
                used.name.clone(),
            );
            self.signature.laws.push(Law {
                machine: used.machine,
                clause: 0,
                statement: entry,
                formals: vec![integer; count as usize],
                premises: premises.clone(),
                goal,
                declared: None,
                proof: LawProof::Integers,
                formal_names: used
                    .parameters
                    .iter()
                    .map(|parameter| parameter.name.clone())
                    .collect(),
                machine_name: used.name.clone(),
                text: "indices of indexed data must be equal".to_owned(),
                source: equality.source,
            });
        }
        Ok(())
    }

    fn family_term_kernel(
        &mut self,
        id: MachineId,
        term: &FamilyTerm,
        index_terms: &[TermHandle],
        payload_terms: &[(semantic_vocabulary::StructuralFieldId, TermHandle)],
    ) -> Checked<TermHandle> {
        use super::families::IndexOp;
        match term {
            FamilyTerm::Index(index) => Ok(index_terms[*index as usize]),
            FamilyTerm::Payload(position) => Ok(payload_terms[*position as usize].1),
            FamilyTerm::Literal(value) => {
                self.integer_literal(id, bignum::BigInt::from_i128(*value))
            }
            FamilyTerm::Binary(op, left, right) => {
                let left = self.family_term_kernel(id, left, index_terms, payload_terms)?;
                let right = self.family_term_kernel(id, right, index_terms, payload_terms)?;
                let vocab = self.integer_vocabulary(id)?;
                let head = self.constant(match op {
                    IndexOp::Add => vocab.add,
                    IndexOp::Sub => vocab.subtract,
                    IndexOp::Mul => vocab.multiply,
                });
                Ok(self.signature.arena.apply_arguments(head, &[left, right]))
            }
            FamilyTerm::Name(_) => Err(unserved(
                id,
                "child of indexed data at something other than an index, a payload value or a literal",
            )),
        }
    }

    fn evidence_term_kernel(
        &mut self,
        id: MachineId,
        term: &super::families::EvidenceTerm,
        open: &[TermHandle],
    ) -> Checked<TermHandle> {
        use super::families::{EvidenceTerm, IndexOp};
        match term {
            EvidenceTerm::Parameter(parameter) => Ok(open[*parameter as usize]),
            EvidenceTerm::Literal(value) => {
                self.integer_literal(id, bignum::BigInt::from_i128(*value))
            }
            EvidenceTerm::Binary(op, left, right) => {
                let left = self.evidence_term_kernel(id, left, open)?;
                let right = self.evidence_term_kernel(id, right, open)?;
                let vocab = self.integer_vocabulary(id)?;
                let head = self.constant(match op {
                    IndexOp::Add => vocab.add,
                    IndexOp::Sub => vocab.subtract,
                    IndexOp::Mul => vocab.multiply,
                });
                Ok(self.signature.arena.apply_arguments(head, &[left, right]))
            }
        }
    }

    /// The index with these components: each paired with the rest.
    fn index_tuple(&mut self, components: &[TermHandle]) -> TermHandle {
        self.signature
            .arena
            .tuple(components)
            .expect("indexed data has at least one index")
    }

    /// `body` as a function of `count` values of `domain`.
    fn over(&mut self, domain: TermHandle, count: u32, mut body: TermHandle) -> TermHandle {
        for _ in 0..count {
            body = self.signature.arena.insert(Term::Lambda { domain, body });
        }
        body
    }

    /// A theorem over indexed data: for every index and every value of the
    /// data at it, each `ensures` clause. Its proof is an induction over
    /// the value, whose cases stage 06 proves from what the data requires
    /// of each case and the theorem at the children the body cites.
    fn family_laws(&mut self, id: MachineId, theorem: &FamilyTheorem) -> Checked<Vec<u32>> {
        let program = self.program;
        let declared = self
            .families
            .iter()
            .find(|declared| declared.record == theorem.record)
            .ok_or_else(|| unserved(id, "theorem over indexed data that is not declared"))?
            .clone();
        let source = program
            .families
            .iter()
            .find(|family| family.record == theorem.record)
            .ok_or_else(|| unserved(id, "theorem over indexed data that is not declared"))?;
        let names = program
            .names
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its names"))?;
        let symbol = *program
            .symbols
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its declaration"))?;
        let vocabulary = self.integer_vocabulary(id)?;
        let integer = self.constant(vocabulary.carrier);
        let indices = source.indices.len() as u32;
        let open: Vec<_> = (0..indices)
            .map(|index| self.variable(indices - 1 - index))
            .collect();
        let family = self.signature.families[declared.family as usize].clone();
        let instance = family
            .at(&mut self.signature.arena, &[])
            .map_err(|_| unserved(id, "indexed data the kernel's family builder refuses"))?;
        // What each step requires, per case, under the indices and the
        // case's payload; and, for the theorem's rank, that it is smaller
        // at each child the step cites.
        let zero = self.integer_literal(id, bignum::BigInt::from_u128(0))?;
        let one = self.integer_literal(id, bignum::BigInt::from_u128(1))?;
        let unsigned = |value: &super::families::FamilyValue| {
            value.integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
        };
        let mut owed = Vec::with_capacity(theorem.cases.len());
        let mut ranked = Vec::with_capacity(theorem.cases.len());
        for ((case, read), terms) in source.cases.iter().zip(&theorem.cases).zip(&declared.cases) {
            let total = indices + terms.payload;
            let index_terms: Vec<_> = (0..indices)
                .map(|index| self.variable(total - 1 - index))
                .collect();
            let payload_terms: Vec<_> = case
                .payload
                .iter()
                .enumerate()
                .map(|(position, value)| {
                    let field = value
                        .field
                        .ok_or_else(|| unserved(id, "payload value lost its field"))?;
                    Ok((field, self.variable(terms.payload - 1 - position as u32)))
                })
                .collect::<Checked<_>>()?;
            let scope = Integers {
                indices: &index_terms,
                payload: Some((case.case, &payload_terms)),
            };
            let mut required = Vec::new();
            for proposition in &read.requires {
                self.integer_proposition(id, proposition, &scope, &mut required)?;
            }
            owed.push(required);
            let at_most = self.constant(vocabulary.less_or_equal);
            let add = self.constant(vocabulary.add);
            let mut natural = Vec::new();
            let held = source.indices.iter().zip(&index_terms).chain(
                case.payload
                    .iter()
                    .zip(payload_terms.iter().map(|(_, term)| term)),
            );
            for (value, &term) in held {
                if unsigned(value) {
                    natural.push(self.signature.arena.apply_arguments(at_most, &[zero, term]));
                }
            }
            let mut descent = Vec::new();
            // Ranked on the evidence, each cited child is a part of it and
            // owes nothing more.
            if let Some(super::families::FamilyRank::Index(rank)) = theorem.rank {
                for (child, _) in read.cited.iter().enumerate().filter(|(_, cited)| **cited) {
                    let smaller = *terms
                        .children
                        .get(child)
                        .and_then(|at| at.get(rank as usize))
                        .ok_or_else(|| unserved(id, "cited child without the ranked index"))?;
                    let arena = &mut self.signature.arena;
                    let next = arena.apply_arguments(add, &[smaller, one]);
                    descent
                        .push(arena.apply_arguments(at_most, &[next, index_terms[rank as usize]]));
                }
            }
            ranked.push((natural, descent));
        }
        let mut laws = Vec::with_capacity(theorem.ensures.len());
        for (clause, proposition) in theorem.ensures.iter().enumerate() {
            let mut components = Vec::new();
            self.integer_proposition(
                id,
                proposition,
                &Integers {
                    indices: &open,
                    payload: None,
                },
                &mut components,
            )?;
            let goal = self
                .conjunction(&components)
                .ok_or_else(|| unserved(id, "theorem over indexed data that guarantees nothing"))?;
            // The goal as a function of the index: each index is a
            // projection of the tuple.
            let tuple = self.variable(0);
            let count = indices as usize;
            let projections: Vec<_> = (0..count)
                .map(|index| self.signature.arena.tuple_component(tuple, index, count))
                .collect();
            let body = instantiate(&mut self.signature.arena, goal, &projections);
            let invariant = self.signature.arena.insert(Term::Lambda {
                domain: instance.index_type,
                body,
            });
            let invariant = instance.declare_invariant(
                &mut self.signature.arena,
                &mut self.signature.declarations,
                invariant,
                Level::Constant(0),
            );
            self.sync(&names.machine);
            // For every index and every value of the data at it.
            let arena = &mut self.signature.arena;
            let at = arena.insert(Term::Variable(0));
            let value = instance.family_type(arena, at);
            let at = arena.insert(Term::Variable(1));
            let holds = arena.insert(Term::Apply {
                function: invariant,
                argument: at,
            });
            let statement = arena.insert(Term::Pi {
                domain: value,
                codomain: holds,
            });
            let statement = arena.insert(Term::Pi {
                domain: instance.index_type,
                codomain: statement,
            });
            let law = index(id, self.signature.laws.len())?;
            index(id, self.signature.declarations.len() + 1)?;
            let entry = self.push(
                Declaration::assumption(0, statement),
                SignatureEntryKind::Statement { law },
                names.machine.clone(),
            );
            let (text, text_source) = names
                .ensures
                .get(clause)
                .map(|text| (names.authored(text), text.source))
                .unwrap_or_default();
            self.signature.laws.push(Law {
                machine: symbol,
                clause: clause as u32,
                statement: entry,
                formals: vec![integer; indices as usize],
                premises: Vec::new(),
                goal,
                declared: None,
                proof: LawProof::Family {
                    family: declared.family,
                    invariant,
                    cases: declared
                        .cases
                        .iter()
                        .zip(&theorem.cases)
                        .zip(owed.iter().zip(&source.cases))
                        .zip(&ranked)
                        .map(
                            |(((terms, read), (owed, case)), (natural, descent))| FamilyLawCase {
                                natural: natural.clone(),
                                descent: descent.clone(),
                                name: format!("{}::{}", self.name(source.declaration), case.name),
                                payload: terms.payload,
                                constraints: terms.constraints.clone(),
                                children: terms.children.clone(),
                                cited: read.cited.clone(),
                                owed: owed.clone(),
                                source: read.source,
                            },
                        )
                        .collect(),
                },
                formal_names: source
                    .indices
                    .iter()
                    .map(|index| index.name.clone())
                    .collect(),
                machine_name: names.machine.clone(),
                text,
                source: text_source.or(names.declared),
            });
            laws.push(law);
        }
        Ok(laws)
    }

    /// A record with no cases whose fields are all mathematical data, as the
    /// product of their types.
    fn product(&mut self, id: MachineId, record: StructuralTypeId) -> Checked<u32> {
        let program = self.program;
        let declared = program
            .records
            .get(record)
            .ok_or_else(|| unserved(id, "mathematical data lost its record"))?;
        let source = program
            .record_sources
            .get(record)
            .ok_or_else(|| unserved(id, "mathematical data lost its declaration"))?;
        if declared.fields().is_empty() || !declared.cases().is_empty() {
            return Err(unserved(
                id,
                "mathematical declaration over a type other than data that contains itself \
                 or a record of such data",
            ));
        }
        let mut fields = Vec::with_capacity(declared.fields().len());
        for field in declared.fields() {
            let ValueType::Record(inner, ..) = field.value_type else {
                return Err(unserved(
                    id,
                    "record of mathematical data with a field of another type",
                ));
            };
            fields.push(self.data(id, inner)?);
        }
        let components: Vec<_> = fields.iter().map(|&field| self.constant(field)).collect();
        let ty = self
            .signature
            .arena
            .product_type(&components)
            .expect("checked not empty");
        index(id, self.signature.declarations.len() + 1)?;
        let universe = self
            .signature
            .arena
            .insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let name = self.name(source.declaration);
        let entry = self.push(
            Declaration::definition(0, universe, ty),
            SignatureEntryKind::Record {
                declaration: source.declaration,
                fields,
                type_identity: declared.identity.clone(),
            },
            name,
        );
        self.data.push((record, entry));
        Ok(entry)
    }

    /// The field at this position of a value of the record at `entry`.
    fn component(
        &mut self,
        id: MachineId,
        entry: u32,
        value: TermHandle,
        field: usize,
    ) -> Checked<(TermHandle, u32)> {
        let SignatureEntryKind::Record { fields, .. } =
            &self.signature.entries[entry as usize].kind
        else {
            return Err(unserved(
                id,
                "field of mathematical data that is not a record",
            ));
        };
        let count = fields.len();
        let ty = *fields
            .get(field)
            .ok_or_else(|| unserved(id, "field position outside its record"))?;
        Ok((
            self.signature.arena.tuple_component(value, field, count),
            ty,
        ))
    }

    fn shape(&self, data: u32) -> PlainData {
        match &self.signature.entries[data as usize].kind {
            SignatureEntryKind::Data { shape, .. } => shape.clone(),
            _ => unreachable!("a data entry was recorded with its shape"),
        }
    }

    /// The entry of `bool`, declaring it the first time.
    fn boolean(&mut self, id: MachineId) -> Checked<u32> {
        if let Some(entry) = self.boolean {
            return Ok(entry);
        }
        index(id, self.signature.declarations.len() + 1)?;
        let universe = self
            .signature
            .arena
            .insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let two = self.signature.arena.insert(Term::Two);
        let entry = self.push(
            Declaration::definition(0, universe, two),
            SignatureEntryKind::Boolean,
            "bool".to_owned(),
        );
        self.boolean = Some(entry);
        Ok(entry)
    }

    /// The closed kernel type of a formal or a result: data, or a plain
    /// `bool`, which an admitted total computation may return
    /// (mathematical_bindings.md). Every other scalar is refused.
    fn ty(&mut self, id: MachineId, value_type: ValueType) -> Checked<(TermHandle, u32)> {
        if value_type == ValueType::plain(ScalarType::Boolean) {
            let entry = self.boolean(id)?;
            return Ok((self.constant(entry), entry));
        }
        let ValueType::Record(record, ..) = value_type else {
            return Err(unserved(
                id,
                "mathematical declaration over a type other than data that contains itself",
            ));
        };
        let entry = self.data(id, record)?;
        Ok((self.constant(entry), entry))
    }

    /// The closed kernel type of a formal or of a payload field: what `ty`
    /// reads, or a fixed-width integer, which is the integer it holds. A
    /// definition only passes such a value along: no result is one, and its
    /// arithmetic is not read.
    fn value_ty(&mut self, id: MachineId, value_type: ValueType) -> Checked<(TermHandle, u32)> {
        if let ValueType::Scalar(ScalarType::Integer(_), _) = value_type {
            let carrier = self.declare_integers(id)?;
            return Ok((self.constant(carrier), carrier));
        }
        // A stored callable of one plain parameter is the function from it:
        // the kernel's `Π`, at the entry of its result.
        if let ValueType::Scalar(ScalarType::Callable(contract), _) = value_type {
            let signature = self
                .program
                .callables
                .contracts
                .get(contract)
                .map(|row| row.signature.clone())
                .ok_or_else(|| unserved(id, "stored callable lost its contract"))?;
            let (&[parameter], Some(result), true, true) = (
                signature.parameters.as_slice(),
                signature.result,
                signature.erased.is_empty(),
                signature.borrowed.is_empty(),
            ) else {
                return Err(unserved(
                    id,
                    "mathematical stored callable of other than one plain parameter and a result",
                ));
            };
            let (domain, _) = self.value_ty(id, parameter)?;
            let (codomain, entry) = self.value_ty(id, result)?;
            let codomain = shift(&mut self.signature.arena, codomain, 0, 1);
            return Ok((
                self.signature.arena.insert(Term::Pi { domain, codomain }),
                entry,
            ));
        }
        self.ty(id, value_type)
    }

    fn formals(&mut self, id: MachineId) -> Checked<Vec<(TermHandle, u32)>> {
        let signature = self
            .program
            .signatures
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its signature"))?;
        if !signature.erased.is_empty() || !signature.borrowed.is_empty() {
            return Err(unserved(
                id,
                "mathematical machine with an erased or borrowed formal",
            ));
        }
        // A premise of a law is a hypothesis of its proof and an obligation
        // at each citation. A premise of a definition is an obligation at
        // each call (`owed`). A machine that states nothing and returns
        // nothing has no reader for one.
        if !signature.requires.is_empty() && !self.states_law(id) && signature.result.is_none() {
            return Err(unserved(
                id,
                "mathematical machine with a premise that states no law",
            ));
        }
        signature
            .parameters
            .clone()
            .into_iter()
            .map(|formal| self.value_ty(id, formal))
            .collect()
    }

    /// Which law of operands, if any, the requirement this machine satisfies
    /// states: associativity or commutativity of an operation, or
    /// distributivity of one over another. Read without the machine's body.
    fn requires_operand_law(
        &mut self,
        satisfaction: &Satisfaction,
    ) -> Checked<Option<terminal_check_tables::mathematics::OperandLaw>> {
        let id = satisfaction.machine;
        let [clause] = satisfaction.ensures.as_slice() else {
            return Ok(None);
        };
        let formals = self.formals(id)?;
        let count = formals.len();
        let open: Vec<_> = (0..count)
            .map(|formal| self.variable((count - 1 - formal) as u32))
            .collect();
        let mut statement = self.equality(id, clause, &open, Some(satisfaction))?;
        for &(formal, _) in formals.iter().rev() {
            statement = self.signature.arena.insert(Term::Pi {
                domain: formal,
                codomain: statement,
            });
        }
        Ok(operand_law(&self.signature.arena, statement).map(|(_, law)| law))
    }

    fn declare(&mut self, id: MachineId) -> Checked<()> {
        if let Some(lifted) = self.lifted_operation(id) {
            return self.lifted(id, lifted);
        }
        let program = self.program;
        if let Some((_, theorem)) = program
            .family_theorems
            .iter()
            .find(|(machine, _)| *machine == id)
        {
            if self
                .begin(id, false, "theorem over indexed data that reaches itself")?
                .is_some()
            {
                return Ok(());
            }
            let read = self.family_laws(id, theorem);
            return self.finish(id, false, read.map(|laws| ((), Read::Laws(laws))));
        }
        let result = self
            .program
            .signatures
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its signature"))?
            .result;
        match result {
            None => self.laws(id).map(|_| ()),
            // A machine that returns a value and states a law is a law. It
            // is a definition too where a term applies it or its statement
            // names its result, and is read as one there: a law may return
            // a value only so that it can be called as a statement, and its
            // body is then an argument, not a definition.
            Some(_) if self.states_law(id) => self.laws(id).map(|_| ()),
            // A nested machine that states nothing is read where it is
            // called, whether or not it returns a value.
            Some(_) if self.helper(id) => self.laws(id).map(|_| ()),
            Some(_) => self.function(id).map(|_| ()),
        }
    }

    /// Whether the machine states an equality, its own or its requirement's.
    fn states_law(&self, id: MachineId) -> bool {
        self.program
            .signatures
            .get(id)
            .is_some_and(|signature| !signature.ensures.is_empty())
            || self
                .satisfactions
                .iter()
                .any(|satisfaction| satisfaction.machine == id && !satisfaction.ensures.is_empty())
    }

    /// Whether one of the machine's own equalities names its result.
    fn names_result(&self, id: MachineId) -> bool {
        self.program.signatures.get(id).is_some_and(|signature| {
            let result = signature.parameters.len() as u64 + 1;
            signature.ensures.iter().any(|clause| {
                matches!(clause, Proposition::DeclarationIdentity(identity)
                if identity.values.iter().any(|value| matches!(
                    &value.operand,
                    ClauseOperand::Input(subject)
                        if subject.path().is_empty() && subject.root().get() == result
                )))
            })
        })
    }

    /// Whether the machine is declared inside another and states no law. Such
    /// a machine is a step of its parent's argument or of its parent's value:
    /// it is read in place, with its formals standing for the call's
    /// arguments, so a recursive call it makes is still the parent's.
    fn helper(&self, id: MachineId) -> bool {
        !self.states_law(id)
            && self
                .program
                .symbols
                .get(id)
                .is_some_and(|symbol| self.program.enclosing_machine(*symbol).is_some())
    }

    fn track(&mut self, function: bool) -> &mut IdTable<MachineId, Read> {
        if function {
            &mut self.functions
        } else {
            &mut self.machines
        }
    }

    fn begin(
        &mut self,
        id: MachineId,
        function: bool,
        cycle: &'static str,
    ) -> Checked<Option<Read>> {
        let state = self
            .track(function)
            .get(id)
            .cloned()
            .ok_or_else(|| unserved(id, "mathematical machine outside this reader"))?;
        match state {
            Read::Unread => {
                *self.track(function).get_mut(id).expect("row read above") = Read::Reading;
                Ok(None)
            }
            Read::Reading => Err(unserved(id, cycle)),
            Read::Refused => Err(unserved(
                id,
                "use of a mathematical machine that is refused above",
            )),
            known => Ok(Some(known)),
        }
    }

    fn finish<T>(&mut self, id: MachineId, function: bool, read: Checked<(T, Read)>) -> Checked<T> {
        let (value, state) = match read {
            Ok((value, state)) => (Ok(value), state),
            Err(refusal) => (Err(refusal), Read::Refused),
        };
        *self.track(function).get_mut(id).expect("row read at begin") = state;
        value
    }

    /// A machine with a result, as a definition.
    fn function(&mut self, id: MachineId) -> Checked<u32> {
        if let Some(lifted) = self.lifted_operation(id) {
            return self.lifted_function(id, lifted);
        }
        match self.begin(
            id,
            true,
            "mathematical machines that call each other in a cycle",
        )? {
            None => {}
            Some(Read::Function(entry)) => return Ok(entry),
            Some(_) => return Err(unserved(id, "term that calls a machine with no result")),
        }
        let outer = (
            std::mem::take(&mut self.callable_scope),
            std::mem::take(&mut self.held),
        );
        let read = self.read_function(id);
        (self.callable_scope, self.held) = outer;
        self.finish(id, true, read.map(|entry| (entry, Read::Function(entry))))
    }

    fn read_function(&mut self, id: MachineId) -> Checked<u32> {
        let program = self.program;
        let signature = program
            .signatures
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its signature"))?;
        if signature.has_structural_guarantees() {
            return Err(unserved(
                id,
                "mathematical machine with a result and a structural guarantee",
            ));
        }
        let formals = self.formals(id)?;
        let (result, result_entry) = self.ty(
            id,
            signature
                .result
                .ok_or_else(|| unserved(id, "mathematical function without a result"))?,
        )?;
        let count = formals.len();
        let body = self
            .bodies
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its body"))?;
        let (aliases, statements) = spine(body);
        let symbol = *program
            .symbols
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its declaration"))?;
        let name = self.name(symbol);
        let open: Vec<_> = (0..count)
            .map(|formal| self.variable((count - 1 - formal) as u32))
            .collect();
        let template = self.template(id);
        let callables = self.enter(id, count)?;
        let mut premises = Vec::with_capacity(signature.requires.len());
        for premise in &signature.requires {
            let premise = self.equality(id, premise, &open, None)?;
            premises.push(premise);
            self.held.push(premise);
        }
        if template.is_some() {
            self.generic_premises.push((id, premises.clone()));
        }
        if let Some(template) = template.filter(|template| !template.lowered) {
            // A machine generic over machines: its value, read as a term
            // with its contract, under its machine parameters and formals.
            let value = template
                .value
                .ok_or_else(|| unserved(id, "generic mathematical machine without a value"))?;
            let (terms, _) = self.operands(id, &template.values, &open, None)?;
            let mut term = terms[value as usize];
            let mut ty = result;
            for &domain in formals
                .iter()
                .map(|(formal, _)| formal)
                .rev()
                .chain(callables.iter().rev())
            {
                term = self
                    .signature
                    .arena
                    .insert(Term::Lambda { domain, body: term });
                ty = self.signature.arena.insert(Term::Pi {
                    domain,
                    codomain: ty,
                });
            }
            index(id, self.signature.declarations.len() + 1)?;
            return Ok(self.push(
                Declaration::definition(0, ty, term),
                SignatureEntryKind::Function {
                    machine: symbol,
                    cases: None,
                },
                name,
            ));
        }
        let ranked = match signature
            .ranking
            .as_ref()
            .map(|ranking| ranking.measures.as_slice())
        {
            None => None,
            Some([measure]) => match measure.kind {
                ExpressionKind::Slot(slot) if slot < count => Some(slot),
                _ => {
                    return Err(unserved(
                        id,
                        "mathematical machine ranked on something other than one formal",
                    ));
                }
            },
            Some(_) => {
                return Err(unserved(
                    id,
                    "mathematical machine ranked on something other than one formal",
                ));
            }
        };
        // A body that is one dispatch on a formal, after any statements a
        // law's script reads, recurses the kernel's own way when it calls
        // itself only at a field of the case.
        let leading = statements
            .iter()
            .take_while(|statement| {
                matches!(
                    statement,
                    Statement::Evaluate { .. } | Statement::Bind { .. }
                )
            })
            .count();
        let by_cases = match &statements[leading..] {
            [
                Statement::Dispatch {
                    subject,
                    arms,
                    default: None,
                    ..
                },
            ] => match subject.kind {
                ExpressionKind::Slot(slot) => {
                    let slot = aliases.get(slot).copied().unwrap_or(slot);
                    (slot < count && ranked.is_none_or(|ranked| ranked == slot))
                        .then_some((slot, arms))
                }
                _ => None,
            },
            _ => None,
        };
        let defining = Defining {
            id,
            symbol,
            name: &name,
            formals: &formals,
            callables: &callables,
            result,
            premises: &premises,
        };
        let structural = by_cases.map(|(subject, arms)| self.by_cases(&defining, subject, arms));
        let (mut term, cases) = match (structural, ranked) {
            (Some(Ok((term, cases))), _) => (term, cases),
            (Some(Err(refusal)), None) => return Err(refusal),
            // It calls itself somewhere else, or chooses by a computed
            // value first: the recursion is on fuel taken from its measure.
            (_, Some(measure)) => (self.by_fuel(&defining, measure, result_entry)?, None),
            (None, None) => {
                // A generic machine's machine parameters are variables of
                // the body too, just outside its formals: a dispatch inside
                // it is a definition over all of them.
                let mut types: Vec<_> = callables.clone();
                types.extend(formals.iter().map(|(formal, _)| *formal));
                let valuing = Valuing {
                    owner: id,
                    context: &types,
                    result,
                    fuel: None,
                    premises: &premises,
                    facts: &[],
                    cited: &[],
                };
                let slots = self.formal_slots(count, body.slots.len());
                (self.body_value(id, slots, None, &valuing)?, None)
            }
        };
        let mut ty = result;
        for &domain in formals
            .iter()
            .map(|(formal, _)| formal)
            .rev()
            .chain(callables.iter().rev())
        {
            term = self
                .signature
                .arena
                .insert(Term::Lambda { domain, body: term });
            ty = self.signature.arena.insert(Term::Pi {
                domain,
                codomain: ty,
            });
        }
        index(id, self.signature.declarations.len() + 1)?;
        Ok(self.push(
            Declaration::definition(0, ty, term),
            SignatureEntryKind::Function {
                machine: symbol,
                cases,
            },
            name,
        ))
    }

    /// A definition by cases on the formal `subject`, as a term under the
    /// machine's formals: the kernel's recursion on that formal. A call the
    /// machine makes of itself must be at a field of the case.
    fn by_cases(
        &mut self,
        defining: &Defining<'_>,
        subject: usize,
        arms: &[(CaseSelector, Target, crate::SourceOrigin)],
    ) -> Checked<(TermHandle, Option<FunctionCases>)> {
        let id = defining.id;
        let formals = defining.formals;
        let count = formals.len();
        let body = self
            .bodies
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its body"))?;
        let (aliases, _) = spine(body);
        let data = formals[subject].1;
        let SignatureEntryKind::Data { shape, .. } = &self.signature.entries[data as usize].kind
        else {
            return Err(unserved(
                id,
                "mathematical dispatch on a value without cases",
            ));
        };
        let shape = shape.clone();
        if shape.is_mutual() {
            return Ok((self.by_mutual_cases(defining, &shape, data, arms)?, None));
        }
        let others: Vec<usize> = (0..count).filter(|formal| *formal != subject).collect();
        // A generic machine's machine parameters come first among the
        // arguments the motive's function takes, so each branch binds them
        // itself and stays closed: `FunctionCases` records closed branches.
        let callables = defining.callables;
        let parameters = callables.len() as u32;
        let rest = parameters + others.len() as u32;
        let mut passed: Vec<TermHandle> = callables.to_vec();
        passed.extend(others.iter().map(|&other| formals[other].0));
        // The motive returns a function of the other formals, so a
        // recursive call may pass them different values.
        let mut landing = defining.result;
        for &domain in passed.iter().rev() {
            landing = self.signature.arena.insert(Term::Pi {
                domain,
                codomain: landing,
            });
        }
        let motive = self.signature.arena.insert(Term::Lambda {
            domain: formals[subject].0,
            body: landing,
        });
        if arms.len() != shape.cases() {
            return Err(unserved(
                id,
                "mathematical dispatch without exactly one value arm per case",
            ));
        }
        let mut branches = Vec::with_capacity(shape.cases());
        for case in 0..shape.cases() {
            let mut selected = arms.iter().filter(|(selector, _, _)| selector.case == case);
            let (Some((selector, Target::Value(value), _)), None) =
                (selected.next(), selected.next())
            else {
                return Err(unserved(
                    id,
                    "mathematical dispatch without exactly one value arm per case",
                ));
            };
            if selector.condition.is_some() {
                return Err(unserved(id, "mathematical dispatch on a field value"));
            }
            let arity = shape.arity(case);
            let arena = &mut self.signature.arena;
            let fields: Vec<_> = (0..arity)
                .map(|field| shape.field(arena, case, field, rest))
                .collect();
            let results: Vec<_> = (0..arity)
                .map(|field| shape.hypothesis(arena, case, field, rest))
                .collect();
            // Inside the case the dispatched formal is the case's value.
            let mut slots = vec![None; body.slots.len().max(count)];
            slots[subject] = Some(Bound {
                term: shape.case_value(arena, data, case, rest),
                field: None,
                applied: None,
            });
            let mut actuals = vec![slots[subject].expect("bound above").term; count];
            let after = others.len() as u32;
            for (position, &other) in others.iter().enumerate() {
                let term = self.variable(after - 1 - position as u32);
                slots[other] = Some(Bound {
                    term,
                    field: None,
                    applied: None,
                });
                actuals[other] = term;
            }
            // The variables of the branch: the case's fields, the recursion
            // at each, the machine parameters, then the other formals.
            let mut context = shape.field_types(&mut self.signature.arena, data, case);
            context.extend(vec![landing; shape.hypotheses(case) as usize]);
            context.extend(passed.iter().copied());
            // Within the branch a machine parameter is the branch's own
            // variable, just outside the other formals.
            let enclosing = std::mem::take(&mut self.callable_scope);
            self.callable_scope = enclosing
                .iter()
                .enumerate()
                .map(|(position, &(symbol, _))| {
                    (
                        symbol,
                        self.variable(after + parameters - 1 - position as u32),
                    )
                })
                .collect();
            let mut every = self.callable_actuals(0);
            every.extend(actuals.iter().copied());
            let premises: Vec<_> = defining
                .premises
                .iter()
                .map(|&premise| instantiate(&mut self.signature.arena, premise, &every))
                .collect();
            let valuing = Valuing {
                owner: id,
                context: &context,
                result: defining.result,
                fuel: None,
                premises: &premises,
                facts: &[],
                cited: &[],
            };
            let scope = Scope {
                slots: &slots,
                aliases: &aliases,
                case: Some((subject, case, &fields)),
                recursion: Some(Recursion {
                    machine: id,
                    subject,
                    results: &results,
                    group: &[],
                    members: &[],
                }),
                depth: 0,
                induction: true,
                value: Some(&valuing),
            };
            let read = self.expression(id, value, &scope);
            self.callable_scope = enclosing;
            let mut branch = read?.term;
            for &domain in passed.iter().rev() {
                branch = self.signature.arena.insert(Term::Lambda {
                    domain,
                    body: branch,
                });
            }
            branches.push(shape.branch(&mut self.signature.arena, data, motive, case, branch));
        }
        let tree = self.variable((count - 1 - subject) as u32);
        let induction = shape.induct(&mut self.signature.arena, data, motive, &branches, tree);
        let mut arguments: Vec<_> = (0..parameters)
            .map(|position| self.variable(count as u32 + parameters - 1 - position))
            .collect();
        arguments.extend(
            others
                .iter()
                .map(|&other| self.variable((count - 1 - other) as u32)),
        );
        Ok((
            self.signature.arena.apply_arguments(induction, &arguments),
            Some(FunctionCases {
                data,
                subject: parameters + subject as u32,
                arity: parameters + count as u32,
                motive,
                branches,
            }),
        ))
    }

    /// A definition by cases on the one formal of a machine over a member of
    /// a mutual block of data, as the block's recursion. The machines it
    /// calls, directly or through each other, that call it again are defined
    /// with it (`mutual_group`): each takes one value of a member and
    /// dispatches on it, and a call of one at a field of its member is the
    /// recursion's result there. The recursion takes one motive per member
    /// and one branch per constructor of the block. A member no machine of
    /// the group dispatches on gets a motive into `Two` and branches that
    /// ignore their fields: nothing reads their value.
    fn by_mutual_cases(
        &mut self,
        defining: &Defining<'_>,
        shape: &PlainData,
        data: u32,
        arms: &[(CaseSelector, Target, crate::SourceOrigin)],
    ) -> Checked<TermHandle> {
        let id = defining.id;
        if defining.formals.len() != 1 || !defining.premises.is_empty() {
            return Err(unserved(
                id,
                "definition by cases over a mutual block of data with other formals or a premise",
            ));
        }
        let first = shape.member_entry(data, 0);
        let group = self
            .mutual_group(id, first, shape.members())?
            .into_iter()
            .map(|(machine, member, result)| {
                result
                    .map(|result| (machine, member, result))
                    .ok_or_else(|| {
                        unserved(machine, "machine of a mutual recursion with no result")
                    })
            })
            .collect::<Checked<Vec<_>>>()?;
        let members: Vec<(MachineId, u32)> = group
            .iter()
            .map(|&(machine, member, _)| (machine, member))
            .collect();
        let two = self.signature.arena.insert(Term::Two);
        let landings: Vec<TermHandle> = (0..shape.members())
            .map(|member| {
                group
                    .iter()
                    .find(|(_, of, _)| *of == member)
                    .map_or(two, |(_, _, result)| *result)
            })
            .collect();
        let motives: Vec<TermHandle> = (0..shape.members())
            .map(|member| {
                let ty = self.constant(first + member);
                self.signature.arena.insert(Term::Lambda {
                    domain: ty,
                    body: landings[member as usize],
                })
            })
            .collect();
        let bodies = self.bodies;
        let mut branches = Vec::new();
        for member in 0..shape.members() {
            let at = shape.at_member(member).expect("a member of the block");
            let entry = first + member;
            let Some(&(machine, _, result)) = group.iter().find(|(_, of, _)| *of == member) else {
                for case in 0..at.cases() {
                    let ignored = self.signature.arena.insert(Term::TwoZero);
                    branches.push(at.branch_in(
                        &mut self.signature.arena,
                        entry,
                        &motives,
                        case,
                        ignored,
                    ));
                }
                continue;
            };
            let body = bodies
                .get(machine)
                .ok_or_else(|| unserved(machine, "mathematical machine lost its body"))?;
            let (aliases, statements) = spine(body);
            let machine_arms = if machine == id {
                arms
            } else {
                match statements.as_slice() {
                    [
                        Statement::Dispatch {
                            arms,
                            default: None,
                            ..
                        },
                    ] => arms.as_slice(),
                    _ => {
                        return Err(unserved(
                            machine,
                            "machine of a mutual recursion that is not one dispatch on its formal",
                        ));
                    }
                }
            };
            if machine_arms.len() != at.cases() {
                return Err(unserved(
                    machine,
                    "mathematical dispatch without exactly one value arm per case",
                ));
            }
            for case in 0..at.cases() {
                let mut selected = machine_arms
                    .iter()
                    .filter(|(selector, _, _)| selector.case == case);
                let (Some((selector, Target::Value(value), _)), None) =
                    (selected.next(), selected.next())
                else {
                    return Err(unserved(
                        machine,
                        "mathematical dispatch without exactly one value arm per case",
                    ));
                };
                if selector.condition.is_some() {
                    return Err(unserved(machine, "mathematical dispatch on a field value"));
                }
                let arity = at.arity(case);
                let arena = &mut self.signature.arena;
                let fields: Vec<_> = (0..arity)
                    .map(|field| at.field(arena, case, field, 0))
                    .collect();
                let results: Vec<_> = (0..arity)
                    .map(|field| at.hypothesis(arena, case, field, 0))
                    .collect();
                let field_members: Vec<_> = (0..arity)
                    .map(|field| at.field_member(case, field))
                    .collect();
                let mut slots = vec![None; body.slots.len().max(1)];
                slots[0] = Some(Bound {
                    term: at.case_value(arena, entry, case, 0),
                    field: None,
                    applied: None,
                });
                // The variables of the branch: the case's fields, then the
                // recursion at each field of a member, into that member's
                // motive.
                let mut context = at.field_types(arena, entry, case);
                context.extend(
                    field_members
                        .iter()
                        .flatten()
                        .map(|&of| landings[of as usize]),
                );
                let valuing = Valuing {
                    owner: machine,
                    context: &context,
                    result,
                    fuel: None,
                    premises: &[],
                    facts: &[],
                    cited: &[],
                };
                let scope = Scope {
                    slots: &slots,
                    aliases: &aliases,
                    case: Some((0, case, &fields)),
                    recursion: Some(Recursion {
                        machine,
                        subject: 0,
                        results: &results,
                        group: &members,
                        members: &field_members,
                    }),
                    depth: 0,
                    induction: true,
                    value: Some(&valuing),
                };
                let branch = self.expression(machine, value, &scope)?.term;
                branches.push(at.branch_in(
                    &mut self.signature.arena,
                    entry,
                    &motives,
                    case,
                    branch,
                ));
            }
        }
        let tree = self.variable(0);
        Ok(shape.induct_in(
            &mut self.signature.arena,
            data,
            Level::Constant(0),
            &motives,
            &branches,
            tree,
        ))
    }

    /// The laws proved with law `id` by induction over a member of a mutual
    /// block of data: the machines its arms cite, directly or through each
    /// other, that cite it again (`mutual_group`), each with the member it
    /// dispatches on, and for each member of the block the machine whose law
    /// is about it. The block's induction takes a motive per member, so each
    /// states one equality about one value of its member, with no premise;
    /// a member none is about gets a motive into `Two` in stage 06.
    #[allow(clippy::type_complexity)]
    fn mutual_laws(
        &mut self,
        id: MachineId,
        shape: &PlainData,
        data: u32,
    ) -> Checked<(Vec<(MachineId, u32)>, Vec<Option<SymbolHandle>>)> {
        let first = shape.member_entry(data, 0);
        let group = self.mutual_group(id, first, shape.members())?;
        if !group.iter().any(|(machine, ..)| *machine == id) {
            return Err(unserved(
                id,
                "law by induction over a mutual block of data with other formals",
            ));
        }
        let mut mutual = vec![None; shape.members() as usize];
        for &(machine, member, _) in &group {
            let single = self
                .program
                .signatures
                .get(machine)
                .is_some_and(|signature| {
                    signature.ensures.len() == 1 && signature.requires.is_empty()
                });
            let satisfies = self
                .satisfactions
                .iter()
                .any(|satisfaction| satisfaction.machine == machine);
            if !single || satisfies {
                return Err(unserved(
                    machine,
                    "law of a mutual induction that is not one equality with no premise",
                ));
            }
            let symbol =
                *self.program.symbols.get(machine).ok_or_else(|| {
                    unserved(machine, "mathematical machine lost its declaration")
                })?;
            mutual[member as usize] = Some(symbol);
        }
        let group = group
            .into_iter()
            .map(|(machine, member, _)| (machine, member))
            .collect();
        Ok((group, mutual))
    }

    /// The machines defined with `id` by mutual recursion over the block
    /// whose first member is at entry `first`: those it calls in its arms,
    /// directly or through each other, that call it again. Each takes one
    /// value of a member, and no two dispatch on one
    /// member. Each with the member it dispatches on and its result type.
    fn mutual_group(
        &mut self,
        id: MachineId,
        first: u32,
        members: u32,
    ) -> Checked<Vec<(MachineId, u32, Option<TermHandle>)>> {
        let mut reached: Vec<(MachineId, u32, Option<TermHandle>, Vec<MachineId>)> = Vec::new();
        let mut queue = vec![id];
        while let Some(machine) = queue.pop() {
            if reached.iter().any(|(known, ..)| *known == machine) {
                continue;
            }
            let Some((member, result)) = self.mutual_member(machine, first, members)? else {
                continue;
            };
            let callees = self.dispatch_callees(machine);
            queue.extend(callees.iter().copied());
            reached.push((machine, member, result, callees));
        }
        // Those that reach `id` again through the reached machines' calls.
        let reaches = |from: MachineId| {
            let mut seen = vec![from];
            let mut at = 0;
            while let Some(&machine) = seen.get(at) {
                at += 1;
                let Some((.., callees)) = reached.iter().find(|(known, ..)| *known == machine)
                else {
                    continue;
                };
                if callees.contains(&id) {
                    return true;
                }
                for &callee in callees {
                    if !seen.contains(&callee) {
                        seen.push(callee);
                    }
                }
            }
            false
        };
        let group: Vec<(MachineId, u32, Option<TermHandle>)> = reached
            .iter()
            .filter(|(machine, ..)| *machine == id || reaches(*machine))
            .map(|(machine, member, result, _)| (*machine, *member, *result))
            .collect();
        for (position, (_, member, _)) in group.iter().enumerate() {
            if group[..position]
                .iter()
                .any(|(_, other, _)| other == member)
            {
                return Err(unserved(
                    id,
                    "mutual recursion with two machines over one member of the block",
                ));
            }
        }
        Ok(group)
    }

    /// The member of the block at `first` that `machine` dispatches on and
    /// its result type, if any, when it takes exactly one value of a member.
    fn mutual_member(
        &mut self,
        machine: MachineId,
        first: u32,
        members: u32,
    ) -> Checked<Option<(u32, Option<TermHandle>)>> {
        let Some(signature) = self.program.signatures.get(machine) else {
            return Ok(None);
        };
        let (&[ValueType::Record(record, ..)], result) =
            (signature.parameters.as_slice(), signature.result)
        else {
            return Ok(None);
        };
        if self.program.self_containing.binary_search(&record).is_err() {
            return Ok(None);
        }
        let entry = self.data(machine, record)?;
        if !(first..first + members).contains(&entry) {
            return Ok(None);
        }
        let result = match result {
            Some(result) => Some(self.ty(machine, result)?.0),
            None => None,
        };
        Ok(Some((entry - first, result)))
    }

    /// Every machine called in the value arms of `machine`'s body.
    fn dispatch_callees(&self, machine: MachineId) -> Vec<MachineId> {
        fn walk(expression: &Expression, found: &mut Vec<MachineId>) {
            match &expression.kind {
                ExpressionKind::Call {
                    callee: CallTarget::Machine(callee),
                    arguments,
                    ..
                } => {
                    if !found.contains(callee) {
                        found.push(*callee);
                    }
                    for argument in arguments {
                        walk(argument, found);
                    }
                }
                ExpressionKind::Case { fields, .. } | ExpressionKind::Record { fields, .. } => {
                    for (_, operand) in fields {
                        walk(operand, found);
                    }
                }
                ExpressionKind::Field { base, .. } | ExpressionKind::Payload { base, .. } => {
                    walk(base, found)
                }
                _ => {}
            }
        }
        let mut found = Vec::new();
        let Some(body) = self.bodies.get(machine) else {
            return found;
        };
        for statement in &body.statements {
            match statement {
                Statement::Dispatch { arms, .. } => {
                    for (_, target, _) in arms {
                        if let Target::Value(value) = target {
                            walk(value, &mut found);
                        }
                    }
                }
                Statement::Bind { value, .. }
                | Statement::Evaluate {
                    expression: value, ..
                } => walk(value, &mut found),
                _ => {}
            }
        }
        found
    }

    /// A definition whose recursion is not at a field of a case, as a term
    /// under the machine's formals.
    ///
    /// The kernel recurses only on the structure of a value. So the machine
    /// is first defined over one more argument, the fuel, by recursion on
    /// it: with a unit of fuel the body is read with each call of the
    /// machine standing for the definition at one unit less, and with none a
    /// call is a fixed value of the result type. The machine itself is that
    /// definition with its ranked formal as the fuel. Nothing is assumed:
    /// both are total definitions the kernel checks. What the machine's
    /// `terminates by` promises, that the ranked argument of each call is
    /// strictly smaller in natural order, is what makes the fuel last: each
    /// call owes that descent as a statement of its own (`Descent`), so a
    /// call at an argument that does not shrink is refused there. A law
    /// about the machine is proved from the definition over fuel alone.
    fn by_fuel(
        &mut self,
        defining: &Defining<'_>,
        measure: usize,
        result_entry: u32,
    ) -> Checked<TermHandle> {
        let id = defining.id;
        let formals = defining.formals;
        let count = formals.len();
        let body = self
            .bodies
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its body"))?;
        let (fuel, data) = formals[measure];
        let SignatureEntryKind::Data { shape, .. } = &self.signature.entries[data as usize].kind
        else {
            return Err(unserved(
                id,
                "mathematical machine ranked on a value without cases",
            ));
        };
        let shape = shape.clone();
        if shape.is_mutual() {
            // The fuel's descent is in the natural order, which only data
            // shaped like `Nat` has (`natural_order`).
            return Err(unserved(
                id,
                "recursive call at a computed value of a mutual block of data, which has no natural order",
            ));
        }
        let default = self.inhabitant(id, result_entry)?;
        // The definition over fuel is a declaration of its own: a generic
        // machine's machine parameters are its arguments too, after the
        // fuel and before the formals.
        let callables = defining.callables;
        let parameters = callables.len() as u32;
        let mut types: Vec<_> = callables.to_vec();
        types.extend(formals.iter().map(|(formal, _)| *formal));
        let mut landing = defining.result;
        for &formal in types.iter().rev() {
            landing = self.signature.arena.insert(Term::Pi {
                domain: formal,
                codomain: landing,
            });
        }
        let motive = self.signature.arena.insert(Term::Lambda {
            domain: fuel,
            body: landing,
        });
        let all = parameters + count as u32;
        let mut branches = Vec::with_capacity(shape.cases());
        for case in 0..shape.cases() {
            // A unit less of fuel is the first field that holds the data.
            let step = shape
                .fields(case)
                .iter()
                .enumerate()
                .find_map(|(field, read)| {
                    matches!(read, PlainField::Recursive).then_some(field as u32)
                });
            let step = step
                .and_then(|field| shape.hypothesis(&mut self.signature.arena, case, field, all));
            let mut context = shape.field_types(&mut self.signature.arena, data, case);
            context.extend(vec![landing; shape.hypotheses(case) as usize]);
            context.extend(types.iter().copied());
            // The case with no fuel left binds nothing besides the formals,
            // so it states each call's descent over them alone.
            let descent = (shape.arity(case) == 0).then(|| Descent {
                measure,
                ranked: self.variable((count - 1 - measure) as u32),
                nat: data,
            });
            let valuing = Valuing {
                owner: id,
                context: &context,
                result: defining.result,
                fuel: Some(Fuel {
                    machine: id,
                    step,
                    default,
                    descent,
                }),
                premises: defining.premises,
                facts: &[],
                cited: &[],
            };
            let slots = self.formal_slots(count, body.slots.len());
            // Within the branch a machine parameter is the branch's own
            // variable, just outside the formals, as it is outside them in
            // the machine: the premises read the same here.
            let enclosing = std::mem::take(&mut self.callable_scope);
            self.callable_scope = enclosing
                .iter()
                .enumerate()
                .map(|(position, &(symbol, _))| {
                    (
                        symbol,
                        self.variable(count as u32 + parameters - 1 - position as u32),
                    )
                })
                .collect();
            let read = self.body_value(id, slots, None, &valuing);
            self.callable_scope = enclosing;
            let mut branch = read?;
            for &formal in types.iter().rev() {
                branch = self.signature.arena.insert(Term::Lambda {
                    domain: formal,
                    body: branch,
                });
            }
            branches.push(shape.branch(&mut self.signature.arena, data, motive, case, branch));
        }
        let tree = self.variable(all);
        let induction = shape.induct(&mut self.signature.arena, data, motive, &branches, tree);
        let own: Vec<_> = (0..all)
            .map(|position| self.variable(all - 1 - position))
            .collect();
        let mut term = self.signature.arena.apply_arguments(induction, &own);
        let mut ty = defining.result;
        for &formal in types.iter().rev() {
            term = self.signature.arena.insert(Term::Lambda {
                domain: formal,
                body: term,
            });
            ty = self.signature.arena.insert(Term::Pi {
                domain: formal,
                codomain: ty,
            });
        }
        term = self.signature.arena.insert(Term::Lambda {
            domain: fuel,
            body: term,
        });
        ty = self.signature.arena.insert(Term::Pi {
            domain: fuel,
            codomain: ty,
        });
        index(id, self.signature.declarations.len() + 1)?;
        let over_fuel = self.push(
            Declaration::definition(0, ty, term),
            SignatureEntryKind::Function {
                machine: defining.symbol,
                cases: Some(FunctionCases {
                    data,
                    subject: 0,
                    arity: all + 1,
                    motive,
                    branches,
                }),
            },
            format!("{} over fuel", defining.name),
        );
        // A law counts the machine parameters among its formals, first.
        self.fueled
            .push((id, over_fuel, data, parameters + measure as u32));
        // The machine is that definition with its ranked formal as the fuel.
        let head = self.constant(over_fuel);
        let mut operands = vec![own[parameters as usize + measure]];
        operands.extend(own);
        Ok(self.signature.arena.apply_arguments(head, &operands))
    }

    /// The entry of the truncated subtraction that orders the data at
    /// `data`, the natural order a recursive call at a computed value of it
    /// descends under.
    ///
    /// The order is recognized by what it computes, not by its name: data
    /// with one case without fields and one case with one field, and a
    /// machine read so far whose definition is exactly core's
    /// `saturating_sub(a, b)`, by cases on `b`: `a` at zero and the
    /// predecessor of the recursion at a successor, where the predecessor is
    /// itself exactly core's `pred`. So core's `Nat` and a program's copy of
    /// it are ordered, and a lookalike that computes anything else orders
    /// nothing. Data without such a machine has no order the descent could
    /// be checked under, and its machine is refused.
    fn natural_order(&mut self, id: MachineId, data: u32) -> Checked<u32> {
        let refused = || {
            unserved(
                id,
                "recursive call at a computed value whose data has no natural order: no machine read so far is the truncated subtraction `saturating_sub` that spells it",
            )
        };
        let SignatureEntryKind::Data { shape, .. } = &self.signature.entries[data as usize].kind
        else {
            return Err(refused());
        };
        let shape = shape.clone();
        let zero = (0..shape.cases()).find(|&case| shape.arity(case) == 0);
        let successor = (0..shape.cases()).find(|&case| shape.arity(case) == 1);
        let (2, Some(zero), Some(successor), false) = (
            shape.cases(),
            zero,
            successor,
            shape.has_payload() || shape.is_mutual(),
        ) else {
            return Err(refused());
        };
        let functions: Vec<u32> = (0..self.signature.entries.len() as u32)
            .filter(|&entry| {
                matches!(
                    self.signature.entries[entry as usize].kind,
                    SignatureEntryKind::Function { .. }
                )
            })
            .collect();
        let defines = |converter: &Self, entry: u32, expected: TermHandle| {
            converter.signature.declarations[entry as usize]
                .body
                .is_some_and(|body| converter.signature.arena.structurally_equal(body, expected))
        };
        let pred = self.canonical_pred(&shape, data, zero, successor);
        for &candidate in &functions {
            if !defines(self, candidate, pred) {
                continue;
            }
            let monus = self.canonical_monus(&shape, data, zero, successor, candidate);
            if let Some(&order) = functions.iter().find(|&&entry| defines(self, entry, monus)) {
                return Ok(order);
            }
        }
        Err(refused())
    }

    /// Core's `pred` over the data at `data`, as `read_function` defines it:
    /// `λ a. a`'s cases, zero at zero and the field at a successor.
    fn canonical_pred(
        &mut self,
        shape: &PlainData,
        data: u32,
        zero: usize,
        successor: usize,
    ) -> TermHandle {
        let ty = self.constant(data);
        let motive = self.signature.arena.insert(Term::Lambda {
            domain: ty,
            body: ty,
        });
        let mut branches = vec![ty; shape.cases()];
        let at_zero = self.constant(data + 1 + zero as u32);
        branches[zero] = shape.branch(&mut self.signature.arena, data, motive, zero, at_zero);
        let field = shape.field(&mut self.signature.arena, successor, 0, 0);
        branches[successor] =
            shape.branch(&mut self.signature.arena, data, motive, successor, field);
        let tree = self.variable(0);
        let body = shape.induct(&mut self.signature.arena, data, motive, &branches, tree);
        self.signature
            .arena
            .insert(Term::Lambda { domain: ty, body })
    }

    /// Core's `saturating_sub(a, b)` over the data at `data`, as
    /// `read_function` defines it with `pred` at entry `pred`: by cases on
    /// `b`, a function of `a` that is `a` at zero and `pred` of the
    /// recursion at a successor.
    fn canonical_monus(
        &mut self,
        shape: &PlainData,
        data: u32,
        zero: usize,
        successor: usize,
        pred: u32,
    ) -> TermHandle {
        let ty = self.constant(data);
        let landing = self.signature.arena.insert(Term::Pi {
            domain: ty,
            codomain: ty,
        });
        let motive = self.signature.arena.insert(Term::Lambda {
            domain: ty,
            body: landing,
        });
        let mut branches = vec![ty; shape.cases()];
        let left = self.variable(0);
        let at_zero = self.signature.arena.insert(Term::Lambda {
            domain: ty,
            body: left,
        });
        branches[zero] = shape.branch(&mut self.signature.arena, data, motive, zero, at_zero);
        let recursion = shape
            .hypothesis(&mut self.signature.arena, successor, 0, 1)
            .expect("the successor's one field holds the data: data with a payload is no number");
        let left = self.variable(0);
        let recursion = self.signature.arena.apply_arguments(recursion, &[left]);
        let pred = self.constant(pred);
        let at_successor = self.signature.arena.apply_arguments(pred, &[recursion]);
        let at_successor = self.signature.arena.insert(Term::Lambda {
            domain: ty,
            body: at_successor,
        });
        branches[successor] = shape.branch(
            &mut self.signature.arena,
            data,
            motive,
            successor,
            at_successor,
        );
        let tree = self.variable(0);
        let induction = shape.induct(&mut self.signature.arena, data, motive, &branches, tree);
        let left = self.variable(1);
        let mut body = self.signature.arena.apply_arguments(induction, &[left]);
        for _ in 0..2 {
            body = self
                .signature
                .arena
                .insert(Term::Lambda { domain: ty, body });
        }
        body
    }

    /// A closed value of the type at `entry`: the first case of data that
    /// has no field, for a record one such value per field, and `false`.
    fn inhabitant(&mut self, id: MachineId, entry: u32) -> Checked<TermHandle> {
        match &self.signature.entries[entry as usize].kind {
            SignatureEntryKind::Boolean => Ok(self.signature.arena.insert(Term::TwoZero)),
            SignatureEntryKind::Data { shape, .. } => {
                let case = (0..shape.cases())
                    .find(|&case| shape.arity(case) == 0)
                    .ok_or_else(|| {
                        unserved(
                            id,
                            "mathematical machine recursing on fuel whose result has no case without fields",
                        )
                    })?;
                let constructor = shape.constructor_entry(entry, case);
                Ok(self.constant(constructor))
            }
            SignatureEntryKind::Record { fields, .. } => {
                let mut values = Vec::with_capacity(fields.len());
                for field in fields.clone() {
                    values.push(self.inhabitant(id, field)?);
                }
                let (&last, before) = values
                    .split_last()
                    .ok_or_else(|| unserved(id, "mathematical record with no field"))?;
                Ok(before.iter().rev().fold(last, |second, &first| {
                    self.signature.arena.insert(Term::Pair { first, second })
                }))
            }
            _ => Err(unserved(
                id,
                "mathematical machine recursing on fuel with a result that is no data",
            )),
        }
    }

    /// The value of `machine`'s body as a term of `valuing`'s scope. `slots`
    /// says what each formal stands for. A `let` is the term of its value. A
    /// call of a machine that states a law is evidence for the calls after
    /// it and nothing more. The body then ends in one value or one dispatch.
    fn body_value(
        &mut self,
        machine: MachineId,
        mut slots: Vec<Option<Bound>>,
        recursion: Option<Recursion<'_>>,
        valuing: &Valuing<'_>,
    ) -> Checked<TermHandle> {
        let body = self
            .bodies
            .get(machine)
            .ok_or_else(|| unserved(machine, "mathematical machine lost its body"))?;
        let (aliases, statements) = spine(body);
        slots.resize(slots.len().max(body.slots.len()), None);
        let Some((last, before)) = statements.split_last() else {
            return Err(unserved(
                machine,
                "mathematical function body with no value",
            ));
        };
        let mut script = LawCase {
            citations: valuing.cited.to_vec(),
            ..LawCase::default()
        };
        for statement in before {
            let (slot, expression) = match statement {
                Statement::Evaluate { expression, .. } => (None, expression),
                Statement::Bind { slot, value } => (Some(*slot), value),
                _ => {
                    return Err(unserved(
                        machine,
                        "mathematical function body with a statement before its value",
                    ));
                }
            };
            // A call of a machine that states a law is evidence. It is a
            // value as well only when that law is about the machine's own
            // result: the other laws return a value just so that they can be
            // called as statements.
            let (cites, valued) = match &expression.kind {
                ExpressionKind::Call {
                    callee: CallTarget::Machine(callee),
                    ..
                } if self.states_law(*callee) => (true, self.names_result(*callee)),
                _ => (false, true),
            };
            let cited = script.citations.clone();
            let here = Valuing {
                cited: &cited,
                ..*valuing
            };
            let scope = Scope {
                slots: &slots,
                aliases: &aliases,
                case: None,
                recursion,
                depth: 0,
                induction: false,
                value: Some(&here),
            };
            if !cites && slot.is_none() {
                return Err(unserved(
                    machine,
                    "mathematical function body with a statement before its value",
                ));
            }
            let evidence = cites.then(|| {
                self.cite_within(
                    valuing.owner,
                    machine,
                    expression,
                    &scope,
                    &mut script,
                    true,
                )
            });
            match (slot, valued) {
                (Some(slot), true) => {
                    // The value stands whether or not its law could be read.
                    let bound = self.expression(machine, expression, &scope)?;
                    *slots.get_mut(slot).ok_or_else(|| {
                        unserved(machine, "mathematical local outside its body's slots")
                    })? = Some(bound);
                }
                _ => evidence.transpose().map(|_| ())?,
            }
        }
        let here = Valuing {
            cited: &script.citations,
            ..*valuing
        };
        let scope = Scope {
            slots: &slots,
            aliases: &aliases,
            case: None,
            recursion,
            depth: 0,
            induction: false,
            value: Some(&here),
        };
        match last {
            Statement::Arm {
                guard: ArmGuard::Always,
                target: Target::Value(value),
                ..
            } => Ok(self.expression(machine, value, &scope)?.term),
            Statement::Dispatch {
                subject,
                arms,
                default: None,
                ..
            } => self.dispatch_value(machine, subject, arms, &scope, &here),
            _ => Err(unserved(
                machine,
                "mathematical function body other than one value or one dispatch",
            )),
        }
    }

    /// A dispatch read for its value: a definition of its own, by cases on
    /// its first argument, applied to the dispatch's subject and to every
    /// variable in scope. Declaring it lets the prover compute with it as
    /// with any definition by cases: once the subject is known to be a
    /// case, it is that case's value.
    fn dispatch_value(
        &mut self,
        machine: MachineId,
        subject: &Expression,
        arms: &[(CaseSelector, Target, crate::SourceOrigin)],
        scope: &Scope<'_>,
        valuing: &Valuing<'_>,
    ) -> Checked<TermHandle> {
        let ExpressionKind::Slot(slot) = subject.kind else {
            return Err(unserved(
                machine,
                "mathematical dispatch on a computed subject",
            ));
        };
        let slot = scope.aliases.get(slot).copied().unwrap_or(slot);
        let bound = scope
            .slots
            .get(slot)
            .copied()
            .flatten()
            .ok_or_else(|| unserved(machine, "mathematical dispatch on an unbound local"))?;
        let Some(ValueType::Record(record, ..)) = subject.result else {
            return Err(unserved(
                machine,
                "mathematical dispatch on a value without cases",
            ));
        };
        let data = self.data(machine, record)?;
        let SignatureEntryKind::Data { shape, .. } = &self.signature.entries[data as usize].kind
        else {
            return Err(unserved(
                machine,
                "mathematical dispatch on a value without cases",
            ));
        };
        let shape = shape.clone();
        if arms.len() != shape.cases() {
            return Err(unserved(
                machine,
                "mathematical dispatch without exactly one value arm per case",
            ));
        }
        let ty = self.constant(data);
        let outer = valuing.context.len() as u32;
        let mut landing = valuing.result;
        for &variable in valuing.context.iter().rev() {
            landing = self.signature.arena.insert(Term::Pi {
                domain: variable,
                codomain: landing,
            });
        }
        let motive = self.signature.arena.insert(Term::Lambda {
            domain: ty,
            body: landing,
        });
        let mut branches = Vec::with_capacity(shape.cases());
        for case in 0..shape.cases() {
            let mut selected = arms.iter().filter(|(selector, _, _)| selector.case == case);
            let (Some((selector, Target::Value(value), _)), None) =
                (selected.next(), selected.next())
            else {
                return Err(unserved(
                    machine,
                    "mathematical dispatch without exactly one value arm per case",
                ));
            };
            if selector.condition.is_some() {
                return Err(unserved(machine, "mathematical dispatch on a field value"));
            }
            // The branch binds the case's fields, then every variable of the
            // scope again in the same order, so the scope's terms are terms
            // of the branch as they stand.
            let arity = shape.arity(case);
            let arena = &mut self.signature.arena;
            let fields: Vec<_> = (0..arity)
                .map(|field| shape.field(arena, case, field, outer))
                .collect();
            let mut slots = scope.slots.to_vec();
            slots[slot] = Some(Bound {
                term: shape.case_value(arena, data, case, outer),
                field: None,
                applied: None,
            });
            // A dispatch is a case analysis: at a field of another member of
            // a mutual block its hypothesis is a `Two` nothing reads.
            let mut context = shape.field_types(&mut self.signature.arena, data, case);
            let two = self.signature.arena.insert(Term::Two);
            for field in 0..arity {
                match shape.field_member(case, field) {
                    Some(member) if member == shape.member() => context.push(landing),
                    Some(_) => context.push(two),
                    None => {}
                }
            }
            context.extend(valuing.context.iter().copied());
            // The terms of the scope are terms of the branch as they stand,
            // so the branch knows its subject is this case.
            let mut facts = valuing.facts.to_vec();
            facts.push(self.signature.arena.insert(Term::Id {
                ty,
                left: bound.term,
                right: slots[slot].expect("bound above").term,
            }));
            let inner = Valuing {
                context: &context,
                facts: &facts,
                ..*valuing
            };
            let branch_scope = Scope {
                slots: &slots,
                aliases: scope.aliases,
                case: Some((slot, case, &fields)),
                recursion: scope.recursion,
                depth: 0,
                induction: false,
                value: Some(&inner),
            };
            let mut branch = self.expression(machine, value, &branch_scope)?.term;
            for &variable in valuing.context.iter().rev() {
                branch = self.signature.arena.insert(Term::Lambda {
                    domain: variable,
                    body: branch,
                });
            }
            branches.push(shape.branch(&mut self.signature.arena, data, motive, case, branch));
        }
        let tree = self.variable(outer);
        let induction = shape.induct(&mut self.signature.arena, data, motive, &branches, tree);
        let variables: Vec<_> = (0..outer).map(|at| self.variable(outer - 1 - at)).collect();
        let mut term = self.signature.arena.apply_arguments(induction, &variables);
        let mut function = valuing.result;
        for &variable in valuing.context.iter().rev() {
            term = self.signature.arena.insert(Term::Lambda {
                domain: variable,
                body: term,
            });
            function = self.signature.arena.insert(Term::Pi {
                domain: variable,
                codomain: function,
            });
        }
        term = self.signature.arena.insert(Term::Lambda {
            domain: ty,
            body: term,
        });
        function = self.signature.arena.insert(Term::Pi {
            domain: ty,
            codomain: function,
        });
        index(machine, self.signature.declarations.len() + 1)?;
        let symbol = *self
            .program
            .symbols
            .get(machine)
            .ok_or_else(|| unserved(machine, "mathematical machine lost its declaration"))?;
        let name = format!("{} by cases", self.name(symbol));
        let entry = self.push(
            Declaration::definition(0, function, term),
            SignatureEntryKind::Function {
                machine: symbol,
                cases: Some(FunctionCases {
                    data,
                    subject: 0,
                    arity: outer + 1,
                    motive,
                    branches,
                }),
            },
            name,
        );
        let head = self.constant(entry);
        let mut operands = vec![bound.term];
        operands.extend(variables);
        Ok(self.signature.arena.apply_arguments(head, &operands))
    }

    /// What a call of `callee` at `operands` owes: each of `callee`'s
    /// premises at those arguments. One that is a premise of the machine
    /// being defined is in hand. Any other is a statement of its own, over
    /// every variable in scope and under the defining machine's premises and
    /// the facts of the enclosing dispatches, proved from what the body has
    /// cited so far. A term outside a definition has nowhere to prove one,
    /// so such a call is refused there.
    fn owed(
        &mut self,
        id: MachineId,
        callee: MachineId,
        operands: &[Bound],
        scope: &Scope<'_>,
    ) -> Checked<()> {
        let program = self.program;
        let called = program
            .signatures
            .get(callee)
            .ok_or_else(|| unserved(id, "call lost its machine's signature"))?;
        if called.requires.is_empty() {
            return Ok(());
        }
        let Some(valuing) = scope.value else {
            return Err(unserved(
                id,
                "term outside a definition calls a machine that has a premise",
            ));
        };
        let callee_name = program
            .symbols
            .get(callee)
            .map(|symbol| self.name(*symbol))
            .unwrap_or_default();
        let actuals: Vec<_> = operands.iter().map(|operand| operand.term).collect();
        for premise in &called.requires {
            let goal = self.equality(callee, premise, &actuals, None)?;
            if valuing
                .premises
                .iter()
                .any(|&held| self.signature.arena.structurally_equal(held, goal))
            {
                continue;
            }
            let source = program
                .names
                .get(valuing.owner)
                .and_then(|names| names.declared);
            self.owe_statement(
                id,
                valuing,
                goal,
                format!("the premise of `{callee_name}` at its call"),
                source,
            )?;
        }
        Ok(())
    }

    /// What a call of `callee` at `actuals` owes: each of `premises`, terms
    /// under its parameters, instantiated at them and proved from what the
    /// body holds there.
    fn owed_at(
        &mut self,
        id: MachineId,
        callee: MachineId,
        premises: &[TermHandle],
        actuals: &[TermHandle],
        scope: &Scope<'_>,
    ) -> Checked<()> {
        if premises.is_empty() {
            return Ok(());
        }
        let Some(valuing) = scope.value else {
            return Err(unserved(
                id,
                "term outside a definition calls a machine that has a premise",
            ));
        };
        let program = self.program;
        let callee_name = program
            .symbols
            .get(callee)
            .map(|symbol| self.name(*symbol))
            .unwrap_or_default();
        for &premise in premises {
            let goal = instantiate(&mut self.signature.arena, premise, actuals);
            if valuing
                .premises
                .iter()
                .any(|&held| self.signature.arena.structurally_equal(held, goal))
            {
                continue;
            }
            let source = program
                .names
                .get(valuing.owner)
                .and_then(|names| names.declared);
            self.owe_statement(
                id,
                valuing,
                goal,
                format!("the premise of `{callee_name}` at its call"),
                source,
            )?;
        }
        Ok(())
    }

    /// What a recursive call at `operands` of a machine defined on fuel
    /// owes: its ranked argument strictly below the caller's in natural
    /// order, instantiated at the call's actual arguments and proved from
    /// the premises, facts and citations in scope there. A call at the
    /// caller's own ranked formal, or at anything the body cannot show to
    /// be smaller, leaves the statement unproved, and the refusal names that
    /// call (`wiki/spec/language/termination.md`, "Computed recursive
    /// arguments in proofs").
    fn descends(
        &mut self,
        id: MachineId,
        descent: Descent,
        operands: &[Bound],
        call: &Expression,
        scope: &Scope<'_>,
    ) -> Checked<()> {
        let Some(valuing) = scope.value else {
            return Err(unserved(id, "recursive call outside a definition"));
        };
        let next = operands
            .get(descent.measure)
            .ok_or_else(|| unserved(id, "recursive call without its ranked argument"))?
            .term;
        let SignatureEntryKind::Data { shape, .. } =
            &self.signature.entries[descent.nat as usize].kind
        else {
            return Err(unserved(id, "natural order over a value without cases"));
        };
        let cases = shape.cases();
        let zero = (0..cases).find(|&case| shape.arity(case) == 0);
        let successor = (0..cases).find(|&case| shape.arity(case) == 1);
        let (Some(zero), Some(successor), false) =
            (zero, successor, shape.has_payload() || shape.is_mutual())
        else {
            return Err(unserved(
                id,
                "natural order over data without zero and successor",
            ));
        };
        let order = self.natural_order(id, descent.nat)?;
        let nat = self.constant(descent.nat);
        let zero = self.constant(descent.nat + 1 + zero as u32);
        let successor = self.constant(descent.nat + 1 + successor as u32);
        let saturating_sub = self.constant(order);
        let arena = &mut self.signature.arena;
        let above = arena.apply_arguments(successor, &[next]);
        let left = arena.apply_arguments(saturating_sub, &[above, descent.ranked]);
        let goal = arena.insert(Term::Id {
            ty: nat,
            left,
            right: zero,
        });
        let owner = self
            .program
            .symbols
            .get(valuing.owner)
            .map(|symbol| self.name(*symbol))
            .unwrap_or_default();
        let source = call.source.span().or_else(|| {
            self.program
                .names
                .get(valuing.owner)
                .and_then(|names| names.declared)
        });
        self.owe_statement(
            id,
            valuing,
            goal,
            format!("the descent of `{owner}` at its recursive call"),
            source,
        )
    }

    /// `goal` as a statement of its own, over every variable of `valuing`'s
    /// scope and under its premises and facts, proved from what the body has
    /// cited so far. A body read twice, once per way of defining it, owes
    /// each statement once.
    fn owe_statement(
        &mut self,
        id: MachineId,
        valuing: &Valuing<'_>,
        goal: TermHandle,
        text: String,
        source: Option<source_text::SourceSpan>,
    ) -> Checked<()> {
        let mut hypotheses = valuing.premises.to_vec();
        hypotheses.extend_from_slice(valuing.facts);
        let mut statement = self.implication(&hypotheses, goal);
        for &variable in valuing.context.iter().rev() {
            statement = self.signature.arena.insert(Term::Pi {
                domain: variable,
                codomain: statement,
            });
        }
        let owner = *self
            .program
            .symbols
            .get(valuing.owner)
            .ok_or_else(|| unserved(id, "mathematical machine lost its declaration"))?;
        if self.signature.laws.iter().any(|law| {
            law.machine == owner
                && self.signature.arena.structurally_equal(
                    self.signature.declarations[law.statement as usize].ty,
                    statement,
                )
        }) {
            return Ok(());
        }
        let law = index(id, self.signature.laws.len())?;
        index(id, self.signature.declarations.len() + 1)?;
        let owner_name = self.name(owner);
        let entry = self.push(
            Declaration::assumption(0, statement),
            SignatureEntryKind::Statement { law },
            owner_name.clone(),
        );
        self.signature.laws.push(Law {
            machine: owner,
            clause: 0,
            statement: entry,
            formals: valuing.context.to_vec(),
            premises: hypotheses,
            goal,
            declared: None,
            proof: LawProof::Direct(LawCase {
                citations: valuing.cited.to_vec(),
                ..LawCase::default()
            }),
            formal_names: Vec::new(),
            machine_name: owner_name,
            text,
            source,
        });
        Ok(())
    }

    /// A recursive machine must rank on the formal it dispatches on: the
    /// kernel's induction descends on that formal and no other.
    fn ranked_on(&self, id: MachineId, subject: usize, required: bool) -> Checked<()> {
        let ranking = self
            .program
            .signatures
            .get(id)
            .and_then(|signature| signature.ranking.as_ref());
        match ranking {
            None if required => Err(unserved(
                id,
                "recursive mathematical machine without `terminates by` its dispatched formal",
            )),
            None => Ok(()),
            Some(ranking)
                if matches!(ranking.measures.as_slice(),
                    [measure] if measure.kind == ExpressionKind::Slot(subject)) =>
            {
                Ok(())
            }
            Some(_) => Err(unserved(
                id,
                "mathematical machine ranked on a formal it does not dispatch on",
            )),
        }
    }

    /// A value expression of a mathematical body as a term.
    fn expression(
        &mut self,
        id: MachineId,
        expression: &Expression,
        scope: &Scope<'_>,
    ) -> Checked<Bound> {
        let plain = |term| Bound {
            term,
            field: None,
            applied: None,
        };
        match &expression.kind {
            ExpressionKind::Boolean(value) => Ok(plain(self.signature.arena.insert(if *value {
                Term::TwoOne
            } else {
                Term::TwoZero
            }))),
            // A fixed-width literal is the integer it holds, as a payload or
            // formal of that type is (`value_ty`).
            ExpressionKind::Integer(literal) => {
                let value = match *literal {
                    IntegerValue::Signed(value) => bignum::BigInt::from_i128(value),
                    IntegerValue::Unsigned(value) => bignum::BigInt::from_u128(value),
                };
                self.declare_integers(id)?;
                Ok(plain(self.integer_literal(id, value)?))
            }
            ExpressionKind::Slot(slot) => scope
                .slots
                .get(scope.aliases.get(*slot).copied().unwrap_or(*slot))
                .copied()
                .flatten()
                .ok_or_else(|| {
                    unserved(
                        id,
                        "mathematical term names the dispatched formal or a local",
                    )
                }),
            ExpressionKind::Payload { base, case, field } => match (&base.kind, scope.case) {
                (ExpressionKind::Slot(slot), Some((subject, selected, fields)))
                    if scope.aliases.get(*slot).copied().unwrap_or(*slot) == subject
                        && *case == selected
                        && *field < fields.len() =>
                {
                    Ok(Bound {
                        term: fields[*field],
                        field: scope.induction.then_some(*field as u32),
                        applied: None,
                    })
                }
                _ => Err(unserved(
                    id,
                    "mathematical term reads a field outside the dispatched case",
                )),
            },
            ExpressionKind::Field { base, field } => {
                let Some(ValueType::Record(record, ..)) = base.result else {
                    return Err(unserved(
                        id,
                        "field of a mathematical value that is no record",
                    ));
                };
                let entry = self.data(id, record)?;
                let value = self.expression(id, base, scope)?.term;
                Ok(plain(self.component(id, entry, value, *field)?.0))
            }
            ExpressionKind::Record { fields, .. } => {
                let mut operands = vec![None; fields.len()];
                for (position, operand) in fields {
                    let term = self.expression(id, operand, scope)?.term;
                    *operands
                        .get_mut(*position)
                        .ok_or_else(|| unserved(id, "mathematical record field position"))? =
                        Some(term);
                }
                let operands: Vec<TermHandle> = operands
                    .into_iter()
                    .collect::<Option<_>>()
                    .ok_or_else(|| unserved(id, "mathematical record misses a field"))?;
                let (&last, before) = operands
                    .split_last()
                    .ok_or_else(|| unserved(id, "mathematical record with no field"))?;
                let mut term = last;
                for &first in before.iter().rev() {
                    term = self.signature.arena.insert(Term::Pair {
                        first,
                        second: term,
                    });
                }
                Ok(plain(term))
            }
            ExpressionKind::Case { case, fields, .. } => {
                let Some(ValueType::Record(record, ..)) = expression.result else {
                    return Err(unserved(id, "mathematical constructor lost its data"));
                };
                let data = self.data(id, record)?;
                let mut operands = vec![None; fields.len()];
                for (position, operand) in fields {
                    let term = self.expression(id, operand, scope)?.term;
                    *operands
                        .get_mut(*position)
                        .ok_or_else(|| unserved(id, "mathematical constructor field position"))? =
                        Some(term);
                }
                let operands: Vec<TermHandle> = operands
                    .into_iter()
                    .collect::<Option<_>>()
                    .ok_or_else(|| unserved(id, "mathematical constructor misses a field"))?;
                let constructor = match &self.signature.entries[data as usize].kind {
                    SignatureEntryKind::Data { shape, .. } => shape.constructor_entry(data, *case),
                    _ => data + 1 + *case as u32,
                };
                let constructor = self.constant(constructor);
                Ok(plain(
                    self.signature.arena.apply_arguments(constructor, &operands),
                ))
            }
            // A call of a stored callable is the function applied. A field
            // that holds a function to the data, applied, is the child there,
            // which a recursion may descend to.
            ExpressionKind::Call {
                callee: CallTarget::Selected { callable, .. },
                arguments,
                ..
            } => {
                let function = self.expression(id, callable, scope)?;
                let mut operands = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    operands.push(self.expression(id, argument, scope)?.term);
                }
                let term = self
                    .signature
                    .arena
                    .apply_arguments(function.term, &operands);
                Ok(
                    match (function.field, function.applied, operands.as_slice()) {
                        (Some(field), None, &[argument]) => Bound {
                            term,
                            field: Some(field),
                            applied: Some(argument),
                        },
                        _ => plain(term),
                    },
                )
            }
            ExpressionKind::Applied {
                head,
                machines,
                arguments,
            } => {
                let outer = callable_shift(scope);
                let head = match *head {
                    AppliedHead::Parameter(symbol) => {
                        let mut operands = Vec::with_capacity(arguments.len());
                        for argument in arguments {
                            operands.push(self.expression(id, argument, scope)?.term);
                        }
                        let callable = self.callable_term(id, symbol, outer)?;
                        return Ok(plain(
                            self.signature.arena.apply_arguments(callable, &operands),
                        ));
                    }
                    AppliedHead::Machine(callee) => callee,
                };
                let own = scope
                    .value
                    .is_some_and(|valuing| valuing.fuel.is_some_and(|fuel| fuel.machine == head))
                    || scope
                        .recursion
                        .is_some_and(|recursion| recursion.machine == head);
                if own {
                    // The machine calling itself, at its own machine
                    // parameters or others: the recursion's function takes
                    // the machines first (`by_cases`, `by_fuel`).
                    let mut operands = Vec::with_capacity(arguments.len());
                    for argument in arguments {
                        operands.push(self.expression(id, argument, scope)?);
                    }
                    let passing = self.passed(id, machines, outer)?;
                    let mut actuals = passing.clone();
                    actuals.extend(operands.iter().map(|operand| operand.term));
                    let premises = self
                        .generic_premises
                        .iter()
                        .find(|(machine, _)| *machine == head)
                        .map(|(_, premises)| premises.clone())
                        .ok_or_else(|| {
                            unserved(id, "generic mathematical machine lost its premises")
                        })?;
                    self.owed_at(id, head, &premises, &actuals, scope)?;
                    if let Some(fuel) = scope.value.and_then(|valuing| valuing.fuel)
                        && fuel.machine == head
                    {
                        if let Some(descent) = fuel.descent {
                            self.descends(id, descent, &operands, expression, scope)?;
                        }
                        return Ok(plain(match fuel.step {
                            Some(step) => self.signature.arena.apply_arguments(step, &actuals),
                            None => fuel.default,
                        }));
                    }
                    let Some(Recursion {
                        machine,
                        subject,
                        results,
                        ..
                    }) = scope.recursion
                    else {
                        return Err(unserved(id, "recursive call outside its recursion"));
                    };
                    let field = operands
                        .get(subject)
                        .and_then(|operand| operand.field)
                        .ok_or_else(|| {
                            unserved(
                                id,
                                "recursive call whose ranked argument is not a field of the dispatched case",
                            )
                        })?;
                    self.ranked_on(machine, subject, true)?;
                    let mut rest = passing;
                    rest.extend(
                        operands
                            .iter()
                            .enumerate()
                            .filter(|(position, _)| *position != subject)
                            .map(|(_, operand)| operand.term),
                    );
                    let result =
                        results
                            .get(field as usize)
                            .copied()
                            .flatten()
                            .ok_or_else(|| {
                                unserved(
                                    id,
                                    "recursive call at a field that does not hold the data",
                                )
                            })?;
                    return Ok(plain(self.signature.arena.apply_arguments(result, &rest)));
                }
                let mut actuals = self.passed(id, machines, outer)?;
                for argument in arguments {
                    actuals.push(self.expression(id, argument, scope)?.term);
                }
                let function = self.function(head)?;
                // Its premises, stated over its machine parameters and
                // formals, are owed at the machines and arguments passed.
                let premises = self
                    .generic_premises
                    .iter()
                    .find(|(machine, _)| *machine == head)
                    .map(|(_, premises)| premises.clone())
                    .ok_or_else(|| {
                        unserved(id, "generic mathematical machine lost its premises")
                    })?;
                self.owed_at(id, head, &premises, &actuals, scope)?;
                let function = self.constant(function);
                Ok(plain(
                    self.signature.arena.apply_arguments(function, &actuals),
                ))
            }
            ExpressionKind::Call {
                callee: CallTarget::Machine(callee),
                arguments,
                ..
            } => {
                let mut operands = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    operands.push(self.expression(id, argument, scope)?);
                }
                if let Some(fuel) = scope.value.and_then(|valuing| valuing.fuel)
                    && fuel.machine == *callee
                {
                    self.owed(id, *callee, &operands, scope)?;
                    if let Some(descent) = fuel.descent {
                        self.descends(id, descent, &operands, expression, scope)?;
                    }
                    let actuals: Vec<_> = operands.iter().map(|operand| operand.term).collect();
                    return Ok(plain(match fuel.step {
                        Some(step) => self.signature.arena.apply_arguments(step, &actuals),
                        None => fuel.default,
                    }));
                }
                if let Some(recursion) = scope.recursion
                    && let Some(member) = recursion.calls(*callee)
                {
                    let (subject, results) = (recursion.subject, recursion.results);
                    self.owed(id, *callee, &operands, scope)?;
                    let field = operands
                        .get(subject)
                        .and_then(|operand| operand.field)
                        .ok_or_else(|| {
                            unserved(
                                id,
                                "recursive call whose ranked argument is not a field of the dispatched case",
                            )
                        })?;
                    // A machine of a mutual group recurses on its own member:
                    // the field must hold that member.
                    if member.is_some()
                        && recursion.members.get(field as usize).copied().flatten() != member
                    {
                        return Err(unserved(
                            id,
                            "mutual recursive call at a field of another member of the block",
                        ));
                    }
                    self.ranked_on(*callee, subject, true)?;
                    let rest: Vec<_> = operands
                        .iter()
                        .enumerate()
                        .filter(|(position, _)| *position != subject)
                        .map(|(_, operand)| operand.term)
                        .collect();
                    let result =
                        results
                            .get(field as usize)
                            .copied()
                            .flatten()
                            .ok_or_else(|| {
                                unserved(
                                    id,
                                    "recursive call at a field that does not hold the data",
                                )
                            })?;
                    // At a field that holds a function to the data, the
                    // recursion is at the child the call applies it to.
                    let result = match operands[subject].applied {
                        Some(argument) => self.signature.arena.insert(Term::Apply {
                            function: result,
                            argument,
                        }),
                        None => result,
                    };
                    return Ok(plain(self.signature.arena.apply_arguments(result, &rest)));
                }
                if self.helper(*callee)
                    && let Some(valuing) = scope.value
                {
                    // Read in place, whatever its body is, and no longer a
                    // machine nothing calls.
                    self.laws(*callee)?;
                    *self.machines.get_mut(*callee).expect("row read by `laws`") =
                        Read::Cites { called: true };
                    let slots: Vec<_> = operands.iter().copied().map(Some).collect();
                    return self
                        .body_value(*callee, slots, scope.recursion, valuing)
                        .map(plain);
                }
                if self.helper(*callee) {
                    let body = self
                        .bodies
                        .get(*callee)
                        .ok_or_else(|| unserved(id, "mathematical machine lost its body"))?;
                    let (aliases, statements) = spine(body);
                    let Some((
                        Statement::Arm {
                            guard: ArmGuard::Always,
                            target: Target::Value(value),
                            ..
                        },
                        before,
                    )) = statements.split_last()
                    else {
                        return Err(unserved(
                            *callee,
                            "nested mathematical machine body other than citations and one value",
                        ));
                    };
                    if !before
                        .iter()
                        .all(|statement| matches!(statement, Statement::Evaluate { .. }))
                    {
                        return Err(unserved(
                            *callee,
                            "nested mathematical machine body other than citations and one value",
                        ));
                    }
                    let slots: Vec<_> = operands.iter().copied().map(Some).collect();
                    let inner = Scope {
                        slots: &slots,
                        aliases: &aliases,
                        case: None,
                        recursion: scope.recursion,
                        depth: 0,
                        induction: true,
                        value: None,
                    };
                    return self.expression(*callee, value, &inner);
                }
                let function = self.function(*callee)?;
                self.owed(id, *callee, &operands, scope)?;
                let function = self.constant(function);
                let operands: Vec<_> = operands.iter().map(|operand| operand.term).collect();
                Ok(plain(
                    self.signature.arena.apply_arguments(function, &operands),
                ))
            }
            // A stored selection of a mathematical machine is its definition,
            // the function itself. Nothing calls it here, so a premise it
            // owes at a call has no arguments to be proved at.
            ExpressionKind::MachineValue { descriptor, .. } => {
                let selected = self
                    .program
                    .callables
                    .descriptors
                    .get(*descriptor)
                    .map(|selection| selection.machine)
                    .ok_or_else(|| unserved(id, "machine value lost its selection"))?;
                if self
                    .program
                    .signatures
                    .get(selected)
                    .is_none_or(|signature| !signature.requires.is_empty())
                {
                    return Err(unserved(
                        id,
                        "mathematical machine value whose machine has a premise",
                    ));
                }
                let function = self.function(selected)?;
                Ok(plain(self.constant(function)))
            }
            _ => Err(unserved(
                id,
                "mathematical term other than a Boolean, a formal, a field, a constructor or a call",
            )),
        }
    }

    /// The laws a machine with no result states, declaring them the first
    /// time. A machine that states none only cites; it is read at its call.
    fn laws(&mut self, id: MachineId) -> Checked<Vec<u32>> {
        if self.lifted_operation(id).is_some() {
            return Err(unserved(
                id,
                "citation of an operation lifted to a quotient",
            ));
        }
        match self.begin(id, false, "law that reaches itself through its citations")? {
            None => {}
            Some(Read::Laws(laws)) => return Ok(laws),
            Some(Read::Cites { .. }) => return Ok(Vec::new()),
            Some(_) => return Err(unserved(id, "citation of a machine with a result")),
        }
        let outer = (
            std::mem::take(&mut self.callable_scope),
            std::mem::take(&mut self.held),
        );
        let read = self.read_laws(id);
        (self.callable_scope, self.held) = outer;
        self.finish(
            id,
            false,
            read.map(|laws| {
                let state = if laws.is_empty() {
                    Read::Cites { called: false }
                } else {
                    Read::Laws(laws.clone())
                };
                (laws, state)
            }),
        )
    }

    fn read_laws(&mut self, id: MachineId) -> Checked<Vec<u32>> {
        let program = self.program;
        let signature = program
            .signatures
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its signature"))?;
        let names = program
            .names
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its names"))?;
        let formals = self.formals(id)?;
        let count = formals.len();
        let open: Vec<_> = (0..count)
            .map(|formal| self.variable((count - 1 - formal) as u32))
            .collect();
        let template = self.template(id);
        let callables = self.enter(id, count)?;
        // The premises first, each resting on the ones before it: a clause
        // may call a machine that has a premise only where that premise is
        // one of this machine's own.
        let mut premises = Vec::with_capacity(signature.requires.len());
        for premise in &signature.requires {
            let premise = self.equality(id, premise, &open, None)?;
            premises.push(premise);
            self.held.push(premise);
        }
        let satisfaction = self
            .satisfactions
            .iter()
            .find(|satisfaction| satisfaction.machine == id);
        let mut declared = Vec::new();
        if let Some(satisfaction) = satisfaction {
            for clause in &satisfaction.ensures {
                declared.push(self.equality(id, clause, &open, Some(satisfaction))?);
            }
        }
        // The machine's own equalities, else the ones its requirement declares.
        let mut goals = Vec::new();
        for (clause, proposition) in signature.ensures.iter().enumerate() {
            let goal = self.equality(id, proposition, &open, None)?;
            let text = names
                .ensures
                .get(clause)
                .map(|text| (names.authored(text), text.source));
            goals.push((goal, text));
        }
        let own = !goals.is_empty();
        let declared = match (own, declared.as_slice()) {
            (false, _) => {
                goals = declared.into_iter().map(|goal| (goal, None)).collect();
                None
            }
            (true, []) => None,
            (true, [declared]) if goals.len() == 1 => Some(*declared),
            (true, _) => {
                return Err(unserved(
                    id,
                    "law with several equalities satisfying a requirement",
                ));
            }
        };
        if goals.is_empty() {
            return Ok(Vec::new());
        }
        let proof = match template.filter(|template| !template.lowered) {
            // A machine generic over machines argues by its citations
            // alone, read as terms with its contract.
            Some(template) => {
                let (terms, _) = self.operands(id, &template.values, &open, None)?;
                let mut script = LawCase::default();
                for (cited, arguments) in &template.citations {
                    let (machine, _) = program
                        .symbols
                        .iter()
                        .find(|(_, known)| **known == *cited)
                        .ok_or_else(|| unserved(id, "citation lost its machine"))?;
                    let laws = self.laws(machine)?;
                    if laws.is_empty() {
                        return Err(unserved(
                            id,
                            "generic mathematical machine cites a machine that states no law",
                        ));
                    }
                    for law in laws {
                        script.citations.push(Citation {
                            evidence: Evidence::Statement(
                                self.signature.laws[law as usize].statement,
                            ),
                            arguments: arguments
                                .iter()
                                .map(|&argument| terms[argument as usize])
                                .collect(),
                        });
                    }
                }
                LawProof::Direct(script)
            }
            None => {
                let body = self
                    .bodies
                    .get(id)
                    .ok_or_else(|| unserved(id, "mathematical machine lost its body"))?;
                self.script(id, body, &formals)?
            }
        };
        let mut over_fuel = Vec::new();
        if matches!(proof, LawProof::Fuel { .. }) {
            if !own {
                return Err(unserved(
                    id,
                    "law of a machine defined on fuel that only its requirement states",
                ));
            }
            let &(_, definition, ..) = self
                .fueled
                .iter()
                .find(|(fueled, ..)| *fueled == id)
                .expect("`script` found the machine defined on fuel");
            // The machine's result with `fuel` units: the definition over
            // fuel at a variable bound outside the formals and any machine
            // parameters, which it takes next.
            let head = self.constant(definition);
            let mut operands = vec![self.variable((count + callables.len()) as u32)];
            operands.extend(self.callable_actuals(0));
            operands.extend(open.iter().copied());
            let result = self.signature.arena.apply_arguments(head, &operands);
            self.result_as = Some((id, result));
            let read: Checked<Vec<_>> = signature
                .ensures
                .iter()
                .map(|proposition| self.equality(id, proposition, &open, None))
                .collect();
            self.result_as = None;
            over_fuel = read?;
        }
        let symbol = *program
            .symbols
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its declaration"))?;
        let mut laws = Vec::with_capacity(goals.len());
        for (clause, (goal, text)) in goals.into_iter().enumerate() {
            let law = index(id, self.signature.laws.len())?;
            index(id, self.signature.declarations.len() + 1)?;
            // For all formals, given a proof of each premise, the equality.
            let mut statement = self.implication(&premises, goal);
            for &domain in formals
                .iter()
                .map(|(formal, _)| formal)
                .rev()
                .chain(callables.iter().rev())
            {
                statement = self.signature.arena.insert(Term::Pi {
                    domain,
                    codomain: statement,
                });
            }
            let entry = self.push(
                Declaration::assumption(0, statement),
                SignatureEntryKind::Statement { law },
                names.machine.clone(),
            );
            let (text, source) = text.unwrap_or_default();
            self.signature.laws.push(Law {
                machine: symbol,
                clause: clause as u32,
                statement: entry,
                // A machine parameter is a formal of the law like any other,
                // bound before the value formals.
                formals: callables
                    .iter()
                    .copied()
                    .chain(formals.iter().map(|(formal, _)| *formal))
                    .collect(),
                premises: premises.clone(),
                goal,
                declared: declared.filter(|_| clause == 0).map(|goal| DeclaredLaw {
                    premises: Vec::new(),
                    goal,
                    refusal: "the proved equality is not the law its requirement declares"
                        .to_owned(),
                }),
                proof: match &proof {
                    LawProof::Fuel {
                        data,
                        formal,
                        script,
                        ..
                    } => LawProof::Fuel {
                        data: *data,
                        formal: *formal,
                        goal: over_fuel[clause],
                        script: script.clone(),
                    },
                    other => other.clone(),
                },
                formal_names: template
                    .iter()
                    .flat_map(|template| &template.callables)
                    .map(|callable| callable.name.clone())
                    .chain(names.formals.iter().cloned())
                    .collect(),
                machine_name: names.machine.clone(),
                text,
                source: source.or(names.declared),
            });
            laws.push(law);
        }
        Ok(laws)
    }

    /// One `ensures` equality under the law's formals.
    fn equality(
        &mut self,
        id: MachineId,
        proposition: &Proposition,
        formals: &[TermHandle],
        satisfaction: Option<&Satisfaction>,
    ) -> Checked<TermHandle> {
        if let Proposition::IsMere(inner) = proposition {
            let prop = self.equality(id, inner, formals, satisfaction)?;
            let arena = &mut self.signature.arena;
            let prop_1 = mathematical_kernel::shift(arena, prop, 0, 1);
            let prop_2 = mathematical_kernel::shift(arena, prop, 0, 2);
            let p = arena.insert(Term::Variable(1));
            let q = arena.insert(Term::Variable(0));
            let eq = arena.insert(Term::Id {
                ty: prop_2,
                left: p,
                right: q,
            });
            let inner_pi = arena.insert(Term::Pi {
                domain: prop_1,
                codomain: eq,
            });
            return Ok(arena.insert(Term::Pi {
                domain: prop,
                codomain: inner_pi,
            }));
        }
        let Proposition::DeclarationIdentity(clause) = proposition else {
            return Err(unserved(id, "law guarantee other than an Identity"));
        };
        if clause.is_float_meaning() {
            return Err(unserved(id, "law guarantee other than an Identity"));
        }
        self.identity(id, clause, formals, satisfaction)
    }

    fn identity(
        &mut self,
        id: MachineId,
        clause: &ClauseIdentity,
        formals: &[TermHandle],
        satisfaction: Option<&Satisfaction>,
    ) -> Checked<TermHandle> {
        clause
            .validate()
            .map_err(|_| unserved(id, "malformed declaration Identity"))?;
        let (terms, carriers) = self.operands(id, &clause.values, formals, satisfaction)?;
        let ty = self.constant(carriers[clause.left as usize]);
        Ok(self.signature.arena.insert(Term::Id {
            ty,
            left: terms[clause.left as usize],
            right: terms[clause.right as usize],
        }))
    }

    /// The generic machine body read for `id`, when it is one
    /// (`read::templates`).
    fn template(&self, id: MachineId) -> Option<&'p super::templates::TemplateBody> {
        let program = self.program;
        program
            .template_bodies
            .iter()
            .find(|(template, _)| *template == id)
            .map(|(_, body)| body)
    }

    /// Start reading machine `id`, which has `count` formals: the type of
    /// each of its machine parameters, in declared order, each now in scope
    /// as a variable bound before the formals.
    fn enter(&mut self, id: MachineId, count: usize) -> Checked<Vec<TermHandle>> {
        let Some(template) = self.template(id) else {
            return Ok(Vec::new());
        };
        let parameters = template.callables.len();
        let mut types = Vec::with_capacity(parameters);
        for (position, callable) in template.callables.iter().enumerate() {
            let (mut ty, _) = self.ty(id, callable.result)?;
            for &parameter in callable.parameters.iter().rev() {
                let (domain, _) = self.ty(id, parameter)?;
                ty = self.signature.arena.insert(Term::Pi {
                    domain,
                    codomain: ty,
                });
            }
            types.push(ty);
            let variable = self.variable((count + parameters - 1 - position) as u32);
            self.callable_scope.push((callable.symbol, variable));
        }
        Ok(types)
    }

    /// The machine parameter `symbol` of the machine being read, in a
    /// scope that binds `outer` variables inside its formals: its variable,
    /// or the machine a law passed it where the law reads it in place.
    fn callable_term(
        &mut self,
        id: MachineId,
        symbol: SymbolHandle,
        outer: u32,
    ) -> Checked<TermHandle> {
        let term = self
            .callable_scope
            .iter()
            .find(|(callable, _)| *callable == symbol)
            .map(|(_, term)| *term)
            .ok_or_else(|| unserved(id, "machine parameter outside the machine that binds it"))?;
        Ok(shift(&mut self.signature.arena, term, 0, outer))
    }

    /// Every machine parameter of the machine being read, outermost first,
    /// in a scope that binds `outer` variables inside its formals.
    fn callable_actuals(&mut self, outer: u32) -> Vec<TermHandle> {
        let terms: Vec<TermHandle> = self.callable_scope.iter().map(|(_, term)| *term).collect();
        terms
            .into_iter()
            .map(|term| shift(&mut self.signature.arena, term, 0, outer))
            .collect()
    }

    /// The machines an application passes, as terms of a scope that binds
    /// `outer` variables inside the formals of the machine being read: a
    /// machine parameter is its variable, a concrete machine its
    /// definition.
    fn passed(
        &mut self,
        id: MachineId,
        machines: &[crate::body::PassedMachine],
        outer: u32,
    ) -> Checked<Vec<TermHandle>> {
        let mut terms = Vec::with_capacity(machines.len());
        for machine in machines {
            terms.push(match *machine {
                crate::body::PassedMachine::Parameter(symbol) => {
                    self.callable_term(id, symbol, outer)?
                }
                crate::body::PassedMachine::Machine(machine) => {
                    let function = self.function(machine)?;
                    self.constant(function)
                }
            });
        }
        Ok(terms)
    }

    /// What a call of the machine `member` at `operands` in a clause owes:
    /// each premise of that machine at those arguments must be one of the
    /// premises the machine being read already has. A clause has no body to
    /// prove a premise in.
    fn owed_in_clause(
        &mut self,
        id: MachineId,
        member: SymbolHandle,
        operands: &[TermHandle],
    ) -> Checked<()> {
        if self.stating_premises {
            return Ok(());
        }
        let program = self.program;
        let Some((machine, _)) = program.symbols.iter().find(|(_, known)| **known == member) else {
            return Ok(());
        };
        let called = program
            .signatures
            .get(machine)
            .ok_or_else(|| unserved(id, "call lost its machine's signature"))?;
        if called.requires.is_empty() {
            return Ok(());
        }
        // The called machine's own machine parameters stand for the
        // functions passed for them, which come first.
        let passed = self
            .template(machine)
            .map_or(&[][..], |template| template.callables.as_slice());
        if operands.len() < passed.len() {
            return Err(unserved(id, "generic machine applied to too few machines"));
        }
        let (functions, values) = operands.split_at(passed.len());
        let outer = std::mem::replace(
            &mut self.callable_scope,
            passed
                .iter()
                .zip(functions)
                .map(|(callable, &function)| (callable.symbol, function))
                .collect(),
        );
        self.stating_premises = true;
        let stated: Checked<Vec<_>> = called
            .requires
            .iter()
            .map(|premise| self.equality(machine, premise, values, None))
            .collect();
        self.stating_premises = false;
        self.callable_scope = outer;
        for premise in stated? {
            if !self
                .held
                .iter()
                .any(|&held| self.signature.arena.structurally_equal(held, premise))
            {
                return Err(unserved(
                    id,
                    "clause calls a machine whose premise is not a premise of the machine that states it",
                ));
            }
        }
        Ok(())
    }

    /// Each operand of a clause as a term under `formals`, and the entry of
    /// each one's carrier.
    fn operands(
        &mut self,
        id: MachineId,
        values: &[ClauseValue],
        formals: &[TermHandle],
        satisfaction: Option<&Satisfaction>,
    ) -> Checked<(Vec<TermHandle>, Vec<u32>)> {
        let mut terms = Vec::<TermHandle>::with_capacity(values.len());
        let mut carriers = Vec::with_capacity(values.len());
        for value in values {
            carriers.push(match value.carrier {
                ClauseCarrier::Structural(record) => self.data(id, record)?,
                ClauseCarrier::Scalar(ScalarType::Boolean) => self.boolean(id)?,
                // A fixed-width value is the integer it holds.
                ClauseCarrier::Scalar(ScalarType::Integer(_)) => self.declare_integers(id)?,
                _ => {
                    return Err(unserved(
                        id,
                        "law over a carrier other than mathematical data, `bool` or an integer",
                    ));
                }
            });
            let term = match &value.operand {
                ClauseOperand::Scalar(ScalarTerm::Boolean(truth))
                    if value.carrier == ClauseCarrier::Scalar(ScalarType::Boolean) =>
                {
                    self.signature
                        .arena
                        .insert(if *truth { Term::TwoOne } else { Term::TwoZero })
                }
                // The `k`-th formal that is a place, counting from zero, is
                // place `k + 1`; a fixed-width formal before it is a value,
                // not a place. The machine's own result is the place after
                // them: the definition it is read as, applied to the formals.
                ClauseOperand::Input(subject)
                    if subject.path().is_empty()
                        && self
                            .place_formal(id, subject.root(), formals.len())
                            .is_none() =>
                {
                    match self.result_as {
                        Some((machine, result)) if machine == id => result,
                        _ => {
                            let function = self.function(id)?;
                            let function = self.constant(function);
                            // A generic machine takes its machine parameters
                            // first.
                            let mut arguments = if self.template(id).is_some() {
                                self.callable_actuals(0)
                            } else {
                                Vec::new()
                            };
                            arguments.extend(formals.iter().copied());
                            self.signature.arena.apply_arguments(function, &arguments)
                        }
                    }
                }
                ClauseOperand::Input(subject) if subject.path().is_empty() => self
                    .place_formal(id, subject.root(), formals.len())
                    .flatten()
                    .and_then(|formal| formals.get(formal))
                    .copied()
                    .ok_or_else(|| unserved(id, "law names storage outside its formals"))?,
                ClauseOperand::Application {
                    declaration,
                    arguments,
                } if declaration.supplier.is_none() => {
                    let member =
                        SymbolHandle::from_parts(declaration.member.0, declaration.member.1);
                    let operands: Vec<_> = arguments
                        .iter()
                        .map(|&argument| terms[argument as usize])
                        .collect();
                    // A field applied to a value is that component of it.
                    let field = match arguments.as_slice() {
                        &[value] => match values[value as usize].carrier {
                            ClauseCarrier::Structural(record) => terminal_check_tables::row(
                                &self.program.record_sources,
                                record,
                                id,
                                Construct::Machine,
                            )?
                            .fields
                            .iter()
                            .position(|field| *field == member)
                            .map(|at| (carriers[value as usize], at)),
                            _ => None,
                        },
                        _ => None,
                    };
                    let parameter = self
                        .callable_scope
                        .iter()
                        .find(|(symbol, _)| *symbol == member)
                        .map(|(_, variable)| *variable);
                    // A quotient's declaration applied to a representative
                    // is that representative's class.
                    if let Some(quotient) = self
                        .program
                        .quotients
                        .iter()
                        .find(|quotient| quotient.declaration == member)
                    {
                        let entry = self.data(id, quotient.record)?;
                        let SignatureEntryKind::Quotient {
                            carrier, relation, ..
                        } = self.signature.entries[entry as usize].kind
                        else {
                            return Err(unserved(id, "quotient lost its declaration"));
                        };
                        let &[representative] = operands.as_slice() else {
                            return Err(unserved(id, "quotient applied to other than one value"));
                        };
                        let family = self.family(carrier, relation);
                        terms.push(family.project(&mut self.signature.arena, representative));
                        continue;
                    }
                    match (field, parameter) {
                        (Some((entry, at)), _) => self.component(id, entry, operands[0], at)?.0,
                        // A machine parameter applied to its arguments, or
                        // passed whole to a generic machine.
                        (None, Some(variable)) => {
                            self.signature.arena.apply_arguments(variable, &operands)
                        }
                        (None, None) => {
                            let head = self.declaration(id, member, satisfaction)?;
                            self.owed_in_clause(id, member, &operands)?;
                            let head = self.constant(head);
                            self.signature.arena.apply_arguments(head, &operands)
                        }
                    }
                }
                // A fixed-width formal is the integer it holds.
                ClauseOperand::Scalar(ScalarTerm::Value { id: named, .. })
                    if matches!(value.carrier, ClauseCarrier::Scalar(ScalarType::Integer(_))) =>
                {
                    let position = self
                        .program
                        .signatures
                        .get(id)
                        .ok_or_else(|| unserved(id, "mathematical machine lost its signature"))?
                        .formal_values()
                        .iter()
                        .position(|known| *known == Some(*named));
                    position
                        .and_then(|position| formals.get(position))
                        .copied()
                        .ok_or_else(|| unserved(id, "law names an integer other than a formal"))?
                }
                ClauseOperand::Scalar(ScalarTerm::Integer { value: literal, .. }) => self
                    .integer_literal(
                        id,
                        match *literal {
                            IntegerValue::Signed(value) => bignum::BigInt::from_i128(value),
                            IntegerValue::Unsigned(value) => bignum::BigInt::from_u128(value),
                        },
                    )?,
                _ => {
                    return Err(unserved(
                        id,
                        "law operand other than a formal, a constructor or a mathematical call",
                    ));
                }
            };
            terms.push(term);
        }
        Ok((terms, carriers))
    }

    /// The formal of `id` that a clause names as place `root`: `Some(None)`
    /// for a place outside its formals, and `None` for its result's place.
    /// When every formal is a place, as for any machine of data alone, the
    /// `k`-th formal is place `k + 1` and the result's place follows them.
    fn place_formal(
        &self,
        id: MachineId,
        root: semantic_vocabulary::PlaceId,
        count: usize,
    ) -> Option<Option<usize>> {
        let place = usize::try_from(root.get().checked_sub(1)?).ok()?;
        let signature = self
            .program
            .signatures
            .get(id)
            .filter(|signature| signature.parameters.len() == count)
            .filter(|signature| (0..count).any(|position| !signature.is_place(position)));
        let Some(signature) = signature else {
            return (place != count).then_some((place < count).then_some(place));
        };
        if signature.structural_result_place() == Some(root) {
            return None;
        }
        Some(
            (0..count)
                .filter(|&position| signature.is_place(position))
                .nth(place),
        )
    }

    /// The entry a clause applies: a machine with a result, a case, or the
    /// satisfier of a trait operation under the law's own trait application.
    fn declaration(
        &mut self,
        id: MachineId,
        member: SymbolHandle,
        satisfaction: Option<&Satisfaction>,
    ) -> Checked<u32> {
        let program = self.program;
        if let Some((machine, _)) = program.symbols.iter().find(|(_, known)| **known == member) {
            return self.function(machine);
        }
        for &record in &program.self_containing {
            let Some(source) = program.record_sources.get(record) else {
                continue;
            };
            if let Some(case) = source.cases.iter().position(|(case, _)| *case == member) {
                let entry = self.data(id, record)?;
                let SignatureEntryKind::Data { shape, .. } =
                    &self.signature.entries[entry as usize].kind
                else {
                    return Err(unserved(id, "case of data without cases"));
                };
                return Ok(shape.constructor_entry(entry, case));
            }
        }
        if let Some(law) = satisfaction {
            let operation = self
                .satisfactions
                .iter()
                .find(|other| {
                    other.declaring_trait == law.declaring_trait
                        && other.scope == law.scope
                        && other.group == law.group
                        && other.requirement == member
                })
                .ok_or_else(|| {
                    unserved(
                        id,
                        "declared law applies an operation that no machine satisfies at these type arguments",
                    )
                })?;
            return self.function(operation.machine);
        }
        Err(unserved(
            id,
            "clause applies a declaration outside the mathematical signature",
        ))
    }

    /// What each slot of a body stands for before any `let`: each formal,
    /// and nothing yet for a local.
    fn formal_slots(&mut self, formals: usize, slots: usize) -> Vec<Option<Bound>> {
        (0..slots.max(formals))
            .map(|slot| {
                (slot < formals).then(|| Bound {
                    term: self.variable((formals - 1 - slot) as u32),
                    field: None,
                    applied: None,
                })
            })
            .collect()
    }

    /// Binds each `let` of a body to the term of its value, in order.
    fn locals(
        &mut self,
        id: MachineId,
        statements: &[&Statement],
        slots: &mut [Option<Bound>],
        aliases: &[usize],
    ) -> Checked<()> {
        for statement in statements {
            let Statement::Bind { slot, value } = statement else {
                continue;
            };
            // A law that is not about its own result returns a value only
            // so that it can be called as a statement.
            if matches!(&value.kind,
                ExpressionKind::Call { callee: CallTarget::Machine(callee), .. }
                    if self.states_law(*callee) && !self.names_result(*callee))
            {
                continue;
            }
            let bound = {
                let scope = Scope {
                    slots: &*slots,
                    aliases,
                    case: None,
                    recursion: None,
                    depth: 0,
                    induction: true,
                    value: None,
                };
                self.expression(id, value, &scope)?
            };
            *slots
                .get_mut(*slot)
                .ok_or_else(|| unserved(id, "mathematical local outside its body's slots"))? =
                Some(bound);
        }
        Ok(())
    }

    /// How the body argues: its citations, per case when it dispatches.
    fn script(
        &mut self,
        id: MachineId,
        body: &Body,
        formals: &[(TermHandle, u32)],
    ) -> Checked<LawProof> {
        let ranked_definition = self
            .program
            .signatures
            .get(id)
            .is_some_and(|signature| signature.result.is_some() && signature.ranking.is_some());
        if let Some(&(_, _, data, formal)) = self.fueled.iter().find(|(fueled, ..)| *fueled == id) {
            return self.script_as(id, body, formals, Some((data, formal)));
        }
        match self.script_as(id, body, formals, None) {
            // A machine with a result and a ranking may be defined on fuel.
            // Its law is then an induction on the fuel, whatever its body
            // dispatches on.
            Err(refusal) if ranked_definition => {
                self.function(id)?;
                match self.fueled.iter().find(|(fueled, ..)| *fueled == id) {
                    Some(&(_, _, data, formal)) => {
                        self.script_as(id, body, formals, Some((data, formal)))
                    }
                    None => Err(refusal),
                }
            }
            read => read,
        }
    }

    fn script_as(
        &mut self,
        id: MachineId,
        body: &Body,
        formals: &[(TermHandle, u32)],
        fuel: Option<(u32, u32)>,
    ) -> Checked<LawProof> {
        let count = formals.len();
        let (aliases, statements) = spine(body);
        let (dispatch, before) = match statements.split_last() {
            Some((
                Statement::Dispatch {
                    subject,
                    arms,
                    default,
                    ..
                },
                before,
            )) => (Some((subject, arms, default)), before),
            _ => (None, statements.as_slice()),
        };
        // A dispatch on a formal is the law's induction. Anything else is
        // an argument with no induction of its own: the citations, then a
        // split on what the body dispatches on, if it does.
        let on_formal = dispatch.as_ref().and_then(|(subject, _, default)| {
            let ExpressionKind::Slot(slot) = subject.kind else {
                return None;
            };
            let slot = aliases.get(slot).copied().unwrap_or(slot);
            (slot < count && default.is_none()).then_some(slot)
        });
        let (Some(subject), Some((_, arms, _)), None) = (on_formal, dispatch, fuel) else {
            let mut slots = self.formal_slots(count, body.slots.len());
            self.locals(id, before, &mut slots, &aliases)?;
            let scope = Scope {
                slots: &slots,
                aliases: &aliases,
                case: None,
                recursion: None,
                depth: 0,
                induction: true,
                value: None,
            };
            let mut case = LawCase::default();
            match dispatch {
                None => self.citations(id, id, before, &scope, &mut case)?,
                Some((subject, arms, default)) => self.split_case(
                    id,
                    id,
                    subject,
                    arms,
                    default.is_some(),
                    before,
                    &scope,
                    &mut case,
                )?,
            }
            return Ok(match fuel {
                // `read_laws` states the goal over the fuel for each clause.
                Some((data, formal)) => LawProof::Fuel {
                    data,
                    formal,
                    goal: self.signature.arena.insert(Term::Dummy),
                    script: case,
                },
                None => LawProof::Direct(case),
            });
        };
        self.ranked_on(id, subject, false)?;
        let data = formals[subject].1;
        let shape = self.shape(data);
        let (group, mutual) = if shape.is_mutual() {
            self.mutual_laws(id, &shape, data)?
        } else {
            (Vec::new(), Vec::new())
        };
        let record = self
            .data
            .iter()
            .find(|(_, entry)| *entry == data)
            .map(|(record, _)| *record)
            .expect("a declared data entry has its record");
        let declared = self
            .program
            .records
            .get(record)
            .ok_or_else(|| unserved(id, "mathematical data lost its record"))?;
        if arms.len() != shape.cases() {
            return Err(unserved(
                id,
                "law dispatch without exactly one arm per case",
            ));
        }
        let mut cases = Vec::with_capacity(shape.cases());
        for case in 0..shape.cases() {
            let mut selected = arms.iter().filter(|(selector, _, _)| selector.case == case);
            let (Some((selector, target, origin)), None) = (selected.next(), selected.next())
            else {
                return Err(unserved(
                    id,
                    "law dispatch without exactly one arm per case",
                ));
            };
            if selector.condition.is_some() {
                return Err(unserved(id, "law dispatch on a field value"));
            }
            // A case's terms live under the formals, then its fields.
            let arity = shape.arity(case);
            let fields: Vec<_> = (0..arity)
                .map(|field| self.variable(arity - 1 - field))
                .collect();
            let field_members: Vec<_> = (0..arity)
                .map(|field| shape.field_member(case, field))
                .collect();
            let slots: Vec<_> = (0..count)
                .map(|formal| {
                    Some(Bound {
                        term: self.variable((count - 1 - formal) as u32 + arity),
                        field: None,
                        applied: None,
                    })
                })
                .collect();
            let scope = Scope {
                slots: &slots,
                aliases: &aliases,
                case: Some((subject, case, &fields)),
                recursion: Some(Recursion {
                    machine: id,
                    subject,
                    results: &[],
                    group: &group,
                    members: &field_members,
                }),
                depth: arity,
                induction: true,
                value: None,
            };
            let mut script = LawCase {
                citations: Vec::new(),
                field_names: declared.cases()[case]
                    .fields
                    .iter()
                    .map(|field| field.identity.clone())
                    .collect(),
                source: match origin {
                    crate::SourceOrigin::Authored(span) => Some(*span),
                    _ => None,
                },
                split: None,
            };
            self.citations(id, id, before, &scope, &mut script)?;
            match target {
                Target::Unit => {}
                Target::Value(value) => {
                    self.cite_within(id, id, value, &scope, &mut script, false)?
                }
                Target::Crash(_) => return Err(unserved(id, "law case that crashes")),
            }
            cases.push(script);
        }
        // A generic law's machine parameters are its first formals.
        let callables = self
            .template(id)
            .map_or(0, |template| template.callables.len());
        Ok(LawProof::Cases {
            formal: (callables + subject) as u32,
            data,
            cases,
            mutual,
        })
    }

    /// Read each case of a split inside a case of `law`. `machine` is the
    /// body that dispatches and `subject` what it dispatches on: a slot that
    /// stands for a formal of the law, for a field of an enclosing case, or
    /// for a computed value. `read` is called once per case, in declared
    /// order, with the scope of that case: the names as before and the
    /// case's fields beneath them. Returns what is split and the entry of
    /// its data.
    fn split(
        &mut self,
        law: MachineId,
        machine: MachineId,
        subject: &Expression,
        scope: &Scope<'_>,
        mut read: impl FnMut(&mut Self, usize, &Scope<'_>) -> Checked<()>,
    ) -> Checked<(SplitSubject, u32)> {
        let ExpressionKind::Slot(slot) = subject.kind else {
            return Err(unserved(machine, "law dispatch on a computed subject"));
        };
        let slot = scope.aliases.get(slot).copied().unwrap_or(slot);
        let bound = scope
            .slots
            .get(slot)
            .copied()
            .flatten()
            .ok_or_else(|| unserved(machine, "law dispatch on an unbound local"))?;
        // A generic law's machine parameters are named before its formals.
        let formals = (self.formals(law)?.len()
            + self
                .template(law)
                .map_or(0, |template| template.callables.len())) as u32;
        let split = match self.signature.arena.get(bound.term) {
            Term::Variable(index) => SplitSubject::Name(
                (formals + scope.depth)
                    .checked_sub(index + 1)
                    .ok_or_else(|| unserved(machine, "law dispatch outside the law's names"))?,
            ),
            _ => SplitSubject::Value(bound.term),
        };
        let Some(ValueType::Record(record, ..)) = subject.result else {
            return Err(unserved(machine, "law dispatch on a value without cases"));
        };
        let data = self.data(machine, record)?;
        let SignatureEntryKind::Data { shape, .. } = &self.signature.entries[data as usize].kind
        else {
            return Err(unserved(machine, "law dispatch on a value without cases"));
        };
        let shape = shape.clone();
        for case in 0..shape.cases() {
            // The names so far move beneath the case's fields.
            let arity = shape.arity(case);
            let fields: Vec<_> = (0..arity)
                .map(|field| self.variable(arity - 1 - field))
                .collect();
            let slots: Vec<_> = scope
                .slots
                .iter()
                .map(|bound| {
                    bound.map(|bound| Bound {
                        term: shift(&mut self.signature.arena, bound.term, 0, arity),
                        field: bound.field,
                        applied: bound
                            .applied
                            .map(|applied| shift(&mut self.signature.arena, applied, 0, arity)),
                    })
                })
                .collect();
            let inner = Scope {
                slots: &slots,
                aliases: scope.aliases,
                case: Some((slot, case, &fields)),
                recursion: scope.recursion,
                depth: scope.depth + arity,
                induction: false,
                value: None,
            };
            read(self, case, &inner)?;
        }
        Ok((split, data))
    }

    /// The cases of a dispatch in a law's argument, added to `script` as its
    /// split: per case, the statements before the dispatch read again in
    /// that case, then the case's own arm.
    #[allow(clippy::too_many_arguments)]
    fn split_case(
        &mut self,
        law: MachineId,
        machine: MachineId,
        subject: &Expression,
        arms: &[(CaseSelector, Target, crate::SourceOrigin)],
        defaulted: bool,
        before: &[&Statement],
        scope: &Scope<'_>,
        script: &mut LawCase,
    ) -> Checked<()> {
        if defaulted {
            return Err(unserved(machine, "law dispatch with a default arm"));
        }
        if script.split.is_some() {
            return Err(unserved(machine, "law case that splits twice"));
        }
        let mut cases = Vec::with_capacity(arms.len());
        let (subject, data) =
            self.split(law, machine, subject, scope, |converter, case, scope| {
                let mut selected = arms.iter().filter(|(selector, _, _)| selector.case == case);
                let (Some((selector, target, origin)), None) = (selected.next(), selected.next())
                else {
                    return Err(unserved(
                        machine,
                        "law dispatch without exactly one arm per case",
                    ));
                };
                if selector.condition.is_some() {
                    return Err(unserved(machine, "law dispatch on a field value"));
                }
                let mut nested = LawCase {
                    source: match origin {
                        crate::SourceOrigin::Authored(span) => Some(*span),
                        _ => None,
                    },
                    ..LawCase::default()
                };
                converter.citations(law, machine, before, scope, &mut nested)?;
                match target {
                    Target::Unit => {}
                    Target::Value(value) => {
                        converter.cite_within(law, machine, value, scope, &mut nested, false)?
                    }
                    Target::Crash(_) => return Err(unserved(machine, "law case that crashes")),
                }
                cases.push(nested);
                Ok(())
            })?;
        if arms.len() != cases.len() {
            return Err(unserved(
                machine,
                "law dispatch without exactly one arm per case",
            ));
        }
        script.split = Some(LawSplit {
            subject,
            data,
            cases,
        });
        Ok(())
    }

    /// The citations a statement list makes. `law` is the law being read and
    /// `machine` the body the statements belong to, the law's or a helper's.
    fn citations(
        &mut self,
        law: MachineId,
        machine: MachineId,
        statements: &[&Statement],
        scope: &Scope<'_>,
        script: &mut LawCase,
    ) -> Checked<()> {
        for statement in statements.iter().copied() {
            match statement {
                Statement::Evaluate { expression, .. } => {
                    self.cite_within(law, machine, expression, scope, script, false)?
                }
                Statement::Bind { value, .. } => {
                    self.cite_within(law, machine, value, scope, script, true)?
                }
                Statement::Arm {
                    guard: ArmGuard::Always,
                    target: Target::Unit,
                    ..
                } => {}
                Statement::Arm {
                    guard: ArmGuard::Always,
                    target: Target::Value(value),
                    ..
                } => self.cite_within(law, machine, value, scope, script, false)?,
                _ => {
                    return Err(unserved(
                        machine,
                        "law body statement other than a citation or one dispatch",
                    ));
                }
            }
        }
        Ok(())
    }

    /// The citations an expression makes: every call in it of a machine
    /// that states a law, innermost first. `nested` says the expression is an
    /// operand of another, where a call of an ordinary function is a term
    /// and cites nothing. The value a law machine returns is such a term too:
    /// its calls are read here and its value by `read_function`.
    fn cite_within(
        &mut self,
        law: MachineId,
        machine: MachineId,
        expression: &Expression,
        scope: &Scope<'_>,
        script: &mut LawCase,
        nested: bool,
    ) -> Checked<()> {
        let returns = self
            .program
            .signatures
            .get(machine)
            .is_some_and(|signature| signature.result.is_some());
        match &expression.kind {
            ExpressionKind::Call {
                callee: CallTarget::Machine(_),
                arguments,
                ..
            } => {
                for argument in arguments {
                    self.cite_within(law, machine, argument, scope, script, true)?;
                }
                self.cite(law, machine, expression, scope, script, nested || returns)
            }
            ExpressionKind::Applied {
                head, arguments, ..
            } => {
                for argument in arguments {
                    self.cite_within(law, machine, argument, scope, script, true)?;
                }
                match head {
                    AppliedHead::Machine(_) => {
                        self.cite(law, machine, expression, scope, script, nested || returns)
                    }
                    // A machine parameter states no law: as an operand it is a
                    // term, and as a statement it would say nothing.
                    AppliedHead::Parameter(_) if nested || returns => Ok(()),
                    AppliedHead::Parameter(_) => Err(unserved(
                        machine,
                        "law body calls a machine parameter as a statement",
                    )),
                }
            }
            ExpressionKind::Case { fields, .. } if nested || returns => {
                for (_, operand) in fields {
                    self.cite_within(law, machine, operand, scope, script, true)?;
                }
                Ok(())
            }
            ExpressionKind::Field { base, .. } if nested || returns => {
                self.cite_within(law, machine, base, scope, script, true)
            }
            ExpressionKind::Record { fields, .. } if nested || returns => {
                for (_, operand) in fields {
                    self.cite_within(law, machine, operand, scope, script, true)?;
                }
                Ok(())
            }
            // A stored callable applied, as an operand: a term, which cites
            // what its operands cite.
            ExpressionKind::Call {
                callee: CallTarget::Selected { callable, .. },
                arguments,
                ..
            } if nested || returns => {
                self.cite_within(law, machine, callable, scope, script, true)?;
                for argument in arguments {
                    self.cite_within(law, machine, argument, scope, script, true)?;
                }
                Ok(())
            }
            ExpressionKind::Slot(_)
            | ExpressionKind::Payload { .. }
            | ExpressionKind::Integer(_)
            | ExpressionKind::Boolean(_)
            | ExpressionKind::MachineValue { .. }
                if nested || returns =>
            {
                Ok(())
            }
            _ => Err(unserved(machine, "law body statement other than a call")),
        }
    }

    fn cite(
        &mut self,
        law: MachineId,
        machine: MachineId,
        call: &Expression,
        scope: &Scope<'_>,
        script: &mut LawCase,
        term: bool,
    ) -> Checked<()> {
        let (callee, machines, arguments) = match &call.kind {
            ExpressionKind::Call {
                callee: CallTarget::Machine(callee),
                arguments,
                ..
            } => (callee, &[][..], arguments),
            ExpressionKind::Applied {
                head: AppliedHead::Machine(callee),
                machines,
                arguments,
            } => (callee, machines.as_slice(), arguments),
            _ => return Err(unserved(machine, "law body statement other than a call")),
        };
        let cited = self
            .program
            .signatures
            .get(*callee)
            .ok_or_else(|| unserved(machine, "citation lost its machine"))?;
        if *callee != law
            && cited.result.is_some()
            && !self.states_law(*callee)
            && !self.helper(*callee)
        {
            // An ordinary function. As an operand it is a term and cites
            // nothing, so its operands are not read: one may be a local
            // bound to a citation, which has no term. As a statement it
            // would say nothing.
            return if term {
                Ok(())
            } else {
                Err(unserved(
                    machine,
                    "law body calls a machine with a result that states no law",
                ))
            };
        }
        let mut operands = Vec::with_capacity(arguments.len());
        for argument in arguments {
            operands.push(self.expression(machine, argument, scope)?);
        }
        // A generic law cites itself at machines, its own or others, ahead
        // of the arguments: the induction is over all of them.
        let passing = if self.template(law).is_some() && *callee == law {
            self.passed(machine, machines, scope.depth)?
        } else {
            Vec::new()
        };
        if *callee == law && self.fueled.iter().any(|(fueled, ..)| *fueled == law) {
            // The law of a machine defined on fuel is proved by induction on
            // the fuel, so it may be cited at any arguments: the citation is
            // the law with one unit less.
            let mut arguments = passing;
            arguments.extend(operands.iter().map(|operand| operand.term));
            script.citations.push(Citation {
                evidence: Evidence::Fuel,
                arguments,
            });
            return Ok(());
        }
        if let Some(recursion) = scope.recursion
            && recursion.machine == law
            && let Some(Some(member)) = recursion.calls(*callee)
        {
            // A law of a mutual induction at a field of the dispatched case
            // that holds its member is the block's induction hypothesis
            // there: each law of the group has the one formal.
            let at = operands.first().and_then(|operand| operand.applied);
            let field = operands
                .first()
                .and_then(|operand| operand.field)
                .ok_or_else(|| {
                    unserved(
                        machine,
                        "law of a mutual induction cited at an argument that is not a field of the dispatched case",
                    )
                })?;
            if recursion.members.get(field as usize).copied().flatten() != Some(member) {
                return Err(unserved(
                    machine,
                    "law of a mutual induction cited at a field of another member of the block",
                ));
            }
            self.ranked_on(*callee, 0, true)?;
            script.citations.push(Citation {
                evidence: Evidence::Hypothesis { field, at },
                arguments: Vec::new(),
            });
            return Ok(());
        }
        if *callee == law {
            // The law at a field of the dispatched case is the induction
            // hypothesis. At any other argument it would be its own evidence.
            let Some(Recursion { subject, .. }) = scope.recursion else {
                return Err(unserved(
                    machine,
                    "law cites itself without dispatching on a formal",
                ));
            };
            let at = operands.get(subject).and_then(|operand| operand.applied);
            let field = operands
                .get(subject)
                .and_then(|operand| operand.field)
                .ok_or_else(|| {
                    unserved(
                        machine,
                        "law cites itself at an argument that is not a field of the dispatched case",
                    )
                })?;
            self.ranked_on(law, subject, true)?;
            // A generic law's hypothesis is stated over its machine
            // parameters too, bound outside the formals: they come first.
            let mut arguments = passing;
            arguments.extend(
                operands
                    .iter()
                    .enumerate()
                    .filter(|(position, _)| *position != subject)
                    .map(|(_, operand)| operand.term),
            );
            script.citations.push(Citation {
                evidence: Evidence::Hypothesis { field, at },
                arguments,
            });
            return Ok(());
        }
        let laws = self.laws(*callee)?;
        if !laws.is_empty() {
            let mut arguments = self.passed(machine, machines, scope.depth)?;
            arguments.extend(operands.iter().map(|operand| operand.term));
            for cited in laws {
                script.citations.push(Citation {
                    evidence: Evidence::Statement(self.signature.laws[cited as usize].statement),
                    arguments: arguments.clone(),
                });
            }
            return Ok(());
        }
        // A generic machine that states nothing is read in place with its
        // machine parameters standing for the machines this call passes. A
        // machine declared inside the law passes none: it shares the law's.
        let generic = match self.template(*callee) {
            Some(template) if !machines.is_empty() => {
                let symbols: Vec<SymbolHandle> = template
                    .callables
                    .iter()
                    .map(|callable| callable.symbol)
                    .collect();
                if symbols.len() != machines.len() {
                    return Err(unserved(
                        machine,
                        "citation of a generic mathematical machine without one machine per parameter",
                    ));
                }
                let passed = self.passed(machine, machines, 0)?;
                Some(symbols.into_iter().zip(passed).collect::<Vec<_>>())
            }
            _ => None,
        };
        let enclosing = generic.map(|scope| std::mem::replace(&mut self.callable_scope, scope));
        let read = self.cite_in_place(law, machine, *callee, &operands, scope, script);
        if let Some(enclosing) = enclosing {
            self.callable_scope = enclosing;
        }
        read
    }

    /// Read the body of `callee`, a machine that states nothing, in place
    /// at `operands` inside a case of `law`.
    fn cite_in_place(
        &mut self,
        law: MachineId,
        machine: MachineId,
        callee: MachineId,
        operands: &[Bound],
        scope: &Scope<'_>,
        script: &mut LawCase,
    ) -> Checked<()> {
        let callee = &callee;
        // A machine that states nothing is read in place, with its formals
        // standing for the arguments of this call.
        *self.machines.get_mut(*callee).expect("row read by `laws`") = Read::Reading;
        let mut slots: Vec<_> = operands.iter().copied().map(Some).collect();
        let body = self
            .bodies
            .get(*callee)
            .ok_or_else(|| unserved(machine, "citation lost its body"))?;
        let (aliases, statements) = spine(body);
        slots.resize(slots.len().max(body.slots.len()), None);
        if let Err(refusal) = self.locals(*callee, &statements, &mut slots, &aliases) {
            *self.machines.get_mut(*callee).expect("row read by `laws`") = Read::Refused;
            return Err(refusal);
        }
        let inner = Scope {
            slots: &slots,
            aliases: &aliases,
            case: None,
            recursion: scope.recursion,
            depth: scope.depth,
            induction: scope.induction,
            value: None,
        };
        // Its last statement may dispatch: the case being read then goes on
        // by cases of what that statement dispatches on.
        let read = match statements.split_last() {
            Some((
                Statement::Dispatch {
                    subject,
                    arms,
                    default,
                    ..
                },
                before,
            )) => self.split_case(
                law,
                *callee,
                subject,
                arms,
                default.is_some(),
                before,
                &inner,
                script,
            ),
            _ => self.citations(law, *callee, &statements, &inner, script),
        };
        *self.machines.get_mut(*callee).expect("row read by `laws`") = match read {
            Ok(()) => Read::Cites { called: true },
            Err(_) => Read::Refused,
        };
        read
    }
}
