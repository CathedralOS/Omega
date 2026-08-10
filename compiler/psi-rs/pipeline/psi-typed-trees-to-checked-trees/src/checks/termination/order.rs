use psi_typed_trees::data::DataMember;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::measure::MeasureDefinition;

/// The well-founded ordering selected for a `terminates by value -> Order` clause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RankingOrder {
    /// Built-in descending naturals (also used for a simple `usize`-valued measure
    /// whose body forwards the parameter directly).
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
    CustomNatDescending,
    /// A declared `measure` whose body projects a field of a struct parameter,
    /// e.g. `measure Card::PowerOrder(card: Card) -> usize { card.power }`. The
    /// stored identifier is the projected field name.
    CustomStructView(psi_typed_trees::name::Identifier),
    /// A declared lexicographic `measure`; the stored field names are the ordered
    /// projection components compared left-to-right.
    Lexicographic(Vec<psi_typed_trees::name::Identifier>),
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
    pub(super) fn resolve(
        program: &psi_typed_trees::TypedTrees,
        state: &psi_typed_trees::state::State,
        subjects: &[ExpressionHandle],
        order: &[&str],
        view_arguments: &[ExpressionHandle],
    ) -> OrderResolution {
        // TPR3: the argumented-view surface resolves (or rejects) FIRST --
        // its spellings are specific enough for directed diagnostics.
        if path_matches(order, &["Nat", "Increasing"]) {
            return OrderResolution::Rejected {
                message: "unbounded `Nat::Increasing` is not a well-founded ranking --                           the bound is part of the view: name it with                           `-> Nat::IncreasingTo(limit)`"
                    .to_string(),
            };
        }
        if path_matches(order, &["Nat", "IncreasingTo"]) {
            if subjects.len() != 1 {
                return OrderResolution::Rejected {
                    message: "`Nat::IncreasingTo(limit)` ranks exactly ONE climbing                               subject -- the bound is the view's argument, not a second                               subject"
                        .to_string(),
                };
            }
            let [limit] = view_arguments else {
                return OrderResolution::Rejected {
                    message: "`Nat::IncreasingTo` names exactly one bound argument --                               spell `-> Nat::IncreasingTo(limit)`"
                        .to_string(),
                };
            };
            return OrderResolution::Resolved(Self::IncreasingTo(*limit));
        }
        if !view_arguments.is_empty() {
            let path = order.iter().copied().collect::<Vec<_>>().join("::");
            return OrderResolution::Rejected {
                message: format!(
                    "view arguments are only meaningful on an argumented view                      (`Nat::IncreasingTo(limit)`); `{path}` takes none"
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

    fn from_path(
        program: &psi_typed_trees::TypedTrees,
        state: &psi_typed_trees::state::State,
        decreases: ExpressionHandle,
        order: &[&str],
    ) -> Option<Self> {
        if path_matches(order, &["Nat", "Descending"]) {
            return Some(Self::NatDescending);
        }
        if path_matches(order, &["Nat", "BoundedDistance"]) {
            // The bounded distance ranks a `(lower, upper)` pair; a single
            // subject has no distance reading to select.
            return None;
        }
        if path_matches(order, &["Slice", "Length"]) {
            return Some(Self::SliceLength);
        }

        let measure = find_declared_measure(program, order)?;

        if measure.lexicographic {
            let components = lexicographic_component_fields(program, measure)?;
            return Some(Self::Lexicographic(components));
        }

        // Simple measure: validate parameter / return shape, then classify the body.
        if !measure_return_is_natural(program, measure) {
            return None;
        }

        match measure_body_shape(program, measure)? {
            MeasureBodyShape::ParameterForward => {
                // The decreasing value is already the numeric quantity.
                if measure_parameter_is_natural(program, measure)
                    && expression_type_name(program, state, decreases)
                        .as_deref()
                        .is_some_and(|name| natural_measure_names_match(name, "u64"))
                {
                    Some(Self::CustomNatDescending)
                } else {
                    None
                }
            }
            MeasureBodyShape::FieldProjection(field) => {
                // The decreasing value must have the measure's parameter type.
                let parameter_type = measure.parameter.as_ref().map(|parameter| {
                    program
                        .type_reference_table
                        .display_name(parameter.type_reference)
                })?;
                if !expression_type_name(program, state, decreases)
                    .as_deref()
                    .is_some_and(|name| natural_measure_names_match(name, parameter_type.as_str()))
                {
                    return None;
                }
                Some(Self::CustomStructView(field))
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
/// upstream in [`RankingOrder::resolve`]; the retired use-site subtraction
/// `terminates by upper - lower` is rejected before inference runs.)
///
/// Anything else (an unrecognized expression, or a value whose type offers no
/// obvious interpretation) is reported as ambiguous so the caller can require
/// the explicit `-> View` form. A declared measure is never selected
/// implicitly — only true builtins infer — but matching measures are carried
/// along as suggestions for the diagnostic.
fn infer_default_order(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    decreases: ExpressionHandle,
) -> OrderResolution {
    match program.expression_table.expression(decreases) {
        // `terminates by value` where `value` is a nat-like scalar counts down; where
        // it is a slice it decreases by length. A `value.len` member is already
        // nat-like.
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            match decreasing_value_kind(program, state, decreases) {
                Some(DecreasingValueKind::Nat) => {
                    OrderResolution::Resolved(RankingOrder::NatDescending)
                }
                Some(DecreasingValueKind::Slice) => {
                    OrderResolution::Resolved(RankingOrder::SliceLength)
                }
                None => {
                    OrderResolution::AmbiguousDefault(describe_ambiguity(program, state, decreases))
                }
            }
        }
        _ => OrderResolution::AmbiguousDefault(describe_ambiguity(program, state, decreases)),
    }
}

/// Build the details the ambiguity diagnostic renders: the clause text, the
/// reason the value's type has no default order, and any declared measures the
/// user could select explicitly.
fn describe_ambiguity(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
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
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> String {
    psi_typed_trees::ranking::witness_expression_text(program, expression)
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
    program: &psi_typed_trees::TypedTrees,
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
        .map(|measure| {
            program
                .measure_path_members(measure.name)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        })
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
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    decreases: ExpressionHandle,
) -> Option<DecreasingValueKind> {
    // `value.len` (or any member named `len`) is a nat-like scalar.
    if let ExpressionNode::Member(member) = program.expression_table.expression(decreases) {
        if member.member.as_str() == "len" {
            return Some(DecreasingValueKind::Nat);
        }
    }

    let parameter = state_parameter_of_expression(program, state, decreases)?;
    // The checker identifies types by display name -- of the UNWRAPPED base
    // type: a Constrained shell (`u64 [0..=100]`, `u64 in Trapping`) renders
    // its constraints in brackets, which the bracket probe below would
    // misread as a slice element type (`[Entry]`). Slices render with a
    // bracketed element type; nat-like scalars render as their bare name.
    let type_name = program
        .type_reference_table
        .display_name(unwrap_constraint_shells(program, parameter.type_reference));
    if type_name.contains('[') {
        return Some(DecreasingValueKind::Slice);
    }
    if is_nat_like_type(type_name.as_str()) {
        return Some(DecreasingValueKind::Nat);
    }
    None
}

/// Strip `Constrained` shells to the base type (ranges and arithmetic
/// domains never change WHAT descends, only which values are legal).
fn unwrap_constraint_shells(
    program: &psi_typed_trees::TypedTrees,
    mut handle: psi_typed_trees::types::TypeReferenceHandle,
) -> psi_typed_trees::types::TypeReferenceHandle {
    use psi_typed_trees::types::TypeReferenceNode;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(handle)
    {
        handle = *base_type;
    }
    handle
}

/// A type whose values descend through the naturals: unsigned / bounded
/// integers. Signed integers are excluded because `value - 1` is not
/// well-founded without a positivity interpretation, so they are treated as
/// ambiguous and require the explicit `-> Order`.
fn is_nat_like_type(name: &str) -> bool {
    matches!(name, "u8" | "u16" | "u32" | "u64" | "nat")
}

fn state_parameter_of_expression<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    state: &'program psi_typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<&'program psi_typed_trees::signature::StateParameter> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let last = program
        .expression_table
        .name_path_members(path.members)
        .last()
        .map(|member| member.as_str());
    program.state_parameters(state).iter().find(|parameter| {
        !parameter.is_self
            && (parameter.symbol == path.symbol
                || last.is_some_and(|name| parameter.name.as_str() == name))
    })
}

enum MeasureBodyShape {
    /// `{ param }` — the body forwards the parameter directly.
    ParameterForward,
    /// `{ param.field }` — the body projects a single field.
    FieldProjection(psi_typed_trees::name::Identifier),
}

fn measure_body_shape(
    program: &psi_typed_trees::TypedTrees,
    measure: &MeasureDefinition,
) -> Option<MeasureBodyShape> {
    let body = program.expression_table.expression_handles(measure.body);
    if body.len() != 1 {
        return None;
    }
    match program.expression_table.expression(body[0]) {
        ExpressionNode::Name(_) => Some(MeasureBodyShape::ParameterForward),
        ExpressionNode::Member(member) => {
            Some(MeasureBodyShape::FieldProjection(member.member.clone()))
        }
        _ => None,
    }
}

fn lexicographic_component_fields(
    program: &psi_typed_trees::TypedTrees,
    measure: &MeasureDefinition,
) -> Option<Vec<psi_typed_trees::name::Identifier>> {
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

fn find_declared_measure<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    order: &[&str],
) -> Option<&'program MeasureDefinition> {
    program.measures().iter().find(|measure| {
        let actual = program.measure_path_members(measure.name);
        actual.len() == order.len()
            && actual
                .iter()
                .zip(order.iter())
                .all(|(actual, expected)| actual.as_str() == *expected)
    })
}

/// `usize` is retired (parse-rejected); `u64` is the natural-measure name,
/// synthesized for `.len` projections and subtraction measures alike, so the
/// comparison is direct.
fn natural_measure_names_match(left: &str, right: &str) -> bool {
    left == right
}

fn measure_return_is_natural(
    program: &psi_typed_trees::TypedTrees,
    measure: &MeasureDefinition,
) -> bool {
    program
        .type_reference_table
        .display_name(measure.return_type)
        == "u64"
}

fn measure_parameter_is_natural(
    program: &psi_typed_trees::TypedTrees,
    measure: &MeasureDefinition,
) -> bool {
    measure.parameter.as_ref().is_some_and(|parameter| {
        program
            .type_reference_table
            .display_name(parameter.type_reference)
            == "u64"
    })
}

fn expression_type_name(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<String> {
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
        ExpressionNode::Member(member) if member.member.as_str() == "len" => {
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
                psi_typed_trees::expression::BinaryOperator::Subtract
            ) =>
        {
            Some("u64".to_string())
        }
        _ => None,
    }
}

fn state_parameter_type_name(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    symbol: psi_symbols::SymbolHandle,
    fallback_name: Option<&str>,
) -> Option<String> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| {
            !parameter.is_self
                && (parameter.symbol == symbol
                    || fallback_name.is_some_and(|name| parameter.name.as_str() == name))
        })
        .map(|parameter| {
            program
                .type_reference_table
                .display_name(parameter.type_reference)
        })
}

fn path_matches(order: &[&str], expected: &[&str]) -> bool {
    order.len() == expected.len()
        && order
            .iter()
            .zip(expected.iter())
            .all(|(actual, expected)| actual == expected)
}
