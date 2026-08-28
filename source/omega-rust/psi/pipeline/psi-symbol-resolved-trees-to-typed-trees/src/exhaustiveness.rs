//! Exhaustiveness counting over implicit case-domains and case-subset domains
//! (chapter 1 "Cases Are Domains", frozen decision 7). Match/transition arms
//! over a case-bearing subject are CLASSIFICATIONS; the first satisfied arm
//! wins, and a dispatch with no satisfied arm falls through at runtime (probed
//! 2026-06-11: native exits 1, the interpreter completes with exit 0 -- a
//! divergence, which is why non-exhaustive dispatch is a compile error).
//!
//! Exhaustiveness counts DECIDABLE arms only:
//! - a case arm covers its one tag (`Command::Quit ->`, braced or bare --
//!   both desugar to membership at parse time);
//! - a PURE CASE-UNION domain arm covers its declared tag set. Recognition is
//!   SYNTACTIC (the chapter-1 footnote ruling): the domain's sole fact must be
//!   literally `self in Type::A | Type::B` -- an or-union of implicit case-domain
//!   memberships over bare `self`, all cases of the domain's own target type.
//!   Anything
//!   else (predicates, intersections, nested domains, extra body facts) is a
//!   predicate domain.
//! - `_` (an Always guard) satisfies coverage outright.
//! - conjunctions produced by tuple patterns form finite boxes over case-tag
//!   and boolean axes; exhaustiveness is the Cartesian union of those boxes,
//!   never the independent marginal coverage of each subject.
//!
//! Everything else -- predicate-domain arms, destructure arms with `if`
//! guards, and non-complementary value compares -- relies on facts the counter
//! cannot decide, so a run that needs them must close with `_`.
//!
//! This pass runs on the RESOLVED trees, like `crate::equality`, and for the
//! same reason: `in` is still a distinct `Membership` node here (typed
//! lowering expands case membership into tag compares and declared-domain
//! membership into fact expansions, after which case arms and
//! arbitrary boolean guards are indistinguishable). A transition block
//! desugars at parse time into consecutive `Transition` statements, so a
//! maximal run of consecutive transitions in a state body IS the dispatch:
//! runtime control tries each guard in order and falls through the whole run
//! when none match.

use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use resolved::SymbolResolvedTrees;
use resolved::data::{DataDefinition, DataMember};
use resolved::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use resolved::statement::{StatementNode, TransitionGuardNode};

pub(crate) fn validate_case_dispatch_exhaustiveness(
    program: &SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    for machine in &program.machines {
        for state_handle in program.machine_state_handles(machine.states) {
            let state = program.machine_state(*state_handle);
            let statements = program
                .tables
                .bodies
                .statements
                .statements(state.statement_nodes);
            let location = if machine.name.as_str().rsplit("::").next() == Some(state.name.as_str())
            {
                machine.name.as_str().to_owned()
            } else {
                format!("{}::{}", machine.name, state.name)
            };

            // Maximal runs of consecutive transitions: the statement-level
            // shape every transition block desugars into. A transition
            // carrying a CONTINUATION (the `if`/`else` desugar) has a
            // well-defined no-match path, so its run cannot fall through --
            // it closes the dispatch like a `_` arm.
            let mut run = Vec::new();
            let mut run_has_continuation = false;
            for statement in statements {
                match statement {
                    StatementNode::Transition(transition) => {
                        run.push(&transition.guard);
                        run_has_continuation |= transition.continuation.is_valid();
                    }
                    _ => {
                        check_dispatch_run(program, &run, run_has_continuation, &location)?;
                        run.clear();
                        run_has_continuation = false;
                    }
                }
            }
            check_dispatch_run(program, &run, run_has_continuation, &location)?;
        }
    }

    Ok(())
}

/// What one arm's guard tells the exhaustiveness counter.
enum ArmShape {
    /// `_` -- always satisfied, closes any dispatch.
    Default,
    /// A decidable conjunction of finite-domain constraints. A tuple pattern
    /// is one such conjunction; missing axes are wildcards. Keeping the whole
    /// arm intact is essential: independently pooling each subject's marginal
    /// coverage would incorrectly accept a diagonal of a Cartesian matrix.
    Finite(Vec<FiniteConstraint>),
    /// The arm touches a case-bearing subject but contains a predicate domain
    /// or another conjunct which prevents the whole arm from being counted.
    UncountedCase(CaseClaim),
    /// An equality compare (`x == k` / `x != k`). A complementary PAIR over
    /// one subject and one value (`x == k ->` plus `x != k ->`) is total and
    /// closes the dispatch.
    Compare {
        left: ExpressionHandle,
        right: ExpressionHandle,
        negated: bool,
    },
    /// No countable content (value compares, arbitrary predicates, domains
    /// over record types). Contributes nothing and decides nothing by itself.
    Opaque,
}

enum FiniteConstraint {
    Case(CaseClaim),
    Bool {
        subject: ExpressionHandle,
        value: bool,
    },
}

struct CaseClaim {
    /// Index into `program.data_definitions` of the subject's sum type.
    data_index: usize,
    /// The classified subject expression (each arm holds a parser copy).
    subject: ExpressionHandle,
    /// Variant indexes (declaration order) the arm decidably covers, or
    /// `None` when the arm touches the sum but cannot be counted (predicate
    /// domain or an otherwise opaque guarded pattern).
    covered: Option<Vec<usize>>,
}

/// What one membership LEAF (`subject in Path`) contributes.
enum LeafContribution {
    /// Case tags of one data definition.
    Tags {
        data_index: usize,
        variants: Vec<usize>,
    },
    /// A declared predicate domain over a case-bearing type: real
    /// classification, undecidable count.
    Predicate { data_index: usize },
    /// Not a case classification at all (domain over a record/scalar type,
    /// or an unresolved path -- later stages own those errors).
    NotCase,
}

fn check_dispatch_run(
    program: &SymbolResolvedTrees,
    run: &[&TransitionGuardNode],
    has_continuation: bool,
    location: &str,
) -> Result<(), Diagnostic> {
    if run.is_empty() {
        return Ok(());
    }
    if has_continuation {
        return Ok(());
    }

    let mut finite_arms: Vec<Vec<FiniteConstraint>> = Vec::new();
    let mut uncounted_claims: Vec<CaseClaim> = Vec::new();
    let mut compare_arms: Vec<(ExpressionHandle, ExpressionHandle, bool)> = Vec::new();
    let mut has_opaque_arm = false;

    for guard in run {
        match classify_arm(program, guard) {
            // A `_` arm catches every fallthrough: the dispatch is closed no
            // matter what the other arms rely on.
            ArmShape::Default => return Ok(()),
            ArmShape::Finite(constraints) => finite_arms.push(constraints),
            ArmShape::UncountedCase(claim) => uncounted_claims.push(claim),
            ArmShape::Compare {
                left,
                right,
                negated,
            } => compare_arms.push((left, right, negated)),
            ArmShape::Opaque => has_opaque_arm = true,
        }
    }

    // `x == k ->` plus `x != k ->` over one subject and value is total.
    let compare_pair_closes = compare_arms.iter().any(|(left, right, negated)| {
        !*negated
            && compare_arms
                .iter()
                .any(|(other_left, other_right, other_negated)| {
                    *other_negated
                        && expressions_structurally_equal(program, *left, *other_left)
                        && expressions_structurally_equal(program, *right, *other_right)
                })
    });
    if compare_pair_closes {
        return Ok(());
    }

    let axes = finite_axes(program, &finite_arms, &uncounted_claims);
    if !axes.is_empty() {
        match first_uncovered_assignment(program, &axes, &finite_arms) {
            MatrixCoverage::Complete => return Ok(()),
            MatrixCoverage::TooLarge => {
                return Err(Diagnostic::error(format!(
                    "transition pattern matrix in `{location}` is too large for the exhaustiveness proof; add a `_` arm"
                )));
            }
            MatrixCoverage::Uncovered(assignment) => {
                // Preserve the established boolean-only fall-through
                // diagnostic. Case matrices can name the missing Cartesian
                // cell; an incomplete boolean guard dispatch is clearer in
                // the language's general no-match terms.
                if axes
                    .iter()
                    .all(|axis| matches!(axis.kind, FiniteAxisKind::Bool))
                {
                    return Err(Diagnostic::error(format!(
                        "transition dispatch in `{location}` can fall through: no arm matches when every guard is false, and no `_` arm exists; add a `_ ->` arm (or complete the `true`/`false` pair)"
                    )));
                }
                if axes.len() == 1
                    && let FiniteAxisKind::Case { data_index } = axes[0].kind
                {
                    let data_definition = &program.data_definitions[data_index];
                    let mut covered = vec![false; case_count(program, data_definition)];
                    for constraints in &finite_arms {
                        for constraint in constraints {
                            if let FiniteConstraint::Case(claim) = constraint
                                && claim.data_index == data_index
                                && expressions_structurally_equal(
                                    program,
                                    axes[0].subject,
                                    claim.subject,
                                )
                                && let Some(variants) = &claim.covered
                            {
                                for variant in variants {
                                    covered[*variant] = true;
                                }
                            }
                        }
                    }
                    let type_name = data_definition.name.as_str();
                    if !uncounted_claims.is_empty() || has_opaque_arm {
                        return Err(Diagnostic::error(format!(
                            "match over `{type_name}` is not exhaustive: it relies on arms the compiler cannot count (predicate domains, guarded patterns, or value compares); add a `_` arm"
                        )));
                    }
                    let uncovered = uncovered_case_names(program, data_definition, &covered);
                    return Err(Diagnostic::error(format!(
                        "match over `{type_name}` does not cover {}; add an arm or `_`",
                        uncovered
                            .iter()
                            .map(|case| format!("`{type_name}::{case}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )));
                }

                if !uncounted_claims.is_empty() || has_opaque_arm {
                    return Err(Diagnostic::error(format!(
                        "transition pattern matrix in `{location}` is not exhaustive: it relies on arms the compiler cannot count (predicate domains, guarded patterns, or value compares); add a `_` arm"
                    )));
                }
                return Err(Diagnostic::error(format!(
                    "transition pattern matrix in `{location}` does not cover {}; add an arm or `_`",
                    format_assignment(program, &axes, &assignment)
                )));
            }
        }
    }

    // NO SILENT FALL-THROUGH (settled 2026-07-02): a dispatch with no `_`
    // arm must PROVABLY cover every case -- full case coverage or a
    // true/false pair over one boolean subject. Anything else could reach
    // runtime with no matching arm, and a no-match dispatch falls off the
    // machine with an undefined exit (probed: the process exits with a
    // leftover register value). That is a compile error, never a behavior.
    Err(Diagnostic::error(format!(
        "transition dispatch in `{location}` can fall through: no arm matches when every \
         guard is false, and no `_` arm exists; add a `_ ->` arm (or complete the \
         `true`/`false` pair)"
    )))
}

struct FiniteAxis {
    subject: ExpressionHandle,
    kind: FiniteAxisKind,
}

#[derive(Clone, Copy)]
enum FiniteAxisKind {
    Case { data_index: usize },
    Bool,
}

enum MatrixCoverage {
    Complete,
    Uncovered(Vec<usize>),
    TooLarge,
}

fn finite_axes(
    program: &SymbolResolvedTrees,
    arms: &[Vec<FiniteConstraint>],
    uncounted_claims: &[CaseClaim],
) -> Vec<FiniteAxis> {
    let mut axes = Vec::new();
    for constraint in arms.iter().flatten() {
        match constraint {
            FiniteConstraint::Case(claim) => push_axis(
                program,
                &mut axes,
                claim.subject,
                FiniteAxisKind::Case {
                    data_index: claim.data_index,
                },
            ),
            FiniteConstraint::Bool { subject, .. } => {
                push_axis(program, &mut axes, *subject, FiniteAxisKind::Bool)
            }
        }
    }
    for claim in uncounted_claims {
        push_axis(
            program,
            &mut axes,
            claim.subject,
            FiniteAxisKind::Case {
                data_index: claim.data_index,
            },
        );
    }
    axes
}

fn push_axis(
    program: &SymbolResolvedTrees,
    axes: &mut Vec<FiniteAxis>,
    subject: ExpressionHandle,
    kind: FiniteAxisKind,
) {
    if axes.iter().any(|axis| {
        axis_kinds_equal(axis.kind, kind)
            && expressions_structurally_equal(program, axis.subject, subject)
    }) {
        return;
    }
    axes.push(FiniteAxis { subject, kind });
}

fn axis_kinds_equal(left: FiniteAxisKind, right: FiniteAxisKind) -> bool {
    match (left, right) {
        (FiniteAxisKind::Bool, FiniteAxisKind::Bool) => true,
        (FiniteAxisKind::Case { data_index: left }, FiniteAxisKind::Case { data_index: right }) => {
            left == right
        }
        _ => false,
    }
}

fn first_uncovered_assignment(
    program: &SymbolResolvedTrees,
    axes: &[FiniteAxis],
    arms: &[Vec<FiniteConstraint>],
) -> MatrixCoverage {
    const MAX_MATRIX_CELLS: usize = 65_536;

    let arities: Vec<usize> = axes
        .iter()
        .map(|axis| match axis.kind {
            FiniteAxisKind::Case { data_index } => {
                case_count(program, &program.data_definitions[data_index])
            }
            FiniteAxisKind::Bool => 2,
        })
        .collect();
    let Some(cell_count) = arities
        .iter()
        .try_fold(1usize, |product, arity| product.checked_mul(*arity))
    else {
        return MatrixCoverage::TooLarge;
    };
    if cell_count > MAX_MATRIX_CELLS {
        return MatrixCoverage::TooLarge;
    }

    for ordinal in 0..cell_count {
        let mut remainder = ordinal;
        let mut assignment = Vec::with_capacity(axes.len());
        for arity in &arities {
            assignment.push(remainder % arity);
            remainder /= arity;
        }
        if !arms
            .iter()
            .any(|arm| arm_matches(program, axes, &assignment, arm))
        {
            return MatrixCoverage::Uncovered(assignment);
        }
    }
    MatrixCoverage::Complete
}

fn arm_matches(
    program: &SymbolResolvedTrees,
    axes: &[FiniteAxis],
    assignment: &[usize],
    constraints: &[FiniteConstraint],
) -> bool {
    constraints.iter().all(|constraint| {
        let (subject, kind) = match constraint {
            FiniteConstraint::Case(claim) => (
                claim.subject,
                FiniteAxisKind::Case {
                    data_index: claim.data_index,
                },
            ),
            FiniteConstraint::Bool { subject, .. } => (*subject, FiniteAxisKind::Bool),
        };
        let Some(axis_index) = axes.iter().position(|axis| {
            axis_kinds_equal(axis.kind, kind)
                && expressions_structurally_equal(program, axis.subject, subject)
        }) else {
            return false;
        };
        match constraint {
            FiniteConstraint::Case(claim) => claim
                .covered
                .as_ref()
                .is_some_and(|variants| variants.contains(&assignment[axis_index])),
            FiniteConstraint::Bool { value, .. } => assignment[axis_index] == usize::from(*value),
        }
    })
}

fn format_assignment(
    program: &SymbolResolvedTrees,
    axes: &[FiniteAxis],
    assignment: &[usize],
) -> String {
    let cells = axes
        .iter()
        .zip(assignment)
        .map(|(axis, value)| match axis.kind {
            FiniteAxisKind::Bool => format!("`{}`", value != &0),
            FiniteAxisKind::Case { data_index } => {
                let definition = &program.data_definitions[data_index];
                let case = program
                    .data_members(definition.members)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Variant(variant) => Some(variant.name.as_str()),
                        DataMember::Field(_) => None,
                    })
                    .nth(*value)
                    .unwrap_or("<unknown>");
                format!("`{}::{case}`", definition.name.as_str())
            }
        })
        .collect::<Vec<_>>();
    if cells.len() == 1 {
        cells[0].clone()
    } else {
        format!("({})", cells.join(", "))
    }
}

fn classify_arm(program: &SymbolResolvedTrees, guard: &TransitionGuardNode) -> ArmShape {
    let TransitionGuardNode::When(expression) = guard else {
        return ArmShape::Default;
    };

    let mut conjuncts = Vec::new();
    flatten_binary(program, *expression, BinaryOperator::And, &mut conjuncts);

    if let [conjunct] = conjuncts[..] {
        if let Some(claim) = classify_membership_union(program, conjunct) {
            return if claim.covered.is_some() {
                ArmShape::Finite(vec![FiniteConstraint::Case(claim)])
            } else {
                ArmShape::UncountedCase(claim)
            };
        }
        // A literal `true` guard is always satisfied (`transition { true ->
        // main() }`); it closes the dispatch like `_`. A literal `false`
        // never matches and contributes nothing.
        if let ExpressionNode::Boolean(value) =
            program.tables.bodies.expressions.expression(conjunct)
        {
            return if *value {
                ArmShape::Default
            } else {
                ArmShape::Opaque
            };
        }
        // `subject == true` / `subject == false` -- the desugar of boolean
        // `true ->` / `false ->` arms (and the user-written equivalent). A
        // complementary pair over one subject covers the whole boolean.
        if let ExpressionNode::Binary(binary) =
            program.tables.bodies.expressions.expression(conjunct)
        {
            if binary.operator == BinaryOperator::Equal {
                // `true == true` (the desugar of `transition true { true ->
                // .. }`) is constant and always satisfied: Default.
                if let (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) = (
                    program.tables.bodies.expressions.expression(binary.left),
                    program.tables.bodies.expressions.expression(binary.right),
                ) {
                    return if left == right {
                        ArmShape::Default
                    } else {
                        ArmShape::Opaque
                    };
                }
                if let ExpressionNode::Boolean(value) =
                    program.tables.bodies.expressions.expression(binary.right)
                {
                    return ArmShape::Finite(vec![FiniteConstraint::Bool {
                        subject: binary.left,
                        value: *value,
                    }]);
                }
                if let ExpressionNode::Boolean(value) =
                    program.tables.bodies.expressions.expression(binary.left)
                {
                    return ArmShape::Finite(vec![FiniteConstraint::Bool {
                        subject: binary.right,
                        value: *value,
                    }]);
                }
            }
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) {
                return ArmShape::Compare {
                    left: binary.left,
                    right: binary.right,
                    negated: binary.operator == BinaryOperator::NotEqual,
                };
            }
        }
        return ArmShape::Opaque;
    }

    // A conjunction is countable exactly when every non-constant conjunct is
    // a finite case-domain membership or a boolean-literal equality. This is
    // the resolved shape of tuple patterns, including mixed bool/case tuples.
    let mut constraints = Vec::new();
    let mut first_case_claim = None;
    let mut countable = true;
    for conjunct in conjuncts {
        if let Some(claim) = classify_membership_union(program, conjunct) {
            if first_case_claim.is_none() {
                first_case_claim = Some(CaseClaim {
                    data_index: claim.data_index,
                    subject: claim.subject,
                    covered: None,
                });
            }
            if claim.covered.is_none() {
                countable = false;
            }
            constraints.push(FiniteConstraint::Case(claim));
            continue;
        }
        match program.tables.bodies.expressions.expression(conjunct) {
            ExpressionNode::Boolean(true) => continue,
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
                if let ExpressionNode::Boolean(value) =
                    program.tables.bodies.expressions.expression(binary.right)
                {
                    constraints.push(FiniteConstraint::Bool {
                        subject: binary.left,
                        value: *value,
                    });
                    continue;
                }
                if let ExpressionNode::Boolean(value) =
                    program.tables.bodies.expressions.expression(binary.left)
                {
                    constraints.push(FiniteConstraint::Bool {
                        subject: binary.right,
                        value: *value,
                    });
                    continue;
                }
                countable = false;
            }
            _ => countable = false,
        }
    }
    if countable && !constraints.is_empty() {
        ArmShape::Finite(constraints)
    } else if let Some(claim) = first_case_claim {
        ArmShape::UncountedCase(claim)
    } else {
        ArmShape::Opaque
    }
}

/// Classify a guard that is exactly an or-union of membership tests over one
/// structurally-equal subject (`s in A`, `s in A | s in B`, ...). Returns
/// `None` when the expression has any other shape, or when no leaf touches a
/// case-bearing type.
fn classify_membership_union(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> Option<CaseClaim> {
    let mut leaves = Vec::new();
    flatten_binary(program, expression, BinaryOperator::Or, &mut leaves);

    let expressions = &program.tables.bodies.expressions;
    let mut subject = ExpressionHandle::invalid();
    let mut data_index: Option<usize> = None;
    let mut variants: Vec<usize> = Vec::new();
    let mut countable = true;

    for leaf in leaves {
        let ExpressionNode::Membership(membership) = expressions.expression(leaf) else {
            return None;
        };

        if subject.is_valid() {
            if !expressions_structurally_equal(program, subject, membership.value) {
                // Two different subjects under one or-union: any case content
                // is uncountable, but it is still a claim on the first sum.
                countable = false;
            }
        } else {
            subject = membership.value;
        }

        match classify_membership_leaf(program, membership) {
            LeafContribution::Tags {
                data_index: leaf_data,
                variants: leaf_variants,
            } => match data_index {
                None => {
                    data_index = Some(leaf_data);
                    variants.extend(leaf_variants);
                }
                Some(existing) if existing == leaf_data => variants.extend(leaf_variants),
                // Cases of two different sums in one union cannot be counted
                // against either.
                Some(_) => countable = false,
            },
            LeafContribution::Predicate {
                data_index: leaf_data,
            } => {
                data_index.get_or_insert(leaf_data);
                countable = false;
            }
            LeafContribution::NotCase => countable = false,
        }
    }

    let data_index = data_index?;
    Some(CaseClaim {
        data_index,
        subject,
        covered: countable.then_some(variants),
    })
}

fn classify_membership_leaf(
    program: &SymbolResolvedTrees,
    membership: &resolved::expression::TableMembershipExpression,
) -> LeafContribution {
    // A resolved domain symbol names a DECLARED domain; the bare `Type::Case`
    // path is the implicit case domain. Same precedence as the executable
    // membership lowering (`crate::expression::domain_membership`).
    if membership.domain_symbol.is_valid() {
        return classify_declared_domain(program, membership.domain_symbol);
    }

    let members = program
        .tables
        .bodies
        .expressions
        .name_path_members(membership.domain);
    let [type_name, case_name] = members else {
        return LeafContribution::NotCase;
    };
    match find_case(program, type_name.as_str(), case_name.as_str()) {
        Some((data_index, variant)) => LeafContribution::Tags {
            data_index,
            variants: vec![variant],
        },
        None => LeafContribution::NotCase,
    }
}

/// A declared domain contributes a tag set only when it is a PURE CASE-UNION
/// (syntactic recognition, chapter-1 footnote ruling): its sole fact is exactly
/// `self in Type::A | Type::B | ...` over cases of its own target type.
/// Anything else over a case-bearing target is a predicate domain.
fn classify_declared_domain(
    program: &SymbolResolvedTrees,
    domain_symbol: psi_symbols::SymbolHandle,
) -> LeafContribution {
    let Some(domain) = program
        .domain_definitions
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return LeafContribution::NotCase;
    };

    let resolved::types::TypeReference::Named { name, .. } = &domain.target_type else {
        return LeafContribution::NotCase;
    };
    let Some(data_index) = program
        .data_definitions
        .iter()
        .position(|definition| definition.name.as_str() == name.as_str())
    else {
        return LeafContribution::NotCase;
    };
    if case_count(program, &program.data_definitions[data_index]) == 0 {
        // A domain over a record or scalar type classifies no tags; record
        // matches are out of exhaustiveness-counting scope.
        return LeafContribution::NotCase;
    }

    let [resolved::domain::ProofFact::Expression(case_union)] = program.proof_facts(domain.facts)
    else {
        return LeafContribution::Predicate { data_index };
    };

    let mut leaves = Vec::new();
    flatten_binary(program, *case_union, BinaryOperator::Or, &mut leaves);

    let expressions = &program.tables.bodies.expressions;
    let mut variants = Vec::new();
    for leaf in leaves {
        let ExpressionNode::Membership(leaf_membership) = expressions.expression(leaf) else {
            return LeafContribution::Predicate { data_index };
        };
        if !is_bare_self(program, leaf_membership.value) {
            return LeafContribution::Predicate { data_index };
        }
        // Leaves must be implicit case domains of the SAME target type --
        // nested declared domains keep the first cut strictly syntactic.
        if leaf_membership.domain_symbol.is_valid() {
            return LeafContribution::Predicate { data_index };
        }
        let members = expressions.name_path_members(leaf_membership.domain);
        let [type_name, case_name] = members else {
            return LeafContribution::Predicate { data_index };
        };
        match find_case(program, type_name.as_str(), case_name.as_str()) {
            Some((leaf_data, variant)) if leaf_data == data_index => variants.push(variant),
            _ => return LeafContribution::Predicate { data_index },
        }
    }

    LeafContribution::Tags {
        data_index,
        variants,
    }
}

fn is_bare_self(program: &SymbolResolvedTrees, expression: ExpressionHandle) -> bool {
    matches!(
        program.tables.bodies.expressions.expression(expression),
        ExpressionNode::Name(path) if path.is_self_value && path.members.count() == 1
    )
}

/// Find `Type::Case` among the data definitions; returns the definition index
/// and the variant's index in declaration order (the tag).
fn find_case(
    program: &SymbolResolvedTrees,
    type_name: &str,
    case_name: &str,
) -> Option<(usize, usize)> {
    let data_index = program
        .data_definitions
        .iter()
        .position(|definition| definition.name.as_str() == type_name)?;
    let variant = program
        .data_members(program.data_definitions[data_index].members)
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .position(|variant| variant.name.as_str() == case_name)?;
    Some((data_index, variant))
}

fn case_count(program: &SymbolResolvedTrees, definition: &DataDefinition) -> usize {
    program
        .data_members(definition.members)
        .iter()
        .filter(|member| matches!(member, DataMember::Variant(_)))
        .count()
}

fn uncovered_case_names(
    program: &SymbolResolvedTrees,
    definition: &DataDefinition,
    covered: &[bool],
) -> Vec<String> {
    program
        .data_members(definition.members)
        .iter()
        .filter_map(|member| match member {
            DataMember::Variant(variant) => Some(variant.name.as_str().to_owned()),
            DataMember::Field(_) => None,
        })
        .zip(covered.iter())
        .filter_map(|(name, covered)| (!covered).then_some(name))
        .collect()
}

/// Flatten a left/right tree of one binary operator into its leaves.
fn flatten_binary(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    leaves: &mut Vec<ExpressionHandle>,
) {
    if let ExpressionNode::Binary(binary) = program.tables.bodies.expressions.expression(expression)
        && binary.operator == operator
    {
        let (left, right) = (binary.left, binary.right);
        flatten_binary(program, left, operator, leaves);
        flatten_binary(program, right, operator, leaves);
        return;
    }
    leaves.push(expression);
}

/// Structural equality over resolved expressions: each transition arm holds a
/// parser COPY of the block subject, so handle identity never matches and the
/// trees must be compared by shape.
fn expressions_structurally_equal(
    program: &SymbolResolvedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if left == right {
        return true;
    }
    if !left.is_valid() || !right.is_valid() {
        return false;
    }

    let expressions = &program.tables.bodies.expressions;
    match (expressions.expression(left), expressions.expression(right)) {
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
        (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            left.is_self_value == right.is_self_value
                && name_paths_equal(
                    expressions.name_path_members(left.members),
                    expressions.name_path_members(right.members),
                )
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member.as_str() == right.member.as_str()
                && expressions_structurally_equal(program, left.receiver, right.receiver)
        }
        (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
            expressions_structurally_equal(program, left.collection, right.collection)
                && expressions_structurally_equal(program, left.index, right.index)
        }
        (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
            left.operator == right.operator
                && expressions_structurally_equal(program, left.operand, right.operand)
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && expressions_structurally_equal(program, left.left, right.left)
                && expressions_structurally_equal(program, left.right, right.right)
        }
        (ExpressionNode::Membership(left), ExpressionNode::Membership(right)) => {
            left.domain_symbol == right.domain_symbol
                && name_paths_equal(
                    expressions.name_path_members(left.domain),
                    expressions.name_path_members(right.domain),
                )
                && expressions_structurally_equal(program, left.value, right.value)
        }
        (ExpressionNode::Borrow(left), ExpressionNode::Borrow(right)) => {
            left.access == right.access
                && expressions_structurally_equal(program, left.target, right.target)
        }
        (ExpressionNode::Cast(left), ExpressionNode::Cast(right)) => {
            expressions_structurally_equal(program, left.value, right.value)
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            // Structurally-equal call subjects share ONE evaluation slot at
            // runtime (the shared transition-guard slot), so they are one
            // classification subject here too.
            left.target.as_str() == right.target.as_str()
                && expressions_structurally_equal(program, left.receiver, right.receiver)
                && {
                    let left_arguments = expressions.expression_handles(left.arguments);
                    let right_arguments = expressions.expression_handles(right.arguments);
                    left_arguments.len() == right_arguments.len()
                        && left_arguments
                            .iter()
                            .zip(right_arguments.iter())
                            .all(|(left, right)| {
                                expressions_structurally_equal(program, *left, *right)
                            })
                }
        }
        _ => false,
    }
}

fn name_paths_equal(
    left: &[resolved::name::DiagnosticName],
    right: &[resolved::name::DiagnosticName],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right.iter())
            .all(|(left, right)| left.as_str() == right.as_str())
}
