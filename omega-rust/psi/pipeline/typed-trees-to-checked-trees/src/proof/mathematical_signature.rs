//! Kernel-term elaboration for mathematical `let`/`boundary let`
//! declarations (PROOF-CONTRACT-MIGRATION).
//!
//! `mathematical_declarations` records each authored declaration onto the
//! checked surface as identity strings; this module re-walks the typed
//! mirror (`typed_trees::mathematical`) and elaborates every declaration
//! into a `mathematical_core::signature::Declaration` — a closed
//! `level_arity`/`ty`/`body` triple over `TermArena` handles — then runs
//! `check_signature` so the kernel itself re-decides that each statement
//! is a type and each transparent body inhabits it. A `boundary let` is a
//! named assumption (`body: None`).
//!
//! Scope model:
//!
//! - `u: core::Level` binders become universe parameters (`level_arity`)
//!   in authored order and take no term position; every other binder and
//!   every telescope parameter is a term entry whose domain is elaborated
//!   under the entries before it. The declared type folds to nested `Pi`
//!   and a body to nested `Lambda` over the same ordered entries, so the
//!   λ-spine reuses each entry's domain handle: entry `k`'s domain was
//!   built in the context of entries `0..k`, which is exactly the context
//!   both the `Pi` domain position and the `Lambda` domain annotation
//!   occupy. A named arrow binder scopes over its codomain while that
//!   codomain elaborates.
//! - A binder whose carrier is `core::Type<u>` elaborates to
//!   `Sort::Type(Level::Parameter u)`; a bare `T` or unapplied
//!   `core::Type` binder claims the next fresh universe parameter, so the
//!   generalized level telescope is deterministic and published exactly
//!   by `level_arity` (the spec's generalization rule) — it is never
//!   silently pinned to a fixed level. `core::Strict<v>` elaborates to
//!   `Sort::Strict` and `core::Squash<A>` to `Squash`.
//! - Fixed integer carriers (`i8`–`u64` and the `Int`/`UInt` proof
//!   integers) share the bounded denotation's `Int : Type 0` carrier;
//!   `bool` and `addr` share one dedicated carrier each, kept distinct
//!   from `Int` because the bounded vocabulary's order relations are
//!   fixed non-address integers. Every other resolved non-core symbol —
//!   authored `data` types, other builtin atoms — interns a per-symbol
//!   `Declaration::assumption(0, Type 0)` in the shared signature prefix
//!   (the `bounded_denotation` pattern), so later declarations can
//!   reference earlier ones through `Term::Constant`.
//! - Machine-valued body expressions denote into the same bounded
//!   vocabulary: closed integer arithmetic evaluates to `Int` literal
//!   constants interned by exact value, open `+`/`-` share the
//!   `IntAdd`/`IntSub` function assumptions, comparisons and `&&`/`||`
//!   denote `Type 0` propositions (`IntLt`/`IntLe`, `Id` over the
//!   operand's scalar carrier, right-nested `Σ`, and the tagged
//!   `Σ(t : Two). caseTwo` sum), and a `core::Strict` result wraps the
//!   proposition in `Squash` at the authored boundary.
//! - Explicit generic applications follow the callee's ordered telescope:
//!   level binders instantiate `Constant.levels`, while the remaining binders
//!   form ordinary `Apply` terms before the ordinary argument prefix. A binder
//!   that claimed a generalized universe parameter infers it from the supplied
//!   type argument: the argument's inferred sort is `Type l`, so `l` is the
//!   instantiation. Authored `core::Level` binders still require their explicit
//!   argument — omitted ones are underdetermined and refuse — and the kernel
//!   rechecks every level's scope and the instantiated dependent applications.
//! - Inside a declaration, references resolve symbols first (parameters,
//!   binders, earlier declarations) and fall back to names only for the
//!   symbol-less nodes the mirror carries — a parameter used as a call
//!   callee arrives with an invalid `target_symbol`, and an arrow binder
//!   only ever scopes by name. An unresolved name refuses rather than
//!   guessing.
//!
//! This leg refuses loudly rather than denoting what it cannot decide:
//! level arguments omitted where no explicit type argument determines them
//! (a bare reference to a generalized declaration), references to the declaration being elaborated or a later one,
//! machine/evidence/quotient/private-layout call payloads, borrow /
//! constrained / dynamic-trait / array / slice / unit type references,
//! computed level expressions other than literals, `!=` and unary
//! operators (the bounded vocabulary holds no negation), order relations
//! and scalar equality over operands whose carriers differ or cannot be
//! determined, open machine arithmetic beyond exact `+`/`-` (products,
//! quotients, shifts and bitwise operations denote only when the whole
//! expression evaluates closed), and every remaining body expression
//! form (members, matches, casts, ...).

mod applications;

use std::collections::{BTreeMap, HashMap};

use numerics::bignum::BigInt;
use numerics::literals::{IntegerLiteral, LandedIntegerType};
use proof_admission::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Level, Signature, Sort, Term,
    TermArena, TermHandle, check_signature, infer_sort, shift,
};
use source::SourceSpan;
use symbols::{BuiltinTypeAtom, SymbolHandle};
use typed_trees::TypedTrees;
use typed_trees::data::{DataProperties, TypeParameter, TypeParameterKind};
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableNamePath,
};
use typed_trees::mathematical::{
    MathematicalBody, MathematicalDefinition, MathematicalType, MathematicalTypeHandle,
};
use typed_trees::name::Identifier;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// A checked kernel signature for a typed program's mathematical
/// declarations: the shared term arena, the re-decided declaration list
/// (carrier assumptions interleaved with authored declarations in use
/// order), and each authored declaration's signature position.
pub(crate) struct CheckedMathematicalSignature {
    // Downstream consumption of the checked signature is a separate leg;
    // only the checking gate and tests read these today.
    #[allow(dead_code)]
    arena: TermArena,
    #[allow(dead_code)]
    signature: Signature,
    /// Kernel signature position of each authored declaration, in
    /// authored declaration order.
    #[allow(dead_code)]
    authored: Vec<u32>,
}

impl CheckedMathematicalSignature {
    #[cfg(test)]
    pub(crate) fn signature(&self) -> &Signature {
        &self.signature
    }

    #[cfg(test)]
    pub(crate) fn authored(&self) -> &[u32] {
        &self.authored
    }

    #[cfg(test)]
    pub(crate) fn term(&self, handle: TermHandle) -> Term {
        self.arena.get(handle)
    }
}

/// One scoped term entry: a telescope entry (non-level binder or
/// parameter) or a named arrow binder. `Variable(depth - 1 - position)`
/// resolves it.
struct ScopeEntry {
    /// `None` for named arrow binders, which scope by authored name only.
    symbol: Option<SymbolHandle>,
    name: Identifier,
    /// The entry's domain, built in the context of the entries before it.
    domain: TermHandle,
}

struct Elaborator<'a> {
    program: &'a TypedTrees,
    arena: TermArena,
    /// Signature-order declarations: lazily interned carrier assumptions
    /// each land immediately before the authored declaration whose
    /// elaboration discovered them.
    declarations: Vec<Declaration>,
    /// Carrier symbol -> signature position (a `Type 0` assumption).
    carriers: HashMap<SymbolHandle, u32>,
    /// `Int : Type 0` — the shared mathematical-integer carrier every
    /// fixed-width integer carrier denotes into: `i8`–`u64`, `Int` and
    /// `UInt` all name it, exactly as the bounded denotation's scalar
    /// terms embed.
    integer: Option<u32>,
    /// `bool`'s shared carrier — `Boolean` literals and `bool`-carried
    /// operands denote at it.
    boolean: Option<u32>,
    /// `addr`'s carrier — deliberately distinct from `Int`: the bounded
    /// vocabulary's order relations are fixed non-address integers only.
    address: Option<u32>,
    /// `IntLt`/`IntLe : Π(_ : Int). Π(_ : Int). Type 0`, interned on demand.
    integer_less_than: Option<u32>,
    integer_less_or_equal: Option<u32>,
    /// `IntAdd`/`IntSub : Π(_ : Int). Π(_ : Int). Int` — open exact
    /// addition and subtraction compose; every other open machine
    /// operation has no bounded denotation and refuses.
    integer_add: Option<u32>,
    integer_subtract: Option<u32>,
    /// Closed scalar literals interned by `(carrier position, exact
    /// value)` — the denotation is by value, so `2 + 0` and `2` name one
    /// constant.
    numeric_literals: BTreeMap<(u32, BigInt), u32>,
    /// `true`/`false` interned at the `bool` carrier.
    boolean_literals: [Option<u32>; 2],
    /// Elaborated authored declaration symbol -> signature position.
    authored_positions: HashMap<SymbolHandle, u32>,
    /// Every authored declaration symbol -> authored index, so a reference
    /// to an unelaborated position refuses as self/forward rather than
    /// unknown.
    authored_index: HashMap<SymbolHandle, usize>,
    /// Kernel positions of authored declarations, in authored order.
    authored: Vec<u32>,
    /// Authored `core::Level` binder symbols in authored order; position
    /// is the `Level::Parameter` index.
    level_binders: Vec<SymbolHandle>,
    /// Matching authored names, for level references that arrive without
    /// a resolved symbol.
    level_names: Vec<Identifier>,
    /// Universe parameters claimed by generalized (bare or unapplied
    /// `Type`) binders; appended after the authored level binders.
    generalized_levels: u32,
    /// Term scope for the declaration under elaboration: ordered
    /// telescope entries plus in-flight arrow binders.
    scope: Vec<ScopeEntry>,
    /// Source span of the declaration under elaboration, for diagnostics.
    span: Option<SourceSpan>,
}

/// Elaborate every mathematical declaration of `program` into a kernel
/// signature and re-decide it. Returns the checked signature, or the
/// elaboration/checking diagnostics for the first declaration that fails.
pub(crate) fn check_mathematical_signature(
    program: &TypedTrees,
) -> Result<CheckedMathematicalSignature, Vec<diagnostics::Diagnostic>> {
    let mut elaborator = Elaborator {
        program,
        arena: TermArena::new(),
        declarations: Vec::new(),
        carriers: HashMap::new(),
        integer: None,
        boolean: None,
        address: None,
        integer_less_than: None,
        integer_less_or_equal: None,
        integer_add: None,
        integer_subtract: None,
        numeric_literals: BTreeMap::new(),
        boolean_literals: [None, None],
        authored_positions: HashMap::new(),
        authored_index: program
            .mathematical_definitions()
            .iter()
            .enumerate()
            .map(|(index, definition)| (definition.symbol, index))
            .collect(),
        authored: Vec::new(),
        level_binders: Vec::new(),
        level_names: Vec::new(),
        generalized_levels: 0,
        scope: Vec::new(),
        span: None,
    };
    for definition in program.mathematical_definitions() {
        elaborator.elaborate_declaration(definition)?;
    }
    elaborator.check()
}

impl<'a> Elaborator<'a> {
    /// Elaborate one authored declaration into a `Declaration` and append
    /// it. Binder carriers classify first so authored level parameters are
    /// fixed before any domain elaborates; then domains elaborate in
    /// authored order under the entries that precede them.
    fn elaborate_declaration(
        &mut self,
        definition: &MathematicalDefinition,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        self.span = self.program.symbols.symbol_source_span(definition.symbol);
        self.level_binders.clear();
        self.level_names.clear();
        self.generalized_levels = 0;
        self.scope.clear();

        let binders = self
            .program
            .data_type_parameters
            .span_or_empty(definition.binders);
        let plans = binders
            .iter()
            .map(|binder| self.plan_binder(binder))
            .collect::<Result<Vec<_>, _>>()?;
        for (binder, plan) in binders.iter().zip(plans) {
            match plan {
                BinderPlan::Level => {}
                BinderPlan::Type(universe) => {
                    let level = match universe {
                        LevelArgument::Authored(reference) => self.elaborate_level(reference)?,
                        LevelArgument::Generalized => self.generalized_level(),
                    };
                    let domain = self.arena.insert(Term::Sort(Sort::Type(level)));
                    self.push_scope(Some(binder.symbol), binder.name.clone(), domain);
                }
                BinderPlan::Term(carrier) => {
                    let domain = self.elaborate_type_reference(carrier)?;
                    self.push_scope(Some(binder.symbol), binder.name.clone(), domain);
                }
            }
        }
        for parameter in self.program.mathematical_parameters(definition.parameters) {
            let domain = self.elaborate_mathematical_type(parameter.ty)?;
            self.push_scope(Some(parameter.symbol), parameter.name.clone(), domain);
        }

        let result = self.elaborate_mathematical_type(definition.result)?;
        // A `core::Strict` result marks the authored body as a
        // proposition: the denotation lands at `Type 0` and `Squash`
        // places it in the strict layer. Nothing else squashes — a
        // `Type` carrier keeps the same proposition relevant.
        let strict_result = matches!(self.arena.get(result), Term::Sort(Sort::Strict(_)));
        let mut ty = result;
        for entry in self.scope.iter().rev() {
            ty = self.arena.insert(Term::Pi {
                domain: entry.domain,
                codomain: ty,
            });
        }
        let body = match &definition.body {
            MathematicalBody::Assumption => None,
            MathematicalBody::Definition(term) => {
                let mut body = self.elaborate_body(*term, strict_result)?;
                for entry in self.scope.iter().rev() {
                    body = self.arena.insert(Term::Lambda {
                        domain: entry.domain,
                        body,
                    });
                }
                Some(body)
            }
        };
        let level_arity = self.level_binders.len() as u32 + self.generalized_levels;
        let position = self.declarations.len() as u32;
        self.declarations.push(Declaration {
            level_arity,
            ty,
            body,
        });
        self.authored_positions.insert(definition.symbol, position);
        self.authored.push(position);
        Ok(())
    }

    /// Classify one generic binder. `core::Level` carriers claim a level
    /// parameter (registered immediately so authored-order arity is fixed
    /// before any domain elaborates); `core::Type` carriers and bare
    /// `Type` binders become term entries over a `Sort::Type` domain at an
    /// authored or generalized level; every other carrier is an ordinary
    /// term domain.
    fn plan_binder(
        &mut self,
        binder: &TypeParameter,
    ) -> Result<BinderPlan, Vec<diagnostics::Diagnostic>> {
        if binder.bounds != DataProperties::default() {
            return Err(self.refuse(format!(
                "mathematical binder `{}` cannot carry property bounds",
                binder.name
            )));
        }
        match &binder.kind {
            TypeParameterKind::Type => Ok(BinderPlan::Type(LevelArgument::Generalized)),
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => {
                match self.carrier_head(*type_reference) {
                    CarrierHead::Level => {
                        self.level_binders.push(binder.symbol);
                        self.level_names.push(binder.name.clone());
                        Ok(BinderPlan::Level)
                    }
                    CarrierHead::MalformedLevel => Err(self.refuse(format!(
                        "mathematical binder `{}` applies arguments to `core::Level`; it takes none",
                        binder.name
                    ))),
                    CarrierHead::Type(universe) => Ok(BinderPlan::Type(universe)),
                    CarrierHead::MalformedType(argument_count) => Err(self.refuse(format!(
                        "mathematical binder `{}` applies `core::Type` to {} arguments; it takes one level",
                        binder.name, argument_count
                    ))),
                    CarrierHead::Term => Ok(BinderPlan::Term(*type_reference)),
                }
            }
            TypeParameterKind::Machine { .. } | TypeParameterKind::Proposition { .. } => Err(self
                .refuse(format!(
                    "mathematical binder `{}` cannot carry a machine or proposition contract",
                    binder.name
                ))),
        }
    }

    /// Peel the fixed-core carrier vocabulary off a binder carrier without
    /// elaborating it. Until the `core::*` declarations exist, authored
    /// names carry the convention (the same rule the checked surface leg
    /// uses); symbol identity replaces name matching once they land.
    fn carrier_head(&self, carrier: TypeReferenceHandle) -> CarrierHead {
        if !self
            .program
            .type_reference_table
            .contains_type_reference(carrier)
        {
            return CarrierHead::Term;
        }
        match self.program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Named { name, .. } => match name.as_str() {
                "core::Level" => CarrierHead::Level,
                "core::Type" => CarrierHead::Type(LevelArgument::Generalized),
                _ => CarrierHead::Term,
            },
            TypeReferenceNode::Generic {
                base_name,
                arguments,
                ..
            } => match base_name.as_str() {
                "core::Level" => CarrierHead::MalformedLevel,
                "core::Type" => {
                    let arguments = self
                        .program
                        .type_reference_table
                        .type_reference_handles(*arguments);
                    match arguments {
                        [universe] => CarrierHead::Type(LevelArgument::Authored(*universe)),
                        _ => CarrierHead::MalformedType(arguments.len()),
                    }
                }
                _ => CarrierHead::Term,
            },
            _ => CarrierHead::Term,
        }
    }

    /// Elaborate a universe-level argument: an authored `core::Level`
    /// binder resolves to `Level::Parameter` by authored position, and an
    /// integer literal (`core::Strict<0>`) is a closed `Level::Constant`.
    /// Computed level expressions are a later leg.
    fn elaborate_level(
        &mut self,
        reference: TypeReferenceHandle,
    ) -> Result<Level, Vec<diagnostics::Diagnostic>> {
        if !self
            .program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err(self.refuse(
                "a universe-level argument references a type that does not exist".to_owned(),
            ));
        }
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, name } => {
                if let Some(index) = self.level_index(*symbol, name) {
                    return Ok(Level::Parameter(index));
                }
                if symbol.is_valid() {
                    return Err(self.refuse(format!(
                        "`{name}` is not a `core::Level` binder of this declaration"
                    )));
                }
                // A literal level arrives as an unresolved name — authored
                // identifiers cannot spell digits.
                if let Ok(value) = name.as_str().parse::<u64>() {
                    if value <= u64::from(u32::MAX) {
                        return Ok(Level::Constant(value as u32));
                    }
                    return Err(self.refuse(format!(
                        "universe level literal `{name}` exceeds the kernel's level range"
                    )));
                }
                Err(self.refuse(format!(
                    "unresolved universe level `{name}`; only `core::Level` binders and literals elaborate"
                )))
            }
            TypeReferenceNode::ConstExpression(expression) => {
                match self.program.expression_table.expression(*expression) {
                    ExpressionNode::Integer(literal) => match literal.value_u64() {
                        Some(value) if value <= u64::from(u32::MAX) => {
                            Ok(Level::Constant(value as u32))
                        }
                        _ => Err(self.refuse(format!(
                            "universe level literal `{}` exceeds the kernel's level range",
                            literal.text()
                        ))),
                    },
                    _ => Err(self.refuse(
                        "computed universe levels are a later leg; use a `core::Level` binder or a literal"
                            .to_owned(),
                    )),
                }
            }
            _ => Err(self.refuse(
                "a universe-level argument must name a `core::Level` binder or a literal"
                    .to_owned(),
            )),
        }
    }

    /// Elaborate a `MathematicalType` node: ordinary references delegate,
    /// arrows become `Pi` with the named binder scoping over the codomain,
    /// and applications fold to `Apply` spines over elaborated argument
    /// terms.
    fn elaborate_mathematical_type(
        &mut self,
        handle: MathematicalTypeHandle,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        match self.program.mathematical_type(handle) {
            MathematicalType::Ordinary(reference) => self.elaborate_type_reference(*reference),
            MathematicalType::Arrow {
                binder,
                domain,
                codomain,
            } => {
                let domain = self.elaborate_mathematical_type(*domain)?;
                // The kernel's `Pi` codomain lives under the binder whether
                // or not it is named, so an anonymous arrow still pushes a
                // scope entry — an unmatchable (missing) name and no symbol.
                self.scope.push(ScopeEntry {
                    symbol: None,
                    name: binder.clone().unwrap_or_default(),
                    domain,
                });
                let codomain = self.elaborate_mathematical_type(*codomain);
                self.scope.pop();
                Ok(self.arena.insert(Term::Pi {
                    domain,
                    codomain: codomain?,
                }))
            }
            MathematicalType::Application { callee, arguments } => {
                let mut term = self.elaborate_mathematical_type(*callee)?;
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(*arguments)
                    .to_vec()
                {
                    let argument = self.elaborate_expression(argument)?;
                    term = self.arena.insert(Term::Apply {
                        function: term,
                        argument,
                    });
                }
                Ok(term)
            }
        }
    }

    /// Elaborate an ordinary type reference in type position. The fixed
    /// `core::*` vocabulary forms sorts and propositions; resolved
    /// non-core symbols intern or reference carrier assumptions and
    /// earlier declarations; everything else refuses.
    fn elaborate_type_reference(
        &mut self,
        reference: TypeReferenceHandle,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        if !self
            .program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return Err(
                self.refuse("a mathematical type references a type that does not exist".to_owned())
            );
        }
        match self.program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, name } => self.named_type(*symbol, name),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                if !lifetime_arguments.is_empty() {
                    return Err(self.refuse(format!(
                        "mathematical type `{base_name}` carries lifetime arguments the kernel cannot express"
                    )));
                }
                let arguments = self
                    .program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .to_vec();
                let base_symbol = *base_symbol;
                let base_name = base_name.clone();
                match base_name.as_str() {
                    "core::Type" => match arguments.as_slice() {
                        [universe] => {
                            let level = self.elaborate_level(*universe)?;
                            Ok(self.arena.insert(Term::Sort(Sort::Type(level))))
                        }
                        _ => Err(self.refuse(format!(
                            "`core::Type` takes one universe level, not {}",
                            arguments.len()
                        ))),
                    },
                    "core::Strict" => match arguments.as_slice() {
                        [universe] => {
                            let level = self.elaborate_level(*universe)?;
                            Ok(self.arena.insert(Term::Sort(Sort::Strict(level))))
                        }
                        _ => Err(self.refuse(format!(
                            "`core::Strict` takes one universe level, not {}",
                            arguments.len()
                        ))),
                    },
                    "core::Squash" => match arguments.as_slice() {
                        [ty] => {
                            let ty = self.elaborate_type_reference(*ty)?;
                            Ok(self.arena.insert(Term::Squash { ty }))
                        }
                        _ => Err(self.refuse(format!(
                            "`core::Squash` takes one type, not {}",
                            arguments.len()
                        ))),
                    },
                    "core::Level" => {
                        Err(self.refuse("`core::Level` is a universe level, not a type".to_owned()))
                    }
                    _ => {
                        if base_symbol.is_valid() && self.authored_index.contains_key(&base_symbol)
                        {
                            return Err(self.refuse(format!(
                                "applying mathematical declaration `{base_name}` as a carrier is a later leg"
                            )));
                        }
                        Err(self.refuse(format!(
                            "applied carrier `{base_name}` has no kernel denotation yet"
                        )))
                    }
                }
            }
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Constrained { .. }
            | TypeReferenceNode::DynamicTrait { .. } => Err(self.refuse(format!(
                "mathematical type `{}` cannot be a borrow, constraint or dynamic trait",
                self.program.display_type_reference(reference)
            ))),
            TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::ConstExpression(_) => Err(self.refuse(format!(
                "mathematical type `{}` has no kernel denotation yet",
                self.program.display_type_reference(reference)
            ))),
            TypeReferenceNode::Unit => {
                Err(self.refuse("the unit type has no kernel denotation yet".to_owned()))
            }
        }
    }

    /// Resolve a `Named` reference in type position. Scope entries
    /// (type/subject binders) resolve by symbol; authored declarations
    /// become `Constant`s; other resolved symbols intern as carrier
    /// assumptions or refuse by kind; the fixed `core::*` vocabulary
    /// refuses unapplied.
    fn named_type(
        &mut self,
        symbol: SymbolHandle,
        name: &Identifier,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        if symbol.is_valid() {
            if let Some(term) = self.scope_term_for_symbol(symbol) {
                return Ok(term);
            }
            if let Some(term) = self.constant_for_declaration(symbol)? {
                return Ok(term);
            }
            if let Some(term) = self.carrier_for_symbol(symbol) {
                return Ok(term);
            }
            return match name.as_str() {
                "core::Level" => {
                    Err(self.refuse("`core::Level` is a universe level, not a type".to_owned()))
                }
                "core::Type" | "core::Strict" | "core::Squash" => self.unapplied_core_type(name),
                _ => Err(self.refuse(format!("`{name}` is not a mathematical type"))),
            };
        }
        match name.as_str() {
            "core::Level" => {
                Err(self.refuse("`core::Level` is a universe level, not a type".to_owned()))
            }
            "core::Type" | "core::Strict" | "core::Squash" => self.unapplied_core_type(name),
            _ => {
                if let Some(term) = self.scope_term_for_name(name) {
                    return Ok(term);
                }
                Err(self.refuse(format!("unresolved mathematical type `{name}`")))
            }
        }
    }

    /// Elaborate a declaration's body: at a `core::Strict` result a
    /// proposition form lands `Squash` around its `Type 0` denotation;
    /// every other body elaborates as an ordinary term.
    fn elaborate_body(
        &mut self,
        term: ExpressionHandle,
        strict_result: bool,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        if strict_result && let Some(proposition) = self.elaborate_proposition(term)? {
            return Ok(self.arena.insert(Term::Squash { ty: proposition }));
        }
        self.elaborate_expression(term)
    }

    /// Elaborate a body or application-argument expression. Name
    /// references, ordinary calls, literals and machine operators denote
    /// in the bounded vocabulary; members, matches, casts and the
    /// remaining forms still refuse.
    fn elaborate_expression(
        &mut self,
        handle: ExpressionHandle,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        match self.program.expression_table.expression(handle) {
            ExpressionNode::Name(path) => {
                let symbol = path.symbol;
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members)
                    .to_vec();
                if symbol.is_valid() {
                    let name = members.last().cloned().unwrap_or_default();
                    return self.named_term(symbol, &name);
                }
                match members.as_slice() {
                    [name] => {
                        let name = name.clone();
                        if let Some(term) = self.scope_term_for_name(&name) {
                            Ok(term)
                        } else {
                            Err(self.refuse(format!("unresolved mathematical name `{name}`")))
                        }
                    }
                    _ => Err(self.refuse("an unresolved mathematical name path".to_owned())),
                }
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    return Err(self.refuse(format!(
                        "member call `{}` has no mathematical denotation yet",
                        call.target
                    )));
                }
                if call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || !call.evidence_arguments.is_empty()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
                    || call.operational_acknowledgement.acknowledges_suspend
                    || call.operational_acknowledgement.acknowledges_block
                {
                    return Err(self.refuse(format!(
                        "call `{}` carries machine, evidence, quotient or operational payload the kernel cannot express",
                        call.target
                    )));
                }
                let target_symbol = call.target_symbol;
                let target = call.target.clone();
                let static_arguments = call.machine_arguments.clone();
                let mut function =
                    self.elaborate_callee(target_symbol, &target, &static_arguments)?;
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec()
                {
                    let argument = self.elaborate_expression(argument)?;
                    function = self.arena.insert(Term::Apply { function, argument });
                }
                Ok(function)
            }
            ExpressionNode::Integer(literal) => {
                let literal = literal.clone();
                self.literal_term(&literal)
            }
            ExpressionNode::Boolean(value) => Ok(self.boolean_literal(*value)),
            ExpressionNode::Binary(binary) => {
                let binary = *binary;
                match binary.operator {
                    BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor => self.integer_operation_term(handle, &binary),
                    _ => match self.elaborate_proposition(handle)? {
                        Some(proposition) => Ok(proposition),
                        None => Err(self.unsupported_expression(handle)),
                    },
                }
            }
            ExpressionNode::ZeroValue(reference) => self.zero_value_term(*reference),
            _ => Err(self.unsupported_expression(handle)),
        }
    }

    /// The shared refusal for body forms that carry no kernel denotation.
    fn unsupported_expression(&self, handle: ExpressionHandle) -> Vec<diagnostics::Diagnostic> {
        self.refuse(format!(
            "expression `{}` has no kernel denotation yet",
            self.program.render_proof_expression(
                handle,
                typed_trees::proposition::ProofSubstitutions::None
            )
        ))
    }

    /// The `Type 0` proposition one machine expression denotes, when it
    /// is a proposition form at all — `None` names an ordinary term and
    /// falls back to `elaborate_expression`, while `Err` is a refused
    /// proposition. Comparisons need a shared scalar carrier: integer
    /// orders (`<`, `<=`, `>`, `>=`) denote `IntLt`/`IntLe` over the
    /// shared `Int`, `==` denotes `Id` over the operands' carrier, `&&`
    /// denotes a right-nested `Σ`, `||` the tagged `Two` sum, `true` and
    /// `false` the `Two` identities, and a bare `bool` subject `x` means
    /// `x = true` — the same proposition `lower_proposition` forms.
    fn elaborate_proposition(
        &mut self,
        handle: ExpressionHandle,
    ) -> Result<Option<TermHandle>, Vec<diagnostics::Diagnostic>> {
        match self.program.expression_table.expression(handle) {
            ExpressionNode::Boolean(value) => Ok(Some(self.boolean_proposition(*value))),
            ExpressionNode::Name(..) => match self.operand_carrier(handle) {
                Some(ScalarCarrier::Boolean) => {
                    let subject = self.scalar_operand_term(handle, ScalarCarrier::Boolean)?;
                    let ty = self.carrier_term(ScalarCarrier::Boolean);
                    let truth = self.boolean_literal(true);
                    Ok(Some(self.arena.insert(Term::Id {
                        ty,
                        left: subject,
                        right: truth,
                    })))
                }
                _ => Ok(None),
            },
            ExpressionNode::Binary(binary) => {
                let binary = *binary;
                match binary.operator {
                    BinaryOperator::Equal => self.equality_proposition(handle, binary).map(Some),
                    BinaryOperator::NotEqual => Err(self.refuse(format!(
                        "proposition `{}` negates equality; the bounded vocabulary holds no negation",
                        self.program.render_proof_expression(
                            handle,
                            typed_trees::proposition::ProofSubstitutions::None
                        )
                    ))),
                    BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual => {
                        self.order_proposition(handle, binary).map(Some)
                    }
                    BinaryOperator::And | BinaryOperator::Or => self
                        .connective_proposition(handle, binary.operator)
                        .map(Some),
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    /// `l = r : Type 0` — `Id` over the operands' shared scalar carrier.
    /// `Int` serves every fixed-width integer comparison, `bool` and
    /// `addr` their own carriers, and an interned `data` carrier serves
    /// structural equality — the generic `Id S` route the bounded
    /// denotation keeps for non-integer scalars. Anonymous literals adopt
    /// the other operand's carrier; distinct determined carriers refuse.
    fn equality_proposition(
        &mut self,
        handle: ExpressionHandle,
        binary: TableBinaryExpression,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let carrier = self
            .operand_carrier(binary.left)
            .or_else(|| self.operand_carrier(binary.right))
            .ok_or_else(|| {
                self.refuse(format!(
                    "cannot determine a shared scalar carrier for `{}`",
                    self.program.render_proof_expression(
                        handle,
                        typed_trees::proposition::ProofSubstitutions::None
                    )
                ))
            })?;
        let mut left = self.scalar_operand_term(binary.left, carrier)?;
        let mut right = self.scalar_operand_term(binary.right, carrier)?;
        // `Id` endpoints order canonically — `x == y` and `y == x`
        // denote one type. The bounded denotation canonicalizes at the
        // proposition level; signatures carry no proposition stage, so
        // the kernel terms themselves order here.
        if self.denotation_key(left) > self.denotation_key(right) {
            std::mem::swap(&mut left, &mut right);
        }
        let ty = self.carrier_term(carrier);
        Ok(self.arena.insert(Term::Id { ty, left, right }))
    }

    /// `IntLt l r` or `IntLe l r : Type 0` — the shared order relations,
    /// reserved for fixed non-address integer operands exactly as the
    /// bounded denotation's `fixed_scalar_relation` gates them. `>` and
    /// `>=` denote the flipped form.
    fn order_proposition(
        &mut self,
        handle: ExpressionHandle,
        binary: TableBinaryExpression,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let carrier = self
            .operand_carrier(binary.left)
            .or_else(|| self.operand_carrier(binary.right));
        if carrier != Some(ScalarCarrier::Integer) {
            return Err(self.refuse(format!(
                "order relation `{}` denotes only over fixed non-address integers",
                self.program.render_proof_expression(
                    handle,
                    typed_trees::proposition::ProofSubstitutions::None
                )
            )));
        }
        let left = self.integer_operand(binary.left)?;
        let right = self.integer_operand(binary.right)?;
        let (strict, left, right) = match binary.operator {
            BinaryOperator::Less => (true, left, right),
            BinaryOperator::LessOrEqual => (false, left, right),
            BinaryOperator::Greater => (true, right, left),
            _ => (false, right, left),
        };
        let position = if strict {
            self.integer_less_than()
        } else {
            self.integer_less_or_equal()
        };
        let relation = self.constant(position);
        let function = self.arena.insert(Term::Apply {
            function: relation,
            argument: left,
        });
        Ok(self.arena.insert(Term::Apply {
            function,
            argument: right,
        }))
    }

    /// `a && b` denotes a right-nested `Σ` and `a || b` the tagged
    /// `Σ(t : Two). caseTwo` sum — the connective denotations of the
    /// bounded vocabulary. Operand spines flatten and order canonically,
    /// so `a && b` and `b && a` denote one type.
    fn connective_proposition(
        &mut self,
        handle: ExpressionHandle,
        operator: BinaryOperator,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let mut operands = Vec::new();
        self.proposition_spine(handle, operator, &mut operands);
        let mut terms = Vec::with_capacity(operands.len());
        for operand in operands {
            match self.elaborate_proposition(operand)? {
                Some(term) => terms.push(term),
                None => {
                    return Err(self.refuse(format!(
                        "operand `{}` of a proposition connective is not a proposition",
                        self.program.render_proof_expression(
                            operand,
                            typed_trees::proposition::ProofSubstitutions::None
                        )
                    )));
                }
            }
        }
        terms.sort_by_key(|term| self.denotation_key(*term));
        Ok(match operator {
            BinaryOperator::And => self.right_nested_sigma(&terms, 0),
            _ => self.tagged_sum(&terms, 0),
        })
    }

    /// Flatten a left-associative `&&`/`||` spine into authored order.
    fn proposition_spine(
        &self,
        handle: ExpressionHandle,
        operator: BinaryOperator,
        operands: &mut Vec<ExpressionHandle>,
    ) {
        match self.program.expression_table.expression(handle) {
            ExpressionNode::Binary(binary) if binary.operator == operator => {
                self.proposition_spine(binary.left, operator, operands);
                self.proposition_spine(binary.right, operator, operands);
            }
            _ => operands.push(handle),
        }
    }

    /// `Σ(P, λ_. Q)` right-nested over `terms`, each denoted in the
    /// ambient scope: `Σ` binds `Variable 0`, so every later term shifts
    /// under the binders before it.
    fn right_nested_sigma(&mut self, terms: &[TermHandle], under: u32) -> TermHandle {
        let domain = shift(&mut self.arena, terms[0], 0, under);
        let codomain = if terms.len() == 2 {
            shift(&mut self.arena, terms[1], 0, under + 1)
        } else {
            self.right_nested_sigma(&terms[1..], under + 1)
        };
        self.arena.insert(Term::Sigma { domain, codomain })
    }

    /// `Σ(t : Two). caseTwo(λ(_ : Two). Type 0, zero, rest, t)` — the
    /// binary coproduct the disjunction denotation nests, matching the
    /// bounded vocabulary's own `tagged_sum` shape.
    fn tagged_sum(&mut self, terms: &[TermHandle], under: u32) -> TermHandle {
        let two = self.arena.insert(Term::Two);
        let type_zero = self.type_zero();
        let motive = self.arena.insert(Term::Lambda {
            domain: two,
            body: type_zero,
        });
        let scrutinee = self.arena.insert(Term::Variable(0));
        let zero_branch = shift(&mut self.arena, terms[0], 0, under + 1);
        let one_branch = if terms.len() == 2 {
            shift(&mut self.arena, terms[1], 0, under + 1)
        } else {
            self.tagged_sum(&terms[1..], under + 1)
        };
        let family = self.arena.insert(Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        });
        self.arena.insert(Term::Sigma {
            domain: two,
            codomain: family,
        })
    }

    /// `true` denotes `Id Two zero zero` and `false` `Id Two zero one` —
    /// the bounded `Truth`/`Falsehood` denotations.
    fn boolean_proposition(&mut self, value: bool) -> TermHandle {
        let ty = self.arena.insert(Term::Two);
        let zero = self.arena.insert(Term::TwoZero);
        let right = if value {
            zero
        } else {
            self.arena.insert(Term::TwoOne)
        };
        self.arena.insert(Term::Id {
            ty,
            left: zero,
            right,
        })
    }

    /// One operand of a relation or connective, denoted at the shared
    /// `carrier` — literals and zero values take it, while names, calls
    /// and compound arithmetic must already inhabit it.
    fn scalar_operand_term(
        &mut self,
        handle: ExpressionHandle,
        carrier: ScalarCarrier,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        match self.program.expression_table.expression(handle) {
            ExpressionNode::Integer(literal) => {
                let Some(value) = literal.value_bignum() else {
                    return Err(self.refuse(format!(
                        "integer literal `{}` has no exact value",
                        literal.text()
                    )));
                };
                let position = self.carrier_position(carrier);
                Ok(self.numeric_literal(position, value))
            }
            ExpressionNode::Boolean(value) => {
                if carrier != ScalarCarrier::Boolean {
                    return Err(self.refuse(format!(
                        "boolean literal is not a `{}` operand",
                        self.carrier_name(carrier)
                    )));
                }
                Ok(self.boolean_literal(*value))
            }
            ExpressionNode::ZeroValue(reference) => {
                let reference = *reference;
                match self.scalar_carrier_of_type_reference(reference) {
                    Some(determined) if determined == carrier => self.zero_literal(carrier),
                    _ => Err(self.refuse(format!(
                        "zero value of `{}` is not a `{}` operand",
                        self.program.display_type_reference(reference),
                        self.carrier_name(carrier)
                    ))),
                }
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                        | BinaryOperator::Modulo
                        | BinaryOperator::ShiftLeft
                        | BinaryOperator::ShiftRight
                        | BinaryOperator::BitwiseAnd
                        | BinaryOperator::BitwiseOr
                        | BinaryOperator::BitwiseXor
                ) =>
            {
                if carrier != ScalarCarrier::Integer {
                    return Err(self.refuse(format!(
                        "arithmetic expression `{}` is not a `{}` operand",
                        self.program.render_proof_expression(
                            handle,
                            typed_trees::proposition::ProofSubstitutions::None
                        ),
                        self.carrier_name(carrier)
                    )));
                }
                let binary = *binary;
                self.integer_operation_term(handle, &binary)
            }
            ExpressionNode::Name(..) | ExpressionNode::Call(..) => {
                match self.operand_carrier(handle) {
                    Some(determined) if determined == carrier => self.elaborate_expression(handle),
                    Some(..) => Err(self.refuse(format!(
                        "operand `{}` inhabits a different scalar carrier",
                        self.program.render_proof_expression(
                            handle,
                            typed_trees::proposition::ProofSubstitutions::None
                        )
                    ))),
                    None => Err(self.refuse(format!(
                        "cannot determine the scalar carrier of operand `{}`",
                        self.program.render_proof_expression(
                            handle,
                            typed_trees::proposition::ProofSubstitutions::None
                        )
                    ))),
                }
            }
            _ => Err(self.unsupported_expression(handle)),
        }
    }

    /// An `Int`-valued operand: a literal, a name or call at an integer
    /// carrier, or compound integer arithmetic.
    fn integer_operand(
        &mut self,
        handle: ExpressionHandle,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        self.scalar_operand_term(handle, ScalarCarrier::Integer)
    }

    /// The scalar carrier one operand expression inhabits, when it is
    /// determinable from literal landings, scope domains, declaration
    /// codomains and operation heads. `None` means the carrier is not a
    /// denoted scalar carrier — anonymous literals defer so the other
    /// operand's carrier supplies theirs, mirroring `infer_scalar_type`.
    fn operand_carrier(&self, handle: ExpressionHandle) -> Option<ScalarCarrier> {
        match self.program.expression_table.expression(handle) {
            ExpressionNode::Integer(literal) => {
                literal.landing().map(|landing| match landing.landed_type {
                    LandedIntegerType::Addr => ScalarCarrier::Address,
                    _ => ScalarCarrier::Integer,
                })
            }
            ExpressionNode::Boolean(_) => Some(ScalarCarrier::Boolean),
            ExpressionNode::Name(path) => self.name_carrier(path),
            ExpressionNode::Call(call) => self.declaration_result_carrier(call.target_symbol),
            ExpressionNode::Binary(binary) => Some(match binary.operator {
                BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor => ScalarCarrier::Integer,
                _ => ScalarCarrier::Boolean,
            }),
            ExpressionNode::ZeroValue(reference) => {
                self.scalar_carrier_of_type_reference(*reference)
            }
            _ => None,
        }
    }

    /// The carrier a `Name` inhabits: a scope entry's domain when the
    /// entry names a `Type 0` carrier assumption, or an earlier
    /// declaration's result carrier. A scope entry bound at a `Sort`,
    /// `Pi` or `Variable` domain — type binders, arrow parameters —
    /// carries no scalar carrier.
    fn name_carrier(&self, path: &TableNamePath) -> Option<ScalarCarrier> {
        if path.symbol.is_valid() {
            if let Some(position) = self
                .scope
                .iter()
                .rposition(|entry| entry.symbol == Some(path.symbol))
            {
                return self.domain_carrier(position);
            }
            return self.declaration_result_carrier(path.symbol);
        }
        let members = self
            .program
            .expression_table
            .name_path_members(path.members);
        let [name] = members else { return None };
        let position = self.scope.iter().rposition(|entry| entry.name == *name)?;
        self.domain_carrier(position)
    }

    /// The carrier class one scope entry's domain names, when it is a
    /// `Type 0` carrier assumption at all.
    fn domain_carrier(&self, scope_position: usize) -> Option<ScalarCarrier> {
        match self.arena.get(self.scope[scope_position].domain) {
            Term::Constant { declaration, .. } => Some(self.carrier_class_at(declaration)),
            _ => None,
        }
    }

    /// The carrier an earlier authored declaration's result inhabits —
    /// the innermost codomain of its `Pi` spine.
    fn declaration_result_carrier(&self, symbol: SymbolHandle) -> Option<ScalarCarrier> {
        let position = *self.authored_positions.get(&symbol)?;
        let mut ty = self.declarations[position as usize].ty;
        while let Term::Pi { codomain, .. } = self.arena.get(ty) {
            ty = codomain;
        }
        match self.arena.get(ty) {
            Term::Constant { declaration, .. } => Some(self.carrier_class_at(declaration)),
            _ => None,
        }
    }

    /// The scalar carrier a type reference names, when it is one —
    /// primitives and the `Int`/`UInt` atoms map onto the shared
    /// carriers without interning; other symbols only name a carrier
    /// already interned.
    fn scalar_carrier_of_type_reference(
        &self,
        reference: TypeReferenceHandle,
    ) -> Option<ScalarCarrier> {
        if let Some(primitive) = self.program.primitive_type_reference(reference) {
            return match primitive {
                PrimitiveType::Bool => Some(ScalarCarrier::Boolean),
                PrimitiveType::Addr => Some(ScalarCarrier::Address),
                PrimitiveType::F32 | PrimitiveType::F64 => None,
                _ => Some(ScalarCarrier::Integer),
            };
        }
        if let TypeReferenceNode::Named { symbol, .. } =
            self.program.type_reference_table.type_reference(reference)
        {
            return match self.program.symbols.builtin_type_atom(*symbol) {
                Some(BuiltinTypeAtom::Bool) => Some(ScalarCarrier::Boolean),
                Some(BuiltinTypeAtom::Address) => Some(ScalarCarrier::Address),
                Some(
                    BuiltinTypeAtom::I8
                    | BuiltinTypeAtom::I16
                    | BuiltinTypeAtom::I32
                    | BuiltinTypeAtom::I64
                    | BuiltinTypeAtom::U8
                    | BuiltinTypeAtom::U16
                    | BuiltinTypeAtom::U32
                    | BuiltinTypeAtom::U64
                    | BuiltinTypeAtom::Int
                    | BuiltinTypeAtom::UInt,
                ) => Some(ScalarCarrier::Integer),
                _ => self
                    .carriers
                    .get(symbol)
                    .map(|&position| self.carrier_class_at(position)),
            };
        }
        None
    }

    /// Which scalar carrier a signature position is: the shared `Int`,
    /// `bool` and `addr` carriers match their classes; every other
    /// interned carrier names its own position.
    fn carrier_class_at(&self, position: u32) -> ScalarCarrier {
        if self.integer == Some(position) {
            ScalarCarrier::Integer
        } else if self.boolean == Some(position) {
            ScalarCarrier::Boolean
        } else if self.address == Some(position) {
            ScalarCarrier::Address
        } else {
            ScalarCarrier::Carrier(position)
        }
    }

    /// A diagnostic-friendly name for the carrier family.
    fn carrier_name(&self, carrier: ScalarCarrier) -> &'static str {
        match carrier {
            ScalarCarrier::Integer => "integer",
            ScalarCarrier::Boolean => "boolean",
            ScalarCarrier::Address => "address",
            ScalarCarrier::Carrier(..) => "scalar",
        }
    }

    /// The carrier's signature position, interning it on first use.
    fn carrier_position(&mut self, carrier: ScalarCarrier) -> u32 {
        match carrier {
            ScalarCarrier::Integer => self.integer_carrier(),
            ScalarCarrier::Boolean => self.boolean_carrier(),
            ScalarCarrier::Address => self.address_carrier(),
            ScalarCarrier::Carrier(position) => position,
        }
    }

    /// The carrier as a `Constant` term.
    fn carrier_term(&mut self, carrier: ScalarCarrier) -> TermHandle {
        let position = self.carrier_position(carrier);
        self.constant(position)
    }

    /// `Int : Type 0` — the shared mathematical-integer carrier
    /// assumption every fixed integer element and relation names.
    fn integer_carrier(&mut self) -> u32 {
        if self.integer.is_none() {
            self.integer = Some(self.push_type_carrier());
        }
        self.integer.unwrap()
    }

    /// `bool : Type 0`.
    fn boolean_carrier(&mut self) -> u32 {
        if self.boolean.is_none() {
            self.boolean = Some(self.push_type_carrier());
        }
        self.boolean.unwrap()
    }

    /// `addr : Type 0` — distinct from `Int`, so address values never
    /// meet the fixed-integer order relations.
    fn address_carrier(&mut self) -> u32 {
        if self.address.is_none() {
            self.address = Some(self.push_type_carrier());
        }
        self.address.unwrap()
    }

    /// `Π(_ : Int). Π(_ : Int). Type 0` — the shape `IntLt`/`IntLe`
    /// share.
    fn integer_relation(&mut self) -> u32 {
        let int = self.integer_constant();
        let codomain = self.type_zero();
        let inner = self.arena.insert(Term::Pi {
            domain: int,
            codomain,
        });
        let ty = self.arena.insert(Term::Pi {
            domain: int,
            codomain: inner,
        });
        self.push_assumption(ty)
    }

    fn integer_less_than(&mut self) -> u32 {
        if self.integer_less_than.is_none() {
            self.integer_less_than = Some(self.integer_relation());
        }
        self.integer_less_than.unwrap()
    }

    fn integer_less_or_equal(&mut self) -> u32 {
        if self.integer_less_or_equal.is_none() {
            self.integer_less_or_equal = Some(self.integer_relation());
        }
        self.integer_less_or_equal.unwrap()
    }

    /// `Π(_ : Int). Π(_ : Int). Int` — the shape `IntAdd`/`IntSub`
    /// share.
    fn integer_operation(&mut self) -> u32 {
        let int = self.integer_constant();
        let inner = self.arena.insert(Term::Pi {
            domain: int,
            codomain: int,
        });
        let ty = self.arena.insert(Term::Pi {
            domain: int,
            codomain: inner,
        });
        self.push_assumption(ty)
    }

    fn integer_add(&mut self) -> u32 {
        if self.integer_add.is_none() {
            self.integer_add = Some(self.integer_operation());
        }
        self.integer_add.unwrap()
    }

    fn integer_subtract(&mut self) -> u32 {
        if self.integer_subtract.is_none() {
            self.integer_subtract = Some(self.integer_operation());
        }
        self.integer_subtract.unwrap()
    }

    /// `Constant` term for a signature position.
    fn constant(&mut self, position: u32) -> TermHandle {
        self.arena.insert(Term::Constant {
            declaration: position,
            levels: Vec::new(),
        })
    }

    /// `Type 0` as a term.
    fn type_zero(&mut self) -> TermHandle {
        self.arena
            .insert(Term::Sort(Sort::Type(Level::Constant(0))))
    }

    /// Push an assumption declaration and return its position.
    fn push_assumption(&mut self, ty: TermHandle) -> u32 {
        let position = self.declarations.len() as u32;
        self.declarations.push(Declaration::assumption(0, ty));
        position
    }

    /// A fresh `Type 0` carrier assumption's position.
    fn push_type_carrier(&mut self) -> u32 {
        let ty = self.type_zero();
        self.push_assumption(ty)
    }

    /// `Int`-constant the `Int` carrier names — interned once.
    fn integer_constant(&mut self) -> TermHandle {
        let position = self.integer_carrier();
        self.constant(position)
    }

    /// One closed scalar literal's constant, interned by exact value at
    /// its carrier — `2 + 0` and `2` name one constant, so evaluated
    /// equalities stay `refl`-provable without a decision assumption.
    fn numeric_literal(&mut self, carrier: u32, value: BigInt) -> TermHandle {
        let key = (carrier, value);
        let position = match self.numeric_literals.get(&key) {
            Some(&position) => position,
            None => {
                let ty = self.constant(carrier);
                let position = self.push_assumption(ty);
                self.numeric_literals.insert(key, position);
                position
            }
        };
        self.constant(position)
    }

    /// `true`/`false` interned at the `bool` carrier.
    fn boolean_literal(&mut self, value: bool) -> TermHandle {
        if let Some(position) = self.boolean_literals[value as usize] {
            return self.constant(position);
        }
        let carrier = self.boolean_carrier();
        let ty = self.constant(carrier);
        let position = self.push_assumption(ty);
        self.boolean_literals[value as usize] = Some(position);
        self.constant(position)
    }

    /// A literal in term position denotes at its landing carrier —
    /// `addr`-landed literals land at the address carrier, every other
    /// integer literal at `Int` — interned by exact value.
    fn literal_term(
        &mut self,
        literal: &IntegerLiteral,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        let Some(value) = literal.value_bignum() else {
            return Err(self.refuse(format!(
                "integer literal `{}` has no exact value",
                literal.text()
            )));
        };
        let carrier = match literal.landing() {
            Some(landing) if landing.landed_type == LandedIntegerType::Addr => {
                self.address_carrier()
            }
            _ => self.integer_carrier(),
        };
        Ok(self.numeric_literal(carrier, value))
    }

    /// The zero value of a scalar carrier.
    fn zero_literal(
        &mut self,
        carrier: ScalarCarrier,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        match carrier {
            ScalarCarrier::Boolean => Ok(self.boolean_literal(false)),
            ScalarCarrier::Carrier(..) => Err(self.refuse(
                "the zero value of a non-numeric carrier has no bounded denotation".to_owned(),
            )),
            _ => {
                let position = self.carrier_position(carrier);
                Ok(self.numeric_literal(position, BigInt::from_u64(0)))
            }
        }
    }

    /// `ZeroValue` denotes the zero literal at its carrier.
    fn zero_value_term(
        &mut self,
        reference: TypeReferenceHandle,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        match self.scalar_carrier_of_type_reference(reference) {
            Some(carrier) => self.zero_literal(carrier),
            None => Err(self.refuse(format!(
                "the zero value of `{}` has no scalar denotation",
                self.program.display_type_reference(reference)
            ))),
        }
    }

    /// Closed integer arithmetic evaluates to the exact literal `Int`
    /// constant — `2 + 0` and `2` name one denotation. Open `+`/`-`
    /// share `IntAdd`/`IntSub` applied to their operands; every other
    /// open machine operation has no bounded denotation (the vocabulary
    /// holds no function for it) and refuses.
    fn integer_operation_term(
        &mut self,
        handle: ExpressionHandle,
        binary: &TableBinaryExpression,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        if let Some(value) = self.program.closed_integer_expression_value(handle) {
            let carrier = self.integer_carrier();
            return Ok(self.numeric_literal(carrier, value));
        }
        let position = match binary.operator {
            BinaryOperator::Add => self.integer_add(),
            BinaryOperator::Subtract => self.integer_subtract(),
            _ => {
                return Err(self.refuse(format!(
                    "open machine operation `{}` has no bounded denotation; only exact `+` and `-` compose",
                    self.program.render_proof_expression(
                        handle,
                        typed_trees::proposition::ProofSubstitutions::None
                    )
                )))
            }
        };
        let left = self.integer_operand(binary.left)?;
        let right = self.integer_operand(binary.right)?;
        let operation = self.constant(position);
        let function = self.arena.insert(Term::Apply {
            function: operation,
            argument: left,
        });
        Ok(self.arena.insert(Term::Apply {
            function,
            argument: right,
        }))
    }

    /// A structural key ordering denoted terms canonically — `x == y` and
    /// `y == x` denote one `Id`, and connective spines sort by it. The
    /// bounded denotation canonicalizes at the proposition level; the
    /// signature carries no proposition stage, so kernel terms order by
    /// structure here.
    fn denotation_key(&self, term: TermHandle) -> Vec<u8> {
        let mut key = Vec::new();
        self.write_denotation_key(term, &mut key);
        key
    }

    fn write_denotation_key(&self, term: TermHandle, out: &mut Vec<u8>) {
        match self.arena.get(term) {
            Term::Dummy => out.push(0),
            Term::Variable(index) => {
                out.push(1);
                out.extend_from_slice(&index.to_le_bytes());
            }
            Term::Sort(sort) => {
                out.push(2);
                out.push(sort.is_strict() as u8);
                self.write_level_key(sort.level(), out);
            }
            Term::Constant {
                declaration,
                levels,
            } => {
                out.push(3);
                out.extend_from_slice(&declaration.to_le_bytes());
                for level in levels {
                    self.write_level_key(level, out);
                }
            }
            Term::Pi { domain, codomain } => self.write_pair_key(4, domain, codomain, out),
            Term::Lambda { domain, body } => self.write_pair_key(5, domain, body, out),
            Term::Apply { function, argument } => self.write_pair_key(6, function, argument, out),
            Term::Sigma { domain, codomain } => self.write_pair_key(7, domain, codomain, out),
            Term::Pair { first, second } => self.write_pair_key(8, first, second, out),
            Term::Fst { pair } => {
                out.push(9);
                self.write_denotation_key(pair, out);
            }
            Term::Snd { pair } => {
                out.push(10);
                self.write_denotation_key(pair, out);
            }
            Term::Two => out.push(11),
            Term::TwoZero => out.push(12),
            Term::TwoOne => out.push(13),
            Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            } => {
                out.push(14);
                self.write_denotation_key(motive, out);
                self.write_denotation_key(zero_branch, out);
                self.write_denotation_key(one_branch, out);
                self.write_denotation_key(scrutinee, out);
            }
            Term::Id { ty, left, right } => {
                out.push(15);
                self.write_denotation_key(ty, out);
                self.write_denotation_key(left, out);
                self.write_denotation_key(right, out);
            }
            Term::Refl { ty, value } => self.write_pair_key(16, ty, value, out),
            Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            } => {
                out.push(17);
                self.write_denotation_key(motive, out);
                self.write_denotation_key(base, out);
                self.write_denotation_key(endpoint, out);
                self.write_denotation_key(proof, out);
            }
            Term::W { carrier, children } => self.write_pair_key(18, carrier, children, out),
            Term::Sup {
                carrier,
                children,
                label,
                function,
            } => {
                out.push(19);
                self.write_denotation_key(carrier, out);
                self.write_denotation_key(children, out);
                self.write_denotation_key(label, out);
                self.write_denotation_key(function, out);
            }
            Term::IndW { motive, step, tree } => {
                out.push(20);
                self.write_denotation_key(motive, out);
                self.write_denotation_key(step, out);
                self.write_denotation_key(tree, out);
            }
            Term::Empty => out.push(21),
            Term::EmptyElim { ty, scrutinee } => self.write_pair_key(22, ty, scrutinee, out),
            Term::Squash { ty } => {
                out.push(23);
                self.write_denotation_key(ty, out);
            }
            Term::SquashIntro { ty, value } => self.write_pair_key(24, ty, value, out),
            Term::SquashElim {
                proposition,
                function,
                scrutinee,
            } => {
                out.push(25);
                self.write_denotation_key(proposition, out);
                self.write_denotation_key(function, out);
                self.write_denotation_key(scrutinee, out);
            }
            Term::Box { ty } => {
                out.push(26);
                self.write_denotation_key(ty, out);
            }
            Term::BoxIntro { ty, value } => self.write_pair_key(27, ty, value, out),
            Term::BoxElim {
                motive,
                body,
                scrutinee,
            } => {
                out.push(28);
                self.write_denotation_key(motive, out);
                self.write_denotation_key(body, out);
                self.write_denotation_key(scrutinee, out);
            }
        }
    }

    fn write_pair_key(&self, tag: u8, first: TermHandle, second: TermHandle, out: &mut Vec<u8>) {
        out.push(tag);
        self.write_denotation_key(first, out);
        self.write_denotation_key(second, out);
    }

    fn write_level_key(&self, level: Level, out: &mut Vec<u8>) {
        match level {
            Level::Constant(value) => {
                out.push(0);
                out.extend_from_slice(&value.to_le_bytes());
            }
            Level::Parameter(index) => {
                out.push(1);
                out.extend_from_slice(&index.to_le_bytes());
            }
            Level::Successor(inner) => {
                out.push(2);
                self.write_level_key(*inner, out);
            }
            Level::Maximum(left, right) => {
                out.push(3);
                self.write_level_key(*left, out);
                self.write_level_key(*right, out);
            }
        }
    }

    /// An unapplied fixed-core universe in type position claims the next
    /// generalized level parameter — `x: core::Type` binds a type at a
    /// published universe parameter exactly as a bare `T` binder does.
    /// `core::Squash` needs a type argument, so it cannot generalize and
    /// refuses.
    fn unapplied_core_type(
        &mut self,
        name: &Identifier,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        match name.as_str() {
            "core::Type" => {
                let level = self.generalized_level();
                Ok(self.arena.insert(Term::Sort(Sort::Type(level))))
            }
            "core::Strict" => {
                let level = self.generalized_level();
                Ok(self.arena.insert(Term::Sort(Sort::Strict(level))))
            }
            _ => Err(self.refuse(format!(
                "`{name}` needs its arguments; it cannot appear unapplied"
            ))),
        }
    }

    /// The next generalized universe parameter — appended after the
    /// authored `core::Level` binders in elaboration order, so the
    /// declaration's published `level_arity` is deterministic.
    fn generalized_level(&mut self) -> Level {
        let parameter = (self.level_binders.len() + self.generalized_levels as usize) as u32;
        self.generalized_levels += 1;
        Level::Parameter(parameter)
    }

    /// Resolve a symbol in term position: scope first, then earlier
    /// authored declarations, then carrier interning or a kind refusal.
    fn named_term(
        &mut self,
        symbol: SymbolHandle,
        name: &Identifier,
    ) -> Result<TermHandle, Vec<diagnostics::Diagnostic>> {
        if let Some(term) = self.scope_term_for_symbol(symbol) {
            return Ok(term);
        }
        if let Some(term) = self.constant_for_declaration(symbol)? {
            return Ok(term);
        }
        if let Some(term) = self.carrier_for_symbol(symbol) {
            return Ok(term);
        }
        Err(self.refuse(format!("`{name}` is not a mathematical term")))
    }

    /// A `Variable` for the scope entry carrying `symbol`, if any —
    /// innermost wins under shadowing.
    fn scope_term_for_symbol(&mut self, symbol: SymbolHandle) -> Option<TermHandle> {
        let position = self
            .scope
            .iter()
            .rposition(|entry| entry.symbol == Some(symbol))?;
        Some(self.scope_variable(position))
    }

    /// A `Variable` for the innermost scope entry named `name`, if any —
    /// how arrow binders and symbol-less callee references resolve.
    fn scope_term_for_name(&mut self, name: &Identifier) -> Option<TermHandle> {
        let position = self.scope.iter().rposition(|entry| entry.name == *name)?;
        Some(self.scope_variable(position))
    }

    /// The `Variable` for scope position `position`: entry `k` of `n` is
    /// de Bruijn index `n - 1 - k`.
    fn scope_variable(&mut self, position: usize) -> TermHandle {
        let index = self.scope.len() - 1 - position;
        self.arena.insert(Term::Variable(index as u32))
    }

    fn push_scope(&mut self, symbol: Option<SymbolHandle>, name: Identifier, domain: TermHandle) {
        self.scope.push(ScopeEntry {
            symbol,
            name,
            domain,
        });
    }

    /// The level-binder position of `symbol`/`name`, if it names one of
    /// this declaration's `core::Level` binders.
    fn level_index(&self, symbol: SymbolHandle, name: &Identifier) -> Option<u32> {
        if symbol.is_valid() {
            return self
                .level_binders
                .iter()
                .position(|binder| *binder == symbol)
                .map(|index| index as u32);
        }
        self.level_names
            .iter()
            .position(|binder| binder == name)
            .map(|index| index as u32)
    }

    /// A `Constant` for an elaborated earlier authored declaration, or a
    /// refusal when the reference is self/forward or the declaration is
    /// universe-polymorphic without an explicit application.
    fn constant_for_declaration(
        &mut self,
        symbol: SymbolHandle,
    ) -> Result<Option<TermHandle>, Vec<diagnostics::Diagnostic>> {
        if let Some(&position) = self.authored_positions.get(&symbol) {
            let declaration = &self.declarations[position as usize];
            if declaration.level_arity != 0 {
                return Err(self.refuse(format!(
                    "referencing mathematical declaration `{}` needs {} explicit level arguments",
                    self.program.symbols.name(symbol),
                    declaration.level_arity
                )));
            }
            let term = self.arena.insert(Term::Constant {
                declaration: position,
                levels: Vec::new(),
            });
            return Ok(Some(term));
        }
        if self.authored_index.contains_key(&symbol) {
            return Err(self.refuse(format!(
                "mathematical declaration `{}` references itself or a later declaration; the signature admits strictly earlier ones",
                self.program.symbols.name(symbol)
            )));
        }
        Ok(None)
    }

    /// Intern a resolved non-core symbol as an uninterpreted carrier
    /// (`assumption(0, Type 0)`), reusing its position on repeat use.
    /// Fixed-width integer and `Int`/`UInt` atoms share the bounded
    /// denotation's `Int` carrier; `bool` and `addr` share their own;
    /// other builtin atoms and authored `data` types intern per-symbol.
    /// Symbols of other kinds decline so the caller can refuse.
    fn carrier_for_symbol(&mut self, symbol: SymbolHandle) -> Option<TermHandle> {
        if let Some(&position) = self.carriers.get(&symbol) {
            return Some(self.constant(position));
        }
        let position = match self.program.symbols.get(symbol).kind {
            symbols::SymbolKind::BuiltinType | symbols::SymbolKind::Data => {
                match self.program.symbols.builtin_type_atom(symbol) {
                    Some(
                        BuiltinTypeAtom::I8
                        | BuiltinTypeAtom::I16
                        | BuiltinTypeAtom::I32
                        | BuiltinTypeAtom::I64
                        | BuiltinTypeAtom::U8
                        | BuiltinTypeAtom::U16
                        | BuiltinTypeAtom::U32
                        | BuiltinTypeAtom::U64
                        | BuiltinTypeAtom::Int
                        | BuiltinTypeAtom::UInt,
                    ) => self.integer_carrier(),
                    Some(BuiltinTypeAtom::Bool) => self.boolean_carrier(),
                    Some(BuiltinTypeAtom::Address) => self.address_carrier(),
                    _ => self.push_type_carrier(),
                }
            }
            _ => return None,
        };
        self.carriers.insert(symbol, position);
        Some(self.constant(position))
    }

    /// Re-decide the built signature: check each authored declaration's
    /// prefix in authored order so a failure names its declaration, then
    /// keep the last prefix's checked signature — it covers the whole
    /// list, since carriers always land before the declaration whose
    /// elaboration interned them.
    fn check(mut self) -> Result<CheckedMathematicalSignature, Vec<diagnostics::Diagnostic>> {
        let mut signature = Signature::new();
        for (index, &position) in self.authored.iter().enumerate() {
            let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
            match check_signature(
                &mut self.arena,
                &self.declarations[..=position as usize],
                &mut budget,
            ) {
                Ok(checked) => signature = checked,
                Err(error) => {
                    let definition = &self.program.mathematical_definitions()[index];
                    self.span = self.program.symbols.symbol_source_span(definition.symbol);
                    return Err(self.refuse(format!(
                        "mathematical declaration `{}` fails kernel checking: {error:?}",
                        definition.name
                    )));
                }
            }
        }
        Ok(CheckedMathematicalSignature {
            arena: self.arena,
            signature,
            authored: self.authored,
        })
    }

    fn refuse(&self, message: String) -> Vec<diagnostics::Diagnostic> {
        let mut diagnostic = diagnostics::Diagnostic::error(message);
        if let Some(span) = self.span {
            diagnostic = diagnostic.with_source_span(span);
        }
        vec![diagnostic]
    }
}

/// What a `name: carrier` binder's carrier head names.
enum CarrierHead {
    /// `core::Level` — an authored universe parameter.
    Level,
    /// `core::Level<args>` — malformed; levels take no arguments.
    MalformedLevel,
    /// `core::Type` or `core::Type<u>` — a type binder at an authored or
    /// generalized level.
    Type(LevelArgument),
    /// `core::Type<args>` with the wrong arity.
    MalformedType(usize),
    /// Anything else — an ordinary term domain.
    Term,
}

/// The universe level a `Type` binder binds at.
enum LevelArgument {
    /// `core::Type<u>` — the authored level reference.
    Authored(TypeReferenceHandle),
    /// Bare `T` or unapplied `core::Type` — claims the next generalized
    /// universe parameter.
    Generalized,
}

/// What a generic binder elaborates to.
enum BinderPlan {
    /// A universe parameter; no term entry.
    Level,
    /// A term entry whose domain is `Sort::Type` at this level.
    Type(LevelArgument),
    /// A term entry over this carrier type reference.
    Term(TypeReferenceHandle),
}

/// Which scalar carrier a denoted machine operand inhabits, mirroring
/// the bounded denotation's carrier split: `Integer` is the shared `Int`
/// every fixed-width integer denotes into, `Boolean` and `Address` are
/// the dedicated `bool`/`addr` carriers, and `Carrier` names an interned
/// per-symbol `Type 0` assumption (`data` types, other builtin atoms).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScalarCarrier {
    Integer,
    Boolean,
    Address,
    Carrier(u32),
}

#[cfg(test)]
mod tests;
