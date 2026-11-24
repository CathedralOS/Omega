//! The mathematical declarations of one compilation as a single kernel
//! signature. Stage 05 writes it from the typed declarations; stage 06 checks
//! it once with the kernel and judges each law against the entries before it.
//!
//! Data that contains itself has no storage, so it and the machines over it
//! exist only here: the data as the canonical trees of a W type with its
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
    /// A plain data declaration: the canonical trees of its cases.
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
    /// A case of the data declared at index `data`, as a function of its fields.
    Constructor { data: u32, case: u32 },
    /// A declaration the encoding of the data at index `data` needs besides
    /// the data and its constructors: its case labels, their child
    /// positions, its raw trees, the canonical-tree condition or its
    /// induction principle.
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
    /// The same formals' equality declared by the trait requirement this
    /// machine satisfies, with each operation resolved to its satisfier.
    pub declared: Option<TermHandle>,
    pub proof: LawProof,
    /// For diagnostics only.
    pub formal_names: Vec<String>,
    pub machine_name: String,
    pub text: String,
    pub source: Option<SourceSpan>,
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
}

#[derive(Debug, Clone, Default)]
pub struct LawCase {
    pub citations: Vec<Citation>,
    /// The fields the case binds, for diagnostics only.
    pub field_names: Vec<String>,
    pub source: Option<SourceSpan>,
}

/// A call the body makes for its conclusion. Each argument is a term under
/// the law's formals followed by the fields of the enclosing case.
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
}
