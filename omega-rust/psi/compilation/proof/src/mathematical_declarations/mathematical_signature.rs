//! Prepare mathematical declarations and source applications before checking.
//!
//! One checked signature owns dependent telescopes, universe instantiation and
//! capture-avoiding beta reduction. Ordinary application consumers borrow its
//! reduced terms with the exact caller captures; they do not rediscover a body
//! from source arity or substitute reconstructed expression trees.
//!
//! Kernel typing and source meaning remain separate. Fixed integer carriers
//! share Int for formation, while source intake retains exact bounded carrier
//! and arithmetic policy. Open source operations are functions over their
//! inputs, never scope-dependent zero-argument constants. Their retained source
//! operation identity does not turn an assumption into a value law. IEEE and
//! custom comparisons likewise do not acquire the kernel's reflexive Id law.
//!
//! Level binders instantiate Constant levels; other binders occupy the same
//! ordered Pi/Lambda telescope. Strict results wrap relevant propositions in
//! Squash. A boundary mathematical declaration supplies a typed neutral term,
//! not an equality or normal-return guarantee. Executable operations embedded
//! in mathematical roots still pass the ordinary Requires/totality gates.

mod applications;
mod carriers;
mod denotation_keys;
mod dependencies;
mod level_types;
mod normalization;
mod operand_terms;
mod propositions;
mod scope;
mod source_domains;

use std::collections::{BTreeMap, HashMap};

use numerics::bignum::BigInt;
use validation::mathematical_applications::{
    PreparedApplication, PreparedCapture, PreparedConstant, PreparedConstantMeaning,
    PreparedMathematicalApplications,
};

use super::{FixedCore, FixedCoreVocabulary};
use proof_admission::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Level, Signature, Sort, Term,
    TermArena, TermHandle, check_signature_with_attribution, infer_sort,
};
use source::SourceSpan;
use symbols::SymbolHandle;
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
    prepared: PreparedMathematicalApplications,
}

impl CheckedMathematicalSignature {
    /// Consume the checked signature into the evidence record the proof
    /// facts retain for the Terminal encoding leg: the arena plus the
    /// re-decided declaration list in signature order. `Signature` shares
    /// its list through `Rc`, which the checked tree cannot hold without
    /// losing `Send`, so the record keeps the plain `Vec`; the codec
    /// producer rebuilds `Signature::from_declarations`.
    pub(crate) fn into_checked_evidence(
        self,
    ) -> (
        checked_trees::CheckedMathematicalSignature,
        PreparedMathematicalApplications,
    ) {
        let signature = checked_trees::CheckedMathematicalSignature {
            arena: self.arena,
            declarations: self.signature.declarations().to_vec(),
            authored: self.authored,
        };
        (signature, self.prepared)
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
    program: &'a dyn ::validation::CheckingInput,
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
    address_bound: Option<u32>,
    /// Source span of the declaration under elaboration, for diagnostics.
    span: Option<SourceSpan>,
    source_owner: SymbolHandle,
    captured_expressions: Vec<(ExpressionHandle, usize)>,
    source_aliases: Vec<source_domains::SourceAlias>,
    source_invocations: Vec<source_domains::SourceInvocation>,
    prepared: PreparedMathematicalApplications,
}

/// Elaborate every mathematical declaration of `program` into a kernel
/// signature and re-decide it. Returns the checked signature, or the
/// elaboration/checking diagnostics for the first declaration that fails.
pub(crate) fn check_mathematical_signature(
    program: &dyn ::validation::CheckingInput,
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
        address_bound: None,
        span: None,
        source_owner: SymbolHandle::default(),
        captured_expressions: Vec::new(),
        source_aliases: Vec::new(),
        source_invocations: Vec::new(),
        prepared: PreparedMathematicalApplications::default(),
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
        self.source_owner = definition.symbol;
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
            TypeParameterKind::Implementation => Err(self.refuse(format!(
                "mathematical binder `{}` cannot select an implementation; implementation slots belong to trait telescopes",
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
        if let Some((_, position)) = self
            .captured_expressions
            .iter()
            .find(|(expression, _)| *expression == handle)
        {
            return Ok(self.scope_variable(*position));
        }
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
                let call = call.clone();
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
                    self.elaborate_callee(handle, target_symbol, &target, &static_arguments)?;
                let callee = function;
                let domains = self.scope.iter().map(|entry| entry.domain).collect();
                let mut inputs = Vec::new();
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec()
                {
                    let term = self.elaborate_expression(argument)?;
                    let source_domain = if matches!(self.program.expression_table.expression(argument),
                        ExpressionNode::Call(call) if self.authored_index.contains_key(&call.target_symbol))
                        || matches!(self.program.expression_table.expression(argument),
                            ExpressionNode::Name(path) if self.program.symbols.builtin_type_atom(path.symbol).is_some()) {
                        None
                    } else {
                        validation::mathematical_applications::operand_type_reference(self.program, argument)
                    }.map(|reference| self.elaborate_type_reference(reference)).transpose()?;
                    inputs.push(source_domains::SourceInput {
                        expression: argument,
                        term,
                        domain: source_domain,
                    });
                    function = self.arena.insert(Term::Apply {
                        function,
                        argument: term,
                    });
                }
                self.source_invocations
                    .push(source_domains::SourceInvocation {
                        expression: handle,
                        callee,
                        inputs,
                        domains,
                        level_arity: self.level_binders.len() as u32 + self.generalized_levels,
                    });
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

    /// Form the complete signature once. The kernel checks each declaration
    /// against its preceding prefix and reports its position; application
    /// judgments then borrow that same checked signature.
    fn check(mut self) -> Result<CheckedMathematicalSignature, Vec<diagnostics::Diagnostic>> {
        let applications = self.source_applications()?;
        let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
        let signature =
            check_signature_with_attribution(&mut self.arena, &self.declarations, &mut budget)
                .map_err(|failure| {
                    let authored = self
                        .authored
                        .iter()
                        .position(|position| *position as usize >= failure.declaration);
                    if let Some(index) = authored {
                        let definition = &self.program.mathematical_definitions()[index];
                        self.span = self.program.symbols.symbol_source_span(definition.symbol);
                        self.refuse(format!(
                            "mathematical declaration `{}` fails kernel checking: {:?}",
                            definition.name, failure.error
                        ))
                    } else {
                        self.span = None;
                        self.refuse(format!(
                            "mathematical application signature fails checking: {:?}",
                            failure.error
                        ))
                    }
                })?;
        self.check_source_domains(&signature)?;
        for (expression, term, domains, captures) in applications {
            let context = domains.into_iter().fold(
                Context::empty().with_signature(signature.clone()),
                |context, domain| context.extend(domain),
            );
            let mut budget = Budget::new(DEFAULT_CONVERSION_STEPS);
            proof_admission::infer_type(&mut self.arena, &context, term, &mut budget).map_err(
                |error| {
                    self.refuse(format!(
                        "mathematical application `{}` fails checking: {error:?}",
                        self.program.render_proof_expression(
                            expression,
                            typed_trees::proposition::ProofSubstitutions::None
                        )
                    ))
                },
            )?;
            let term = normalization::normalize(&mut self.arena, &signature, term, &mut budget)
                .map_err(|error| {
                    self.refuse(format!(
                        "mathematical application reduction failed: {error:?}"
                    ))
                })?;
            let captures = self.prepared.captures.insert_many(captures);
            self.prepared.applications.append(PreparedApplication {
                expression,
                term,
                captures,
            });
        }
        self.retain_constant_meanings();
        Ok(CheckedMathematicalSignature {
            arena: self.arena,
            signature,
            authored: self.authored,
            prepared: self.prepared,
        })
    }

    fn retain_constant_meanings(&mut self) {
        let mut retain = |declaration, meaning| {
            self.prepared.constants.append(PreparedConstant {
                declaration,
                meaning,
            });
        };
        for ((_, value), position) in &self.numeric_literals {
            retain(*position, PreparedConstantMeaning::Integer(value.clone()));
        }
        for (value, position) in self.boolean_literals.iter().enumerate() {
            if let Some(position) = position {
                retain(*position, PreparedConstantMeaning::Boolean(value != 0));
            }
        }
        if let Some(position) = self.integer {
            retain(position, PreparedConstantMeaning::IntegerCarrier);
        }
        if let Some(position) = self.boolean {
            retain(position, PreparedConstantMeaning::BooleanCarrier);
        }
        if let Some(position) = self.address {
            retain(position, PreparedConstantMeaning::AddressCarrier);
        }
        if let Some(position) = self.address_bound {
            if let Some(symbol) = self
                .program
                .symbols
                .builtin_type_member_symbol(symbols::BuiltinTypeMember::AddressBound)
            {
                retain(position, PreparedConstantMeaning::AddressBound(symbol));
            }
        }
        if let Some(position) = self.integer_add {
            retain(
                position,
                PreparedConstantMeaning::IntegerOperation(BinaryOperator::Add),
            );
        }
        if let Some(position) = self.integer_subtract {
            retain(
                position,
                PreparedConstantMeaning::IntegerOperation(BinaryOperator::Subtract),
            );
        }
        if let Some(position) = self.integer_less_than {
            retain(
                position,
                PreparedConstantMeaning::IntegerRelation(BinaryOperator::Less),
            );
        }
        if let Some(position) = self.integer_less_or_equal {
            retain(
                position,
                PreparedConstantMeaning::IntegerRelation(BinaryOperator::LessOrEqual),
            );
        }
        if let Some(position) = self.proposition_negation {
            retain(position, PreparedConstantMeaning::Negation);
        }
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
