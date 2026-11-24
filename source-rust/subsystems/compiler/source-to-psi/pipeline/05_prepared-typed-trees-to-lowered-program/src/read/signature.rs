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

A law's `requires` equalities are its premises. Its statement takes a proof
of each after its formals, and stage 06 proves each wherever the law is
cited.

A cited law is read before the law that cites it, and a law that reaches
itself through its citations is refused, so no statement can justify itself.
*/

use arena::IdTable;
use mathematical_kernel::{
    Declaration, Level, PlainData, Sort, Term, TermHandle, instantiate, shift,
};
use semantic_vocabulary::{
    ClauseCarrier, ClauseIdentity, ClauseOperand, MachineId, Proposition, StructuralTypeId,
};
use symbols::SymbolHandle;
use terminal_check_tables::mathematics::{
    Citation, Evidence, FunctionCases, Law, LawCase, LawProof, LawSplit, MathematicalSignature,
    OperandLicense, SignatureEntry, SignatureEntryKind, SplitSubject, operand_law,
};
use terminal_check_tables::{Checked, Construct, Refusal};

use super::Program;
use crate::lowering::body::{
    ArmGuard, Body, CallTarget, CaseSelector, Expression, ExpressionKind, Statement, Target,
    ValueType,
};
use crate::lowering::{Unserved, refuse};

/// The trait requirement a mathematical machine satisfies, read at the
/// trait's actual type arguments.
pub(crate) struct Satisfaction {
    machine: MachineId,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
    scope: Vec<ValueType>,
    ensures: Vec<Proposition>,
}

impl Program<'_> {
    /// Reads the requirement named by the machine's `satisfies` clause under
    /// that clause's type arguments. The requirement's equalities apply the
    /// trait's operations; the signature resolves each to its satisfier.
    pub(crate) fn satisfaction(&mut self, id: MachineId) -> Checked<Option<Satisfaction>> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
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
            || edge.alias.is_some()
            || !edge.trait_lifetime_arguments.is_empty()
        {
            return Err(unserved(
                "`satisfies` clause with a binding, an alias or lifetimes",
            ));
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
            && (!required.ensures.is_empty() || !required.membership_ensures.is_empty())
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
            scope: scope.into_iter().map(|(_, value)| value).collect(),
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
            machines,
            functions,
            fueled: Vec::new(),
            result_as: None,
        };
        let mut refused = Vec::new();
        let diagnostic = MachineId::new(1).expect("1 is nonzero");
        for &record in &self.self_containing {
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
    Refused,
}

/// What a name of the body stands for, and whether it is a field of the
/// dispatched case: only such a term may carry a recursive call.
#[derive(Clone, Copy)]
struct Bound {
    term: TermHandle,
    field: Option<u32>,
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
    recursion: Option<(MachineId, usize, &'s [TermHandle])>,
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
}

/// The machine a definition is being read for.
struct Defining<'d> {
    id: MachineId,
    symbol: SymbolHandle,
    name: &'d str,
    formals: &'d [(TermHandle, u32)],
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
}

fn unserved(id: MachineId, what: &'static str) -> Refusal {
    refuse(id, Construct::Machine, Unserved::Machine(what))
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

impl Converter<'_, '_> {
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
        let mut arities = Vec::with_capacity(declared.cases().len());
        for case in declared.cases() {
            if case
                .fields
                .iter()
                .any(|field| field.value_type != ValueType::record(record))
            {
                return Err(unserved(
                    id,
                    "data that contains itself with a field of another type",
                ));
            }
            arities.push(index(id, case.fields.len())?);
        }
        let shape = PlainData::new(arities)
            .ok_or_else(|| unserved(id, "data that contains itself without a case"))?;
        let first = index(id, self.signature.declarations.len())?;
        let declarations = shape.declarations(&mut self.signature.arena, first);
        index(id, self.signature.declarations.len() + declarations.len())?;
        let entry = PlainData::data_entry(first);
        let name = self.name(source.declaration);
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
        let (&last, before) = fields.split_last().expect("checked not empty");
        let mut ty = self.constant(last);
        for &field in before.iter().rev() {
            let domain = self.constant(field);
            ty = self.signature.arena.insert(Term::Sigma {
                domain,
                codomain: ty,
            });
        }
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
        if count == 1 {
            return Ok((value, ty));
        }
        let mut term = value;
        for _ in 0..field {
            term = self.signature.arena.insert(Term::Snd { pair: term });
        }
        if field + 1 < count {
            term = self.signature.arena.insert(Term::Fst { pair: term });
        }
        Ok((term, ty))
    }

    fn shape(&self, data: u32) -> PlainData {
        match &self.signature.entries[data as usize].kind {
            SignatureEntryKind::Data { shape, .. } => shape.clone(),
            _ => unreachable!("a data entry was recorded with its shape"),
        }
    }

    /// The closed kernel type of a formal or a result.
    fn ty(&mut self, id: MachineId, value_type: ValueType) -> Checked<(TermHandle, u32)> {
        let ValueType::Record(record, ..) = value_type else {
            return Err(unserved(
                id,
                "mathematical declaration over a type other than data that contains itself",
            ));
        };
        let entry = self.data(id, record)?;
        Ok((self.constant(entry), entry))
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
            .map(|formal| self.ty(id, formal))
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
        match self.begin(
            id,
            true,
            "mathematical machines that call each other in a cycle",
        )? {
            None => {}
            Some(Read::Function(entry)) => return Ok(entry),
            Some(_) => return Err(unserved(id, "term that calls a machine with no result")),
        }
        let read = self.read_function(id);
        self.finish(id, true, read.map(|entry| (entry, Read::Function(entry))))
    }

    fn read_function(&mut self, id: MachineId) -> Checked<u32> {
        let program = self.program;
        let signature = program
            .signatures
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its signature"))?;
        if !signature.membership_ensures.is_empty() {
            return Err(unserved(
                id,
                "mathematical machine with a result and a membership guarantee",
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
        let mut premises = Vec::with_capacity(signature.requires.len());
        for premise in &signature.requires {
            premises.push(self.equality(id, premise, &open, None)?);
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
            result,
            premises: &premises,
        };
        let structural = by_cases.map(|(subject, arms)| self.by_cases(&defining, subject, arms));
        let (mut term, cases) = match (structural, ranked) {
            (Some(Ok((term, cases))), _) => (term, Some(cases)),
            (Some(Err(refusal)), None) => return Err(refusal),
            // It calls itself somewhere else, or chooses by a computed
            // value first: the recursion is on fuel taken from its measure.
            (_, Some(measure)) => (self.by_fuel(&defining, measure, result_entry)?, None),
            (None, None) => {
                let types: Vec<_> = formals.iter().map(|(formal, _)| *formal).collect();
                let valuing = Valuing {
                    owner: id,
                    context: &types,
                    result,
                    fuel: None,
                    premises: &premises,
                    cited: &[],
                };
                let slots = self.formal_slots(count, body.slots.len());
                (self.body_value(id, slots, None, &valuing)?, None)
            }
        };
        let mut ty = result;
        for &(formal, _) in formals.iter().rev() {
            term = self.signature.arena.insert(Term::Lambda {
                domain: formal,
                body: term,
            });
            ty = self.signature.arena.insert(Term::Pi {
                domain: formal,
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
        arms: &[(CaseSelector, Target, crate::lowering::SourceOrigin)],
    ) -> Checked<(TermHandle, FunctionCases)> {
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
        let others: Vec<usize> = (0..count).filter(|formal| *formal != subject).collect();
        let rest = others.len() as u32;
        // The motive returns a function of the other formals, so a
        // recursive call may pass them different values.
        let mut landing = defining.result;
        for &other in others.iter().rev() {
            landing = self.signature.arena.insert(Term::Pi {
                domain: formals[other].0,
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
            });
            let mut actuals = vec![slots[subject].expect("bound above").term; count];
            for (position, &other) in others.iter().enumerate() {
                let term = self.variable(rest - 1 - position as u32);
                slots[other] = Some(Bound { term, field: None });
                actuals[other] = term;
            }
            // The variables of the branch: the case's fields, the recursion
            // at each, then the other formals.
            let mut context = vec![formals[subject].0; arity as usize];
            context.extend(vec![landing; arity as usize]);
            context.extend(others.iter().map(|&other| formals[other].0));
            let premises: Vec<_> = defining
                .premises
                .iter()
                .map(|&premise| instantiate(&mut self.signature.arena, premise, &actuals))
                .collect();
            let valuing = Valuing {
                owner: id,
                context: &context,
                result: defining.result,
                fuel: None,
                premises: &premises,
                cited: &[],
            };
            let scope = Scope {
                slots: &slots,
                aliases: &aliases,
                case: Some((subject, case, &fields)),
                recursion: Some((id, subject, &results)),
                depth: 0,
                induction: true,
                value: Some(&valuing),
            };
            let mut branch = self.expression(id, value, &scope)?.term;
            for &other in others.iter().rev() {
                branch = self.signature.arena.insert(Term::Lambda {
                    domain: formals[other].0,
                    body: branch,
                });
            }
            branches.push(shape.branch(&mut self.signature.arena, data, motive, case, branch));
        }
        let tree = self.variable((count - 1 - subject) as u32);
        let induction = shape.induct(&mut self.signature.arena, data, motive, &branches, tree);
        let arguments: Vec<_> = others
            .iter()
            .map(|&other| self.variable((count - 1 - other) as u32))
            .collect();
        Ok((
            self.signature.arena.apply_arguments(induction, &arguments),
            FunctionCases {
                data,
                subject: subject as u32,
                arity: count as u32,
                motive,
                branches,
            },
        ))
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
    /// smaller, is what makes the fuel last, and is not checked here
    /// (`OWNER_QUESTIONS.md`, "decrease of a computed subject"); a law about
    /// the machine is proved from the definition over fuel alone.
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
        let default = self.inhabitant(id, result_entry)?;
        let types: Vec<_> = formals.iter().map(|(formal, _)| *formal).collect();
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
        let all = count as u32;
        let mut branches = Vec::with_capacity(shape.cases());
        for case in 0..shape.cases() {
            let arity = shape.arity(case);
            let step =
                (arity > 0).then(|| shape.hypothesis(&mut self.signature.arena, case, 0, all));
            let mut context = vec![fuel; arity as usize];
            context.extend(vec![landing; arity as usize]);
            context.extend(types.iter().copied());
            let valuing = Valuing {
                owner: id,
                context: &context,
                result: defining.result,
                fuel: Some(Fuel {
                    machine: id,
                    step,
                    default,
                }),
                premises: defining.premises,
                cited: &[],
            };
            let slots = self.formal_slots(count, body.slots.len());
            let mut branch = self.body_value(id, slots, None, &valuing)?;
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
        let own: Vec<_> = (0..count)
            .map(|formal| self.variable((count - 1 - formal) as u32))
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
        self.fueled.push((id, over_fuel, data, measure as u32));
        // The machine is that definition with its ranked formal as the fuel.
        let head = self.constant(over_fuel);
        let mut operands = vec![own[measure]];
        operands.extend(own);
        Ok(self.signature.arena.apply_arguments(head, &operands))
    }

    /// A closed value of the type at `entry`: the first case of data that
    /// has no field, and for a record one such value per field.
    fn inhabitant(&mut self, id: MachineId, entry: u32) -> Checked<TermHandle> {
        match &self.signature.entries[entry as usize].kind {
            SignatureEntryKind::Data { shape, .. } => {
                let case = (0..shape.cases())
                    .find(|&case| shape.arity(case) == 0)
                    .ok_or_else(|| {
                        unserved(
                            id,
                            "mathematical machine recursing on fuel whose result has no case without fields",
                        )
                    })?;
                Ok(self.constant(entry + 1 + case as u32))
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
        recursion: Option<(MachineId, usize, &[TermHandle])>,
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
        arms: &[(CaseSelector, Target, crate::lowering::SourceOrigin)],
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
            });
            let mut context = vec![ty; arity as usize];
            context.extend(vec![landing; arity as usize]);
            context.extend(valuing.context.iter().copied());
            let inner = Valuing {
                context: &context,
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
    /// every variable in scope and under the defining machine's premises,
    /// proved from what the body has cited so far. A term outside a
    /// definition has nowhere to prove one, so such a call is refused there.
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
            let held = valuing.premises.len() as u32;
            let arena = &mut self.signature.arena;
            let mut statement = shift(arena, goal, 0, held);
            for (position, &premise) in valuing.premises.iter().enumerate().rev() {
                let domain = shift(arena, premise, 0, position as u32);
                statement = arena.insert(Term::Pi {
                    domain,
                    codomain: statement,
                });
            }
            for &variable in valuing.context.iter().rev() {
                statement = arena.insert(Term::Pi {
                    domain: variable,
                    codomain: statement,
                });
            }
            let owner = *program
                .symbols
                .get(valuing.owner)
                .ok_or_else(|| unserved(id, "mathematical machine lost its declaration"))?;
            // A body read twice, once per way of defining it, owes the
            // premise once.
            if self.signature.laws.iter().any(|law| {
                law.machine == owner
                    && self.signature.arena.structurally_equal(
                        self.signature.declarations[law.statement as usize].ty,
                        statement,
                    )
            }) {
                continue;
            }
            let law = index(id, self.signature.laws.len())?;
            index(id, self.signature.declarations.len() + 1)?;
            let owner_name = self.name(owner);
            let callee_name = program
                .symbols
                .get(callee)
                .map(|symbol| self.name(*symbol))
                .unwrap_or_default();
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
                premises: valuing.premises.to_vec(),
                goal,
                declared: None,
                proof: LawProof::Direct(LawCase {
                    citations: valuing.cited.to_vec(),
                    ..LawCase::default()
                }),
                formal_names: Vec::new(),
                machine_name: owner_name,
                text: format!("the premise of `{callee_name}` at its call"),
                source: program
                    .names
                    .get(valuing.owner)
                    .and_then(|names| names.declared),
            });
        }
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
        let plain = |term| Bound { term, field: None };
        match &expression.kind {
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
            ExpressionKind::Record { fields } => {
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
            ExpressionKind::Case { case, fields } => {
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
                let constructor = self.constant(data + 1 + *case as u32);
                Ok(plain(
                    self.signature.arena.apply_arguments(constructor, &operands),
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
                    let actuals: Vec<_> = operands.iter().map(|operand| operand.term).collect();
                    return Ok(plain(match fuel.step {
                        Some(step) => self.signature.arena.apply_arguments(step, &actuals),
                        None => fuel.default,
                    }));
                }
                if let Some((machine, subject, results)) = scope.recursion
                    && machine == *callee
                {
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
                    self.ranked_on(machine, subject, true)?;
                    let rest: Vec<_> = operands
                        .iter()
                        .enumerate()
                        .filter(|(position, _)| *position != subject)
                        .map(|(_, operand)| operand.term)
                        .collect();
                    return Ok(plain(
                        self.signature
                            .arena
                            .apply_arguments(results[field as usize], &rest),
                    ));
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
            _ => Err(unserved(
                id,
                "mathematical term other than a formal, a field, a constructor or a call",
            )),
        }
    }

    /// The laws a machine with no result states, declaring them the first
    /// time. A machine that states none only cites; it is read at its call.
    fn laws(&mut self, id: MachineId) -> Checked<Vec<u32>> {
        match self.begin(id, false, "law that reaches itself through its citations")? {
            None => {}
            Some(Read::Laws(laws)) => return Ok(laws),
            Some(Read::Cites { .. }) => return Ok(Vec::new()),
            Some(_) => return Err(unserved(id, "citation of a machine with a result")),
        }
        let read = self.read_laws(id);
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
        let mut premises = Vec::with_capacity(signature.requires.len());
        for premise in &signature.requires {
            premises.push(self.equality(id, premise, &open, None)?);
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
        let body = self
            .bodies
            .get(id)
            .ok_or_else(|| unserved(id, "mathematical machine lost its body"))?;
        let proof = self.script(id, body, &formals)?;
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
            // fuel at a variable bound outside the formals.
            let head = self.constant(definition);
            let mut operands = vec![self.variable(count as u32)];
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
            let arena = &mut self.signature.arena;
            let mut statement = shift(arena, goal, 0, premises.len() as u32);
            for (position, &premise) in premises.iter().enumerate().rev() {
                let domain = shift(arena, premise, 0, position as u32);
                statement = arena.insert(Term::Pi {
                    domain,
                    codomain: statement,
                });
            }
            for &(formal, _) in formals.iter().rev() {
                statement = self.signature.arena.insert(Term::Pi {
                    domain: formal,
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
                formals: formals.iter().map(|(formal, _)| *formal).collect(),
                premises: premises.clone(),
                goal,
                declared: declared.filter(|_| clause == 0),
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
                formal_names: names.formals.clone(),
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
        let Proposition::DeclarationIdentity(clause) = proposition else {
            return Err(unserved(id, "law guarantee other than an Identity"));
        };
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
        let mut terms = Vec::<TermHandle>::with_capacity(clause.values.len());
        let mut carriers = Vec::with_capacity(clause.values.len());
        for value in &clause.values {
            let ClauseCarrier::Structural(record) = value.carrier else {
                return Err(unserved(
                    id,
                    "law over a carrier other than mathematical data",
                ));
            };
            carriers.push(self.data(id, record)?);
            let term = match &value.operand {
                // Every formal is data, so the formal at position `k` is place `k + 1`.
                // The machine's own result is the place after its formals:
                // the definition it is read as, applied to those formals.
                ClauseOperand::Input(subject)
                    if subject.path().is_empty()
                        && subject.root().get() == formals.len() as u64 + 1 =>
                {
                    match self.result_as {
                        Some((machine, result)) if machine == id => result,
                        _ => {
                            let function = self.function(id)?;
                            let function = self.constant(function);
                            self.signature.arena.apply_arguments(function, formals)
                        }
                    }
                }
                ClauseOperand::Input(subject) if subject.path().is_empty() => subject
                    .root()
                    .get()
                    .checked_sub(1)
                    .and_then(|formal| formals.get(usize::try_from(formal).ok()?))
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
                        &[value] => match clause.values[value as usize].carrier {
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
                    match field {
                        Some((entry, at)) => self.component(id, entry, operands[0], at)?.0,
                        None => {
                            let head = self.declaration(id, member, satisfaction)?;
                            let head = self.constant(head);
                            self.signature.arena.apply_arguments(head, &operands)
                        }
                    }
                }
                _ => {
                    return Err(unserved(
                        id,
                        "law operand other than a formal, a constructor or a mathematical call",
                    ));
                }
            };
            terms.push(term);
        }
        let ty = self.constant(carriers[clause.left as usize]);
        Ok(self.signature.arena.insert(Term::Id {
            ty,
            left: terms[clause.left as usize],
            right: terms[clause.right as usize],
        }))
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
            if program
                .signatures
                .get(machine)
                .is_some_and(|signature| !signature.requires.is_empty())
            {
                return Err(unserved(id, "clause applies a machine that has a premise"));
            }
            return self.function(machine);
        }
        for &record in &program.self_containing {
            let Some(source) = program.record_sources.get(record) else {
                continue;
            };
            if let Some(case) = source.cases.iter().position(|(case, _)| *case == member) {
                return Ok(self.data(id, record)? + 1 + case as u32);
            }
        }
        if let Some(law) = satisfaction {
            let operation = self
                .satisfactions
                .iter()
                .find(|other| {
                    other.declaring_trait == law.declaring_trait
                        && other.scope == law.scope
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
            let slots: Vec<_> = (0..count)
                .map(|formal| {
                    Some(Bound {
                        term: self.variable((count - 1 - formal) as u32 + arity),
                        field: None,
                    })
                })
                .collect();
            let scope = Scope {
                slots: &slots,
                aliases: &aliases,
                case: Some((subject, case, &fields)),
                recursion: Some((id, subject, &[])),
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
                    crate::lowering::SourceOrigin::Authored(span) => Some(*span),
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
        Ok(LawProof::Cases {
            formal: subject as u32,
            data,
            cases,
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
        let formals = self.formals(law)?.len() as u32;
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
        arms: &[(CaseSelector, Target, crate::lowering::SourceOrigin)],
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
                        crate::lowering::SourceOrigin::Authored(span) => Some(*span),
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
            ExpressionKind::Case { fields, .. } if nested || returns => {
                for (_, operand) in fields {
                    self.cite_within(law, machine, operand, scope, script, true)?;
                }
                Ok(())
            }
            ExpressionKind::Field { base, .. } if nested || returns => {
                self.cite_within(law, machine, base, scope, script, true)
            }
            ExpressionKind::Record { fields } if nested || returns => {
                for (_, operand) in fields {
                    self.cite_within(law, machine, operand, scope, script, true)?;
                }
                Ok(())
            }
            ExpressionKind::Slot(_) | ExpressionKind::Payload { .. } if nested || returns => Ok(()),
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
        let ExpressionKind::Call {
            callee: CallTarget::Machine(callee),
            arguments,
            ..
        } = &call.kind
        else {
            return Err(unserved(machine, "law body statement other than a call"));
        };
        let mut operands = Vec::with_capacity(arguments.len());
        for argument in arguments {
            operands.push(self.expression(machine, argument, scope)?);
        }
        if *callee == law && self.fueled.iter().any(|(fueled, ..)| *fueled == law) {
            // The law of a machine defined on fuel is proved by induction on
            // the fuel, so it may be cited at any arguments: the citation is
            // the law with one unit less.
            script.citations.push(Citation {
                evidence: Evidence::Fuel,
                arguments: operands.iter().map(|operand| operand.term).collect(),
            });
            return Ok(());
        }
        if *callee == law {
            // The law at a field of the dispatched case is the induction
            // hypothesis. At any other argument it would be its own evidence.
            let Some((_, subject, _)) = scope.recursion else {
                return Err(unserved(
                    machine,
                    "law cites itself without dispatching on a formal",
                ));
            };
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
            script.citations.push(Citation {
                evidence: Evidence::Hypothesis { field },
                arguments: operands
                    .iter()
                    .enumerate()
                    .filter(|(position, _)| *position != subject)
                    .map(|(_, operand)| operand.term)
                    .collect(),
            });
            return Ok(());
        }
        let cited = self
            .program
            .signatures
            .get(*callee)
            .ok_or_else(|| unserved(machine, "citation lost its machine"))?;
        if cited.result.is_some() && !self.states_law(*callee) && !self.helper(*callee) {
            // An ordinary function. As an operand it is a term and cites
            // nothing; as a statement it would say nothing.
            return if term {
                Ok(())
            } else {
                Err(unserved(
                    machine,
                    "law body calls a machine with a result that states no law",
                ))
            };
        }
        let laws = self.laws(*callee)?;
        if !laws.is_empty() {
            let arguments: Vec<_> = operands.iter().map(|operand| operand.term).collect();
            for cited in laws {
                script.citations.push(Citation {
                    evidence: Evidence::Statement(self.signature.laws[cited as usize].statement),
                    arguments: arguments.clone(),
                });
            }
            return Ok(());
        }
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
