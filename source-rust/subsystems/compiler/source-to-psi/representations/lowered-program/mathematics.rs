//! The mathematical declarations of one compilation as a single kernel
//! signature. Stage 05 writes it from the typed declarations; stage 06 checks
//! it once with the kernel and judges each law against the entries before it.
//!
//! Data that contains itself has no storage, so it and the machines over it
//! exist only here: the data as a native inductive declaration with its
//! constructors and induction principle, each machine defined by cases as a
//! definition, and each law as a statement whose proof stage 06 produces. A law's own statement is not among the entries its
//! proof is checked against, so a law cannot supply its own evidence.

use mathematical_kernel::{Declaration, PlainData, Term, TermArena, TermHandle};
use source_text::SourceSpan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default)]
pub struct MathematicalSignature {
    pub arena: TermArena,
    /// In dependency order: a declaration refers only to earlier ones.
    pub declarations: Vec<Declaration>,
    /// What each declaration is, by the same index.
    pub entries: Vec<SignatureEntry>,
    pub laws: Vec<Law>,
    /// The laws that let a proof regroup or reorder the operands of one
    /// operation, in the order of their statements. A proof may use the
    /// ones whose statement comes before its own law's.
    pub licenses: Vec<OperandLicense>,
    /// Where the integer vocabulary is declared, when the program has data
    /// indexed by integers: the carrier, its relations and operations, and
    /// the numeral constructors. Stage 06 searches for proofs against it.
    pub integers: Option<IntegerPositions>,
    /// Each data declaration indexed by values, as the kernel's builder
    /// declared it. A law over one names it by its position here.
    pub families: Vec<mathematical_kernel::DeclaredIndexedData>,
    /// What the Identity goals of each generic check form with const
    /// record parameters name that is the same in every one of its
    /// obligations: the carriers its trait operations take and give, those
    /// operations and the laws its machines may cite. Stage 06 declares and
    /// checks each table once, and every Identity obligation of the form
    /// starts from it.
    pub identities: Vec<IdentityTable>,
}

/// The Identity table of one generic check form. Its carriers and law
/// instances are identities of that form's module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityTable {
    /// The check form: its generic declaration's position in
    /// `PreparedCompilation::forms`, then its own among that declaration's
    /// forms.
    pub form: (u32, u32),
    /// The form's const record parameters, the suppliers of its trait
    /// operations. An Identity obligation that applies an operation of one of
    /// them must find it here.
    pub suppliers: Vec<symbols::SymbolHandle>,
    /// Every carrier the operations and laws name: the form's type
    /// parameters and whatever data or scalars the operations take and give.
    pub carriers: Vec<semantic_vocabulary::ClauseCarrier>,
    pub operations: Vec<IdentityOperation>,
    pub laws: Vec<IdentityLaw>,
}

/// A trait operation, as a function from its domains to its result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityOperation {
    pub declaration: semantic_vocabulary::ClauseDeclaration,
    pub domains: Vec<semantic_vocabulary::ClauseCarrier>,
    pub result: semantic_vocabulary::ClauseCarrier,
}

/// One `ensures` equality of a law requirement of one of the form's const
/// record parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityLaw {
    /// The machine that stands for the requirement in the form's module.
    pub instance: semantic_vocabulary::MachineId,
    /// Its position among that machine's `ensures` clauses.
    pub clause: u32,
    /// The carrier of each formal, structural formals first and then scalar
    /// ones, as a citation supplies them.
    pub formals: Vec<semantic_vocabulary::ClauseCarrier>,
    /// The equality, with formal `k` written `ClauseOperand::Formal(k)`.
    pub statement: semantic_vocabulary::ClauseIdentity,
}

/// The declarations of the integer vocabulary, by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerPositions {
    pub carrier: u32,
    pub less_than: u32,
    pub less_or_equal: u32,
    pub add: u32,
    pub subtract: u32,
    pub multiply: u32,
    pub numeral_zero: u32,
    pub numeral_double: u32,
    pub numeral_odd: u32,
    pub numeral_negate: u32,
}

/// What a law says about the operands of one binary operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandLaw {
    /// `op(x, y) = op(y, x)`.
    Commutes,
    /// `op(op(x, y), z) = op(x, op(y, z))`, or the same equality with its
    /// sides exchanged when `left_nested_first` is false.
    Associates { left_nested_first: bool },
    /// `op` distributes over the operation at entry `over`: from the left,
    /// `op(x, over(y, z)) = over(op(x, y), op(x, z))`; from the right,
    /// `op(over(x, y), z) = over(op(x, z), op(y, z))`. When `product_first`
    /// is false the statement has its sides exchanged.
    Distributes {
        over: u32,
        from_left: bool,
        product_first: bool,
    },
}

/// A law that satisfies a requirement of a trait application and states
/// `law` of an operation satisfying a requirement of the same application.
/// For distributivity the operation it distributes over must satisfy a
/// requirement of that application too. A program licenses rearrangement by
/// declaring the conformance; a law that merely has the right shape does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperandLicense {
    /// The entry of the operation.
    pub operation: u32,
    /// The entry of the law's statement.
    pub statement: u32,
    pub law: OperandLaw,
}

/// Reads a closed statement as commutativity or associativity of the one
/// operation it applies, or as distributivity of one operation over another,
/// over formals of one type. Stage 05 uses it on a requirement to read such
/// laws first and on each statement to record the license; nothing else
/// decides what counts as any of these laws.
pub fn operand_law(arena: &TermArena, statement: TermHandle) -> Option<(u32, OperandLaw)> {
    let mut body = statement;
    let mut formals = Vec::new();
    while let Term::Pi { domain, codomain } = arena.get(body) {
        formals.push(domain);
        body = codomain;
    }
    let Term::Id { ty, left, right } = arena.get(body) else {
        return None;
    };
    if formals
        .iter()
        .any(|&formal| !arena.structurally_equal(formal, ty))
    {
        return None;
    }
    let applied = |term| {
        let Term::Apply {
            function,
            argument: second,
        } = arena.get(term)
        else {
            return None;
        };
        let Term::Apply {
            function: head,
            argument: first,
        } = arena.get(function)
        else {
            return None;
        };
        match arena.get(head) {
            Term::Constant { declaration, .. } => Some((declaration, first, second)),
            _ => None,
        }
    };
    let is = |term, index| arena.get(term) == Term::Variable(index);
    // `op(first, second)` over two formals, for a given operation.
    let over_formals = |term, op, first, second| {
        applied(term).is_some_and(|(head, a, b)| head == op && is(a, first) && is(b, second))
    };
    // A product of a formal and a sum of two formals, and the sum of the two
    // products it expands to, under two different operations.
    let distribution = |product, sum| -> Option<(u32, u32, bool)> {
        let (outer, first, second) = applied(product)?;
        let (inner, x, y) = applied(sum)?;
        if inner == outer {
            return None;
        }
        if is(first, 2)
            && over_formals(second, inner, 1, 0)
            && over_formals(x, outer, 2, 1)
            && over_formals(y, outer, 2, 0)
        {
            return Some((outer, inner, true));
        }
        if over_formals(first, inner, 2, 1)
            && is(second, 0)
            && over_formals(x, outer, 2, 0)
            && over_formals(y, outer, 1, 0)
        {
            return Some((outer, inner, false));
        }
        None
    };
    if formals.len() == 3 {
        for (product, sum, product_first) in [(left, right, true), (right, left, false)] {
            if let Some((outer, over, from_left)) = distribution(product, sum) {
                return Some((
                    outer,
                    OperandLaw::Distributes {
                        over,
                        from_left,
                        product_first,
                    },
                ));
            }
        }
    }
    let (operation, ..) = applied(left)?;
    // `op(first, second)` over two formals, by their distance from the body.
    let pair = |term, first, second| {
        applied(term).is_some_and(|(head, a, b)| head == operation && is(a, first) && is(b, second))
    };
    let law = match formals.len() {
        2 if pair(left, 1, 0) && pair(right, 0, 1) => OperandLaw::Commutes,
        3 => {
            let left_nested = |term| {
                applied(term).is_some_and(|(head, inner, last)| {
                    head == operation && is(last, 0) && pair(inner, 2, 1)
                })
            };
            let right_nested = |term| {
                applied(term).is_some_and(|(head, first, inner)| {
                    head == operation && is(first, 2) && pair(inner, 1, 0)
                })
            };
            if left_nested(left) && right_nested(right) {
                OperandLaw::Associates {
                    left_nested_first: true,
                }
            } else if right_nested(left) && left_nested(right) {
                OperandLaw::Associates {
                    left_nested_first: false,
                }
            } else {
                return None;
            }
        }
        _ => return None,
    };
    Some((operation, law))
}

impl MathematicalSignature {
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty() && self.laws.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct SignatureEntry {
    pub kind: SignatureEntryKind,
    /// The source spelling, for diagnostics only.
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum SignatureEntryKind {
    /// A plain data declaration, or one application of generic data: its
    /// type, whose cases and fields `shape` describes. The constructors and
    /// the induction principle follow it.
    Data {
        declaration: SymbolHandle,
        shape: PlainData,
        /// The data's source type identity text. Stage 06 publishes it as
        /// the identity of the data's mathematical carrier row; nothing
        /// compares it while checking.
        type_identity: String,
    },
    /// A record with no cases whose fields are all mathematical data: the
    /// product of its fields' types, right-nested in declared order. A value
    /// is the tuple of its fields, a field is a projection of it, and a
    /// record of one field is that field. `fields` holds the entry of each
    /// field's type.
    Record {
        declaration: SymbolHandle,
        fields: Vec<u32>,
        type_identity: String,
    },
    /// `bool`, the carrier of a Boolean formal or result: a definition equal
    /// to the kernel's `Two`, with `false` its zero and `true` its one
    /// (kernel_metatheory.md, "Boolean values denote `Two`"). A signature
    /// declares it once, the first time a machine needs it.
    Boolean,
    /// The inductive former of a generic data declaration (`Seq<T>`), a
    /// function from its type parameters to a type. Its constructors and
    /// recursor follow it as `Encoding` entries; each application the
    /// program uses is declared after them as a `Data` entry equal to the
    /// former at the application's type arguments.
    GenericData { declaration: SymbolHandle },
    /// A case of the data declared at index `data`, as a function of its fields.
    Constructor { data: u32, case: u32 },
    /// A declaration of the data at index `data` besides the data and its
    /// constructors: its induction principle, or a constructor or the
    /// recursor of a generic block.
    Encoding { data: u32 },
    /// A machine with a result. `cases` says how it computes when its body
    /// dispatches on one parameter.
    Function {
        machine: SymbolHandle,
        cases: Option<FunctionCases>,
    },
    /// The statement of `laws[law]`, closed over its formals. It has no body:
    /// a proof that cites the law depends on it as an assumption, and the
    /// law's own proof is judged separately.
    Statement { law: u32 },
    /// One declaration of the kernel's quotient scheme
    /// (`mathematical_kernel::quotient_scheme`). The kernel numbers those
    /// declarations from zero, so a signature that has a quotient starts
    /// with them, in the kernel's order.
    Scheme,
    /// A quotient of the data at entry `carrier` by the relation defined at
    /// entry `relation`: the kernel's quotient of the two. A value's class
    /// is the kernel's projection of it. The quotient, its projection and
    /// its soundness law are assumptions of the scheme, so a proof that
    /// uses them reports them in its closure.
    Quotient {
        declaration: SymbolHandle,
        carrier: u32,
        relation: u32,
        /// Statement entries for the four selected `MereEquivalence` laws:
        /// `[propositional, reflexive, symmetric, transitive]`.
        laws: [u32; 4],
        type_identity: String,
    },
}

/// A function defined by cases on one parameter: the induction over the data
/// at entry `data` whose subject is formal `subject`. `motive` and each
/// branch are closed, as `PlainData::induct` takes them. Applied to a case
/// of the data in that position, the function is that case's branch over the
/// case's fields, then the function itself at each field, then its other
/// arguments.
#[derive(Debug, Clone)]
pub struct FunctionCases {
    pub data: u32,
    pub subject: u32,
    /// How many formals the function has.
    pub arity: u32,
    pub motive: TermHandle,
    pub branches: Vec<TermHandle>,
}

/// One `ensures` equality of a machine with no result, and how its body
/// argues for it.
#[derive(Debug, Clone)]
pub struct Law {
    pub machine: SymbolHandle,
    /// Which `ensures` clause of the machine this is.
    pub clause: u32,
    /// The index of this law's statement entry.
    pub statement: u32,
    /// The closed type of each formal, in order.
    pub formals: Vec<TermHandle>,
    /// The equalities the machine requires, each under one binder per
    /// formal. The statement quantifies a proof of each after the formals,
    /// the proof may use them as hypotheses, and whoever cites the law must
    /// prove each at the cited arguments.
    pub premises: Vec<TermHandle>,
    /// The equality, under one binder per formal.
    pub goal: TermHandle,
    /// The statement this law must be, when something other than its own
    /// machine decides it.
    pub declared: Option<DeclaredLaw>,
    pub proof: LawProof,
    /// For diagnostics only.
    pub formal_names: Vec<String>,
    pub machine_name: String,
    pub text: String,
    pub source: Option<SourceSpan>,
}

/// The statement a law must be: the one the trait requirement its machine
/// satisfies declares, with each operation resolved to its satisfier, or
/// the one owed by the declaration that selects the machine as its
/// evidence (an equivalence of a quotient, a theorem of a quotient
/// operation).
#[derive(Debug, Clone)]
pub struct DeclaredLaw {
    /// Each premise, under one binder per formal. The law must have exactly
    /// these and no other. Their order is not compared: a machine's
    /// `requires` clauses are kept in canonical order, not as written.
    pub premises: Vec<TermHandle>,
    /// The equality, under one binder per formal.
    pub goal: TermHandle,
    /// What to tell the author when the law is another statement: who
    /// needs this one. For diagnostics only.
    pub refusal: String,
}

#[derive(Debug, Clone)]
pub enum LawProof {
    /// The body cites its evidence without a case split.
    Direct(LawCase),
    /// The body dispatches on one formal: an induction over the data
    /// declared at entry `data`, with one script per case in declared order.
    Cases {
        formal: u32,
        data: u32,
        cases: Vec<LawCase>,
    },
    /// The machine is defined by recursion on fuel, over the data at entry
    /// `data`, and takes its fuel from formal `formal`. Its law is proved
    /// for every amount of fuel, by induction on it: `goal` is the law's
    /// equality with the machine's result read as that definition, stated
    /// under the fuel and then the formals. `script` is the body's
    /// argument, the same in every case of the fuel.
    Fuel {
        data: u32,
        formal: u32,
        goal: TermHandle,
        script: LawCase,
    },
    /// A goal about integers, proved from the law's premises by the integer
    /// search: what a case of indexed data requires of a value a running
    /// machine builds.
    Integers,
    /// A theorem over data indexed by values (`families[family]`): an
    /// induction over the evidence. The law's formals are the indices, and
    /// its goal, as a function of the index, is the declared `invariant`.
    /// There is one case per case of the data, in declared order.
    Family {
        family: u32,
        invariant: TermHandle,
        cases: Vec<FamilyLawCase>,
    },
}

/// One case of an induction over data indexed by values. Its terms are
/// stated under one binder per index, then one per payload value.
#[derive(Debug, Clone, Default)]
pub struct FamilyLawCase {
    /// How many payload values the case holds.
    pub payload: u32,
    /// What the data's declaration requires of a value in this case.
    pub constraints: Vec<TermHandle>,
    /// For each child, the index it sits at, one term per index.
    pub children: Vec<Vec<TermHandle>>,
    /// For each child, whether the body cites the theorem at it: only then
    /// is the theorem at that child a hypothesis of the case.
    pub cited: Vec<bool>,
    /// What the step the case calls requires. Each must follow from
    /// `constraints`.
    pub owed: Vec<TermHandle>,
    /// That each index and payload value of an unsigned type is not
    /// negative. The family's values are integers; these say what the
    /// source's types add, and only `descent` rests on them.
    pub natural: Vec<TermHandle>,
    /// For each child the body cites the theorem at: the index the theorem
    /// is ranked on is smaller at the child. Each must follow from
    /// `constraints` and `natural`.
    pub descent: Vec<TermHandle>,
    /// The case as the source names it. For diagnostics only.
    pub name: String,
    pub source: Option<SourceSpan>,
}

#[derive(Debug, Clone, Default)]
pub struct LawCase {
    pub citations: Vec<Citation>,
    /// The fields the case binds, for diagnostics only.
    pub field_names: Vec<String>,
    pub source: Option<SourceSpan>,
    /// The case's own cases, when its argument goes on by splitting another
    /// value after the citations above.
    pub split: Option<LawSplit>,
}

/// A split inside a case of a law: one script per case of the value split,
/// in declared order. It is a case analysis only. The law as a hypothesis
/// comes from the induction of [`LawProof::Cases`], never from here.
#[derive(Debug, Clone)]
pub struct LawSplit {
    pub subject: SplitSubject,
    pub data: u32,
    pub cases: Vec<LawCase>,
}

/// What a case of a law splits on.
#[derive(Debug, Clone, Copy)]
pub enum SplitSubject {
    /// A formal of the law by position or, past the formals, a field of an
    /// enclosing case, in the order the cases bind them.
    Name(u32),
    /// A computed value, as a term under those names. Each case then also
    /// holds the equality of the value and the case.
    Value(TermHandle),
}

/// A call the body makes for its conclusion. Each argument is a term under
/// the law's formals followed by the fields of every enclosing case,
/// outermost first.
#[derive(Debug, Clone)]
pub struct Citation {
    pub evidence: Evidence,
    pub arguments: Vec<TermHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// Another law, by its statement entry. Stage 05 admits only an earlier
    /// entry, so citations cannot form a cycle.
    Statement(u32),
    /// The law itself at one field of the case: the induction hypothesis.
    /// `arguments` then holds the remaining formals' actuals in order.
    Hypothesis { field: u32 },
    /// The law itself with one unit of fuel less, in a law proved by
    /// induction on fuel. `arguments` holds every formal's actual. With no
    /// fuel left there is no such hypothesis and the citation is nothing.
    Fuel,
}
