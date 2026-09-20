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
//! - Every other resolved non-core symbol in a type position — builtin
//!   types like `u64`, authored `data` types — is an uninterpreted
//!   carrier: on first use it interns `Declaration::assumption(0, Type 0)`
//!   into the shared signature prefix (the `bounded_denotation` pattern),
//!   so later declarations can reference earlier ones through
//!   `Term::Constant`.
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
//! computed level expressions other than literals, and every body
//! expression form beyond name references and ordinary application
//! (integer and operator denotation is the bounded-denotation leg).

mod applications;

use std::collections::HashMap;

use proof_admission::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Level, Signature, Sort, Term,
    TermArena, TermHandle, check_signature, infer_sort,
};
use source::SourceSpan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataProperties, TypeParameter, TypeParameterKind};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::mathematical::{
    MathematicalBody, MathematicalDefinition, MathematicalType, MathematicalTypeHandle,
};
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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

        let mut ty = self.elaborate_mathematical_type(definition.result)?;
        for entry in self.scope.iter().rev() {
            ty = self.arena.insert(Term::Pi {
                domain: entry.domain,
                codomain: ty,
            });
        }
        let body = match &definition.body {
            MathematicalBody::Assumption => None,
            MathematicalBody::Definition(term) => {
                let mut body = self.elaborate_expression(*term)?;
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

    /// Elaborate a body or application-argument expression. Name
    /// references and ordinary calls denote; every other form (literals,
    /// operators, members, ...) belongs to the bounded-denotation leg and
    /// refuses.
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
            _ => Err(self.refuse(format!(
                "expression `{}` has no kernel denotation yet",
                self.program.render_proof_expression(
                    handle,
                    typed_trees::proposition::ProofSubstitutions::None
                )
            ))),
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
    /// Symbols of other kinds decline so the caller can refuse.
    fn carrier_for_symbol(&mut self, symbol: SymbolHandle) -> Option<TermHandle> {
        if let Some(&position) = self.carriers.get(&symbol) {
            let term = self.arena.insert(Term::Constant {
                declaration: position,
                levels: Vec::new(),
            });
            return Some(term);
        }
        match self.program.symbols.get(symbol).kind {
            symbols::SymbolKind::BuiltinType | symbols::SymbolKind::Data => {}
            _ => return None,
        }
        let ty = self
            .arena
            .insert(Term::Sort(Sort::Type(Level::Constant(0))));
        let position = self.declarations.len() as u32;
        self.declarations.push(Declaration::assumption(0, ty));
        self.carriers.insert(symbol, position);
        Some(self.arena.insert(Term::Constant {
            declaration: position,
            levels: Vec::new(),
        }))
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

#[cfg(test)]
mod tests;
