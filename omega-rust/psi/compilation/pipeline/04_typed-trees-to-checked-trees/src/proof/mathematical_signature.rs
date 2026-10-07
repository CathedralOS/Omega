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
//!   fixed non-address integers. `f32` and `f64` intern one dedicated
//!   `Type 0` carrier per format, kept distinct so an `f32` element
//!   never denotes into `f64`. The unit type `()` interns one dedicated
//!   `Unit : Type 0` carrier of its own. Every other resolved non-core
//!   symbol —
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
//!   `Σ(t : Two). caseTwo` sum), `!=` denotes the interned `Not`
//!   assumption applied to the shared-carrier `Id`, a bare `bool`
//!   subject `x` means `x = true`, and a `core::Strict` result wraps
//!   the proposition in `Squash` at the authored boundary. Float literals intern one
//!   constant per exact value at their format carrier — `1.5` and
//!   `1.50` share — and unlanded literals defer to the demanded
//!   carrier exactly as anonymous integer literals do. Every other
//!   machine expression whose scalar carrier is determinable — open
//!   products, quotients, shifts and bitwise operations including
//!   float arithmetic at its format carrier, unary `~`/`!`, field
//!   projections and casts — interns an opaque assumption constant
//!   keyed by the expression's own structure, the same rule the
//!   bounded denotation applies to open terms it does not name
//!   (`MathTermKey::Open`,
//!   `scalar_integer_terms`): `x * y` and `x * y` share one constant,
//!   `x * y` and `x * z` do not. Machine calls follow the same rule:
//!   the callee carries no kernel declaration, so `next(x)` interns
//!   opaquely at the machine's declared result carrier, while calls
//!   hauling machine, evidence, quotient or operational payload still
//!   refuse. Member calls keep refusing — resolution leaves their
//!   `target_symbol` unbound inside `let` bodies. `data` literals and
//!   `match` expressions intern the same way: the literal inhabits its
//!   type's interned carrier, and a match denotes at the one carrier
//!   every arm inhabits.
//! - Explicit generic applications follow the callee's ordered telescope:
//!   level binders instantiate `Constant.levels`, while the remaining binders
//!   form ordinary `Apply` terms before the ordinary argument prefix. A binder
//!   that claimed a generalized universe parameter infers it from the supplied
//!   type argument: the argument's inferred sort is `Type l`, so `l` is the
//!   instantiation. Authored `core::Level` arguments may all be omitted when
//!   supplied type arguments determine each one through its `core::Type<u>`
//!   carrier. Undetermined levels require explicit arguments. The kernel
//!   rechecks every level's scope and every dependent application, including
//!   repeated constraints on one level; inference grants no resizing.
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
//! constrained / dynamic-trait / array / slice type references,
//! computed level expressions other than literals, order relations over
//! non-integer operands — floats included, since `IntLt`/`IntLe` are
//! fixed non-address integers only — scalar equality over operands
//! whose carriers differ or cannot be determined, a whole-body float
//! literal with no landing to determine its carrier, and every body
//! expression whose scalar carrier cannot be determined at all
//! (matches, aggregates, indexing, strings, ranges, borrows, atomics).

mod applications;
mod carriers;
mod denotation_keys;
mod level_types;
mod operand_terms;
mod propositions;
mod scope;

use std::collections::{BTreeMap, HashMap};

use numerics::bignum::BigInt;

use crate::proof::{FixedCore, FixedCoreVocabulary};
use proof_admission::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Level, Signature, Sort, Term,
    TermArena, TermHandle, check_signature, infer_sort,
};
use source::SourceSpan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataProperties, TypeParameter, TypeParameterKind};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::mathematical::{MathematicalBody, MathematicalDefinition};
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// A checked kernel signature for a typed program's mathematical
/// declarations: the shared term arena, the re-decided declaration list
/// (carrier assumptions interleaved with authored declarations in use
/// order), and each authored declaration's signature position.
pub(crate) struct CheckedMathematicalSignature {
    /// The arena every `TermHandle` in `signature` resolves against; the
    /// two travel together — a signature's declarations mean nothing
    /// without it.
    arena: TermArena,
    /// The re-decided declaration list: carrier assumptions interleaved
    /// with authored declarations in signature order.
    signature: Signature,
    /// Kernel signature position of each authored declaration, in
    /// authored declaration order.
    authored: Vec<u32>,
}

impl CheckedMathematicalSignature {
    /// Consume the checked signature into the evidence record the proof
    /// facts retain for the Terminal encoding leg: the arena plus the
    /// re-decided declaration list in signature order. `Signature` shares
    /// its list through `Rc`, which the checked tree cannot hold without
    /// losing `Send`, so the record keeps the plain `Vec`; the codec
    /// producer rebuilds `Signature::from_declarations`.
    pub(crate) fn into_checked_evidence(self) -> checked_trees::CheckedMathematicalSignature {
        checked_trees::CheckedMathematicalSignature {
            arena: self.arena,
            declarations: self.signature.declarations().to_vec(),
            authored: self.authored,
        }
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
    /// `f32`'s carrier — the float formats intern one `Type 0` carrier
    /// each, so an `f32` element never meets an `f64` one.
    float32: Option<u32>,
    /// `f64`'s carrier.
    float64: Option<u32>,
    /// `()`'s carrier — unit is a `Type 0` assumption like every other
    /// fixed scalar, so a `()` binder domain or machine-call carrier
    /// denotes without naming `Int` or `Two`.
    unit: Option<u32>,
    /// `IntLt`/`IntLe : Π(_ : Int). Π(_ : Int). Type 0`, interned on demand.
    integer_less_than: Option<u32>,
    integer_less_or_equal: Option<u32>,
    /// `IntAdd`/`IntSub : Π(_ : Int). Π(_ : Int). Int` — open exact
    /// addition and subtraction compose; every other open machine
    /// operation interns opaquely through `open_terms` instead.
    integer_add: Option<u32>,
    integer_subtract: Option<u32>,
    /// `Not : Π(_ : Type 0). Type 0` — proposition negation stays at
    /// `Type 0` so `x != y` composes inside the `Σ`-conjunction and
    /// `Two`-disjunction vocabulary exactly like `x == y`.
    proposition_negation: Option<u32>,
    /// Open machine terms the composing vocabulary does not name intern
    /// as opaque carrier assumptions keyed by expression structure — the
    /// `MathTermKey::Open`/`scalar_integer_terms` rule of the bounded
    /// denotation, so `x * y` and `x * y` name one constant while
    /// `x * y` and `x * z` do not.
    open_terms: BTreeMap<Vec<u8>, u32>,

    /// Closed scalar literals interned by `(carrier position, exact
    /// value)` — the denotation is by value, so `2 + 0` and `2` name one
    /// constant.
    numeric_literals: BTreeMap<(u32, BigInt), u32>,
    /// `true`/`false` interned at the `bool` carrier.
    boolean_literals: [Option<u32>; 2],
    /// Float literals interned by `(carrier position, exact bits at the
    /// carrier's format)` — the denotation is by value, so `1.5` and
    /// `1.50` name one constant at a format while the formats stay
    /// distinct.
    float_literals: BTreeMap<(u32, u64), u32>,
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
    /// The fixed `core::*` vocabulary's canonical symbols in this program,
    /// resolved once — sort classification consults it before any authored
    /// spelling.
    fixed_core: FixedCoreVocabulary,
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
        float32: None,
        float64: None,
        unit: None,
        integer_less_than: None,
        integer_less_or_equal: None,
        integer_add: None,
        integer_subtract: None,
        proposition_negation: None,
        numeric_literals: BTreeMap::new(),
        open_terms: BTreeMap::new(),
        boolean_literals: [None, None],
        float_literals: BTreeMap::new(),
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
        fixed_core: FixedCoreVocabulary::resolve(&program.symbols),
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
    /// elaborating it. Identity is canonical: a resolved symbol is whatever
    /// it resolves to, so only the fixed `core::*` declarations' own symbols
    /// — or, until those declarations exist, the unresolved authored
    /// spelling — classify as vocabulary (the same rule the checked surface
    /// leg uses).
    fn carrier_head(&self, carrier: TypeReferenceHandle) -> CarrierHead {
        if !self
            .program
            .type_reference_table
            .contains_type_reference(carrier)
        {
            return CarrierHead::Term;
        }
        match self.program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Named { symbol, name } => {
                match self.fixed_core.classify(*symbol, name) {
                    Some(FixedCore::Level) => CarrierHead::Level,
                    Some(FixedCore::Type) => CarrierHead::Type(LevelArgument::Generalized),
                    _ => CarrierHead::Term,
                }
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                arguments,
                ..
            } => match self.fixed_core.classify(*base_symbol, base_name) {
                Some(FixedCore::Level) => CarrierHead::MalformedLevel,
                Some(FixedCore::Type) => {
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
    /// references, ordinary calls, literals and composing machine
    /// operators denote in the bounded vocabulary; every other
    /// expression whose scalar carrier is determinable interns an opaque
    /// constant (`open_term`), and the rest refuse.
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
                let target_symbol = call.target_symbol;
                if target_symbol.is_valid()
                    && !self.authored_index.contains_key(&target_symbol)
                    && !self
                        .scope
                        .iter()
                        .any(|entry| entry.symbol == Some(target_symbol))
                {
                    if call.static_machine_parameter.is_valid()
                        || !call.selects_only_nominal_route()
                        || !call.evidence_arguments.is_empty()
                        || call.operational_acknowledgement.acknowledges_suspend
                        || call.operational_acknowledgement.acknowledges_block
                    {
                        return Err(self.refuse(format!(
                            "call `{}` carries machine, evidence, quotient or operational payload the kernel cannot express",
                            call.target
                        )));
                    }
                    // A machine callee has no kernel declaration; the
                    // call interns as an opaque constant at the machine's
                    // declared result carrier (the structural key already
                    // carries the receiver when one exists).
                    return self.open_term(handle);
                }
                if call.receiver.is_valid() {
                    return Err(self.refuse(format!(
                        "member call `{}` has no mathematical denotation yet",
                        call.target
                    )));
                }
                if call.static_machine_parameter.is_valid()
                    || !call.selects_only_nominal_route()
                    || !call.evidence_arguments.is_empty()
                    || call.operational_acknowledgement.acknowledges_suspend
                    || call.operational_acknowledgement.acknowledges_block
                {
                    return Err(self.refuse(format!(
                        "call `{}` carries machine, evidence, quotient or operational payload the kernel cannot express",
                        call.target
                    )));
                }
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
            ExpressionNode::Float(literal) => {
                let literal = literal.clone();
                self.float_literal_term(&literal)
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
                    | BinaryOperator::BitwiseXor => match self.operand_carrier(handle) {
                        // Open arithmetic at another determined carrier —
                        // floats, `data` carriers — composes no named
                        // function and interns opaquely there. An
                        // undetermined carrier refuses inside
                        // `open_term` rather than claiming `Int`.
                        Some(ScalarCarrier::Integer) => {
                            self.integer_operation_term(handle, &binary)
                        }
                        _ => self.open_term(handle),
                    },
                    _ => match self.elaborate_proposition(handle)? {
                        Some(proposition) => Ok(proposition),
                        None => self.open_term(handle),
                    },
                }
            }
            ExpressionNode::ZeroValue(reference) => self.zero_value_term(*reference),
            _ => self.open_term(handle),
        }
    }

    /// The two-way carrier failure: the operand's carrier was determined
    /// and differs, or it could not be determined at all.
    fn unexpected_carrier_error(
        &self,
        determined: Option<ScalarCarrier>,
        handle: ExpressionHandle,
    ) -> Vec<diagnostics::Diagnostic> {
        match determined {
            Some(..) => self.refuse(format!(
                "operand `{}` inhabits a different scalar carrier",
                self.program.render_proof_expression(
                    handle,
                    typed_trees::proposition::ProofSubstitutions::None
                )
            )),
            None => self.refuse(format!(
                "cannot determine the scalar carrier of operand `{}`",
                self.program.render_proof_expression(
                    handle,
                    typed_trees::proposition::ProofSubstitutions::None
                )
            )),
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

    /// Re-decide the built signature: check each authored declaration's
    /// The `Type 0` proposition one machine expression denotes, when it
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
/// the dedicated `bool`/`addr` carriers, `Float32`/`Float64` the
/// dedicated `f32`/`f64` carriers, and `Carrier` names an interned
/// per-symbol `Type 0` assumption (`data` types, other builtin atoms).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScalarCarrier {
    Integer,
    Boolean,
    Address,
    Float32,
    Float64,
    Unit,
    Carrier(u32),
}
