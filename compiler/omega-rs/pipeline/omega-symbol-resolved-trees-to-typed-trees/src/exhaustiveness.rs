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
//!   SYNTACTIC (the chapter-1 footnote ruling): the domain's `when` classifier
//!   must be literally `self in Type::A | Type::B` -- an or-union of implicit
//!   case-domain memberships over bare `self`, all cases of the domain's own
//!   target type -- and the domain body must carry no further facts. Anything
//!   else (predicates, intersections, nested domains, extra body facts) is a
//!   predicate domain.
//! - `_` (an Always guard) satisfies coverage outright.
//!
//! Everything else -- predicate-domain arms, destructure arms with `if`
//! guards, value compares -- relies on facts the counter cannot decide, so a
//! run that needs them must close with `_`.
//!
//! This pass runs on the RESOLVED trees, like `crate::equality`, and for the
//! same reason: `in` is still a distinct `Membership` node here (typed
//! lowering expands case membership into tag compares and declared-domain
//! membership into classifier/fact expansions, after which case arms and
//! arbitrary boolean guards are indistinguishable). A transition block
//! desugars at parse time into consecutive `Transition` statements, so a
//! maximal run of consecutive transitions in a state body IS the dispatch:
//! runtime control tries each guard in order and falls through the whole run
//! when none match.

use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
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
            let location = format!("{}::{}", machine.name, state.name);

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
    /// A classification over a case-bearing subject.
    Claim(CaseClaim),
    /// A version match arm over a `Versioned<T>` subject (frozen decision
    /// 14): the parser desugars `Counter::v1(old)` / `Counter(current)` into
    /// a marker membership, so the era an arm covers is still recognizable
    /// here.
    Version(VersionClaim),
    /// A boolean compare against a literal (`subject == true` -- the desugar
    /// of a `true ->` arm, or user-written). A `true` arm and a `false` arm
    /// over ONE subject cover the whole boolean and close the dispatch.
    Bool {
        subject: ExpressionHandle,
        value: bool,
    },
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

struct CaseClaim {
    /// Index into `program.data_definitions` of the subject's sum type.
    data_index: usize,
    /// The classified subject expression (each arm holds a parser copy).
    subject: ExpressionHandle,
    /// Variant indexes (declaration order) the arm decidably covers, or
    /// `None` when the arm touches the sum but cannot be counted (predicate
    /// domain, `if`-guarded pattern, mixed-subject conjunction).
    covered: Option<Vec<usize>>,
}

struct VersionClaim {
    /// The payload data type the arm names (`Counter` of a
    /// `Counter::v1(old)` / `Counter(current)` arm).
    type_name: String,
    /// The matched subject expression (each arm holds a parser copy).
    subject: ExpressionHandle,
    /// The era index the arm covers (decision-10 numbering: `vN` = zero-based
    /// declaration index, current = the number of declared blocks), or `None`
    /// when the arm names an unknown era or a type without version blocks --
    /// the typed lowering owns those diagnostics, so the run is not counted.
    era: Option<usize>,
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

    let mut claims: Vec<CaseClaim> = Vec::new();
    let mut version_claims: Vec<VersionClaim> = Vec::new();
    let mut bool_arms: Vec<(ExpressionHandle, bool)> = Vec::new();
    let mut compare_arms: Vec<(ExpressionHandle, ExpressionHandle, bool)> = Vec::new();
    let mut has_opaque_arm = false;

    for guard in run {
        match classify_arm(program, guard) {
            // A `_` arm catches every fallthrough: the dispatch is closed no
            // matter what the other arms rely on.
            ArmShape::Default => return Ok(()),
            ArmShape::Claim(claim) => claims.push(claim),
            ArmShape::Version(claim) => version_claims.push(claim),
            ArmShape::Bool { subject, value } => bool_arms.push((subject, value)),
            ArmShape::Compare {
                left,
                right,
                negated,
            } => compare_arms.push((left, right, negated)),
            ArmShape::Opaque => has_opaque_arm = true,
        }
    }

    // A `true` arm AND a `false` arm over one subject cover the boolean.
    let bool_pair_closes = bool_arms.iter().any(|(subject, value)| {
        *value
            && bool_arms.iter().any(|(other, other_value)| {
                !*other_value && expressions_structurally_equal(program, *subject, *other)
            })
    });

    // `x == k ->` plus `x != k ->` over one subject and value is total.
    let compare_pair_closes = compare_arms.iter().any(|(left, right, negated)| {
        !*negated
            && compare_arms.iter().any(|(other_left, other_right, other_negated)| {
                *other_negated
                    && expressions_structurally_equal(program, *left, *other_left)
                    && expressions_structurally_equal(program, *right, *other_right)
            })
    });

    let version_closes =
        check_version_dispatch_coverage(program, &version_claims, has_opaque_arm)?;

    // Check coverage per claimed sum type (one dispatch normally classifies
    // one subject; tuple dispatches can touch several).
    let mut checked: Vec<usize> = Vec::new();
    for claim in &claims {
        if checked.contains(&claim.data_index) {
            continue;
        }
        checked.push(claim.data_index);

        let data_definition = &program.data_definitions[claim.data_index];
        let mut covered = vec![false; case_count(program, data_definition)];
        let mut has_uncounted_arm = false;

        for other in &claims {
            if other.data_index != claim.data_index {
                continue;
            }
            // All counted arms must classify the SAME subject; a second
            // subject of the same sum type cannot pool coverage with it.
            if !expressions_structurally_equal(program, claim.subject, other.subject) {
                has_uncounted_arm = true;
                continue;
            }
            match &other.covered {
                Some(variants) => {
                    for variant in variants {
                        covered[*variant] = true;
                    }
                }
                None => has_uncounted_arm = true,
            }
        }

        let uncovered = uncovered_case_names(program, data_definition, &covered);
        if uncovered.is_empty() {
            continue;
        }

        let type_name = data_definition.name.as_str();
        if has_uncounted_arm || has_opaque_arm {
            return Err(Diagnostic::error(format!(
                "match over `{type_name}` is not exhaustive: it relies on arms the compiler cannot count (predicate domains, guarded patterns, or value compares); add a `_` arm"
            )));
        }
        return Err(Diagnostic::error(format!(
            "match over `{type_name}` does not cover {}; add an arm or `_`",
            uncovered
                .iter()
                .map(|case| format!("`{type_name}::{case}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    // NO SILENT FALL-THROUGH (settled 2026-07-02): a dispatch with no `_`
    // arm must PROVABLY cover every case -- full case/version coverage or a
    // true/false pair over one boolean subject. Anything else could reach
    // runtime with no matching arm, and a no-match dispatch falls off the
    // machine with an undefined exit (probed: the process exits with a
    // leftover register value). That is a compile error, never a behavior.
    let case_closes = !claims.is_empty();
    if !case_closes
        && !version_closes
        && !bool_pair_closes
        && !compare_pair_closes
        && !has_continuation
    {
        return Err(Diagnostic::error(format!(
            "transition dispatch in `{location}` can fall through: no arm matches when every \
             guard is false, and no `_` arm exists; add a `_ ->` arm (or complete the \
             `true`/`false` pair)"
        )));
    }

    Ok(())
}

/// Exhaustiveness over VERSION match arms: the decidable arm set of a
/// `Versioned<T>` subject is {each declared era `vN`} + {current}, so a run
/// of version arms with no `_` must cover every era. Diagnostics the typed
/// lowering owns (unknown eras, mixed container types, plain subjects) make
/// the run uncountable here and stand the check down.
///
/// Returns whether the run's version arms CLOSE the dispatch: `true` when a
/// countable run covers every era, and also on every deferred path (the typed
/// lowering owns those errors -- the fall-through check must not pile on);
/// `false` only when there are no version arms at all.
fn check_version_dispatch_coverage(
    program: &SymbolResolvedTrees,
    claims: &[VersionClaim],
    has_opaque_arm: bool,
) -> Result<bool, Diagnostic> {
    if claims.is_empty() {
        return Ok(false);
    }

    // Any arm whose era cannot be resolved (unknown era name, type without
    // version blocks) defers to the typed lowering's own error.
    if claims.iter().any(|claim| claim.era.is_none()) {
        return Ok(true);
    }

    // Two version arms naming DIFFERENT types over one subject is a
    // wrong-container error the typed lowering names precisely; counting
    // either type's eras here would bury it under a coverage message.
    for (index, claim) in claims.iter().enumerate() {
        for other in &claims[index + 1..] {
            if claim.type_name != other.type_name
                && expressions_structurally_equal(program, claim.subject, other.subject)
            {
                return Ok(true);
            }
        }
    }

    let mut checked: Vec<&str> = Vec::new();
    for claim in claims {
        if checked.contains(&claim.type_name.as_str()) {
            continue;
        }
        checked.push(claim.type_name.as_str());

        // A subject with a DETERMINABLE non-container type is the typed
        // lowering's chapter-21 legality error, not a coverage gap.
        let container_name = omega_core::versioning::versioned_container_name(&claim.type_name);
        if let Some(subject_type) = crate::expression::version_membership::subject_declared_type_name(
            program,
            &program.tables.bodies.expressions,
            claim.subject,
        ) && subject_type != container_name
        {
            continue;
        }

        let declared_versions = declared_versions(program, &claim.type_name);
        // Eras: one per declared `version vN` block, plus the CURRENT shape.
        let mut covered = vec![false; declared_versions.len() + 1];
        let mut has_uncounted_arm = false;

        for other in claims {
            if other.type_name != claim.type_name {
                continue;
            }
            // All counted arms must classify the SAME subject; a second
            // subject of the same container type cannot pool coverage.
            if !expressions_structurally_equal(program, claim.subject, other.subject) {
                has_uncounted_arm = true;
                continue;
            }
            if let Some(era) = other.era
                && era < covered.len()
            {
                covered[era] = true;
            }
        }

        let uncovered: Vec<usize> = (0..covered.len())
            .filter(|era| !covered[*era])
            .collect();
        if uncovered.is_empty() {
            continue;
        }

        let type_name = claim.type_name.as_str();
        if has_uncounted_arm || has_opaque_arm {
            return Err(Diagnostic::error(format!(
                "match over `Versioned<{type_name}>` is not exhaustive: it relies on arms the compiler cannot count (predicate domains, guarded patterns, or value compares); add a `_` arm"
            )));
        }

        let described = uncovered
            .iter()
            .map(|era| match declared_versions.get(*era) {
                Some(version) => format!("era `{version}`"),
                None => "the current era".to_owned(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let suggested_arm = match declared_versions.get(uncovered[0]) {
            Some(version) => format!("{type_name}::{version}(value)"),
            None => format!("{type_name}(value)"),
        };
        return Err(Diagnostic::error(format!(
            "match over `Versioned<{type_name}>` does not cover {described}; add a `{suggested_arm}` arm or `_`"
        )));
    }

    Ok(true)
}

/// Classify a version arm's marker membership: which payload type it names
/// and which era it covers. Unknown eras and types without version blocks
/// yield `era: None` (uncountable -- the typed lowering owns the error).
fn classify_version_arm(
    program: &SymbolResolvedTrees,
    membership: &resolved::expression::TableMembershipExpression,
) -> VersionClaim {
    let members = program
        .tables
        .bodies
        .expressions
        .name_path_members(membership.domain);
    let (type_name, version_name) = match members {
        [type_name, _marker] => (type_name.as_str(), None),
        [type_name, version, _marker] => (type_name.as_str(), Some(version.as_str())),
        _ => {
            return VersionClaim {
                type_name: String::new(),
                subject: membership.value,
                era: None,
            };
        }
    };

    let declared_versions = declared_versions(program, type_name);
    // Decision-10 era assignment, mirroring the executable lowering
    // (`crate::expression::version_membership`): vN = zero-based declaration
    // index; the current shape = the number of declared blocks.
    let era = if declared_versions.is_empty() {
        None
    } else {
        match version_name {
            Some(version) => declared_versions
                .iter()
                .position(|declared| *declared == version),
            None => Some(declared_versions.len()),
        }
    };

    VersionClaim {
        type_name: type_name.to_owned(),
        subject: membership.value,
        era,
    }
}

/// The declared eras of `type_name`, in declaration order (the historical
/// version shapes are data definitions named `Type::vN`).
fn declared_versions<'program>(
    program: &'program SymbolResolvedTrees,
    type_name: &str,
) -> Vec<&'program str> {
    program
        .data_definitions
        .iter()
        .filter_map(|definition| {
            omega_core::versioning::split_version_shape_name(definition.name.as_str())
                .filter(|(data_name, _)| *data_name == type_name)
                .map(|(_, version)| version)
        })
        .collect()
}

fn classify_arm(program: &SymbolResolvedTrees, guard: &TransitionGuardNode) -> ArmShape {
    let TransitionGuardNode::When(expression) = guard else {
        return ArmShape::Default;
    };

    let mut conjuncts = Vec::new();
    flatten_binary(program, *expression, BinaryOperator::And, &mut conjuncts);

    if let [conjunct] = conjuncts[..] {
        // A version match arm is always the WHOLE pattern (the parser rejects
        // any other embedding), so the marker membership can only appear as
        // the lone conjunct.
        if let ExpressionNode::Membership(membership) =
            program.tables.bodies.expressions.expression(conjunct)
            && crate::expression::version_membership::is_version_arm_domain(
                &program.tables.bodies.expressions,
                membership.domain,
            )
        {
            return ArmShape::Version(classify_version_arm(program, membership));
        }
        if let Some(claim) = classify_membership_union(program, conjunct) {
            return ArmShape::Claim(claim);
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
                    return ArmShape::Bool {
                        subject: binary.left,
                        value: *value,
                    };
                }
                if let ExpressionNode::Boolean(value) =
                    program.tables.bodies.expressions.expression(binary.left)
                {
                    return ArmShape::Bool {
                        subject: binary.right,
                        value: *value,
                    };
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

    // A conjunction (destructure arm with an `if` guard, or a tuple arm
    // pairing a case test with other subjects): any case classification
    // inside it is real but not countable -- the extra conjunct narrows it.
    for conjunct in conjuncts {
        if let Some(claim) = classify_membership_union(program, conjunct) {
            return ArmShape::Claim(CaseClaim {
                covered: None,
                ..claim
            });
        }
    }
    ArmShape::Opaque
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
/// (syntactic recognition, chapter-1 footnote ruling): its `when` classifier
/// is exactly `self in Type::A | Type::B | ...` over cases of its own target
/// type, and its body declares no further facts. Anything else over a
/// case-bearing target is a predicate domain.
fn classify_declared_domain(
    program: &SymbolResolvedTrees,
    domain_symbol: omega_core::symbols::SymbolHandle,
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

    if !domain.classifier.is_valid() || !domain.facts.is_empty() {
        return LeafContribution::Predicate { data_index };
    }

    let mut leaves = Vec::new();
    flatten_binary(program, domain.classifier, BinaryOperator::Or, &mut leaves);

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
        (ExpressionNode::Mutable(left), ExpressionNode::Mutable(right)) => {
            expressions_structurally_equal(program, *left, *right)
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
