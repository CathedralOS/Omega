//! A declared identity view shares natural-number proofs only after its exact
//! input, result, and subject carriers agree. Classification supplies neither
//! range membership nor descent, and never rewrites custom witness identity to
//! a builtin view. Diagnostic type spellings are not identity-view application evidence.

use language_semantics::RankingViewId;
use language_semantics::declaration_selection::CollectionMeasure;
use symbols::JoinedPath;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::measure::MeasureDefinition;

use validation::{
    MeasureBodyShape, ProjectionStep, declared_scalar_view, find_declared_measure,
    identity_subject_matches, measure_body_shape, measure_constraints_cover_subject,
};

/// The well-founded ordering selected for a `terminates by value -> Order` clause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RankingOrder {
    /// Built-in descending naturals.
    NatDescending,
    /// Built-in `Nat::BoundedDistance`: the named bounded-distance ranking over
    /// a two-subject tuple. `terminates by (index, limit) -> Nat::BoundedDistance`
    /// binds the subjects in order to the view's `(lower, upper)` parameters
    /// and ranks the pair by the natural-number distance from `lower` up to
    /// `upper`, which descends as the lower value climbs toward the fixed
    /// upper bound. Inferred for a plain two-subject tuple (the only builtin
    /// two-subject view) and selectable explicitly as `-> Nat::BoundedDistance`.
    BoundedDistance,
    /// Built-in `Slice::Length`.
    SliceLength,
    /// Built-in argumented `Nat::IncreasingTo(limit)` (TPR3, decision 23's
    /// acceptance test 5): one climbing subject ranked by its distance UP TO
    /// the bound the view names -- well-founded BECAUSE the bound is part of
    /// the view; authors never write the `limit - subject` subtraction. The
    /// stored handle is the bound argument; the provers run it through the
    /// bounded-distance machinery with `(subject, limit)` as `(lower,
    /// upper)`.
    IncreasingTo(ExpressionHandle),
    /// A declared `measure` whose body forwards the (already numeric) parameter.
    CustomNatDescending { measure: symbols::SymbolHandle },
    /// A declared `measure` whose body computes over its parameter with `+`
    /// and `*` (`{ value * 2 }`), admitted by validation's
    /// `declared_scalar_view` as strictly increasing on the naturals with
    /// builtin operator meaning. The produced rank is `body` with `parameter`
    /// bound to the subject; strict monotonicity is the only reason the
    /// subject's descent stands for that rank's descent, and the range
    /// judgment still proves membership and formation inside `carrier`.
    CustomScalarView {
        measure: symbols::SymbolHandle,
        parameter: symbols::SymbolHandle,
        body: ExpressionHandle,
        carrier: symbols::BuiltinTypeAtom,
    },
    /// A declared `measure` whose body projects a field of a struct parameter,
    /// e.g. `measure Card::PowerOrder(card: Card) -> u64 { card.power }`, or a
    /// nested path `{ card.stats.power }` whose record-typed steps `path` lead
    /// from the parameter's record to `owner`, the record declaring `field`.
    /// The subject is that root record, owned or reached through a reference;
    /// the stored field type retains its exact declaration's range constraints.
    /// `measure` is the declaration itself, which the relational range route
    /// hands to validation to re-resolve the projection independently.
    CustomStructView {
        measure: symbols::SymbolHandle,
        path: Vec<ProjectionStep>,
        field: typed_trees::name::Identifier,
        field_type: typed_trees::types::TypeReferenceHandle,
        field_symbol: symbols::SymbolHandle,
        owner: symbols::SymbolHandle,
    },
    /// A declared lexicographic `measure`; the stored field names are the ordered
    /// projection components compared left-to-right.
    Lexicographic(Vec<typed_trees::name::Identifier>),
}

/// The result of resolving the ordering for a `terminates by` clause: either a
/// concrete [`RankingOrder`], or a reason it could not be resolved that lets the
/// caller emit a precise diagnostic.
pub(super) enum OrderResolution {
    /// A well-founded order was selected.
    Resolved(RankingOrder),
    /// The clause omitted `-> View` and the decreasing value has no single
    /// builtin well-founded interpretation, so an explicit ranking view is
    /// required. Carries the details the caller renders into the diagnostic.
    AmbiguousDefault(AmbiguousDefault),
    /// An explicit `-> Order` was supplied but does not name a supported or
    /// declared, well-formed measure.
    Unsupported,
    /// A DIRECTED rejection (TPR3): the spelling is recognizably wrong in a
    /// specific way (an unbounded increasing view, an argument-arity
    /// mismatch, arguments on a plain view) and the message names the fix.
    Rejected { message: String },
}

/// Why a plain `terminates by value` clause could not infer a default ranking, plus
/// everything the diagnostic needs to suggest the explicit `-> View` form.
pub(super) struct AmbiguousDefault {
    /// The decreasing value rendered as source-like text, e.g. `self.health`.
    pub(super) clause: String,
    /// The specific reason inference failed.
    pub(super) reason: AmbiguityReason,
    /// Declared measures whose parameter (or lexicographic owner) matches the
    /// decreasing value's type, rendered as `Type::Name` selection paths. These
    /// are suggestions only: a declared measure is NEVER selected implicitly,
    /// even when it is the only one, so that declaring a second measure later
    /// cannot silently change or break distant `terminates by` clauses.
    pub(super) declared_measures: Vec<String>,
}

/// The reason a plain `terminates by value` has no inferable default order.
pub(super) enum AmbiguityReason {
    /// The value is a signed integer: `value - 1` is not well-founded without a
    /// positivity interpretation.
    SignedInteger,
    /// The value's type is known but offers no builtin well-founded order
    /// (floats, structs, and other non-natural shapes).
    NoBuiltinOrder { type_name: String },
    /// The value's type could not be determined, or the expression shape is not
    /// one the default inference understands.
    UnknownShape,
}

impl RankingOrder {
    /// Resolve the ordering for a `terminates by subjects -> order` clause. A
    /// single subject with an empty `order` (plain `terminates by value`) infers
    /// from the value's type when that interpretation is unambiguous;
    /// otherwise an [`OrderResolution::AmbiguousDefault`] is returned so the
    /// caller can ask for the explicit form. The explicit
    /// `terminates by value -> Type::Name` path is unchanged. A two-subject tuple
    /// `terminates by (lower, upper)` is the argumented bounded distance: the
    /// subjects bind in order to `Nat::BoundedDistance`'s `(lower, upper)`
    /// parameters, and the view is inferred when omitted because it is the
    /// only builtin two-subject ranking.
    fn resolve(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        subjects: &[ExpressionHandle],
        order: &[&str],
        view_arguments: &[ExpressionHandle],
    ) -> OrderResolution {
        // TPR3: the argumented-view surface resolves (or rejects) FIRST --
        // its spellings are specific enough for directed diagnostics.
        if path_matches(order, &["Nat", "Increasing"]) {
            return OrderResolution::Rejected {
                message: "unbounded `Nat::Increasing` is not a well-founded ranking -- the bound is part of the view: name it with `-> Nat::IncreasingTo(limit)`"
                    .to_string(),
            };
        }
        if path_matches(order, &["Nat", "IncreasingTo"]) {
            if subjects.len() != 1 {
                return OrderResolution::Rejected {
                    message: "`Nat::IncreasingTo(limit)` ranks exactly ONE climbing subject -- the bound is the view's argument, not a second subject"
                        .to_string(),
                };
            }
            let [limit] = view_arguments else {
                return OrderResolution::Rejected {
                    message: "`Nat::IncreasingTo` names exactly one bound argument -- spell `-> Nat::IncreasingTo(limit)`"
                        .to_string(),
                };
            };
            return OrderResolution::Resolved(Self::IncreasingTo(*limit));
        }
        if !view_arguments.is_empty() {
            let path = order.to_vec().join("::");
            return OrderResolution::Rejected {
                message: format!(
                    "view arguments are only meaningful on an argumented view (`Nat::IncreasingTo(limit)`); `{path}` takes none"
                ),
            };
        }
        // A path that names neither a builtin view nor a declared measure is a
        // misspelling, not a ranking the checker failed to prove.
        if !order.is_empty()
            && RankingViewId::canonical(&order.join("::")).is_none()
            && find_declared_measure(program, order).is_none()
        {
            return OrderResolution::Rejected {
                message: format!(
                    "`{}` names no ranking view; the builtin views are `Nat::Descending`, \
                     `Nat::BoundedDistance`, `Nat::IncreasingTo(limit)` and `Slice::Length`",
                    order.join("::")
                ),
            };
        }
        match subjects {
            [single] => {
                if order.is_empty() {
                    return infer_default_order(program, state, *single);
                }
                match Self::from_path(program, state, *single, order) {
                    Some(resolved) => OrderResolution::Resolved(resolved),
                    None => OrderResolution::Unsupported,
                }
            }
            [_lower, _upper] => {
                if order.is_empty() || path_matches(order, &["Nat", "BoundedDistance"]) {
                    OrderResolution::Resolved(Self::BoundedDistance)
                } else {
                    // No other builtin or declared view takes two subjects.
                    OrderResolution::Unsupported
                }
            }
            _ => OrderResolution::Unsupported,
        }
    }

    /// Resolve the order a machine's ranking witness selects, reading its
    /// names in `root_state`. The recorded `view_path` is the authored
    /// `-> View` or the canonical default the syntax->resolved lowering
    /// elaborated from the declared carrier (decision 23); an empty path is a
    /// pending short form, which [`Self::resolve`] infers from the subject's
    /// checked type. Every termination judgment reads the recorded view
    /// through this one site.
    pub(super) fn resolve_witness(
        program: &dyn ::validation::CheckingInput,
        root_state: &typed_trees::state::State,
        witness: &language_semantics::RankingWitness,
        subjects: &[ExpressionHandle],
        view_arguments: &[ExpressionHandle],
    ) -> OrderResolution {
        let order = witness
            .view_path
            .split("::")
            .filter(|member| !member.is_empty())
            .collect::<Vec<_>>();
        Self::resolve(program, root_state, subjects, &order, view_arguments)
    }

    fn from_path(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        decreases: ExpressionHandle,
        order: &[&str],
    ) -> Option<Self> {
        if let Some(view) = RankingViewId::canonical(&order.join("::")) {
            // A canonical view is keyed on its catalog identity, and a view
            // with a declaration row is consulted at that declaration: the
            // spelling alone selects nothing when the program declares a
            // same-named `Nat::Descending` that is not the sealed one.
            if !catalog_declaration_is_authentic(program, view) {
                return None;
            }
            return match view {
                RankingViewId::NAT_DESCENDING => Some(Self::NatDescending),
                RankingViewId::SLICE_LENGTH => Some(Self::SliceLength),
                // The bounded distance ranks a `(lower, upper)` pair; a single
                // subject has no distance reading to select. The argumented
                // views resolve (or reject) upstream in `resolve`.
                _ => None,
            };
        }

        let measure = find_declared_measure(program, order)?;

        if measure.lexicographic {
            let components = lexicographic_component_fields(program, measure)?;
            return Some(Self::Lexicographic(components));
        }

        let Some(shape) = measure_body_shape(program, measure) else {
            // Neither a forward nor a projection: only a computation admitted
            // for this exact subject by the shared classification remains.
            let view = declared_scalar_view(program, state, decreases, &order.join("::"))?;
            let computation = view.computation?;
            return Some(Self::CustomScalarView {
                measure: measure.symbol,
                parameter: computation.parameter,
                body: computation.body,
                carrier: view.carrier,
            });
        };
        match shape {
            MeasureBodyShape::ParameterForward {
                carrier,
                constraints,
            } => {
                if identity_subject_matches(program, state, decreases, carrier)
                    && measure_constraints_cover_subject(program, state, decreases, &constraints)
                {
                    Some(Self::CustomNatDescending {
                        measure: measure.symbol,
                    })
                } else {
                    None
                }
            }
            MeasureBodyShape::FieldProjection {
                path,
                field,
                owner,
                field_type,
                field_symbol,
            } => {
                // Applying the projection requires the exact nominal carrier,
                // not another declaration with the same displayed type name.
                // The subject may reach that record through a reference: the
                // view reads the referent's fields, and the self-loop prover
                // separately keeps the binding and the ranked path unwritten.
                let subject_path = program.expression_table.name_path_node(decreases)?;
                let subject = program.state_parameters(state).iter().find(|parameter| {
                    (parameter.symbol.is_valid())
                        && parameter.symbol == subject_path.symbol
                        && subject_path.head_symbol == subject_path.symbol
                })?;
                let root = path.first().map_or(owner, |step| step.owner);
                let mut reference = program
                    .type_reference_table
                    .unconstrained_type_reference(subject.type_reference);
                if let typed_trees::types::TypeReferenceNode::Reference { referee, .. } =
                    program.type_reference_table.type_reference(reference)
                {
                    reference = program
                        .type_reference_table
                        .unconstrained_type_reference(*referee);
                }
                if !matches!(program.type_reference_table.type_reference(reference),
                    typed_trees::types::TypeReferenceNode::Named { symbol, .. } if *symbol == root)
                {
                    return None;
                }
                Some(Self::CustomStructView {
                    measure: measure.symbol,
                    path,
                    field,
                    field_type,
                    field_symbol,
                    owner,
                })
            }
        }
    }
}

/// Infer the well-founded order for a plain `terminates by value` clause (no
/// explicit `-> Order`) from the shape and type of the decreasing value.
///
/// Inference only succeeds when the value has a single obvious well-founded
/// reading:
///   * a `usize`/nat-like scalar (or `slice.len`) counts down via descending
///     naturals;
///   * a slice parameter decreases by its length.
///
/// (The two-subject bounded distance `terminates by (index, limit)` is inferred
/// upstream in [`RankingOrder::resolve`].)
///
/// Anything else (an unrecognized expression, or a value whose type offers no
/// obvious interpretation) is reported as ambiguous so the caller can require
/// the explicit `-> View` form. A declared measure is never selected
/// implicitly — only true builtins infer — but matching measures are carried
/// along as suggestions for the diagnostic.
fn infer_default_order(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    decreases: ExpressionHandle,
) -> OrderResolution {
    match decreasing_value_kind(program, state, decreases) {
        Some(DecreasingValueKind::Nat) => OrderResolution::Resolved(RankingOrder::NatDescending),
        Some(DecreasingValueKind::Slice) => OrderResolution::Resolved(RankingOrder::SliceLength),
        None => OrderResolution::AmbiguousDefault(describe_ambiguity(program, state, decreases)),
    }
}

/// Build the details the ambiguity diagnostic renders: the clause text, the
/// reason the value's type has no default order, and any declared measures the
/// user could select explicitly.
fn describe_ambiguity(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    decreases: ExpressionHandle,
) -> AmbiguousDefault {
    let clause = decreasing_value_text(program, decreases);
    let type_name = expression_type_name(program, state, decreases);
    let reason = match type_name.as_deref() {
        Some(name) if is_signed_integer_type(name) => AmbiguityReason::SignedInteger,
        Some(name) => AmbiguityReason::NoBuiltinOrder {
            type_name: name.to_string(),
        },
        None => AmbiguityReason::UnknownShape,
    };
    let declared_measures = type_name
        .as_deref()
        .map(|name| declared_measures_for_type(program, name))
        .unwrap_or_default();
    AmbiguousDefault {
        clause,
        reason,
        declared_measures,
    }
}

/// Render the decreasing value as source-like text for diagnostics. Falls back
/// to the generic word `value` for shapes the renderer does not understand.
pub(super) fn decreasing_value_text(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> String {
    typed_trees::ranking::witness_expression_text(program, expression)
}

/// A type whose `value - 1` step is not well-founded without a positivity
/// interpretation: the signed integer primitives.
fn is_signed_integer_type(name: &str) -> bool {
    matches!(name, "i8" | "i16" | "i32" | "i64" | "isize")
}

/// Declared measures applicable to a value of the named type, rendered as
/// `Type::Name` selection paths. A simple measure applies when its parameter
/// has the value's type; a lexicographic measure applies when its owner (the
/// first path segment) is the value's type. These are diagnostic suggestions
/// only — plain `terminates by value` never selects a declared measure implicitly.
fn declared_measures_for_type(
    program: &dyn ::validation::CheckingInput,
    type_name: &str,
) -> Vec<String> {
    program
        .measures()
        .iter()
        .filter(|measure| {
            let path = program.measure_path_members(measure.name);
            match measure.parameter.as_ref() {
                Some(parameter) => {
                    program
                        .type_reference_table
                        .display_name(parameter.type_reference)
                        == type_name
                }
                None => path
                    .first()
                    .is_some_and(|owner| owner.as_str() == type_name),
            }
        })
        .map(|measure| program.measure_path_members(measure.name).joined_path())
        .collect()
}

/// The well-founded interpretation a plain decreasing value's type admits.
enum DecreasingValueKind {
    /// A `usize`/nat-like scalar that counts down through the naturals.
    Nat,
    /// A slice that decreases by its length.
    Slice,
}

fn decreasing_value_kind(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    decreases: ExpressionHandle,
) -> Option<DecreasingValueKind> {
    // `value.len` (or any member spelled `len`) is a nat-like scalar.
    if ::validation::length_member(program, decreases).is_some() {
        return Some(DecreasingValueKind::Nat);
    }

    let reference = ranking_subject_type_reference(program, state, decreases)?;
    if program
        .primitive_type_reference(reference)
        .is_some_and(|primitive| {
            matches!(
                primitive,
                typed_trees::unsigned_integer_primitive_pattern!()
            )
        })
    {
        return Some(DecreasingValueKind::Nat);
    }
    let base = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    if matches!(
        program.type_reference_table.type_reference(base),
        typed_trees::types::TypeReferenceNode::Slice { .. }
            | typed_trees::types::TypeReferenceNode::FixedArray { .. }
    ) {
        return Some(DecreasingValueKind::Slice);
    }
    None
}

/// Ranking interpretation follows the exact declared subject carrier, including
/// machine Value/Const binders. Specialization keeps the view selected before
/// replacing such a binder by a value; a new anonymous literal supplies no
/// carrier merely because it prints the same number.
fn ranking_subject_type_reference(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if let ExpressionNode::Name(path) = program.expression_table.expression(expression)
        && path.symbol.is_valid()
        && path.head_symbol == path.symbol
    {
        if let Some(parameter) = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == path.symbol)
        {
            return Some(parameter.type_reference);
        }
        if let Some(parameter) = program.data_type_parameter_by_symbol(path.symbol) {
            match parameter.kind {
                typed_trees::data::TypeParameterKind::Value { type_reference }
                | typed_trees::data::TypeParameterKind::Const { type_reference } => {
                    return Some(type_reference);
                }
                _ => return None,
            }
        }
    }
    validation::mathematical_applications::operand_type_reference(program, expression)
}

fn lexicographic_component_fields(
    program: &dyn ::validation::CheckingInput,
    measure: &MeasureDefinition,
) -> Option<Vec<typed_trees::name::Identifier>> {
    let body = program.expression_table.expression_handles(measure.body);
    if body.is_empty() {
        return None;
    }
    let mut fields = Vec::with_capacity(body.len());
    for component in body {
        match program.expression_table.expression(*component) {
            ExpressionNode::Name(path) => {
                let member = program
                    .expression_table
                    .name_path_members(path.members)
                    .last()
                    .cloned()?;
                fields.push(member);
            }
            ExpressionNode::Member(member) => fields.push(member.member.clone()),
            _ => return None,
        }
    }
    Some(fields)
}

fn expression_type_name(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<String> {
    if let Some(reference) = ranking_subject_type_reference(program, state, expression) {
        return Some(
            program.type_reference_table.display_name(
                program
                    .type_reference_table
                    .unconstrained_type_reference(reference),
            ),
        );
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => state_parameter_type_name(
            program,
            state,
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .last()
                .map(|member| member.as_str()),
        ),
        ExpressionNode::Member(member)
            if CollectionMeasure::from_authored_spelling(member.member.as_str())
                == Some(CollectionMeasure::Length) =>
        {
            Some("u64".to_string())
        }
        ExpressionNode::Member(member) => program
            .data_definitions()
            .iter()
            .flat_map(|data| program.data_members(data).iter())
            .find_map(|data_member| match data_member {
                DataMember::Field(field)
                    if field.symbol == member.member_symbol
                        || field.name.as_str() == member.member.as_str() =>
                {
                    Some(
                        program
                            .type_reference_table
                            .display_name(field.type_reference),
                    )
                }
                _ => None,
            }),
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                typed_trees::expression::BinaryOperator::Subtract
            ) =>
        {
            Some("u64".to_string())
        }
        _ => None,
    }
}

fn state_parameter_type_name(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    symbol: symbols::SymbolHandle,
    fallback_name: Option<&str>,
) -> Option<String> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| {
            parameter.symbol == symbol
                || fallback_name.is_some_and(|name| parameter.name.as_str() == name)
        })
        .map(|parameter| {
            program.type_reference_table.display_name(
                program
                    .type_reference_table
                    .unconstrained_type_reference(parameter.type_reference),
            )
        })
}

/// Whether every declaration the program carries at a canonical view's
/// catalog path is the sealed toolchain declaration the row names: hermetic
/// identity `toolchain::Nat::Descending` declared by a Toolchain-origin
/// `nat.omg` under its package root. A source-free program (no declaration
/// at the path) keeps the builtin reading; a view without a declaration row
/// is a spelling-only builtin. Symbol resolution already refuses a bare
/// bodyless lookalike outside the sealed source, so this consult closes the
/// bodied and boundary spellings of the same path.
fn catalog_declaration_is_authentic(
    program: &dyn ::validation::CheckingInput,
    view: RankingViewId,
) -> bool {
    let Some(row) = view.catalog_declaration() else {
        return true;
    };
    let sealed = |symbol: symbols::SymbolHandle| {
        program
            .normalized_hermetic_symbol_identity(symbol)
            .ok()
            .as_deref()
            == Some(format!("toolchain::{}", row.path()).as_str())
            && program
                .symbols
                .symbol_source_span(symbol)
                .and_then(|span| program.symbols.source_file(span))
                .is_some_and(|source| {
                    source.origin == source::SourceOrigin::Toolchain
                        && source
                            .path
                            .strip_prefix(&source.package_root)
                            .ok()
                            .is_some_and(|relative| relative == std::path::Path::new(row.source))
                })
    };
    let operators_authentic = program.operators().iter().all(|operator| {
        let path = program.operator_path_members(operator.name);
        let at_row =
            path.len() == 2 && path[0].as_str() == row.namespace && path[1].as_str() == row.name;
        !at_row || sealed(operator.symbol)
    });
    let path = row.path();
    let machines_authentic = program.machines().iter().all(|machine| {
        // An attached machine's name is already its qualified path.
        let at_row = machine.name.as_str() == path
            || (machine
                .attached_data
                .as_ref()
                .is_some_and(|owner| owner.as_str() == row.namespace)
                && machine.name.as_str() == row.name);
        !at_row || sealed(machine.symbol)
    });
    operators_authentic && machines_authentic
}

fn path_matches(order: &[&str], expected: &[&str]) -> bool {
    order.len() == expected.len()
        && order
            .iter()
            .zip(expected.iter())
            .all(|(actual, expected)| actual == expected)
}
