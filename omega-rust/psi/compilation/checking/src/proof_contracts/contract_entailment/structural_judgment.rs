use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Machine, StatementNode, SymbolHandle,
    TransitionGuardNode, TransitionTargetNode, is_arm_pattern_marker, structural_call_machine_name,
    term_contains,
};
use std::sync::Arc;
use symbols::JoinedPath;
use typed_trees::types::TypeReferenceHandle;

mod domain_premises;
mod integer_predicates;
mod licenses;
pub(super) mod normal_returns;
mod returned_fields;
pub(super) mod selected_contracts;
mod storage_effects;
mod value_correspondence;
mod value_predicates;

pub(crate) use licenses::{EntryMachines, LicenseCandidates, declared_index_algebra_laws};
use licenses::{
    RingLicense, SemiringLicense, additive_addends, fold_scalar_binary, polynomial_normal_form,
    sorted_multiset_subtract, term_uses_application,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum StructuralJudgment {
    Proven,
    Refuted,
    Unknown,
}

/// Shared operational eligibility for an authored value application. This
/// supplies no truth, precondition, or termination evidence: formation and
/// the ordinary prefix judgment retain those obligations independently.
pub(super) fn specification_application_candidate(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    target: SymbolHandle,
    selections: &[typed_trees::expression::StaticMachineArgument],
    frames: Option<&crate::CallFrameResolver<'_>>,
) -> Result<(), &'static str> {
    if !crate::machine_calls::contract_dependencies::independent_application(
        program, owner, target, selections,
    ) {
        return Err("the selected application depends on the active declaration");
    }
    let machine = super::structural_terms::selected_application_machine(program, target)
        .ok_or("the application has no exact selected entry")?;
    let state = program
        .machine_states(machine)
        .first()
        .ok_or("the selected machine has no entry state")?;
    let proof_only = crate::proof_only_classification(program);
    if crate::proof_contracts::quotients::type_has_forbidden_denotational_content(
        program,
        &proof_only,
        state.return_type,
    ) || program.state_parameters(state).iter().any(|parameter| {
        crate::proof_contracts::quotients::type_has_forbidden_denotational_content(
            program,
            &proof_only,
            parameter.type_reference,
        )
    }) {
        return Err("the application carries forbidden denotational content");
    }
    let plans = crate::machine_calls::calls::operand_call_plans(program, frames);
    crate::machine_calls::denotational_calls::checked_value_call_candidate(
        program, machine, state, &plans.0, &plans.1,
    )
}

/// Observation authority is established on resolved terms. Judgment consumes
/// those exact results; equation intake instead needs the original terms,
/// because installing a case equation can change subsequent resolution.
struct ObservedEqualityTerms {
    source: [StructuralTerm; 2],
    resolved: [StructuralTerm; 2],
}

#[derive(Clone, PartialEq)]
pub(super) struct CaseGuarantee {
    subject: StructuralTerm,
    data: SymbolHandle,
    case: SymbolHandle,
}

#[derive(Clone, PartialEq)]
struct MembershipGuarantee {
    subject: StructuralTerm,
    domain: SymbolHandle,
    semantic_domain: language_semantics::SemanticDomainId,
}

#[derive(Clone)]
pub(crate) enum StructuralTerm {
    Variable(String),
    /// A frame-authenticated observation of an input referent. Scalar reads
    /// retain their declared content authority. A collection additionally keeps
    /// its completed descriptor: its metadata never identifies referent contents.
    ObservedValue {
        observed_type: TypeReferenceHandle,
        value: Box<StructuralTerm>,
        descriptor: Option<Box<StructuralTerm>>,
    },
    /// A completed call's exact local result. Its contract may classify this
    /// value, but neither its spelling nor its callee's body defines it here.
    BoundValue(SymbolHandle),
    /// One completed value observation on this normal-return proof path.
    /// Calls retain the exact public callable and result type; other selected
    /// observations have no callable. Equal operands never identify two events.
    CapturedValue {
        expression: ExpressionHandle,
        callable: SymbolHandle,
        occurrence: usize,
        result_type: TypeReferenceHandle,
    },
    /// A field path on the same completed value, never a callee-local spelling.
    BoundProjection {
        subject: SymbolHandle,
        path: String,
    },
    /// Exact literal value, independent of radix or authored spelling.
    Integer(numerics::bignum::BigInt),
    /// Exact mathematical observation of one already evaluated integer or
    /// address. Its declared carrier supplies bounds, never a new value or
    /// permission to reread the source storage.
    IntegerEmbedding {
        reference: TypeReferenceHandle,
        value: Box<StructuralTerm>,
    },
    /// Builtin unbounded integer meaning, distinct from every finite runtime
    /// carrier and its arithmetic policy. Children retain captured identities.
    MathematicalBinary {
        operator: BinaryOperator,
        left: Box<StructuralTerm>,
        right: Box<StructuralTerm>,
    },
    /// A determinate float literal, carried as its exact decimal spelling.
    /// UndeterminedNaN never termifies, so this term is never NaN from
    /// source text; synthesized spellings are still NaN-checked where the
    /// value is read.
    Float(numerics::literals::FloatLiteral),
    /// Data name, case name, and complete common/payload fields (sorted by
    /// field name; empty for a fieldless value like `Nat::Zero`). Payload-carrying
    /// terms spell as parenthesized case literals in fact position
    /// (`(Nat::Succ { prev: a })` -- the parens re-enable struct literals in
    /// the contract grammar), and both lowering fences stand down for
    /// recursive data so the raw Binary reaches this judge.
    Constructor {
        data: String,
        case: String,
        fields: Vec<(String, StructuralTerm)>,
    },
    /// A FREE call whose arguments all term-ify (`add(Nat::Zero, b)`). Static
    /// machine selections and target symbols are retained independently of
    /// diagnostic display, and generic unfolding substitutes their identities.
    /// Resolution UNFOLDS it when the callee is a single-state proof
    /// machine of the case-arm shape and the matched argument resolves to
    /// a constructor -- the compute-mode of N3's operator routing.
    Application {
        /// Effective caller-side result carrier and policy. Zero means that
        /// this term lacks typed authority for selected contract expansion.
        result_type: TypeReferenceHandle,
        target: SymbolHandle,
        selections: Vec<typed_trees::expression::StaticMachineArgument>,
        machine: String,
        arguments: Vec<StructuralTerm>,
    },
    /// One validated direct plain-record projection from an exact checked
    /// call. Symbol identities prevent overloaded textual names from
    /// collapsing; `machine` retains the complete static application for
    /// diagnostics and canonical display.
    CallProjection {
        selections: Vec<typed_trees::expression::StaticMachineArgument>,
        target: SymbolHandle,
        machine: String,
        result_type: TypeReferenceHandle,
        field: SymbolHandle,
        field_name: String,
        arguments: Vec<StructuralTerm>,
    },
    /// A substituted receiver whose field cannot yet be reduced. Retaining
    /// the receiver prevents a selected application from becoming display text.
    Projection {
        subject: Box<StructuralTerm>,
        path: String,
    },
    /// Authenticated collection value metadata, independent of referent
    /// contents. The exact descriptor remains the subject; observing this
    /// measure never makes that descriptor a readable referent snapshot.
    CollectionMeasure {
        measure: language_semantics::declaration_selection::CollectionMeasure,
        collection: Box<StructuralTerm>,
    },
    /// A builtin indexed observation of the same retained collection value and
    /// evaluated selector. The canonical element carrier is part of identity;
    /// the declaration handle retains observation authority after substitution.
    IndexedValue {
        element_type: TypeReferenceHandle,
        element_identity: typed_trees::type_identity::TypeIdentityId,
        collection: Box<StructuralTerm>,
        index: Box<StructuralTerm>,
    },
    /// A selected builtin operation, with substituted operands and original
    /// carrier/policy meaning. No scalar arithmetic is evaluated by this term.
    ScalarBinary {
        /// Raw declarations authorize observation after each substitution;
        /// selected meaning and value operands own scalar congruence.
        references: [TypeReferenceHandle; 2],
        operator: BinaryOperator,
        meaning: [(
            typed_trees::types::PrimitiveType,
            numerics::arithmetic::ArithmeticDomain,
        ); 2],
        left: Box<StructuralTerm>,
        right: Box<StructuralTerm>,
    },
    /// Anything else, compared by canonical display name only.
    Opaque(String),
}

#[path = "structural_judgment/rewrite_table.rs"]
mod rewrite_table;
#[path = "structural_judgment/term_identity.rs"]
mod term_identity;

impl StructuralTerm {
    pub(super) fn is_mathematical_predicate(&self) -> bool {
        if let Self::MathematicalBinary {
            operator: BinaryOperator::And,
            left,
            right,
        } = self.semantic_value()
        {
            return left.is_mathematical_predicate() && right.is_mathematical_predicate();
        }
        matches!(self.semantic_value(), Self::MathematicalBinary { operator, .. }
            if matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual
                | BinaryOperator::Less | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater | BinaryOperator::GreaterOrEqual))
    }

    fn is_closed_value(&self) -> bool {
        match self {
            Self::Integer(_) => true,
            Self::Constructor { fields, .. } => {
                fields.iter().all(|(_, value)| value.is_closed_value())
            }
            _ => false,
        }
    }

    pub(super) fn semantic_value(&self) -> &Self {
        match self {
            Self::ObservedValue { value, .. } => value.semantic_value(),
            _ => self,
        }
    }

    pub(super) fn boolean_value(&self) -> Option<bool> {
        let Self::Constructor { data, case, fields } = self.semantic_value() else {
            return None;
        };
        if data != "bool" || !fields.is_empty() {
            return None;
        }
        language_semantics::const_value::boolean_literal_spelling(case)
    }

    pub(super) fn into_semantic_value(self) -> Self {
        match self {
            Self::ObservedValue { value, .. } => (*value).into_semantic_value(),
            other => other,
        }
    }

    /// Metadata observes the completed descriptor, while ordinary observation
    /// identity continues to name the separately authenticated referent value.
    pub(super) fn into_collection_descriptor(self) -> Self {
        match self {
            Self::ObservedValue {
                descriptor: Some(descriptor),
                ..
            } => descriptor.into_collection_descriptor(),
            Self::ObservedValue { value, .. } => value.into_collection_descriptor(),
            value => value,
        }
    }

    /// Rebuild `self` with `map` applied over every argument — the
    /// variant-preserving argument rewrite the term walks share for
    /// `Application` and `CallProjection`.
    pub(super) fn map_argument_terms(self, mut map: impl FnMut(Self) -> Self) -> Self {
        match self {
            StructuralTerm::Application {
                result_type,
                target,
                selections,
                machine,
                arguments,
            } => StructuralTerm::Application {
                result_type,
                target,
                selections,
                machine,
                arguments: arguments.into_iter().map(&mut map).collect(),
            },
            StructuralTerm::CallProjection {
                selections,
                target,
                machine,
                result_type,
                field,
                field_name,
                arguments,
            } => StructuralTerm::CallProjection {
                selections,
                target,
                machine,
                result_type,
                field,
                field_name,
                arguments: arguments.into_iter().map(&mut map).collect(),
            },
            term => term,
        }
    }
}

/// Static observations supplied to one expression walk. Selected computation
/// carries its actual machine bindings; only a body-proof entry supplies the
/// declaring telescope's symbolic value subjects. Recursive child walks retain
/// that exact context, while unfolding another application starts a new one.
#[derive(Clone, Copy)]
struct StaticObservations<'scope> {
    machine_bindings: &'scope [(
        SymbolHandle,
        String,
        typed_trees::expression::StaticMachineArgument,
    )],
    body_parameters: &'scope [typed_trees::data::TypeParameter],
}

pub(super) struct StructuralJudge<'program> {
    program: &'program dyn crate::CheckingInput,
    entry_machines: std::sync::Arc<EntryMachines>,
    machine_symbol: SymbolHandle,
    // Runtime prefix facts belong to the current state invocation. Keep its
    // declaration frame separate from the machine's dependency identity.
    declaration_state: SymbolHandle,
    // Abstract owners supply their actual telescope, without a synthetic machine.
    declaration_parameters: Arc<Vec<typed_trees::signature::StateParameter>>,
    // This body's declared telescope supplies symbolic value observations,
    // never a missing selected callee argument or an assumed constant value.
    body_type_parameters: &'program [typed_trees::data::TypeParameter],
    premise_facts: Vec<ExpressionHandle>,
    // Term-bearing fact vectors are Arc-shared so the per-leaf judge clone
    // (one per recognized case-arm path) costs refcount bumps instead of
    // deep StructuralTerm copies; writers go through Arc::make_mut and pay
    // one copy only for the field they actually change.
    normal_results: Arc<Vec<(ExpressionHandle, StructuralTerm)>>,
    // Declaration-level field correspondence never recursively re-enters an
    // active body. This belongs to the query, including all cloned path judges.
    returned_field_owners: Arc<Vec<SymbolHandle>>,
    normal_memberships: Arc<Vec<MembershipGuarantee>>,
    pub(super) normal_content_guarantees:
        Arc<Vec<super::content_conservation::ObservedContentEquation>>,
    invalidated_storage: Arc<Vec<SymbolHandle>>,
    // Precise exclusive exposures of reference-free storage. An opaque record
    // retires the whole binding; a field path retires only overlapping reads.
    storage_exposures: Arc<storage_effects::StorageExposureRecord>,
    // Typed storage observations retain live values and detached entry snapshots.
    stored_values: Arc<Vec<storage_effects::CapturedStorageValue>>,
    pub(super) capture_sequence: usize,
    storage_write_sequence: usize,
    pub(super) normal_guarantees: Arc<Vec<(StructuralTerm, StructuralTerm)>>,
    // Producer identity comes only from the captured public result contract.
    // Selected branch equalities remain ordinary normal-path proof facts.
    captured_result_equations: Arc<Vec<(StructuralTerm, StructuralTerm)>>,
    resolve_applications: bool,
    // Closed computation owes completion of every eager operand. Symbolic
    // law normalization can retain applications without asserting completion.
    require_closed_computation: bool,
    runtime_body_values: bool,
    // Contract terms retain live operand observations, but denote admitted pure
    // calls rather than creating new executed result occurrences. Guard
    // refinement shares this term policy after its evaluations have returned;
    // actual body values still belong to the completed-occurrence owner.
    specification_terms: bool,
    case_facts_only: bool,
    pub(super) substitutions: Arc<Vec<(String, StructuralTerm)>>,
    /// Application REWRITES (`add_zero_right(prev) -> prev`): hypothesis
    /// equations with an application side orient REDUCING -- the inductive
    /// hypothesis rewrites the self-application away instead of expanding a
    /// variable into it, which also serves asymmetric goals.
    rewrites: Arc<rewrite_table::RewriteTable>,
    // Inequality facts constrain complete captured values, not case tags.
    disequalities: Arc<Vec<(StructuralTerm, StructuralTerm)>>,
    // Tag predicates are not value equations: even a payload-free case may
    // have common fields. Retain exact classifier identities while ordinary
    // argument substitution acts only on the observed subject.
    case_facts: Arc<Vec<(StructuralTerm, SymbolHandle, SymbolHandle)>>,
    // Keep raw, application-free equations separate from the legacy value
    // normalizer, which can unfold a name-selected call before storing it.
    // Copying its substitutions would already have lost callable identity.
    case_substitutions: Arc<Vec<(String, StructuralTerm)>>,
    pub(super) hypotheses_contradictory: bool,
    ring_licenses: Vec<RingLicense>,
    /// Tier-2: paired add/mul licenses with a conformed distributivity law.
    semiring_licenses: Vec<SemiringLicense>,
    /// Frame resolution is program-pure for every judge on one program: the
    /// shared cell builds it once per judge tree instead of once per probe
    /// site (per-leaf clones all share the same cell).
    frame_resolver:
        Arc<std::sync::OnceLock<Option<crate::machine_calls::calls::CallFrameResolver<'program>>>>,
}

impl Clone for StructuralJudge<'_> {
    fn clone(&self) -> Self {
        Self {
            program: self.program,
            entry_machines: std::sync::Arc::clone(&self.entry_machines),
            machine_symbol: self.machine_symbol,
            declaration_state: self.declaration_state,
            declaration_parameters: Arc::clone(&self.declaration_parameters),
            body_type_parameters: self.body_type_parameters,
            premise_facts: self.premise_facts.clone(),
            normal_results: Arc::clone(&self.normal_results),
            returned_field_owners: Arc::clone(&self.returned_field_owners),
            normal_memberships: Arc::clone(&self.normal_memberships),
            normal_content_guarantees: Arc::clone(&self.normal_content_guarantees),
            invalidated_storage: Arc::clone(&self.invalidated_storage),
            storage_exposures: Arc::clone(&self.storage_exposures),
            stored_values: Arc::clone(&self.stored_values),
            capture_sequence: self.capture_sequence,
            storage_write_sequence: self.storage_write_sequence,
            normal_guarantees: Arc::clone(&self.normal_guarantees),
            captured_result_equations: Arc::clone(&self.captured_result_equations),
            resolve_applications: self.resolve_applications,
            require_closed_computation: self.require_closed_computation,
            runtime_body_values: self.runtime_body_values,
            specification_terms: self.specification_terms,
            case_facts_only: self.case_facts_only,
            substitutions: Arc::clone(&self.substitutions),
            rewrites: Arc::clone(&self.rewrites),
            disequalities: Arc::clone(&self.disequalities),
            case_facts: Arc::clone(&self.case_facts),
            case_substitutions: Arc::clone(&self.case_substitutions),
            hypotheses_contradictory: self.hypotheses_contradictory,
            ring_licenses: self.ring_licenses.clone(),
            semiring_licenses: self.semiring_licenses.clone(),
            frame_resolver: Arc::clone(&self.frame_resolver),
        }
    }
}

impl<'program> StructuralJudge<'program> {
    pub(super) fn from_case_requires(
        program: &'program dyn crate::CheckingInput,
        judged_machine: &Machine,
        requires: &[ExpressionHandle],
    ) -> Self {
        Self::from_requires(program, judged_machine, requires, false)
    }

    pub(super) fn has_exact_case_subjects(&self) -> bool {
        !self.resolve_applications
    }

    /// True when `self` has performed no storage writes and added no call
    /// guarantees, memberships, content equations, or case facts since `base`.
    pub(super) fn has_no_new_effects_or_guarantees_since(&self, base: &Self) -> bool {
        !self.hypotheses_contradictory
            && self.hypotheses_contradictory == base.hypotheses_contradictory
            && self.storage_write_sequence == base.storage_write_sequence
            && self.invalidated_storage.len() == base.invalidated_storage.len()
            && self.storage_exposures == base.storage_exposures
            && self.captured_result_equations.len() == base.captured_result_equations.len()
            && self.normal_memberships.len() == base.normal_memberships.len()
            && self.normal_content_guarantees.len() == base.normal_content_guarantees.len()
            && self.case_facts.len() == base.case_facts.len()
    }

    /// True when `self` has added no non-boolean rewrites, non-boolean
    /// substitutions, or integer ordering/equality guarantees since `base`.
    pub(super) fn has_no_new_non_boolean_facts_since(&self, base: &Self) -> bool {
        let is_bool = |term: &StructuralTerm| term.boolean_value().is_some();
        self.substitutions
            .get(base.substitutions.len()..)
            .is_some_and(|added| added.iter().all(|(_, term)| is_bool(term)))
            && self
                .rewrites
                .entries()
                .get(base.rewrites.entries().len()..)
                .is_some_and(|added| added.iter().all(|(_, rhs)| is_bool(rhs)))
            && self
                .normal_guarantees
                .get(base.normal_guarantees.len()..)
                .is_some_and(|added| {
                    added.iter().all(|(left, right)| {
                        is_bool(left)
                            || (is_bool(right)
                                && !matches!(
                                    (left.semantic_value(), right.boolean_value()),
                                    (
                                        StructuralTerm::MathematicalBinary {
                                            operator:
                                                typed_trees::expression::BinaryOperator::Equal
                                                | typed_trees::expression::BinaryOperator::Less
                                                | typed_trees::expression::BinaryOperator::LessOrEqual
                                                | typed_trees::expression::BinaryOperator::Greater
                                                | typed_trees::expression::BinaryOperator::GreaterOrEqual,
                                            ..
                                        },
                                        Some(true)
                                    ) | (
                                        StructuralTerm::MathematicalBinary {
                                            operator:
                                                typed_trees::expression::BinaryOperator::NotEqual
                                                | typed_trees::expression::BinaryOperator::Less
                                                | typed_trees::expression::BinaryOperator::LessOrEqual
                                                | typed_trees::expression::BinaryOperator::Greater
                                                | typed_trees::expression::BinaryOperator::GreaterOrEqual,
                                            ..
                                        },
                                        Some(false)
                                    ) | (
                                        StructuralTerm::ScalarBinary {
                                            operator:
                                                typed_trees::expression::BinaryOperator::Equal
                                                | typed_trees::expression::BinaryOperator::Less
                                                | typed_trees::expression::BinaryOperator::LessOrEqual
                                                | typed_trees::expression::BinaryOperator::Greater
                                                | typed_trees::expression::BinaryOperator::GreaterOrEqual,
                                            ..
                                        },
                                        Some(true)
                                    ) | (
                                        StructuralTerm::ScalarBinary {
                                            operator:
                                                typed_trees::expression::BinaryOperator::NotEqual
                                                | typed_trees::expression::BinaryOperator::Less
                                                | typed_trees::expression::BinaryOperator::LessOrEqual
                                                | typed_trees::expression::BinaryOperator::Greater
                                                | typed_trees::expression::BinaryOperator::GreaterOrEqual,
                                            ..
                                        },
                                        Some(false)
                                    )
                                ))
                    })
                })
    }

    /// Rejoin branch outcomes that performed no storage writes and added no
    /// call guarantees since `base`, preserving `base`'s branch-neutral
    /// hypotheses while intersecting `stored_values` and `normal_results`.
    pub(super) fn rejoin_pure_outcomes(
        &self,
        outcomes: &[&Self],
        exclude_result: Option<ExpressionHandle>,
    ) -> Self {
        let mut rejoined = self.clone();
        let Some(&first) = outcomes.first() else {
            return rejoined;
        };
        rejoined.capture_sequence = outcomes
            .iter()
            .map(|judge| judge.capture_sequence)
            .max()
            .unwrap_or(self.capture_sequence);
        if outcomes[1..].iter().all(|judge| {
            Arc::ptr_eq(&first.stored_values, &judge.stored_values)
                || first.stored_values == judge.stored_values
        }) {
            rejoined.stored_values = Arc::clone(&first.stored_values);
        } else {
            let mut intersected = Vec::with_capacity(first.stored_values.len());
            for entry in first.stored_values.iter() {
                if outcomes[1..]
                    .iter()
                    .all(|judge| judge.stored_values.contains(entry))
                {
                    intersected.push(entry.clone());
                    continue;
                }
                let mut matched_read = entry.completed_read;
                let mut matched_current = entry.current.clone();
                let mut matched_entry = entry.entry.clone();
                let mut all_have_coord = true;
                for judge in &outcomes[1..] {
                    let Some(other) = judge
                        .stored_values
                        .iter()
                        .find(|s| s.coordinate == entry.coordinate)
                    else {
                        all_have_coord = false;
                        break;
                    };
                    if matched_read != other.completed_read {
                        matched_read = None;
                    }
                    if matched_entry != other.entry {
                        matched_entry = None;
                    }
                    if matched_current != other.current {
                        if let (
                            Some(StructuralTerm::CapturedValue {
                                expression: e1,
                                callable: c1,
                                result_type: t1,
                                ..
                            }),
                            Some(StructuralTerm::CapturedValue {
                                expression: e2,
                                callable: c2,
                                result_type: t2,
                                ..
                            }),
                        ) = (matched_current.as_ref(), other.current.as_ref())
                            && e1 == e2
                            && c1 == c2
                            && t1 == t2
                        {
                            // Keep matched_current (same expression/callable/type).
                        } else {
                            matched_current = None;
                        }
                    }
                }
                if all_have_coord && (matched_current.is_some() || matched_entry.is_some()) {
                    if matched_current.is_none() {
                        matched_read = None;
                    }
                    intersected.push(storage_effects::CapturedStorageValue {
                        coordinate: entry.coordinate.clone(),
                        entry: matched_entry,
                        current: matched_current,
                        completed_read: matched_read,
                    });
                }
            }
            rejoined.stored_values = Arc::new(intersected);
        }
        if exclude_result.is_none()
            && outcomes[1..].iter().all(|judge| {
                Arc::ptr_eq(&first.normal_results, &judge.normal_results)
                    || first.normal_results == judge.normal_results
            })
        {
            rejoined.normal_results = Arc::clone(&first.normal_results);
        } else {
            let mut intersected = Vec::with_capacity(first.normal_results.len());
            for (expr, val) in first.normal_results.iter() {
                if Some(*expr) == exclude_result {
                    continue;
                }
                if outcomes[1..].iter().all(|judge| {
                    judge
                        .normal_results
                        .iter()
                        .any(|(other_expr, other_val)| other_expr == expr && other_val == val)
                }) {
                    intersected.push((*expr, val.clone()));
                } else if let StructuralTerm::CapturedValue {
                    expression: e1,
                    callable: c1,
                    result_type: t1,
                    ..
                } = val
                    && outcomes[1..].iter().all(|judge| {
                        judge.normal_results.iter().any(|(other_expr, other_val)| {
                            other_expr == expr
                                && matches!(
                                    other_val,
                                    StructuralTerm::CapturedValue {
                                        expression: e2,
                                        callable: c2,
                                        result_type: t2,
                                        ..
                                    } if e1 == e2 && c1 == c2 && t1 == t2
                                )
                        })
                    })
                {
                    intersected.push((*expr, val.clone()));
                }
            }
            rejoined.normal_results = Arc::new(intersected);
        }
        rejoined
    }

    /// Executed calls are values from individual invocations, not applications
    /// of a mathematical function. Contracts retain their denotational terms;
    /// body substitution must not identify two independent observations.
    pub(super) fn from_runtime_requires(
        program: &'program dyn crate::CheckingInput,
        judged_machine: &Machine,
        requires: &[ExpressionHandle],
    ) -> Self {
        // Install premises under the runtime value resolver. Installing them
        // in tag-only mode first can resolve the new equation through its own
        // case substitution and discard its scalar value consequence.
        let mut judge = Self::from_case_requires(program, judged_machine, &[]);
        judge.runtime_body_values = true;
        judge.body_type_parameters = program.machine_type_parameters(judged_machine);
        judge.premise_facts.extend_from_slice(requires);
        for fact in requires {
            judge.intake(program, *fact);
        }
        // Capture entry comparison truth before any prefix effect. A later
        // write retires current storage observations, while detached input
        // values and their predicates keep the original invocation identity.
        for fact in requires {
            judge.intake_preserved_predicate(*fact);
        }
        judge
    }

    /// Every arrival proves this state's declared entry contract separately.
    /// Its prefix may consume those facts without unrolling the state cycle or
    /// importing the machine entry's unrelated invocation assumptions.
    pub(super) fn from_runtime_state_requires(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &typed_trees::state::State,
        requires: &[ExpressionHandle],
    ) -> Self {
        let mut judge = Self::from_runtime_requires(program, machine, &[]);
        judge.declaration_state = state.symbol;
        judge.retain_entry_owned_values(state);
        judge.premise_facts.extend_from_slice(requires);
        for fact in requires {
            judge.intake(program, *fact);
        }
        judge.intake_entry_domain_predicates(machine, state);
        // Capture entry comparison truth before any prefix effect. A later
        // write retires current storage observations, while detached input
        // values and their predicates keep the original invocation identity.
        for fact in requires {
            judge.intake_preserved_predicate(*fact);
        }
        judge
    }

    pub(super) fn premise_facts(&self) -> &[ExpressionHandle] {
        &self.premise_facts
    }

    pub(super) fn has_runtime_body_values(&self) -> bool {
        self.runtime_body_values
    }

    pub(super) fn requires_value_preservation(&self) -> bool {
        self.runtime_body_values && !self.case_facts_only
    }

    /// Frame writes are program-pure; every probe on a judge tree resolves
    /// the same tables, so the resolver is built once behind the shared cell.
    pub(super) fn frame_resolver(
        &self,
    ) -> Option<&crate::machine_calls::calls::CallFrameResolver<'program>> {
        self.frame_resolver
            .get_or_init(|| crate::machine_calls::calls::CallFrameResolver::new(self.program))
            .as_ref()
    }

    /// Tag preservation does not preserve scalar premises or payload equations.
    pub(super) fn restrict_to_case_facts(&mut self) {
        self.case_facts_only = true;
        self.substitutions = Arc::new(Vec::new());
        self.rewrites = Arc::new(rewrite_table::RewriteTable::default());
        self.disequalities = Arc::new(Vec::new());
        self.case_substitutions = Arc::new(Vec::new());
        self.hypotheses_contradictory = false;
    }

    pub(crate) fn from_requires(
        program: &'program dyn crate::CheckingInput,
        judged_machine: &Machine,
        requires: &[ExpressionHandle],
        resolve_applications: bool,
    ) -> Self {
        Self::from_declaration_requires(
            program,
            judged_machine.symbol,
            program
                .machine_states(judged_machine)
                .first()
                .map(|entry| program.state_parameters(entry))
                .unwrap_or_default(),
            requires,
            resolve_applications,
        )
    }

    pub(super) fn from_declaration_requires(
        program: &'program dyn crate::CheckingInput,
        owner: SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        requires: &[ExpressionHandle],
        resolve_applications: bool,
    ) -> Self {
        let licenses = crate::frozen_program::frozen_memo(
            program,
            |memos| &memos.license_candidates,
            || LicenseCandidates::of(program),
        );
        let mut judge = Self {
            program,
            entry_machines: crate::frozen_program::frozen_memo(
                program,
                |memos| &memos.entry_machines,
                || EntryMachines::of(program),
            ),
            machine_symbol: owner,
            declaration_state: SymbolHandle::invalid(),
            declaration_parameters: Arc::new(parameters.to_vec()),
            body_type_parameters: &[],
            premise_facts: requires.to_vec(),
            normal_results: Arc::new(Vec::new()),
            returned_field_owners: Arc::new(Vec::new()),
            normal_memberships: Arc::new(Vec::new()),
            normal_content_guarantees: Arc::new(Vec::new()),
            invalidated_storage: Arc::new(Vec::new()),
            storage_exposures: Arc::new(storage_effects::StorageExposureRecord::default()),
            stored_values: Arc::new(Vec::new()),
            capture_sequence: 0,
            storage_write_sequence: 0,
            normal_guarantees: Arc::new(Vec::new()),
            captured_result_equations: Arc::new(Vec::new()),
            resolve_applications,
            require_closed_computation: false,
            runtime_body_values: false,
            specification_terms: false,
            case_facts_only: false,
            substitutions: Arc::new(Vec::new()),
            rewrites: Arc::new(rewrite_table::RewriteTable::default()),
            disequalities: Arc::new(Vec::new()),
            frame_resolver: Arc::new(std::sync::OnceLock::new()),
            case_facts: Arc::new(Vec::new()),
            case_substitutions: Arc::new(Vec::new()),
            hypotheses_contradictory: false,
            ring_licenses: program
                .machine(owner)
                .map(|machine| licenses.ring_licenses(program, machine))
                .unwrap_or_default(),
            semiring_licenses: program
                .machine(owner)
                .map(|machine| licenses.semiring_licenses(program, machine))
                .unwrap_or_default(),
        };
        for fact in requires {
            judge.intake(program, *fact);
        }
        judge
    }

    fn declaration_scope(
        &self,
    ) -> Option<(SymbolHandle, &[typed_trees::signature::StateParameter])> {
        // Declaration formation supplies its complete telescope. It can be a
        // non-entry state or a data/trait owner; substituting machine entry
        // parameters here would change the identity of its exact binders.
        if !self.runtime_body_values {
            return Some((self.machine_symbol, &self.declaration_parameters));
        }
        let Some(machine) = self.program.machine(self.machine_symbol) else {
            return Some((self.machine_symbol, &self.declaration_parameters));
        };
        let state = if self.declaration_state.is_valid() {
            self.program
                .state_in_machine(machine, self.declaration_state)?
        } else {
            self.program.machine_states(machine).first()?
        };
        Some((machine.symbol, self.program.state_parameters(state)))
    }

    fn declaration_term(&self, expression: ExpressionHandle) -> Option<StructuralTerm> {
        if self.requires_value_preservation() {
            let machine = self.program.machine(self.machine_symbol)?;
            let state = if self.declaration_state.is_valid() {
                self.program
                    .state_in_machine(machine, self.declaration_state)?
            } else {
                self.program.machine_states(machine).first()?
            };
            let mut environment = self
                .program
                .state_parameters(state)
                .iter()
                .map(|parameter| {
                    let name = parameter.name.as_str().to_owned();
                    (name.clone(), StructuralTerm::Variable(name))
                })
                .collect::<Vec<_>>();
            if !environment
                .iter()
                .any(|(name, _)| name == super::RESULT_BINDER)
            {
                environment.push((
                    super::RESULT_BINDER.to_owned(),
                    StructuralTerm::Variable(super::RESULT_BINDER.to_owned()),
                ));
            }
            // A runtime predicate observes this path's current storage through
            // the same read owner as completed operands. Exact replacements
            // can be observed; retired roots still decline there. Abstract
            // entry terms must never reinterpret an old binder as new contents.
            return self.predicate_term_in_scope(
                (machine, state, state.return_type),
                expression,
                &environment,
                0,
            );
        }
        super::structural_terms::structural_term(
            self.program,
            None,
            Some(self.declaration_scope()?),
            expression,
            None,
        )
    }

    /// The ordinary predicate reader can also consume an executed prefix's
    /// exact local bindings. These bindings affect the question only; the
    /// judge's admitted premises and captured storage epochs remain intact.
    pub(super) fn predicate_term_in_scope(
        &self,
        scope: (&Machine, &typed_trees::state::State, TypeReferenceHandle),
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        depth: usize,
    ) -> Option<StructuralTerm> {
        let mut specification = self.clone();
        specification.specification_terms = true;
        specification.callee_term_in_scope(Some(scope), expression, environment, depth)
    }

    /// Judge a Boolean question without replacing a prefix's current bindings
    /// by declaration parameters. Logical connectives decompose the question;
    /// no part of that question is installed as a premise for another leaf.
    pub(super) fn judge_predicate_in_scope(
        &self,
        scope: (&Machine, &typed_trees::state::State, TypeReferenceHandle),
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        depth: usize,
    ) -> StructuralJudgment {
        if depth >= 32 {
            return StructuralJudgment::Unknown;
        }
        if let Some(binary) = self.program.expression_table.binary_node(expression)
            && matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
            && crate::has_builtin_decomposed_guard_meaning(
                self.program,
                scope.0,
                Some(scope.1),
                expression,
            )
        {
            return StructuralJudgment::connective(
                binary.operator,
                self.judge_predicate_in_scope(scope, binary.left, environment, depth + 1),
                self.judge_predicate_in_scope(scope, binary.right, environment, depth + 1),
            );
        }
        let Some(predicate) =
            self.predicate_term_in_scope(scope, expression, environment, depth + 1)
        else {
            return StructuralJudgment::Unknown;
        };
        self.judge_observed_predicate(predicate, depth)
    }

    pub(super) fn application_is_independent(
        &self,
        callee: SymbolHandle,
        selections: &[typed_trees::expression::StaticMachineArgument],
    ) -> bool {
        crate::machine_calls::contract_dependencies::independent_application(
            self.program,
            self.machine_symbol,
            callee,
            selections,
        )
    }

    pub(super) fn declaration_expression_type(
        &self,
        expression: ExpressionHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let (owner, parameters) = self.declaration_scope()?;
        crate::parameter_expression_result_type_reference(
            self.program,
            owner,
            parameters,
            expression,
        )
    }

    fn declaration_term_pair(
        &self,
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<ObservedEqualityTerms> {
        let source_term = |expression, side| {
            let term = self.declaration_term(expression);
            if super::struct_trace() && term.is_none() {
                eprintln!(
                    "STRUCT equality-part-refused caller={:?} side={side} expression={}",
                    self.machine_symbol,
                    self.program.expression_table.display_name(expression)
                );
            }
            term
        };
        let left = source_term(binary.left, "left")?;
        let right = source_term(binary.right, "right")?;
        // Anonymous literals have no standalone declaration type. Resolve the
        // selected binary's operand carriers together, just as guarantee
        // capture does; an untyped literal must not erase an entry equation.
        let references = [binary.left, binary.right]
            .map(|expression| self.declaration_expression_type(expression));
        let references = crate::value_custody::expression_types::builtin_scalar_operand_references(
            self.program,
            binary,
            references,
            None,
        )?;
        let resolve_observed = |reference, term: &StructuralTerm| {
            let mut specification = self.clone();
            specification.specification_terms = true;
            let resolved = specification.resolve(term.clone());
            if !super::structural_terms::scalar_operand_is_observed(
                self.program,
                reference,
                &resolved,
            ) {
                if super::struct_trace() {
                    eprintln!(
                        "STRUCT equality-observation-refused caller={:?} reference={reference:?} source={term:?} resolved={resolved:?}",
                        self.machine_symbol
                    );
                }
                return None;
            }
            Some(resolved)
        };
        let resolved_left = resolve_observed(references[0], &left)?;
        let resolved_right = resolve_observed(references[1], &right)?;
        Some(ObservedEqualityTerms {
            source: [left, right],
            resolved: [resolved_left, resolved_right],
        })
    }

    fn has_identity_equality_meaning(&self, expression: ExpressionHandle) -> bool {
        let Some((owner, parameters)) = self.declaration_scope() else {
            return false;
        };
        super::structural_terms::has_identity_equality_meaning(
            self.program,
            owner,
            parameters,
            expression,
        )
    }

    pub(super) fn intake(&mut self, program: &dyn crate::CheckingInput, fact: ExpressionHandle) {
        let Some(binary) = program.expression_table.binary_node(fact) else {
            return;
        };
        if super::structural_terms::is_case_observation(program, fact) {
            if !self.is_case_membership(fact, binary) {
                return;
            }
            let Some((definition, variant)) =
                super::structural_terms::case_classifier(program, binary.right)
            else {
                return;
            };
            let Some(subject) = self.declaration_term(binary.left) else {
                return;
            };
            Arc::make_mut(&mut self.case_facts).push((
                subject.clone(),
                definition.symbol,
                variant.symbol,
            ));
            // A tag fact alone establishes a complete value only when there
            // are no common or payload fields. Other arm refinements retain
            // their symbolic fields at the case-flow owner.
            if program
                .data_members(definition)
                .iter()
                .any(|member| matches!(member, typed_trees::data::DataMember::Field(_)))
                || !program.data_payload_fields(variant).is_empty()
            {
                return;
            }
            let value = StructuralTerm::Constructor {
                data: definition.name.as_str().to_owned(),
                case: variant.name.as_str().to_owned(),
                fields: Vec::new(),
            };
            self.intake_case_equation(subject.clone(), value.clone(), 0);
            self.intake_equation(subject, value, 0);
            return;
        }
        match binary.operator {
            BinaryOperator::And if self.has_boolean_connective_meaning(fact) => {
                self.intake(program, binary.left);
                self.intake(program, binary.right);
            }
            BinaryOperator::NotEqual if !self.case_facts_only => {
                self.intake_preserved_predicate(fact);
            }
            BinaryOperator::Equal
                if !self.case_facts_only && self.has_identity_equality_meaning(fact) =>
            {
                let Some(terms) = self.declaration_term_pair(binary) else {
                    return;
                };
                let [left, right] = terms.source;
                self.intake_case_equation(left.clone(), right.clone(), 0);
                self.intake_equation(left, right, 0);
            }
            _ => {}
        }
    }

    /// A preserved runtime prefix may retain a selected Boolean predicate as
    /// truth. Entry hypotheses alone do not license this transport: the path
    /// owner must first keep their subjects through every preceding effect.
    pub(super) fn intake_preserved_predicate(&mut self, fact: ExpressionHandle) {
        if self.case_facts_only {
            return;
        }
        let program = self.program;
        if let Some(binary) = program.expression_table.binary_node(fact)
            && binary.operator == BinaryOperator::NotEqual
            && self.has_identity_equality_meaning(fact)
            && let Some(terms) = self.declaration_term_pair(binary)
        {
            let [left, right] = terms.source;
            let values = (self.resolve(left), self.resolve(right));
            if !self.disequalities.contains(&values) {
                Arc::make_mut(&mut self.disequalities).push(values);
            }
        }
        if let Some(binary) = program.expression_table.binary_node(fact)
            && binary.operator == BinaryOperator::And
            && self.has_boolean_connective_meaning(fact)
        {
            self.intake_preserved_predicate(binary.left);
            self.intake_preserved_predicate(binary.right);
        } else if let Some(term) = self.declaration_term(fact) {
            let predicate = match &term {
                StructuralTerm::MathematicalBinary { .. } => term.is_mathematical_predicate(),
                StructuralTerm::ScalarBinary { .. } | StructuralTerm::Application { .. } => {
                    self.declaration_expression_type(fact)
                        .and_then(|reference| program.primitive_type_reference(reference))
                        == Some(typed_trees::types::PrimitiveType::Bool)
                }
                _ => false,
            };
            if predicate {
                self.intake_equation(term, value_predicates::boolean_term(true), 0);
            }
        }
    }

    /// One structural equation: constructor pairs DECOMPOSE (injectivity --
    /// `Succ(a) == Succ(b)` yields `a == b`), distinct cases of one data
    /// make the hypotheses contradictory (disjointness), and a variable side
    /// becomes a directed substitution (first binding wins).
    pub(super) fn intake_equation(
        &mut self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
    ) {
        if self.case_facts_only || depth >= 32 {
            return;
        }
        let left = self.resolve(left);
        let right = self.resolve(right);
        if super::structural_terms::term_is_unobserved_descriptor(self.program, &left)
            || super::structural_terms::term_is_unobserved_descriptor(self.program, &right)
        {
            return;
        }
        let left = left.into_semantic_value();
        let right = right.into_semantic_value();
        if right == value_predicates::boolean_term(true)
            && let StructuralTerm::MathematicalBinary {
                operator: BinaryOperator::And,
                left,
                right,
            } = &left
            && left.is_mathematical_predicate()
            && right.is_mathematical_predicate()
        {
            self.intake_equation(
                *left.clone(),
                value_predicates::boolean_term(true),
                depth + 1,
            );
            self.intake_equation(
                *right.clone(),
                value_predicates::boolean_term(true),
                depth + 1,
            );
            return;
        }
        if let Some((operand, truth)) = self
            .boolean_equality_operand(&left, &right)
            .or_else(|| self.boolean_equality_operand(&right, &left))
        {
            self.intake_equation(operand, truth, depth + 1);
            return;
        }
        match (&left, &right) {
            (
                StructuralTerm::Constructor {
                    data: data_l,
                    case: case_l,
                    fields: fields_l,
                },
                StructuralTerm::Constructor {
                    data: data_r,
                    case: case_r,
                    fields: fields_r,
                },
            ) if data_l == data_r => {
                if case_l != case_r {
                    self.hypotheses_contradictory = true;
                    return;
                }
                for (name_l, value_l) in fields_l {
                    if let Some((_, value_r)) = fields_r.iter().find(|(name_r, _)| name_r == name_l)
                    {
                        self.intake_equation(value_l.clone(), value_r.clone(), depth + 1);
                    }
                }
            }
            (
                StructuralTerm::Application { .. }
                | StructuralTerm::CapturedValue { .. }
                | StructuralTerm::CollectionMeasure { .. }
                | StructuralTerm::Projection { .. }
                | StructuralTerm::IndexedValue { .. }
                | StructuralTerm::MathematicalBinary { .. }
                | StructuralTerm::IntegerEmbedding { .. }
                | StructuralTerm::ScalarBinary { .. },
                _,
            ) => {
                if !term_contains(&right, &left) {
                    Arc::make_mut(&mut self.rewrites).push((left, right));
                }
            }
            (
                _,
                StructuralTerm::Application { .. }
                | StructuralTerm::CapturedValue { .. }
                | StructuralTerm::CollectionMeasure { .. }
                | StructuralTerm::Projection { .. }
                | StructuralTerm::IndexedValue { .. }
                | StructuralTerm::MathematicalBinary { .. }
                | StructuralTerm::IntegerEmbedding { .. }
                | StructuralTerm::ScalarBinary { .. },
            ) => {
                if !term_contains(&left, &right) {
                    Arc::make_mut(&mut self.rewrites).push((right, left));
                }
            }
            (StructuralTerm::Variable(name), _) => {
                if left != right {
                    Arc::make_mut(&mut self.substitutions).push((name.clone(), right));
                }
            }
            (_, StructuralTerm::Variable(name)) => {
                Arc::make_mut(&mut self.substitutions).push((name.clone(), left));
            }
            // A parameter-rooted member place resolves to its Opaque display
            // (`left.code`), so it carries the same rewrite as the unresolved
            // `Projection` above. Variables keep their substitution priority.
            (StructuralTerm::Opaque(_), _) => {
                if !term_contains(&right, &left) {
                    Arc::make_mut(&mut self.rewrites).push((left, right));
                }
            }
            (_, StructuralTerm::Opaque(_)) => {
                if !term_contains(&left, &right) {
                    Arc::make_mut(&mut self.rewrites).push((right, left));
                }
            }
            // Distinct literals are disjoint, as distinct cases are above:
            // a rewrite can resolve both sides of a later premise to them.
            (StructuralTerm::Integer(_), StructuralTerm::Integer(_)) if left != right => {
                self.hypotheses_contradictory = true;
            }
            _ => {}
        }
    }

    /// Follow variable substitutions to a fixpoint, depth-capped (a cyclic
    /// substitution chain resolves to wherever the cap lands, which only
    /// weakens judgments toward Unknown -- never unsound). Constructor
    /// fields resolve recursively under the same budget.
    /// Semantic term equality deliberately ignores observation authority and
    /// literal landing metadata. Resolution preserves those payloads, so an
    /// equal term is not an interchangeable memoized result. Keep one general
    /// resolver until a complete representation key and state custody exist.
    pub(super) fn resolve(&self, term: StructuralTerm) -> StructuralTerm {
        self.resolve_at(term, 0)
    }

    fn resolve_at(&self, mut term: StructuralTerm, depth: usize) -> StructuralTerm {
        // A call-premise traversal also extracts locals and state arguments.
        // Keep its entire environment free of name-selected unfolding, not
        // just the final argument: a projected local could otherwise hide
        // where its constructor came from before the call-site gate sees it.
        if !self.resolve_applications && !self.runtime_body_values {
            return self
                .resolve_case_subject(term.clone(), depth)
                .unwrap_or(term);
        }
        if depth >= 32 {
            return term;
        }
        for _ in 0..32 {
            match term {
                StructuralTerm::ObservedValue {
                    observed_type,
                    value,
                    descriptor,
                } => {
                    return StructuralTerm::ObservedValue {
                        observed_type,
                        value: Box::new(self.resolve_at(*value, depth + 1)),
                        descriptor: descriptor
                            .map(|descriptor| Box::new(self.resolve_at(*descriptor, depth + 1))),
                    };
                }
                StructuralTerm::CapturedValue { .. } => {
                    let Some((_, replacement)) = self.rewrites.find(&term) else {
                        return term;
                    };
                    term = replacement.clone();
                }
                StructuralTerm::Variable(ref name) => {
                    let Some((_, replacement)) = self
                        .substitutions
                        .iter()
                        .find(|(variable, _)| variable == name)
                    else {
                        return term;
                    };
                    term = replacement.clone();
                }
                StructuralTerm::Constructor { data, case, fields } => {
                    return StructuralTerm::Constructor {
                        data,
                        case,
                        fields: fields
                            .into_iter()
                            .map(|(name, value)| (name, self.resolve_at(value, depth + 1)))
                            .collect(),
                    };
                }
                StructuralTerm::Application {
                    result_type,
                    target,
                    selections,
                    machine,
                    arguments,
                } => {
                    let arguments: Vec<StructuralTerm> = arguments
                        .into_iter()
                        .map(|argument| self.resolve_at(argument, depth + 1))
                        .collect();
                    let resolved = StructuralTerm::Application {
                        result_type,
                        target,
                        selections,
                        machine,
                        arguments,
                    };
                    // Case-only reasoning keeps application subjects opaque.
                    // Runtime reasoning instead captures executed calls before
                    // term formation; the applications retained in contracts
                    // still denote mathematical values and must normalize.
                    if !self.resolve_applications && !self.runtime_body_values {
                        return resolved;
                    }
                    if self.require_closed_computation {
                        let StructuralTerm::Application {
                            target,
                            selections,
                            machine,
                            arguments,
                            ..
                        } = &resolved
                        else {
                            unreachable!();
                        };
                        // Conditional normal-return guarantees are not proof
                        // that this evaluation completes. Compute its body
                        // using the same continuing invocation depth instead.
                        return self
                            .unfold_application(*target, selections, machine, arguments, depth + 1)
                            .map_or(resolved.clone(), |value| self.resolve_at(value, depth + 1));
                    }
                    // Hypothesis rewrites first (the inductive hypothesis
                    // reduces the self-application), then unfolding.
                    if let Some((_, replacement)) = self.rewrites.find(&resolved) {
                        term = replacement.clone();
                        continue;
                    }
                    if let Some(truth) = self.resolved_float_comparison(&resolved, depth + 1) {
                        return truth;
                    }
                    if let Some(normalized) =
                        self.selected_contract_normal_form(&resolved, depth + 1)
                    {
                        term = normalized;
                        continue;
                    }
                    let StructuralTerm::Application {
                        result_type: _,
                        target,
                        selections,
                        machine,
                        arguments,
                    } = &resolved
                    else {
                        unreachable!();
                    };
                    if let Some(unfolded) =
                        self.unfold_application(*target, selections, machine, arguments, depth + 1)
                    {
                        term = unfolded;
                        continue;
                    }
                    return resolved;
                }
                StructuralTerm::CallProjection {
                    selections,
                    target,
                    machine,
                    result_type,
                    field,
                    field_name,
                    arguments,
                } => {
                    let arguments = arguments
                        .into_iter()
                        .map(|argument| self.resolve_at(argument, depth + 1))
                        .collect::<Vec<_>>();
                    let resolved = StructuralTerm::CallProjection {
                        selections,
                        target,
                        machine,
                        result_type,
                        field,
                        field_name,
                        arguments,
                    };
                    if !self.resolve_applications && !self.runtime_body_values {
                        return resolved;
                    }
                    let StructuralTerm::CallProjection {
                        selections,
                        target,
                        result_type,
                        field,
                        arguments,
                        ..
                    } = &resolved
                    else {
                        unreachable!()
                    };
                    if let Some(unfolded) = self.unfold_exact_call_projection(
                        *target,
                        selections,
                        *result_type,
                        *field,
                        arguments,
                        depth + 1,
                    ) {
                        term = unfolded;
                        continue;
                    }
                    return resolved;
                }
                StructuralTerm::IndexedValue {
                    element_type,
                    element_identity,
                    collection,
                    index,
                } => {
                    let resolved = StructuralTerm::IndexedValue {
                        element_type,
                        element_identity,
                        collection: Box::new(self.resolve_at(*collection, depth + 1)),
                        index: Box::new(self.resolve_at(*index, depth + 1)),
                    };
                    if let Some((_, replacement)) = self.rewrites.find(&resolved) {
                        term = replacement.clone();
                        continue;
                    }
                    return resolved;
                }
                StructuralTerm::CollectionMeasure {
                    measure,
                    collection,
                } => {
                    let collection = self
                        .resolve_at(*collection, depth + 1)
                        .into_collection_descriptor();
                    if let Some(value) = super::structural_terms::completed_collection_measure(
                        self.program,
                        measure,
                        &collection,
                    ) {
                        return value;
                    }
                    let resolved = StructuralTerm::CollectionMeasure {
                        measure,
                        collection: Box::new(collection),
                    };
                    if let Some((_, replacement)) = self.rewrites.find(&resolved) {
                        term = replacement.clone();
                        continue;
                    }
                    return resolved;
                }
                StructuralTerm::Projection { subject, path } => {
                    let subject = self.resolve_at(*subject, depth + 1);
                    let resolved = StructuralTerm::Projection {
                        subject: Box::new(subject.clone()),
                        path: path.clone(),
                    };
                    if let Some((_, replacement)) = self.rewrites.find(&resolved) {
                        term = replacement.clone();
                        continue;
                    }
                    if (!self.require_closed_computation || subject.is_closed_value())
                        && let Some(value) = path.split('.').try_fold(subject, |value, field| {
                            super::observed_projections::project_declared_field(
                                self.program,
                                value,
                                field,
                            )
                        })
                        && value != resolved
                    {
                        term = value;
                        continue;
                    }
                    return resolved;
                }
                StructuralTerm::ScalarBinary {
                    references,
                    operator,
                    meaning,
                    left,
                    right,
                } => {
                    let left = self.resolve_at(*left, depth + 1);
                    let right = self.resolve_at(*right, depth + 1);
                    let resolved = StructuralTerm::ScalarBinary {
                        references,
                        operator,
                        meaning,
                        left: Box::new(left.clone()),
                        right: Box::new(right.clone()),
                    };
                    if !super::structural_terms::scalar_operand_is_observed(
                        self.program,
                        references[0],
                        &left,
                    ) || !super::structural_terms::scalar_operand_is_observed(
                        self.program,
                        references[1],
                        &right,
                    ) {
                        return resolved;
                    }
                    if let Some((_, replacement)) = self.rewrites.find(&resolved) {
                        term = replacement.clone();
                        continue;
                    }
                    if let Some(folded) = fold_scalar_binary(operator, meaning, &left, &right) {
                        return folded;
                    }
                    return resolved;
                }
                StructuralTerm::IntegerEmbedding { reference, value } => {
                    let value = self.resolve_at(*value, depth + 1);
                    return StructuralTerm::IntegerEmbedding {
                        reference,
                        value: Box::new(value),
                    };
                }
                StructuralTerm::MathematicalBinary {
                    operator,
                    left,
                    right,
                } => {
                    let resolved = StructuralTerm::MathematicalBinary {
                        operator,
                        left: Box::new(self.resolve_at(*left, depth + 1)),
                        right: Box::new(self.resolve_at(*right, depth + 1)),
                    };
                    if let Some((_, replacement)) = self.rewrites.find(&resolved) {
                        term = replacement.clone();
                        continue;
                    }
                    return resolved;
                }
                StructuralTerm::Opaque(_) => {
                    let Some((_, replacement)) = self.rewrites.find(&term) else {
                        return term;
                    };
                    term = replacement.clone();
                }
                StructuralTerm::Integer(_)
                | StructuralTerm::Float(_)
                | StructuralTerm::BoundValue(_)
                | StructuralTerm::BoundProjection { .. } => return term,
            }
        }
        term
    }

    /// Unfold one exact checked record-returning call far enough to interpret
    /// a direct result field. This is proof checking, not public-contract
    /// derivation: the statement remains coupled to the callee interface,
    /// while checked body computation or a proven functional ensures supplies
    /// the value. Reuse application normalization for locals and terminal values.
    /// The direct-literal shortcut observes only the selected field, preserving
    /// proofs whose unrelated record fields have no structural normal form.
    fn unfold_exact_call_projection(
        &self,
        target: SymbolHandle,
        selections: &[typed_trees::expression::StaticMachineArgument],
        result_type: TypeReferenceHandle,
        field: SymbolHandle,
        arguments: &[StructuralTerm],
        depth: usize,
    ) -> Option<StructuralTerm> {
        if depth >= 32 {
            return None;
        }
        let program = self.program;
        let Some((machine, state)) =
            crate::machine_calls::calls::machine_state_by_symbol(program, target)
        else {
            return None;
        };
        if state.return_type != result_type {
            return None;
        }
        // The selected state is authoritative. Its display spelling may be
        // qualified or aliased and is not an additional declaration identity.
        let data = crate::value_custody::places::data_definition_for_type(program, result_type)?;
        let projected_field = program.data_field(data, field)?;
        let direct_literal_field = || {
            let machine_parameters = program
                .machine_kind_type_parameters(machine)
                .collect::<Vec<_>>();
            if machine_parameters.len() != selections.len() {
                return None;
            }
            let static_environment = machine_parameters
                .iter()
                .zip(selections)
                .map(|(parameter, selected)| {
                    (
                        parameter.symbol,
                        parameter.name.as_str().to_owned(),
                        selected.clone(),
                    )
                })
                .collect::<Vec<_>>();
            let parameters = program.state_parameters(state);
            if parameters.len() != arguments.len() {
                return None;
            }
            let environment = parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.name.as_str().to_owned(), argument.clone()))
                .collect::<Vec<_>>();
            let [StatementNode::Transition(transition)] =
                program.statement_table.statements(state.statement_nodes)
            else {
                return None;
            };
            if !matches!(transition.guard, TransitionGuardNode::Always)
                || transition.continuation.is_valid()
            {
                return None;
            }
            let TransitionTargetNode::Value(value) =
                program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            let literal = program.expression_table.struct_literal_node(*value)?;
            let projected = program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .find(|candidate| candidate.field_symbol == field)?;
            self.callee_term(
                projected.value,
                &environment,
                StaticObservations {
                    machine_bindings: &static_environment,
                    body_parameters: &[],
                },
                None,
                depth + 1,
            )
        };
        if !self.require_closed_computation
            && let Some(value) = direct_literal_field()
        {
            return Some(value);
        }
        let result = self.unfold_application(
            target,
            selections,
            machine.name.as_str(),
            arguments,
            depth + 1,
        )?;
        let result = self.resolve_at(result, depth + 1);
        if self.require_closed_computation && !result.is_closed_value() {
            return None;
        }
        let StructuralTerm::Constructor {
            data: result_data,
            case,
            fields,
        } = result
        else {
            return None;
        };
        // The checked return type fixes the owner; spelling only selects within
        // that owner's normalized value, after the exact field-handle join.
        if result_data != data.name.as_str() || !case.is_empty() {
            return None;
        }
        fields
            .into_iter()
            .find_map(|(name, value)| (name == projected_field.name.as_str()).then_some(value))
    }

    /// Closed computation is separate from runtime-prefix resolution. This
    /// declaration judge borrows no caller hypotheses, snapshots or loans.
    /// Invocation eligibility excludes observations, effects, crash routes and
    /// selected preconditions requiring evidence; lowering still independently
    /// establishes termination before publishing the checked application.
    pub(super) fn closed_application_value(
        program: &'program dyn crate::CheckingInput,
        result_type: TypeReferenceHandle,
        target: SymbolHandle,
        selections: &[typed_trees::expression::StaticMachineArgument],
        arguments: &[StructuralTerm],
        projection: Option<SymbolHandle>,
    ) -> Option<StructuralTerm> {
        if !arguments.iter().all(StructuralTerm::is_closed_value) {
            return None;
        }
        let plans = crate::machine_calls::calls::operand_call_plans(program, None);
        let (machine, state) =
            crate::machine_calls::fact_call_projections::checked_call_target_candidate(
                program,
                target,
                &plans.0,
                &plans.1,
                &|_, _| {},
                &mut Vec::new(),
            )?;
        if program.machine_states(machine).first()?.symbol != state.symbol
            || (result_type.is_valid() && state.return_type != result_type)
        {
            return None;
        }
        // Shape-only law matching has no caller telescope. The selected
        // declaration still fixes its own result; never guess from a peer or
        // borrow a caller's expected carrier. Typed applications must agree.
        let result_type = state.return_type;
        if crate::reference_shell(program, result_type).is_some() {
            return None;
        }
        let mut judge = Self::from_requires(program, machine, &[], true);
        judge.require_closed_computation = true;
        // Finite proof constructors (including recursive Nat) are values,
        // while a nominal shell cannot hide references or opaque contents.
        // This is representation authority, not runtime layout or Copy.
        if !judge
            .frame_resolver()?
            .proof_value_is_caller_isolated(result_type)
        {
            return None;
        }
        let value = match projection {
            Some(field) => judge.unfold_exact_call_projection(
                target,
                selections,
                result_type,
                field,
                arguments,
                0,
            )?,
            None => {
                judge.unfold_application(target, selections, machine.name.as_str(), arguments, 0)?
            }
        };
        let value = judge.resolve(value);
        value.is_closed_value().then_some(value)
    }

    /// Compute an admitted single-state value body under exact completed
    /// operands. Case arms keep their constructor fast path. A non-case scalar
    /// guard must be proved or refuted by the ordinary typed predicate judge
    /// before its arm is selected; Unknown supplies no result. Callee locals
    /// and selected computations retain the same declaration-bound environment.
    /// No source initializer is replayed to reconstruct a caller snapshot.
    fn unfold_application(
        &self,
        target: SymbolHandle,
        selections: &[typed_trees::expression::StaticMachineArgument],
        machine_name: &str,
        arguments: &[StructuralTerm],
        depth: usize,
    ) -> Option<StructuralTerm> {
        if super::struct_trace() {
            eprintln!("STRUCT unfold? {machine_name} args {arguments:?} depth {depth}");
        }
        if depth >= 32 {
            return None;
        }
        let program = self.program;
        if self.require_closed_computation {
            if !arguments.iter().all(StructuralTerm::is_closed_value) {
                return None;
            }
            let plans =
                crate::machine_calls::calls::operand_call_plans(program, self.frame_resolver());
            let (_, selected) =
                crate::machine_calls::fact_call_projections::checked_call_target_candidate(
                    program,
                    target,
                    &plans.0,
                    &plans.1,
                    &|_, _| {},
                    &mut Vec::new(),
                )?;
            if !self
                .frame_resolver()?
                .proof_value_is_caller_isolated(selected.return_type)
            {
                return None;
            }
        }
        let machine = self.entry_machines.machine(program, target)?;
        let static_parameters = program.machine_type_parameters(machine);
        if static_parameters.len() != selections.len() {
            return None;
        }
        // Callable and scalar operands share one declaration-ordered telescope.
        // Filtering it to callable binders loses const operands before their
        // body can be interpreted. Admission still owns kinds and carriers;
        // this substitution supplies neither a call's premises nor totality.
        let mut static_environment = Vec::new();
        for (parameter, selected) in static_parameters.iter().zip(selections) {
            match parameter.kind {
                typed_trees::data::TypeParameterKind::Machine { .. }
                | typed_trees::data::TypeParameterKind::Const { .. }
                | typed_trees::data::TypeParameterKind::Value { .. } => {
                    static_environment.push((
                        parameter.symbol,
                        parameter.name.as_str().to_owned(),
                        selected.clone(),
                    ));
                }
                _ => return None,
            }
        }
        let [state] = program.machine_states(machine) else {
            return None;
        };
        let parameters = program.state_parameters(state);
        if parameters.len() != arguments.len() {
            return None;
        }
        let environment: Vec<(String, StructuralTerm)> = parameters
            .iter()
            .zip(arguments.iter())
            .map(|(parameter, argument)| (parameter.name.as_str().to_owned(), argument.clone()))
            .collect();
        // The leading entries are the caller's arguments, which `resolve_at`
        // resolved before unfolding. Resolving one again as an arm subject
        // re-attempts every unfold nested inside it, once per arm and per
        // enclosing unfold, which grows exponentially with the term's depth.
        let parameter_count = environment.len();

        let mut environment = environment;
        let statements = program.statement_table.statements(state.statement_nodes);
        for (index, statement) in statements.iter().enumerate() {
            // A `let` (spelled, or the lowering's __hoist_N of a call-valued
            // terminal -- e.g. a definitional wrapper like
            // `snoc(s, x) = (append(s, [x]))`) BINDS: its initializer
            // termifies under the environment built so far and the local
            // joins it, so the terminal's name resolves. Mirrors the
            // sole-arm and case-arm recognizers.
            if is_arm_pattern_marker(statement) {
                continue; // exhaustiveness carrier, not shape
            }
            if let Some(local) = statement.as_local_data() {
                let term = self.callee_term(
                    local.initial_value,
                    &environment,
                    StaticObservations {
                        machine_bindings: &static_environment,
                        body_parameters: &[],
                    },
                    Some((machine, state, local.type_reference)),
                    depth + 1,
                )?;
                let term = if self.require_closed_computation {
                    let completed = self.resolve_at(term, depth + 1);
                    if !completed.is_closed_value() {
                        return None;
                    }
                    completed
                } else {
                    term
                };
                environment.push((local.name.as_str().to_owned(), term));
                continue;
            }
            // Ordinary body tails carry the same result value as a terminal
            // transition. An earlier expression is not a return and cannot
            // bypass subsequent statements or their effects.
            if let Some(expression) = statement.as_expression() {
                return (index + 1 == statements.len())
                    .then(|| {
                        self.callee_term(
                            *expression,
                            &environment,
                            StaticObservations {
                                machine_bindings: &static_environment,
                                body_parameters: &[],
                            },
                            Some((machine, state, state.return_type)),
                            depth + 1,
                        )
                    })
                    .flatten();
            }
            let transition = statement.as_transition()?;
            if transition.continuation.is_valid() {
                return None;
            }
            let fires = match transition.guard {
                TransitionGuardNode::Always => true,
                TransitionGuardNode::When(guard) => {
                    if let Some((arm_data, arm_case)) =
                        super::structural_terms::case_guard_classifier(
                            program,
                            machine,
                            Some(state),
                            guard,
                        )
                    {
                        let Some(comparison) = program.expression_table.binary_node(guard) else {
                            return None;
                        };
                        if !matches!(
                            comparison.operator,
                            BinaryOperator::Equal | BinaryOperator::CaseMembership
                        ) {
                            return None;
                        }
                        let subject = super::structural_terms::structural_term(
                            program,
                            Some((machine, state, Default::default())),
                            None,
                            comparison.left,
                            None,
                        )?;
                        let StructuralTerm::Variable(subject_name) = subject else {
                            return None;
                        };
                        let position = environment
                            .iter()
                            .position(|(name, _)| name == &subject_name)?;
                        let local_subject;
                        let subject_term = if position < parameter_count {
                            &environment[position].1
                        } else {
                            local_subject =
                                self.resolve_at(environment[position].1.clone(), depth + 1);
                            &local_subject
                        };
                        let StructuralTerm::Constructor {
                            data: got_data,
                            case: got_case,
                            ..
                        } = subject_term
                        else {
                            // The matched argument is not (yet) a constructor:
                            // arm selection is undecidable, no unfold.
                            return None;
                        };
                        got_data == arm_data.name.as_str() && got_case == arm_case.name.as_str()
                    } else {
                        // Preserve the case-arm fast path above: resolving its
                        // already-completed constructor arguments again repeats
                        // recursive unfolding. Non-case scalar guards use the
                        // same captured environment and selected source meaning.
                        // Unknown never chooses an arm or assumes the goal.
                        match self.judge_predicate_in_scope(
                            (machine, state, state.return_type),
                            guard,
                            &environment,
                            depth + 1,
                        ) {
                            StructuralJudgment::Proven => true,
                            StructuralJudgment::Refuted => false,
                            StructuralJudgment::Unknown => return None,
                        }
                    }
                }
            };
            if !fires {
                continue;
            }
            let TransitionTargetNode::Value(value) =
                program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            return self.callee_term(
                *value,
                &environment,
                StaticObservations {
                    machine_bindings: &static_environment,
                    body_parameters: &[],
                },
                Some((machine, state, state.return_type)),
                depth + 1,
            );
        }
        None
    }

    /// Project an already observed value using an authenticated declared field.
    /// Source expressions and live-frame subjects share the same term algebra;
    /// neither route reconstructs a field read under a different program point.
    fn observed_field_term(
        &self,
        receiver_term: StructuralTerm,
        field_name: &str,
        depth: usize,
    ) -> Option<StructuralTerm> {
        let value = if self.runtime_body_values {
            // Runtime projections retain the producing operation tree. A
            // completed constructor's arithmetic is not folded merely because
            // a later member read observes it through a public identity.
            self.completed_value_identity(receiver_term)
        } else {
            self.resolve_at(receiver_term, depth + 1)
        };
        if self.require_closed_computation && !value.is_closed_value() {
            return None;
        }
        if super::structural_terms::owned_observation_type(self.program, &value).is_some()
            || super::structural_terms::shared_inline_observation_type(self.program, &value)
                .is_some()
        {
            // Both authenticated inline observations use the typed projection
            // owner. A shared borrow freezes these cells but does not turn an
            // observation shell into an untyped constructor: the raw field
            // algebra must still refuse it. The typed owner masks a
            // reference child. Returning that descriptor supports metadata,
            // not a later unchecked read of its referent.
            return super::observed_projections::project_declared_field(
                self.program,
                value,
                field_name,
            );
        }
        let projected = super::observed_projections::project_field(value, field_name)?;
        // A completed reference is a descriptor, not a snapshot of its
        // referent. Project owned fields normally, but leave a projection
        // through an unobserved descriptor to the executing read, which can
        // capture a fresh value at the current storage epoch. Keeping the
        // descriptor projection here would make later scalar operations
        // reject the entire prefix, including independent facts.
        (!super::structural_terms::term_is_unobserved_descriptor(self.program, &projected))
            .then_some(projected)
    }

    /// Form an expression under its actual declaring machine/state before
    /// substituting the captured environment. The frame retains selected call
    /// result carriers/policies and builtin computation meaning. The final
    /// reference is only a caller's retained landing for anonymous arithmetic;
    /// it does not replace an expression's own selected result type. Abstract
    /// contract telescopes use `structural_term` instead.
    pub(super) fn callee_term_in_scope(
        &self,
        scope: Option<(&Machine, &typed_trees::state::State, TypeReferenceHandle)>,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        depth: usize,
    ) -> Option<StructuralTerm> {
        self.callee_term(
            expression,
            environment,
            StaticObservations {
                machine_bindings: &[],
                body_parameters: if scope
                    .is_some_and(|(machine, _, _)| machine.symbol == self.machine_symbol)
                {
                    self.body_type_parameters
                } else {
                    &[]
                },
            },
            scope,
            depth,
        )
    }

    /// Source formation separately establishes exact preconditions and checked
    /// termination. This gate supplies operational eligibility only, retaining
    /// the same current-storage observation environment for every operand.
    fn specification_call_candidate(
        &self,
        call: &typed_trees::expression::TableCallExpression,
    ) -> bool {
        if !self.specification_terms {
            return false;
        }
        let candidate = specification_application_candidate(
            self.program,
            self.machine_symbol,
            call.target_symbol,
            &call.machine_arguments,
            self.frame_resolver(),
        );
        if super::struct_trace()
            && let Err(reason) = candidate
        {
            eprintln!(
                "STRUCT specification-call-refused caller={:?} target={:?} reason={reason}",
                self.machine_symbol, call.target_symbol
            );
        }
        candidate.is_ok()
    }

    fn callee_term(
        &self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        static_observations: StaticObservations<'_>,
        scope: Option<(&Machine, &typed_trees::state::State, TypeReferenceHandle)>,
        depth: usize,
    ) -> Option<StructuralTerm> {
        if depth >= 32 {
            return None;
        }
        if let Some((_, value)) = self
            .normal_results
            .iter()
            .find(|(source, _)| *source == expression)
        {
            return Some(value.clone());
        }
        if let Some(value) = self.completed_descriptor_place(scope, expression, environment) {
            return Some(value);
        }
        if let Some(value) = self.stored_value_observation(
            scope.map(|(machine, state, _)| (machine, state)),
            expression,
        ) {
            return Some(value);
        }
        let program = self.program;
        // A selected collection measure observes the completed descriptor, not
        // its retired referent. Resolve the receiver through the ordinary value
        // owner first: only an exact completed value or a separately proved
        // stable descriptor binding survives a write. Applying the referent
        // fence to `.len` itself loses that distinction after any earlier write
        // conservatively invalidated reference-bearing state parameters.
        // Ordinary fields (including a user field named `len`) still cross the
        // storage fence below; selection supplies no contents or bounds proof.
        if let ExpressionNode::Member(member) = program.expression_table.expression(expression)
            && let Some((machine, state, _)) = scope
            && let Some(reference) =
                crate::expression_result_type_reference(program, machine, state, member.receiver)
            && let Some(measure) =
                super::structural_terms::collection_measure_term(program, member, reference, || {
                    self.callee_term(
                        member.receiver,
                        environment,
                        static_observations,
                        scope,
                        depth + 1,
                    )
                })
        {
            return Some(measure);
        }
        if self.storage_observation_invalidated(expression) {
            if super::struct_trace() {
                eprintln!(
                    "STRUCT term-storage-refused caller={:?} expression={}",
                    self.machine_symbol,
                    program.expression_table.display_name(expression)
                );
            }
            return None;
        }
        if let Some((_, argument)) =
            crate::proof_contracts::proof_embeddings::integer_embedding_argument(
                program, expression,
            )
        {
            let reference = scope
                .and_then(|(machine, state, _)| {
                    crate::expression_result_type_reference(program, machine, state, argument)
                })
                .or_else(|| {
                    crate::proof_contracts::proof_embeddings::expression_type_reference(
                        program, argument,
                    )
                })?;
            return super::structural_terms::integer_embedding_term(
                program,
                expression,
                reference,
                self.callee_term(argument, environment, static_observations, scope, depth + 1)?,
            );
        }
        if program
            .expression_table
            .call_node(expression)
            .is_some_and(|call| {
                crate::mathematical_applications::selected_definition(program, call).is_some()
            })
        {
            return super::structural_terms::mathematical_application_term(
                program,
                expression,
                |capture| {
                    self.callee_term(capture, environment, static_observations, scope, depth + 1)
                },
            );
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                // Static values use the resolved declaration, not the runtime
                // environment's spelling. A local may shadow a const binder;
                // that never substitutes the local for a retained static use.
                if let Some((_, _, selected)) = static_observations
                    .machine_bindings
                    .iter()
                    .find(|(symbol, _, _)| *symbol == path.symbol)
                    && let Some(parameter) = program.data_type_parameter_by_symbol(path.symbol)
                    && let (typed_trees::data::TypeParameterKind::Const { type_reference }
                    | typed_trees::data::TypeParameterKind::Value { type_reference }) =
                        parameter.kind
                {
                    return match super::static_applications::constant_value(program, selected) {
                        Some(super::static_applications::ConstantValue::Integer(value))
                            if program
                                .primitive_type_reference(type_reference)
                                .is_some_and(|primitive| primitive.accepts_integer_literal()) =>
                        {
                            Some(StructuralTerm::Integer(value))
                        }
                        Some(super::static_applications::ConstantValue::Boolean(value))
                            if program.primitive_type_reference(type_reference)
                                == Some(typed_trees::types::PrimitiveType::Bool) =>
                        {
                            Some(value_predicates::boolean_term(value))
                        }
                        None => {
                            let forwarded = crate::machine_calls::static_arguments::forwarded_type(
                                program, selected,
                            );
                            (forwarded.is_valid()
                                && program.normalized_type_identity(forwarded)
                                    == program.normalized_type_identity(type_reference))
                            .then_some(StructuralTerm::BoundValue(selected.symbol))
                        }
                        _ => None,
                    };
                }
                // Only this body context declares these static const value
                // subjects. Runtime Value binders keep ordinary observation
                // custody. Imported contracts still need exact application
                // bindings; their unbound static symbols cannot enter here.
                if path.symbol.is_valid()
                    && static_observations.body_parameters.iter().any(|parameter| {
                        parameter.symbol == path.symbol
                            && matches!(
                                parameter.kind,
                                typed_trees::data::TypeParameterKind::Const { .. }
                            )
                    })
                {
                    return Some(StructuralTerm::BoundValue(path.symbol));
                }
                if path.symbol.is_valid()
                    && (program.symbols.get(path.symbol).kind == symbols::SymbolKind::Const
                        || program.symbols.builtin_type_member(path.symbol)
                            == Some(symbols::BuiltinTypeMember::AddressBound))
                {
                    return Some(StructuralTerm::BoundValue(path.symbol));
                }
                // An open Value binder is an immutable application subject,
                // not a fresh unknown on each read. Keep its resolved identity
                // through constructor operands as contracts do. Membership in
                // this exact machine telescope excludes same-spelled locals
                // and unrelated type/proof parameters.
                if scope.is_some_and(|(machine, _, _)| {
                    program
                        .machine_type_parameters(machine)
                        .iter()
                        .any(|parameter| {
                            parameter.symbol.is_valid()
                                && parameter.symbol == path.symbol
                                && matches!(
                                    parameter.kind,
                                    typed_trees::data::TypeParameterKind::Value { .. }
                                        | typed_trees::data::TypeParameterKind::Const { .. }
                                )
                        })
                }) {
                    return Some(StructuralTerm::BoundValue(path.symbol));
                }
                let members = program.expression_table.name_path_members(path.members);
                match members {
                    [single] => environment
                        .iter()
                        .find(|(name, _)| name == single.as_str())
                        .map(|(_, term)| term.clone()),
                    [_, _, ..] => super::structural_terms::case_value_term(program, expression),
                    _ => None,
                }
            }
            ExpressionNode::Member(member) => {
                if let ExpressionNode::Call(call) =
                    program.expression_table.expression(member.receiver)
                    && (!call.receiver.is_valid()
                        || super::structural_terms::static_call_receiver(
                            program,
                            scope.map_or(self.machine_symbol, |(machine, _, _)| machine.symbol),
                            call,
                        ))
                    && call.evidence_arguments.is_empty()
                    && call.static_requirement_dispatch.is_none()
                    && !self.runtime_body_values
                    && crate::machine_calls::calls::signature_queries::unique_state_by_symbol(
                        program,
                        call.target_symbol,
                    )
                    .is_some()
                {
                    let arguments = program
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .map(|argument| {
                            self.callee_term(
                                *argument,
                                environment,
                                static_observations,
                                scope,
                                depth + 1,
                            )
                        })
                        .collect::<Option<Vec<_>>>()?;
                    let Some(state) =
                        crate::machine_calls::calls::signature_queries::unique_state_by_symbol(
                            program,
                            call.target_symbol,
                        )
                    else {
                        return None;
                    };
                    let Some(data_symbol) = crate::value_custody::type_shapes::head_data_symbol(
                        program,
                        state.return_type,
                    ) else {
                        return None;
                    };
                    let data = crate::declarations::data_queries::data_definition_by_symbol(
                        program,
                        data_symbol,
                    )?;
                    let field = program.data_members(data).iter().find_map(|candidate| {
                        let field = candidate.as_field()?;
                        (field.name.as_str() == member.member.as_str()).then_some(field.symbol)
                    })?;
                    return Some(StructuralTerm::CallProjection {
                        target: call.target_symbol,
                        selections: call.machine_arguments.to_vec(),
                        machine: structural_call_machine_name(
                            call.target.as_str(),
                            &call.machine_arguments,
                            static_observations.machine_bindings,
                        ),
                        result_type: state.return_type,
                        field,
                        field_name: member.member.as_str().to_owned(),
                        arguments,
                    });
                }
                let receiver_term = self.callee_term(
                    member.receiver,
                    environment,
                    static_observations,
                    scope,
                    depth + 1,
                )?;
                self.observed_field_term(receiver_term, member.member.as_str(), depth)
            }
            ExpressionNode::StructLiteral(literal) => {
                super::structural_terms::constructor_literal_term(program, literal, |_, field| {
                    let field = field?;
                    self.callee_term(
                        field.value,
                        environment,
                        static_observations,
                        scope,
                        depth + 1,
                    )
                })
            }
            ExpressionNode::Unary(unary) => super::structural_terms::logical_not_term_in_scope(
                program,
                scope,
                None,
                expression,
                self.callee_term(
                    unary.operand,
                    environment,
                    static_observations,
                    scope,
                    depth + 1,
                )?,
            ),
            ExpressionNode::Integer(value) => value.value_bignum().map(StructuralTerm::Integer),
            ExpressionNode::Cast(cast) => {
                let (machine, state, _) = scope?;
                let source =
                    crate::expression_result_type_reference(program, machine, state, cast.value)?;
                super::structural_terms::denotation_preserving_conversion(program, cast, source)
                    .then(|| {
                        self.callee_term(
                            cast.value,
                            environment,
                            static_observations,
                            scope,
                            depth + 1,
                        )
                    })
                    .flatten()
            }
            ExpressionNode::Float(value) => Some(StructuralTerm::Float(value.clone())),
            ExpressionNode::ZeroValue(type_reference) => {
                super::structural_terms::zero_value_structural_term(program, *type_reference)
            }
            ExpressionNode::Call(call) => {
                let refusal = if self.runtime_body_values
                    && !normal_returns::call_has_mathematical_denotation(
                        program,
                        scope.map_or(self.machine_symbol, |(machine, _, _)| machine.symbol),
                        call,
                    )
                    && !self.specification_call_candidate(call)
                {
                    Some("value-identity")
                } else if call.receiver.is_valid()
                    && !super::structural_terms::static_call_receiver(
                        program,
                        scope.map_or(self.machine_symbol, |(machine, _, _)| machine.symbol),
                        call,
                    )
                {
                    Some("runtime-receiver")
                } else if !call.evidence_arguments.is_empty() {
                    Some("evidence-telescope")
                } else if call.static_requirement_dispatch.is_some()
                    && super::structural_terms::realized_requirement_state(call).is_none()
                {
                    Some("unrealized-static-requirement")
                } else {
                    None
                };
                if let Some(reason) = refusal {
                    if super::struct_trace() {
                        eprintln!(
                            "STRUCT term-call-refused caller={:?} expression={} target={:?} reason={reason}",
                            self.machine_symbol,
                            program.expression_table.display_name(expression),
                            call.target_symbol
                        );
                    }
                    return None;
                }
                let selected_target = static_observations
                    .machine_bindings
                    .iter()
                    .find(|(symbol, _, _)| *symbol == call.target_symbol)
                    .map(|(_, _, selected)| selected);
                // A selected generic/evidence application is retained on the
                // enclosing term, but this body substitution does not apply
                // its nested telescope. Refuse unfolding instead of dropping it.
                if selected_target.is_some_and(|selected| {
                    selected.application.is_some()
                        || selected.evidence_projection.is_some()
                        || selected.type_reference.is_valid()
                        || selected.const_literal.is_some()
                }) {
                    return None;
                }
                let mut arguments = Vec::new();
                for argument in program.expression_table.expression_handles(call.arguments) {
                    arguments.push(self.callee_term(
                        *argument,
                        environment,
                        static_observations,
                        scope,
                        depth + 1,
                    )?);
                }
                // Substitution changes the selected callable telescope as well
                // as its symbol. Retain its own result declaration rather than
                // the enclosing machine parameter's separately allocated type.
                let selected_result = selected_target.and_then(|selected| {
                    program
                        .machine_parameter_signature(selected.symbol)
                        .map(|(_, signature)| signature.return_type)
                        .or_else(|| {
                            program
                                .state_by_symbol(selected.symbol)
                                .map(|state| state.return_type)
                        })
                });
                let result_type = selected_result
                    .or_else(|| {
                        scope.and_then(|(machine, state, _)| {
                            crate::expression_result_type_reference(
                                program, machine, state, expression,
                            )
                        })
                    })
                    .unwrap_or_default();
                let realization = super::structural_terms::realized_requirement_state(call);
                Some(StructuralTerm::Application {
                    result_type,
                    target: realization.clone().unwrap_or_else(|| {
                        selected_target.map_or_else(
                            || {
                                super::structural_terms::structural_call_target(
                                    program,
                                    scope.map_or(self.machine_symbol, |(machine, _, _)| {
                                        machine.symbol
                                    }),
                                    call,
                                )
                            },
                            |selected| selected.symbol,
                        )
                    }),
                    selections: if realization.is_some() {
                        Vec::new()
                    } else {
                        call.machine_arguments
                            .iter()
                            .map(|argument| {
                                static_observations
                                    .machine_bindings
                                    .iter()
                                    .find(|(symbol, _, _)| *symbol == argument.symbol)
                                    .map_or_else(
                                        || argument.clone(),
                                        |(_, _, selected)| selected.clone(),
                                    )
                            })
                            .collect()
                    },
                    machine: structural_call_machine_name(
                        call.target.as_str(),
                        &call.machine_arguments,
                        static_observations.machine_bindings,
                    ),
                    arguments,
                })
            }
            // A checked borrow selects its referent; it does not create a new
            // case-bearing value. Preserve aliases to the same saved subject.
            ExpressionNode::Borrow(borrow) => self.callee_term(
                borrow.target,
                environment,
                static_observations,
                scope,
                depth + 1,
            ),
            ExpressionNode::Boolean(value) => Some(StructuralTerm::Constructor {
                data: "bool".to_owned(),
                case: value.to_string(),
                fields: Vec::new(),
            }),
            ExpressionNode::Indexed(indexed) => super::structural_terms::indexed_term_in_scope(
                program,
                scope,
                None,
                expression,
                self.callee_term(
                    indexed.collection,
                    environment,
                    static_observations,
                    scope,
                    depth + 1,
                )?,
                self.callee_term(
                    indexed.index,
                    environment,
                    static_observations,
                    scope,
                    depth + 1,
                )?,
            ),
            ExpressionNode::Binary(binary) => super::structural_terms::binary_term_in_scope(
                program,
                scope,
                None,
                expression,
                self.callee_term(
                    binary.left,
                    environment,
                    static_observations,
                    scope,
                    depth + 1,
                )?,
                self.callee_term(
                    binary.right,
                    environment,
                    static_observations,
                    scope,
                    depth + 1,
                )?,
            ),
            _ => None,
        }
    }

    /// Substitute variables in a term (used to instantiate the machine's
    /// own ensures as the INDUCTIVE HYPOTHESIS at a self-call: params -> the
    /// call's argument terms, `result` -> the application term).
    pub(super) fn substitute_term(
        term: &StructuralTerm,
        map: &[(String, StructuralTerm)],
    ) -> StructuralTerm {
        Self::substitute_call_term(term, map, &[])
    }

    /// A call substitutes value subjects and resolved static callable binders
    /// together. Rewriting operands alone leaves a callee's abstract callable
    /// in the caller's fact, even when the authored premises are identical.
    pub(super) fn substitute_call_term(
        term: &StructuralTerm,
        map: &[(String, StructuralTerm)],
        machine_map: &[(
            SymbolHandle,
            TypeReferenceHandle,
            typed_trees::expression::StaticMachineArgument,
        )],
    ) -> StructuralTerm {
        match term {
            StructuralTerm::IntegerEmbedding { reference, value } => {
                StructuralTerm::IntegerEmbedding {
                    reference: *reference,
                    value: Box::new(Self::substitute_call_term(value, map, machine_map)),
                }
            }
            StructuralTerm::MathematicalBinary {
                operator,
                left,
                right,
            } => StructuralTerm::MathematicalBinary {
                operator: *operator,
                left: Box::new(Self::substitute_call_term(left, map, machine_map)),
                right: Box::new(Self::substitute_call_term(right, map, machine_map)),
            },
            StructuralTerm::ObservedValue {
                observed_type,
                value,
                descriptor,
            } => StructuralTerm::ObservedValue {
                observed_type: *observed_type,
                value: Box::new(Self::substitute_call_term(value, map, machine_map)),
                descriptor: descriptor.as_ref().map(|descriptor| {
                    Box::new(Self::substitute_call_term(descriptor, map, machine_map))
                }),
            },
            StructuralTerm::Integer(_)
            | StructuralTerm::Float(_)
            | StructuralTerm::BoundValue(_)
            | StructuralTerm::CapturedValue { .. }
            | StructuralTerm::BoundProjection { .. } => term.clone(),
            StructuralTerm::IndexedValue {
                element_type,
                element_identity,
                collection,
                index,
            } => StructuralTerm::IndexedValue {
                element_type: *element_type,
                element_identity: *element_identity,
                collection: Box::new(Self::substitute_call_term(collection, map, machine_map)),
                index: Box::new(Self::substitute_call_term(index, map, machine_map)),
            },
            StructuralTerm::CollectionMeasure {
                measure,
                collection,
            } => StructuralTerm::CollectionMeasure {
                measure: *measure,
                collection: Box::new(
                    Self::substitute_call_term(collection, map, machine_map)
                        .into_collection_descriptor(),
                ),
            },
            StructuralTerm::Projection { subject, path } => StructuralTerm::Projection {
                subject: Box::new(Self::substitute_call_term(subject, map, machine_map)),
                path: path.clone(),
            },
            StructuralTerm::ScalarBinary {
                references,
                operator,
                meaning,
                left,
                right,
            } => StructuralTerm::ScalarBinary {
                references: *references,
                operator: *operator,
                meaning: *meaning,
                left: Box::new(Self::substitute_call_term(left, map, machine_map)),
                right: Box::new(Self::substitute_call_term(right, map, machine_map)),
            },
            StructuralTerm::Variable(name) => map
                .iter()
                .find(|(variable, _)| variable == name)
                .map(|(_, replacement)| replacement.clone())
                .unwrap_or_else(|| term.clone()),
            StructuralTerm::Constructor { data, case, fields } => StructuralTerm::Constructor {
                data: data.clone(),
                case: case.clone(),
                fields: fields
                    .iter()
                    .map(|(name, value)| {
                        (
                            name.clone(),
                            Self::substitute_call_term(value, map, machine_map),
                        )
                    })
                    .collect(),
            },
            StructuralTerm::Application { .. } | StructuralTerm::CallProjection { .. } => {
                let mut substituted = term.clone().map_argument_terms(|argument| {
                    Self::substitute_call_term(&argument, map, machine_map)
                });
                let (target, selections, machine, result_type) = match &mut substituted {
                    StructuralTerm::Application {
                        target,
                        selections,
                        machine,
                        result_type,
                        ..
                    }
                    | StructuralTerm::CallProjection {
                        target,
                        selections,
                        machine,
                        result_type,
                        ..
                    } => (target, selections, machine, result_type),
                    _ => unreachable!(),
                };
                let selected = machine_map.iter().find(|(symbol, _, _)| symbol == target);
                let mut changed = false;
                if let Some((_, selected_result, selected)) = selected {
                    *target = selected.symbol;
                    *machine = selected.path.joined_path();
                    // The callee binder's result reference is declaration-local.
                    // Keep the selected callable's exact carrier and policy,
                    // just as direct caller-side term formation does.
                    *result_type = *selected_result;
                    changed = true;
                }
                for selection in selections.iter_mut() {
                    if let Some((_, _, actual)) = machine_map
                        .iter()
                        .find(|(symbol, _, _)| *symbol == selection.symbol)
                    {
                        *selection = actual.clone();
                        changed = true;
                    }
                }
                // Resolved symbols choose the substitution. Rebuild its display
                // after the selection changes, retaining untouched spellings.
                if changed && !selections.is_empty() {
                    *machine = structural_call_machine_name(
                        machine.split('<').next().unwrap_or(machine),
                        selections,
                        &[],
                    );
                }
                substituted
            }
            StructuralTerm::Opaque(display) => {
                // Symbolic record member places currently share the Opaque
                // vocabulary (`p.den`). Citation instantiation must still
                // alpha-substitute their exact root parameter; otherwise a
                // cited Rat law leaks callee names into the caller frame.
                // Restrict the rewrite to an exact `<parameter>.` prefix.
                // A symbolic static-machine application is also a legitimate
                // place root (`Middle(index).den`); retain its complete
                // structural receiver.  A concrete constructor can be
                // projected structurally.  Arbitrary opaque arithmetic never
                // gains substring-rewrite semantics.
                for (parameter, replacement) in map {
                    let prefix = format!("{parameter}.");
                    let Some(suffix) = display.strip_prefix(&prefix) else {
                        continue;
                    };
                    return match replacement {
                        StructuralTerm::BoundValue(subject) => StructuralTerm::BoundProjection {
                            subject: *subject,
                            path: suffix.to_owned(),
                        },
                        StructuralTerm::BoundProjection { subject, path } => {
                            StructuralTerm::BoundProjection {
                                subject: *subject,
                                path: format!("{path}.{suffix}"),
                            }
                        }
                        StructuralTerm::Variable(root) | StructuralTerm::Opaque(root) => {
                            StructuralTerm::Opaque(format!("{root}.{suffix}"))
                        }
                        StructuralTerm::Integer(_)
                        | StructuralTerm::Float(_)
                        | StructuralTerm::Application { .. }
                        | StructuralTerm::CallProjection { .. }
                        | StructuralTerm::CapturedValue { .. }
                        | StructuralTerm::ObservedValue { .. }
                        | StructuralTerm::ScalarBinary { .. }
                        | StructuralTerm::MathematicalBinary { .. }
                        | StructuralTerm::IntegerEmbedding { .. }
                        | StructuralTerm::CollectionMeasure { .. }
                        | StructuralTerm::Projection { .. }
                        | StructuralTerm::IndexedValue { .. } => StructuralTerm::Projection {
                            subject: Box::new(replacement.clone()),
                            path: suffix.to_owned(),
                        },
                        StructuralTerm::Constructor { .. } => suffix
                            .split('.')
                            .try_fold(replacement, |subject, field_name| {
                                let StructuralTerm::Constructor { fields, .. } = subject else {
                                    return None;
                                };
                                fields
                                    .iter()
                                    .find_map(|(name, field)| (name == field_name).then_some(field))
                            })
                            .cloned()
                            .unwrap_or_else(|| StructuralTerm::Projection {
                                // An unresolved path still belongs to the actual
                                // receiver. Keeping the callee's spelling here
                                // could alias an unrelated caller parameter.
                                subject: Box::new(replacement.clone()),
                                path: suffix.to_owned(),
                            }),
                    };
                }
                term.clone()
            }
        }
    }

    /// Collect every self-application (calls to `machine_name`) in a term.
    pub(super) fn self_applications<'term>(
        term: &'term StructuralTerm,
        selected_target: SymbolHandle,
        found: &mut Vec<&'term StructuralTerm>,
    ) {
        let term = term.semantic_value();
        if let StructuralTerm::Application { target, .. } = term
            && *target == selected_target
        {
            found.push(term);
        }
        term.for_each_child(|child| Self::self_applications(child, selected_target, found));
    }

    pub(super) fn judge(
        &self,
        program: &dyn crate::CheckingInput,
        fact: ExpressionHandle,
    ) -> StructuralJudgment {
        if self.case_facts_only && !super::call_requirements::case_fact_only(program, fact) {
            return StructuralJudgment::Unknown;
        }
        if program
            .expression_table
            .call_node(fact)
            .is_some_and(|call| {
                crate::mathematical_applications::selected_definition(program, call).is_some()
            })
        {
            // This engine consumes source entry observations rather than this
            // path's completed storage terms. Keep its separate epoch fence.
            if !self.entry_storage_observations_preserved(fact) {
                return StructuralJudgment::Unknown;
            }
            // Mathematical facts consume their prepared term in the same
            // judgment entrance, including proof-only Strict results. A result
            // carrier or source call shape does not select another body walk.
            let mut engine = super::arithmetic_judgment::Engine::for_proof_integer_formation(
                self.program,
                if self.declaration_state.is_valid() {
                    self.declaration_state
                } else {
                    self.machine_symbol
                },
                &[],
                &[],
            );
            return match engine.judge_mathematical_application(fact) {
                super::arithmetic_judgment::Judgment::Proven => StructuralJudgment::Proven,
                super::arithmetic_judgment::Judgment::Refuted
                | super::arithmetic_judgment::Judgment::ConstantFalse => {
                    StructuralJudgment::Refuted
                }
                super::arithmetic_judgment::Judgment::Unknown { .. } => StructuralJudgment::Unknown,
            };
        }
        let Some(binary) = program.expression_table.binary_node(fact) else {
            return self
                .judge_value_predicate(fact)
                .unwrap_or(StructuralJudgment::Unknown);
        };
        if super::structural_terms::is_case_observation(program, fact) {
            if !self.is_case_membership(fact, binary) {
                return StructuralJudgment::Unknown;
            }
            let Some((definition, variant)) =
                super::structural_terms::case_classifier(program, binary.right)
            else {
                return StructuralJudgment::Unknown;
            };
            let Some(subject) = self.declaration_term(binary.left) else {
                return StructuralJudgment::Unknown;
            };
            // Existing result/guarantee checking retains its value-normalizer
            // route. Call-premise transport below never uses that route.
            let resolved = self.resolve(subject.clone());
            return self.judge_case(
                if matches!(resolved, StructuralTerm::Constructor { .. }) {
                    resolved
                } else {
                    subject
                },
                definition.symbol,
                variant.symbol,
            );
        }
        match binary.operator {
            BinaryOperator::And if self.has_boolean_connective_meaning(fact) => {
                StructuralJudgment::connective(
                    BinaryOperator::And,
                    self.judge(program, binary.left),
                    self.judge(program, binary.right),
                )
            }
            BinaryOperator::Or if self.has_boolean_connective_meaning(fact) => {
                self.judge_disjunction(binary.left, binary.right)
            }
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                if let Some(judgment) = self.judge_value_predicate(fact) {
                    return judgment;
                }
                if let Some(judgment) = self.judge_ieee_equality(fact) {
                    return judgment;
                }
                if self.entry_storage_observations_preserved(fact)
                    && let Some(holds) = crate::proof_contracts::float_projection_bindings::semantic_values::closed_float_meaning_equality(
                    program, self.machine_symbol, fact,
                ) {
                    return if holds { StructuralJudgment::Proven } else { StructuralJudgment::Refuted };
                }
                if !self.has_identity_equality_meaning(fact) {
                    if super::struct_trace() {
                        eprintln!(
                            "STRUCT equality-refused caller={:?} fact={} reason=identity-authority",
                            self.machine_symbol,
                            program.expression_table.display_name(fact)
                        );
                    }
                    return StructuralJudgment::Unknown;
                }
                let Some(terms) = self.declaration_term_pair(binary) else {
                    if super::struct_trace() {
                        eprintln!(
                            "STRUCT equality-refused caller={:?} fact={} reason=term-formation",
                            self.machine_symbol,
                            program.expression_table.display_name(fact)
                        );
                    }
                    return StructuralJudgment::Unknown;
                };
                // No facts change between the authority check and judgment.
                // Reuse its exact resolution, including its depth budget; do
                // not resolve the result again or memoize by term equality,
                // which deliberately ignores some observation metadata.
                let [left, right] = terms.resolved;
                if super::struct_trace() {
                    eprintln!(
                        "STRUCT equality caller={:?} left={left:?} right={right:?} rewrites={:?}",
                        self.machine_symbol, self.rewrites,
                    );
                }
                let equality = match self.judge_equation(left.clone(), right.clone(), 0) {
                    // Identity equality of a record is the conjunction of its
                    // field equalities (structural predicates, "Equality
                    // expansion"), so judge the sides field by field.
                    StructuralJudgment::Unknown => self
                        .declaration_expression_type(binary.left)
                        .map_or(StructuralJudgment::Unknown, |reference| {
                            self.judge_equation(
                                self.record_fields_term(left, reference, &mut Vec::new()),
                                self.record_fields_term(right, reference, &mut Vec::new()),
                                0,
                            )
                        }),
                    judgment => judgment,
                };
                if binary.operator == BinaryOperator::Equal {
                    equality
                } else {
                    match equality {
                        StructuralJudgment::Proven => StructuralJudgment::Refuted,
                        StructuralJudgment::Refuted => StructuralJudgment::Proven,
                        StructuralJudgment::Unknown => StructuralJudgment::Unknown,
                    }
                }
            }
            _ => self
                .judge_value_predicate(fact)
                .unwrap_or(StructuralJudgment::Unknown),
        }
    }

    fn is_case_membership(
        &self,
        expression: ExpressionHandle,
        comparison: &typed_trees::expression::TableBinaryExpression,
    ) -> bool {
        self.program
            .machine(self.machine_symbol)
            .is_some_and(|machine| {
                crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
                    self.program,
                    machine,
                    self.declaration_state
                        .is_valid()
                        .then(|| {
                            self.program
                                .state_in_machine(machine, self.declaration_state)
                        })
                        .flatten(),
                    expression,
                    comparison,
                )
            })
    }

    /// A callee's classifier is checked in its own parameter scope before its
    /// subject is substituted into the caller. Checking the uninstantiated
    /// expression against the caller would confuse identically named binders.
    pub(super) fn instantiated_case_judgment(
        &self,
        scope: SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        fact: ExpressionHandle,
        map: &[(String, StructuralTerm)],
    ) -> StructuralJudgment {
        self.instantiated_selected_case_judgment(scope, parameters, fact, map, &[])
    }

    pub(super) fn instantiated_selected_case_judgment(
        &self,
        scope: SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        fact: ExpressionHandle,
        map: &[(String, StructuralTerm)],
        machine_map: &[(
            SymbolHandle,
            TypeReferenceHandle,
            typed_trees::expression::StaticMachineArgument,
        )],
    ) -> StructuralJudgment {
        let Some(binary) = self.program.expression_table.binary_node(fact) else {
            return StructuralJudgment::Unknown;
        };
        if !crate::proof_contracts::bound_expression_meaning::has_exact_parameter_case_membership_meaning(
            self.program, parameters, fact, binary,
        ) {
            return StructuralJudgment::Unknown;
        }
        let Some((definition, variant)) =
            super::structural_terms::case_classifier(self.program, binary.right)
        else {
            return StructuralJudgment::Unknown;
        };
        let Some(subject) = super::structural_terms::structural_term(
            self.program,
            None,
            Some((scope, parameters)),
            binary.left,
            None,
        ) else {
            return StructuralJudgment::Unknown;
        };
        self.judge_case(
            Self::substitute_call_term(&subject, map, machine_map),
            definition.symbol,
            variant.symbol,
        )
    }

    /// Import a checked selected callee's tag guarantee under its own scope.
    /// The receiver supplies only post-call bindings and has already proved
    /// the invocation's premises. A tag never contributes a value equation.
    pub(super) fn instantiated_case_guarantees(
        &self,
        callee: &Machine,
        fact: ExpressionHandle,
        map: &[(String, StructuralTerm)],
        guarantees: &mut Vec<CaseGuarantee>,
    ) {
        if !self.has_exact_case_subjects() {
            return;
        }
        let Some(binary) = self.program.expression_table.binary_node(fact) else {
            return;
        };
        if binary.operator == BinaryOperator::And {
            self.instantiated_case_guarantees(callee, binary.left, map, guarantees);
            self.instantiated_case_guarantees(callee, binary.right, map, guarantees);
            return;
        }
        if !super::structural_terms::is_case_observation(self.program, fact)
            || !crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
                self.program,
                callee,
                None,
                fact,
                binary,
            )
        {
            return;
        }
        let Some((definition, variant)) =
            super::structural_terms::case_classifier(self.program, binary.right)
        else {
            return;
        };
        let Some(entry) = self.program.machine_states(callee).first() else {
            return;
        };
        let Some(subject) = super::structural_terms::structural_term(
            self.program,
            None,
            Some((callee.symbol, self.program.state_parameters(entry))),
            binary.left,
            None,
        ) else {
            return;
        };
        let Some(subject) = self.resolve_case_subject(Self::substitute_term(&subject, map), 0)
        else {
            return;
        };
        guarantees.push(CaseGuarantee {
            subject,
            data: definition.symbol,
            case: variant.symbol,
        });
    }

    pub(super) fn intake_case_guarantee(&mut self, guarantee: &CaseGuarantee) {
        Arc::make_mut(&mut self.case_facts).push((
            guarantee.subject.clone(),
            guarantee.data,
            guarantee.case,
        ));
    }

    fn judge_case(
        &self,
        subject: StructuralTerm,
        data_symbol: SymbolHandle,
        case_symbol: SymbolHandle,
    ) -> StructuralJudgment {
        let Some(subject) = self.resolve_case_subject(subject, 0) else {
            return StructuralJudgment::Unknown;
        };
        if let StructuralTerm::Constructor { data, case, .. } = &subject {
            // Legacy value terms retain nominal names. Rejoin only a unique
            // declaration; same-spelled foreign data is not this classifier.
            let Some(definition) = self.program.data_definition_by_symbol(data_symbol) else {
                return StructuralJudgment::Unknown;
            };
            if definition.name.as_str() != data
                || self
                    .program
                    .data_definitions()
                    .iter()
                    .filter(|candidate| candidate.name.as_str() == data)
                    .count()
                    != 1
            {
                return StructuralJudgment::Unknown;
            }
            let observed = self
                .program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == case =>
                    {
                        Some(variant.symbol)
                    }
                    _ => None,
                });
            return match observed {
                Some(observed) if observed == case_symbol => StructuralJudgment::Proven,
                Some(_) => StructuralJudgment::Refuted,
                None => StructuralJudgment::Unknown,
            };
        }
        // A captured value is one occurrence. Mathematical applications have
        // already rejoined their exact declaration and denotational closure in
        // resolve_case_subject; their tags never require body unfolding.
        if !matches!(
            subject,
            StructuralTerm::Variable(_)
                | StructuralTerm::BoundValue(_)
                | StructuralTerm::CapturedValue { .. }
                | StructuralTerm::Application { .. }
        ) {
            return StructuralJudgment::Unknown;
        }
        for (known_subject, known_data, known_case) in self.case_facts.iter() {
            if *known_data == data_symbol
                && self.resolve_case_subject(known_subject.clone(), 0).as_ref() == Some(&subject)
            {
                return if *known_case == case_symbol {
                    StructuralJudgment::Proven
                } else {
                    StructuralJudgment::Refuted
                };
            }
        }
        StructuralJudgment::Unknown
    }

    /// Only source premises and checked arm refinements feed this store. In
    /// particular, legacy citation results and already-unfolded value equations
    /// are not evidence for new tag judgments. No callable body is interpreted.
    pub(super) fn intake_case_equation(
        &mut self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
    ) {
        if depth >= 32 {
            return;
        }
        let (Some(left), Some(right)) = (
            self.resolve_case_subject(left, 0),
            self.resolve_case_subject(right, 0),
        ) else {
            return;
        };
        match (&left, &right) {
            (StructuralTerm::Variable(name), _)
                if left != right && !term_contains(&right, &left) =>
            {
                Arc::make_mut(&mut self.case_substitutions).push((name.clone(), right));
            }
            (_, StructuralTerm::Variable(name))
                if left != right && !term_contains(&left, &right) =>
            {
                Arc::make_mut(&mut self.case_substitutions).push((name.clone(), left));
            }
            (
                StructuralTerm::Constructor {
                    data: left_data,
                    case: left_case,
                    fields: left_fields,
                },
                StructuralTerm::Constructor {
                    data: right_data,
                    case: right_case,
                    fields: right_fields,
                },
            ) if left_data == right_data
                && left_case == right_case
                && left_fields.len() == right_fields.len() =>
            {
                for (name, left) in left_fields {
                    if let Some((_, right)) = right_fields.iter().find(|(field, _)| field == name) {
                        self.intake_case_equation(left.clone(), right.clone(), depth + 1);
                    }
                }
            }
            _ => {}
        }
    }

    fn resolve_case_subject(&self, term: StructuralTerm, depth: usize) -> Option<StructuralTerm> {
        if depth >= 32 {
            return None;
        }
        match term {
            StructuralTerm::ObservedValue {
                observed_type,
                value,
                descriptor,
            } => Some(StructuralTerm::ObservedValue {
                observed_type,
                descriptor,
                value: Box::new(self.resolve_case_subject(*value, depth + 1)?),
            }),
            StructuralTerm::Variable(ref name) => match self
                .case_substitutions
                .iter()
                .find(|(variable, _)| variable == name)
            {
                Some((_, replacement)) => self.resolve_case_subject(replacement.clone(), depth + 1),
                None => Some(term),
            },
            StructuralTerm::Constructor { data, case, fields } => {
                Some(StructuralTerm::Constructor {
                    data,
                    case,
                    fields: fields
                        .into_iter()
                        .map(|(name, field)| {
                            Some((name, self.resolve_case_subject(field, depth + 1)?))
                        })
                        .collect::<Option<Vec<_>>>()?,
                })
            }
            StructuralTerm::Integer(_)
            | StructuralTerm::Float(_)
            | StructuralTerm::BoundValue(_) => Some(term),
            StructuralTerm::CapturedValue { .. } => Some(term),
            application @ StructuralTerm::Application { .. } => {
                self.selected_application_case_subject(&application, depth)
            }
            StructuralTerm::CallProjection { .. }
            | StructuralTerm::BoundProjection { .. }
            | StructuralTerm::Opaque(_)
            | StructuralTerm::ScalarBinary { .. }
            | StructuralTerm::MathematicalBinary { .. }
            | StructuralTerm::IntegerEmbedding { .. }
            | StructuralTerm::CollectionMeasure { .. }
            | StructuralTerm::Projection { .. }
            | StructuralTerm::IndexedValue { .. } => None,
        }
    }

    /// Judge one resolved structural equation: identical terms prove,
    /// same-case constructors decompose pairwise (all fields prove =>
    /// proven, any refutes => refuted), distinct cases refute. A stuck
    /// equation gets the REARRANGE tier before standing down: under a ring
    /// license, both sides flatten to addend MULTISETS over the licensed op
    /// (the commutativity + associativity closure the carrier's conformance
    /// proved) -- equal multisets prove; unequal ones stay Unknown (atoms may
    /// alias, so rearrangement never refutes).
    pub(super) fn judge_equation(
        &self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
    ) -> StructuralJudgment {
        if self.case_facts_only || depth >= 32 {
            return StructuralJudgment::Unknown;
        }
        if super::structural_terms::term_is_unobserved_descriptor(self.program, &left)
            || super::structural_terms::term_is_unobserved_descriptor(self.program, &right)
        {
            return StructuralJudgment::Unknown;
        }
        let left = left.into_semantic_value();
        let right = right.into_semantic_value();
        if left == right {
            return StructuralJudgment::Proven;
        }
        if self.disequalities.iter().any(|(known_left, known_right)| {
            let known_left = self
                .resolve_at(known_left.clone(), depth + 1)
                .into_semantic_value();
            let known_right = self
                .resolve_at(known_right.clone(), depth + 1)
                .into_semantic_value();
            (known_left == left && known_right == right)
                || (known_left == right && known_right == left)
        }) {
            return StructuralJudgment::Refuted;
        }
        if matches!(
            (&left, &right),
            (StructuralTerm::Integer(_), StructuralTerm::Integer(_))
        ) {
            return StructuralJudgment::Refuted;
        }
        // Two float literals compare on VALUE, not spelling: the text equality
        // above already proved identical spellings; here `0.0` and `0.00` are
        // the same binary64 number and `0.0`/`1.5` are not. A NaN reading
        // declines rather than decide IEEE's false-equality.
        if let (StructuralTerm::Float(left), StructuralTerm::Float(right)) = (&left, &right) {
            let (left, right) = (left.value_f64(), right.value_f64());
            if left.is_nan() || right.is_nan() {
                return StructuralJudgment::Unknown;
            }
            return if left == right {
                StructuralJudgment::Proven
            } else {
                StructuralJudgment::Refuted
            };
        }
        let (
            StructuralTerm::Constructor {
                data: data_l,
                case: case_l,
                fields: fields_l,
            },
            StructuralTerm::Constructor {
                data: data_r,
                case: case_r,
                fields: fields_r,
            },
        ) = (&left, &right)
        else {
            // RECORD ETA (product extensionality): a record literal that
            // rebuilds EVERY declared field of a variable from that same
            // variable (`IntPair { neg: a.neg, pos: a.pos } == a`) IS the
            // variable -- the shape identity lemmas reduce to. Field values
            // must be the variable's own field reads by name; a permuted
            // rebuild (neg: a.pos) does NOT match.
            if self.record_eta_matches(&left, &right) || self.record_eta_matches(&right, &left) {
                return StructuralJudgment::Proven;
            }
            if self.ring_rearranged_equal(&left, &right) {
                return StructuralJudgment::Proven;
            }
            return StructuralJudgment::Unknown;
        };
        if data_l != data_r {
            return StructuralJudgment::Unknown;
        }
        if case_l != case_r {
            return StructuralJudgment::Refuted;
        }
        // Constructor equality compares complete values, not a left-side
        // pattern. In particular a case-tag term with no fields cannot prove
        // equality to a constructed value with common fields or payload.
        if fields_l.len() != fields_r.len() {
            return StructuralJudgment::Unknown;
        }
        let mut verdict = StructuralJudgment::Proven;
        for (name_l, value_l) in fields_l {
            let Some((_, value_r)) = fields_r.iter().find(|(name_r, _)| name_r == name_l) else {
                verdict = StructuralJudgment::Unknown;
                continue;
            };
            match self.judge_equation(value_l.clone(), value_r.clone(), depth + 1) {
                StructuralJudgment::Proven => {}
                StructuralJudgment::Refuted => return StructuralJudgment::Refuted,
                StructuralJudgment::Unknown => verdict = StructuralJudgment::Unknown,
            }
        }
        verdict
    }

    /// `term` of type `reference` expanded to its record's field terms, each
    /// resolved under the hypotheses and expanded again by its own declared
    /// type. Sums, recursive records and unreadable projections stay whole.
    fn record_fields_term(
        &self,
        term: StructuralTerm,
        reference: typed_trees::types::TypeReferenceHandle,
        active: &mut Vec<typed_trees::types::TypeReferenceHandle>,
    ) -> StructuralTerm {
        if super::structural_terms::owned_observation_type(self.program, &term).is_some()
            || active.contains(&reference)
            || active.len() >= 32
        {
            return term;
        }
        let Some(data) =
            crate::value_custody::places::data_definition_for_type(self.program, reference)
        else {
            return term;
        };
        let members = self.program.data_members(data);
        if members
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            return term;
        }
        let term = term.into_semantic_value();
        let mut fields = Vec::new();
        active.push(reference);
        for field in self.program.data_member_fields(members) {
            let name = field.name.as_str();
            let value = match &term {
                StructuralTerm::Constructor {
                    data: constructed,
                    case,
                    fields,
                } if constructed == data.name.as_str() && case.is_empty() => fields
                    .iter()
                    .find_map(|(field, value)| (field == name).then(|| value.clone())),
                StructuralTerm::Constructor { .. } => None,
                _ => super::observed_projections::project_field(term.clone(), name)
                    .map(|value| self.resolve(value)),
            };
            let Some(value) = value else {
                active.pop();
                return term;
            };
            fields.push((
                name.to_owned(),
                self.record_fields_term(value, field.type_reference, active),
            ));
        }
        active.pop();
        if fields.is_empty() {
            return term;
        }
        fields.sort_by(|left, right| left.0.cmp(&right.0));
        StructuralTerm::Constructor {
            data: data.name.as_str().to_owned(),
            case: String::new(),
            fields,
        }
    }

    /// RECORD ETA: does `constructor` rebuild every declared field of the
    /// plain-record variable `variable` from that variable's own field
    /// reads? All fields must be present, matched BY NAME to `{v}.{field}`
    /// opaques -- a permutation or a partial rebuild does not match.
    fn record_eta_matches(&self, constructor: &StructuralTerm, variable: &StructuralTerm) -> bool {
        let StructuralTerm::Constructor { data, case, fields } = constructor else {
            return false;
        };
        if !case.is_empty() {
            return false;
        }
        let StructuralTerm::Variable(name) = variable else {
            return false;
        };
        let Some(definition) = self.program.data_definition_by_name_str(data.as_str()) else {
            return false;
        };
        let declared: Vec<&str> = self
            .program
            .data_members(definition)
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field) => Some(field.name.as_str()),
                typed_trees::data::DataMember::Variant(_) => None,
            })
            .collect();
        if declared.is_empty() || declared.len() != fields.len() {
            return false;
        }
        declared.iter().all(|field_name| {
            fields.iter().any(|(field, value)| {
                field == field_name
                    && matches!(
                        self.resolve(value.clone()),
                        StructuralTerm::Opaque(opaque)
                            if opaque == format!("{name}.{field_name}")
                    )
            })
        })
    }

    /// The rearrange tier's comparison: for each ring license whose op
    /// appears in the equation, flatten both sides into addend multisets
    /// (nested applications of the licensed op associate away; everything
    /// else is a structural atom) and compare. At least two
    /// addends must appear -- a single atom has nothing to rearrange.
    fn ring_rearranged_equal(&self, left: &StructuralTerm, right: &StructuralTerm) -> bool {
        for license in &self.ring_licenses {
            let op = &license.add_machine;
            if !term_uses_application(left, op) && !term_uses_application(right, op) {
                continue;
            }
            let mut left_addends = Vec::new();
            additive_addends(left, op, &mut left_addends);
            let mut right_addends = Vec::new();
            additive_addends(right, op, &mut right_addends);
            if left_addends.len() < 2 {
                continue;
            }
            left_addends.sort_by_key(|term| format!("{term:?}"));
            right_addends.sort_by_key(|term| format!("{term:?}"));
            if left_addends.len() == right_addends.len() && left_addends == right_addends {
                return true;
            }
            // HYPOTHESIS EXCHANGE (bounded, depth 2): a requires / citation /
            // IH equation whose sides flatten over this SAME licensed op
            // licenses swapping that sub-multiset of addends -- sum(left) ==
            // sum(left - from + to) because sum(from) == sum(to) is the
            // hypothesis and the op's comm+assoc closure is exactly what the
            // license's conformance proved. This is what makes QUOTIENT
            // lemmas provable: congruence needs ONE exchange (a.pos + a2.neg
            // exchanges inside a.pos + b.pos + a2.neg + b.neg), transitivity
            // needs TWO (h1 then h2 inside the cancellation citation's
            // requires). Whole-term matches were already rewritten during
            // resolve; this reaches the sub-multisets the rewriter cannot
            // see. Frontier-capped BFS -- over-refusal past the cap, never
            // unsound.
            let mut rewrite_addends: Vec<(Vec<StructuralTerm>, Vec<StructuralTerm>)> = Vec::new();
            for (pattern, replacement) in self.rewrites.iter() {
                for (from, to) in [(pattern, replacement), (replacement, pattern)] {
                    let mut from_addends = Vec::new();
                    additive_addends(from, op, &mut from_addends);
                    let mut to_addends = Vec::new();
                    additive_addends(to, op, &mut to_addends);
                    from_addends.sort_by_key(|term| format!("{term:?}"));
                    rewrite_addends.push((from_addends, to_addends));
                }
            }
            let mut frontier: Vec<Vec<StructuralTerm>> = vec![left_addends.clone()];
            for _depth in 0..2 {
                let mut next: Vec<Vec<StructuralTerm>> = Vec::new();
                for current in &frontier {
                    for (from_addends, to_addends) in &rewrite_addends {
                        let Some(mut candidate) = sorted_multiset_subtract(current, from_addends)
                        else {
                            continue;
                        };
                        candidate.extend(to_addends.iter().cloned());
                        candidate.sort_by_key(|term| format!("{term:?}"));
                        if candidate == right_addends {
                            return true;
                        }
                        if next.len() < 64 {
                            next.push(candidate);
                        }
                    }
                }
                frontier = next;
                if frontier.is_empty() {
                    break;
                }
            }
        }
        // Tier-2 FULL POLYNOMIAL: under a paired license, both sides
        // normalize by distributing the licensed mul through the licensed
        // add into a multiset of monomials (each a sorted factor multiset).
        // The distributivity law is CONFORMED (machine-checked), so the
        // normal form is exactly what the carrier proved.
        for license in &self.semiring_licenses {
            if !term_uses_application(left, &license.mul_machine)
                && !term_uses_application(right, &license.mul_machine)
            {
                continue;
            }
            let (Some(mut left_poly), Some(mut right_poly)) = (
                polynomial_normal_form(left, license),
                polynomial_normal_form(right, license),
            ) else {
                continue;
            };
            left_poly.sort_by_key(|term| format!("{term:?}"));
            right_poly.sort_by_key(|term| format!("{term:?}"));
            if left_poly == right_poly {
                return true;
            }
            // SCALED-HYPOTHESIS EXCHANGE (tier-2 twin of the addend
            // exchange): a hypothesis equation polynomial-normalizes to a
            // monomial-multiset pair (hl, hr), and multiplying BOTH sides by
            // any monomial factor m keeps it an equation (the semiring's
            // conformed distributivity is exactly that license), so hl*m
            // exchanges for hr*m inside the goal's monomials. Factors are
            // drawn from the goal's own atoms (plus unscaled) -- this is
            // what proves mul-CONGRUENCE over a quotient: the cross-sum
            // hypothesis scaled by b.pos and by b.neg equalizes the product
            // components in two exchanges. Depth-2 frontier-capped BFS.
            let mut atoms: Vec<StructuralTerm> = left_poly
                .iter()
                .chain(right_poly.iter())
                .flatten()
                .cloned()
                .collect();
            atoms.sort_by_key(|term| format!("{term:?}"));
            atoms.dedup();
            let mut scales: Vec<Vec<StructuralTerm>> = vec![Vec::new()];
            scales.extend(atoms.into_iter().map(|atom| vec![atom]));
            let mut hypothesis_polys: Vec<(Vec<Vec<StructuralTerm>>, Vec<Vec<StructuralTerm>>)> =
                Vec::new();
            for (pattern, replacement) in self.rewrites.iter() {
                if let (Some(mut hl), Some(mut hr)) = (
                    polynomial_normal_form(pattern, license),
                    polynomial_normal_form(replacement, license),
                ) {
                    hl.sort_by_key(|term| format!("{term:?}"));
                    hr.sort_by_key(|term| format!("{term:?}"));
                    hypothesis_polys.push((hl, hr));
                }
            }
            let scaled = |poly: &[Vec<StructuralTerm>],
                          scale: &[StructuralTerm]|
             -> Vec<Vec<StructuralTerm>> {
                poly.iter()
                    .map(|monomial| {
                        let mut product = monomial.clone();
                        product.extend(scale.iter().cloned());
                        product.sort_by_key(|term| format!("{term:?}"));
                        product
                    })
                    .collect()
            };
            let mut frontier: Vec<Vec<Vec<StructuralTerm>>> = vec![left_poly.clone()];
            for _depth in 0..2 {
                let mut next: Vec<Vec<Vec<StructuralTerm>>> = Vec::new();
                for current in &frontier {
                    for (hypothesis_left, hypothesis_right) in &hypothesis_polys {
                        for (from, to) in [
                            (hypothesis_left, hypothesis_right),
                            (hypothesis_right, hypothesis_left),
                        ] {
                            for scale in &scales {
                                let from_scaled = scaled(from, scale);
                                let Some(mut candidate) =
                                    sorted_multiset_subtract(current, &from_scaled)
                                else {
                                    continue;
                                };
                                candidate.extend(scaled(to, scale));
                                candidate.sort_by_key(|term| format!("{term:?}"));
                                if candidate == right_poly {
                                    return true;
                                }
                                if next.len() < 64 {
                                    next.push(candidate);
                                }
                            }
                        }
                    }
                }
                frontier = next;
                if frontier.is_empty() {
                    break;
                }
            }
        }
        false
    }
}
