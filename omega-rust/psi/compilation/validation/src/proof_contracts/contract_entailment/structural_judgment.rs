//! Contract facts over exact completed value meanings.
//!
//! Transparent mathematical predicates enter the same observed-value reader
//! as inline predicates after the checked calculus substitutes their captures.
//! Predicate classification must therefore include selected IEEE leaves, not
//! just integer relations. Their truth can compose through conjunction, while
//! each leaf retains its own carrier, format and comparison laws: IEEE equality
//! is never a value-identity rewrite, and a false order does not exclude NaN.

use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Machine, StatementNode, SymbolHandle,
    TransitionGuardNode, TransitionTargetNode, is_arm_pattern_marker, structural_call_machine_name,
    term_contains,
};
use crate::frozen_program::expression_table;
use crate::machine_calls::calls::CallFrameResolver;
use std::sync::Arc;
use symbols::JoinedPath;
use typed_trees::data::TypeParameterKind;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;
use typed_trees::types::TypeReferenceHandle;

mod algebraic_identity;
mod cases;
pub(crate) use cases::CaseGuaranteeEligibility;
mod conditional_guarantees;
mod constructor_identity;
pub(super) use constructor_identity::ConstructorIdentity;
mod data_identity;
mod domain_premises;
mod equations;
mod identity_elimination;
pub(in crate::proof_contracts::contract_entailment) use domain_premises::data_application_static_bindings;
mod integer_predicates;
pub(super) use integer_predicates::{CapturedIntegerInput, CapturedIntegerRank, FormedIntegerRank};
mod identity_normalization;
mod induction_values;
mod invocation_entry;
mod licenses;
mod logical_resolution;
mod mathematical_predicates;
pub(super) mod normal_returns;
mod resolution_inputs;
mod resolution_queries;
mod returned_fields;
mod runtime_induction;
pub(super) mod selected_contracts;
mod storage_effects;
pub(crate) use storage_effects::stored_encoding_preserves_field;
mod traversal_inputs;
pub(in crate::proof_contracts::contract_entailment) use traversal_inputs::StructuralTraversalInputs;
mod value_correspondence;
mod value_predicates;

pub(crate) use licenses::{
    EntryMachines, LicenseCandidates, declared_index_algebra_laws, literal_index_identities,
};
use licenses::{RingLicense, SemiringLicense, fold_scalar_binary};

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
    selections: &[StaticMachineArgument],
    frames: Option<&crate::CallFrameResolver<'_>>,
) -> Result<(), &'static str> {
    if !crate::machine_calls::contract_dependencies::independent_application(
        program, owner, target, selections,
    ) {
        return Err("the selected application depends on the active declaration");
    }
    if let Some(operator) = program.operator_definition_by_symbol(target) {
        if !selections.is_empty() {
            return Err("a closed catalog operation has no static selection telescope");
        }
        // The exact sealed catalog already supplies this operation's total
        // value meaning. A bodyless signature or an admitted runtime boundary
        // alone cannot close this leaf, and a failed selection cannot fall
        // through to a different checked machine.
        return crate::machine_calls::denotational_calls::selected_operator_has_closed_value_denotation(
            program, operator,
        )
        .then_some(())
        .ok_or("the selected operator has no exact closed value denotation");
    }
    let machine = super::structural_terms::selected_application_machine(program, target)
        .ok_or("the application has no exact selected entry")?;
    let state = program
        .machine_states(machine)
        .first()
        .ok_or("the selected machine has no entry state")?;
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

impl CaseGuarantee {
    pub(super) fn same_resolution_payload(&self, other: &Self) -> bool {
        self.data == other.data
            && self.case == other.case
            && self.subject.same_resolution_payload(&other.subject)
    }
}

pub(crate) mod owned_snapshots;

#[derive(Clone, PartialEq)]
struct MembershipGuarantee {
    subject: StructuralTerm,
    domain: SymbolHandle,
    semantic_domain: language_semantics::SemanticDomainId,
}

#[derive(Clone)]
pub(crate) enum StructuralTerm {
    /// A formed core Id proposition. Its carrier is not an executable
    /// equality selection, and no truth follows from constructing this node.
    Identity {
        carrier: typed_trees::type_identity::TypeIdentityId,
        left: Box<StructuralTerm>,
        right: Box<StructuralTerm>,
    },
    /// A value of the closed canonical content algebra. The source literal
    /// former's fields are not projections of this mathematical value.
    ContentValue {
        algebra: language_semantics::content::ContentAlgebraIdentity,
        value: Box<StructuralTerm>,
    },
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
        // A statement call has no result expression. Its actual statement
        // handle separates equal-counter captures on different branch arms.
        statement: arena::Handle<typed_trees::statement::StatementNode>,
        callable: SymbolHandle,
        occurrence: usize,
        result_type: TypeReferenceHandle,
        // Immutable producer evidence for this exact completed observation.
        // Snapshot comparisons include this evidence so joins cannot retain
        // one predecessor's bounds. It never authorizes a storage reread.
        integer_range: Option<Arc<checked_trees::IntegerRangeRecipe>>,
    },
    /// A field path on the same completed value, never a callee-local spelling.
    BoundProjection {
        subject: SymbolHandle,
        path: String,
    },
    /// The same captured scalar bits viewed in another admitted carrier.
    /// This records representation identity, never IEEE numeric equality.
    /// Construction retains checked shared-read custody; reversing the view
    /// recovers the original value without quieting or canonicalizing NaNs.
    ScalarRepresentation {
        source: PrimitiveType,
        target: PrimitiveType,
        value: Box<StructuralTerm>,
    },
    /// Exact literal value, independent of radix or authored spelling.
    Integer(numerics::bignum::BigInt),
    /// A complete uniform array value, represented once regardless of extent.
    /// Empty arrays have no element; nonempty arrays retain the independently
    /// established element value. This conveys no storage or borrow authority.
    RepeatedArray {
        length: u64,
        element: Option<Box<StructuralTerm>>,
    },
    /// Exact mathematical observation of one already evaluated integer or
    /// address. Its declared carrier supplies bounds, never a new value or
    /// permission to reread the source storage.
    IntegerEmbedding {
        reference: TypeReferenceHandle,
        value: Box<StructuralTerm>,
    },
    /// Builtin unbounded integer operations and logical connectives, distinct
    /// from finite runtime arithmetic and selected IEEE comparison leaves.
    /// Children retain their own captured identities and scalar meanings.
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
    /// terms retain their exact complete carrier and selected case. Explicit
    /// Id can compare them without selecting executable equality.
    Constructor {
        identity: ConstructorIdentity,
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
        selections: Vec<StaticMachineArgument>,
        machine: String,
        arguments: Vec<StructuralTerm>,
    },
    /// One validated direct plain-record projection from an exact checked
    /// call. Symbol identities prevent overloaded textual names from
    /// collapsing; `machine` retains the complete static application for
    /// diagnostics and canonical display.
    CallProjection {
        selections: Vec<StaticMachineArgument>,
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
        meaning: [(PrimitiveType, numerics::arithmetic::ArithmeticDomain); 2],
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
pub(super) use term_identity::same_static_selection_payload;

impl StructuralTerm {
    /// Every contributing completed value must carry a recipe. The result
    /// describes the union of those values, never one chosen predecessor.
    pub(super) fn joined_integer_ranges<'a>(
        values: impl IntoIterator<Item = Option<&'a StructuralTerm>>,
    ) -> Option<Arc<checked_trees::IntegerRangeRecipe>> {
        let mut joined: Option<checked_trees::IntegerRangeRecipe> = None;
        for value in values {
            let StructuralTerm::CapturedValue {
                integer_range: Some(recipe),
                ..
            } = value?.semantic_value()
            else {
                return None;
            };
            joined =
                Some(joined.map_or_else(|| recipe.as_ref().clone(), |prior| prior.union(recipe)));
        }
        joined.map(Arc::new)
    }

    pub(super) fn is_checked_predicate(&self) -> bool {
        if matches!(self.semantic_value(), Self::Identity { .. })
            || value_predicates::boolean_value(self.semantic_value()).is_some()
        {
            return true;
        }
        if let Self::MathematicalBinary {
            operator: BinaryOperator::And | BinaryOperator::Or,
            left,
            right,
        } = self.semantic_value()
        {
            return left.is_checked_predicate() && right.is_checked_predicate();
        }
        matches!(self.semantic_value(),
            Self::MathematicalBinary { operator, .. } | Self::ScalarBinary { operator, .. }
            if operator.is_comparison())
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

    pub(super) fn boolean(value: bool) -> Self {
        Self::Constructor {
            identity: ConstructorIdentity::Boolean(value),
            data: "bool".to_owned(),
            case: value.to_string(),
            fields: Vec::new(),
        }
    }

    pub(super) fn boolean_constructor(&self) -> Option<bool> {
        let Self::Constructor {
            identity, fields, ..
        } = self
        else {
            return None;
        };
        fields.is_empty().then(|| identity.boolean()).flatten()
    }

    pub(super) fn boolean_value(&self) -> Option<bool> {
        self.semantic_value().boolean_constructor()
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

    /// Rebuild `self` with `map` applied over every immediate child — the
    /// mutable counterpart of [`Self::visit_children`], and its single owner.
    /// A walk that transforms subterms states only what it does differently
    /// (a substitution, a constant unfolding, a slot binding) and sends every
    /// other variant through this method, so a new child edge reaches every
    /// walk at once instead of being re-listed in each copy.
    ///
    /// A `CollectionMeasure` re-takes its subject's collection descriptor,
    /// which is the invariant that variant carries: its subject is metadata
    /// about a completed collection, never the referent's contents.
    pub(super) fn map_children(self, mut map: impl FnMut(Self) -> Self) -> Self {
        match self {
            StructuralTerm::ContentValue { algebra, value } => StructuralTerm::ContentValue {
                algebra,
                value: Box::new(map(*value)),
            },
            StructuralTerm::Identity {
                carrier,
                left,
                right,
            } => StructuralTerm::Identity {
                carrier,
                left: Box::new(map(*left)),
                right: Box::new(map(*right)),
            },
            StructuralTerm::RepeatedArray { length, element } => StructuralTerm::RepeatedArray {
                length,
                element: element.map(|element| Box::new(map(*element))),
            },
            StructuralTerm::ScalarRepresentation {
                source,
                target,
                value,
            } => StructuralTerm::ScalarRepresentation {
                source,
                target,
                value: Box::new(map(*value)),
            },
            StructuralTerm::IntegerEmbedding { reference, value } => {
                StructuralTerm::IntegerEmbedding {
                    reference,
                    value: Box::new(map(*value)),
                }
            }
            StructuralTerm::MathematicalBinary {
                operator,
                left,
                right,
            } => StructuralTerm::MathematicalBinary {
                operator,
                left: Box::new(map(*left)),
                right: Box::new(map(*right)),
            },
            StructuralTerm::ObservedValue {
                observed_type,
                value,
                descriptor,
            } => StructuralTerm::ObservedValue {
                observed_type,
                value: Box::new(map(*value)),
                descriptor: descriptor.map(|descriptor| Box::new(map(*descriptor))),
            },
            StructuralTerm::Constructor {
                identity,
                data,
                case,
                fields,
            } => StructuralTerm::Constructor {
                identity,
                data,
                case,
                fields: fields
                    .into_iter()
                    .map(|(name, value)| (name, map(value)))
                    .collect(),
            },
            StructuralTerm::ScalarBinary {
                references,
                operator,
                meaning,
                left,
                right,
            } => StructuralTerm::ScalarBinary {
                references,
                operator,
                meaning,
                left: Box::new(map(*left)),
                right: Box::new(map(*right)),
            },
            StructuralTerm::IndexedValue {
                element_type,
                element_identity,
                collection,
                index,
            } => StructuralTerm::IndexedValue {
                element_type,
                element_identity,
                collection: Box::new(map(*collection)),
                index: Box::new(map(*index)),
            },
            StructuralTerm::CollectionMeasure {
                measure,
                collection,
            } => StructuralTerm::CollectionMeasure {
                measure,
                collection: Box::new(map(*collection).into_collection_descriptor()),
            },
            StructuralTerm::Projection { subject, path } => StructuralTerm::Projection {
                subject: Box::new(map(*subject)),
                path,
            },
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
            leaf => leaf,
        }
    }
}

/// Static observations supplied to one expression walk. Selected computation
/// carries its actual machine bindings; only a body-proof entry supplies the
/// declaring telescope's symbolic value subjects. Recursive child walks retain
/// that exact context, while unfolding another application starts a new one.
#[derive(Clone, Copy)]
struct StaticObservations<'scope> {
    machine_bindings: &'scope [(SymbolHandle, String, StaticMachineArgument)],
    body_parameters: &'scope [typed_trees::data::TypeParameter],
}

/// Admission belongs to one completed owned value and its producing occurrence.
/// A source name or equal reconstructed value cannot recover this receipt.
#[derive(Clone)]
struct CompletedNominalValue {
    anchor: ExpressionHandle,
    reference: TypeReferenceHandle,
    value: StructuralTerm,
    observed_value: StructuralTerm,
}

impl CompletedNominalValue {
    fn same_admission(&self, other: &Self) -> bool {
        self.anchor == other.anchor
            && self.reference == other.reference
            && self
                .observed_value
                .observation_identity_key()
                .is_some_and(|key| {
                    other.observed_value.observation_identity_key().as_ref() == Some(&key)
                })
            && self
                .value
                .observation_identity_key()
                .is_some_and(|key| other.value.observation_identity_key().as_ref() == Some(&key))
    }
}

/// Internal tree-walk context, not a source feature or a cached body request.
/// Application body resolution itself remains the ordinary general operation.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ResolutionPurpose {
    General,
    IntegerProjection,
}

pub(super) struct StructuralJudge<'program> {
    program: &'program dyn crate::CheckingInput,
    entry_machines: std::sync::Arc<EntryMachines>,
    machine_symbol: SymbolHandle,
    // Runtime prefix facts belong to the current state invocation. Keep its
    // declaration frame separate from the machine's dependency identity.
    declaration_state: SymbolHandle,
    // Abstract owners supply their actual telescope, without a synthetic machine.
    declaration_parameters: Arc<Vec<StateParameter>>,
    // This body's declared telescope supplies symbolic value observations,
    // never a missing selected callee argument or an assumed constant value.
    body_type_parameters: &'program [typed_trees::data::TypeParameter],
    premise_facts: Vec<ExpressionHandle>,
    // Term-bearing fact vectors are Arc-shared so the per-leaf judge clone
    // (one per recognized case-arm path) costs refcount bumps instead of
    // deep StructuralTerm copies; writers go through Arc::make_mut and pay
    // one copy only for the field they actually change.
    normal_results: Arc<Vec<(ExpressionHandle, StructuralTerm)>>,
    returned_value_source: Option<normal_returns::CompletedValueSource<'program>>,
    completed_nominal_values: Arc<Vec<CompletedNominalValue>>,
    // Declaration-level field correspondence never recursively re-enters an
    // active body. This belongs to the query, including all cloned path judges.
    returned_field_owners: Arc<Vec<SymbolHandle>>,
    // Active executed bodies bound ordinary calls as well as terminal transfers.
    // A recurrent call consumes its checked contract instead of unfolding again.
    body_invocations: Arc<Vec<SymbolHandle>>,
    runtime_induction: Arc<runtime_induction::RuntimeInduction>,
    // Exact value applications licensed by conditional induction on this path.
    induction_values: Arc<Vec<StructuralTerm>>,
    active_application: Option<Arc<resolution_queries::ApplicationResolution<'program>>>,
    resolution_observations: Option<Arc<resolution_queries::ResolutionObservations<'program>>>,
    normal_memberships: Arc<Vec<MembershipGuarantee>>,
    pub(super) normal_content_guarantees:
        Arc<Vec<super::content_conservation::ObservedContentEquation>>,
    // Bindings an opaque write retired in a judge no storage graph reaches.
    invalidated_storage: Arc<Vec<SymbolHandle>>,
    // Position in the storage graph after the last observed effect; None when
    // the program has no graph or this judge walks no runtime body.
    storage_version: Option<storage_effects::StorageVersion>,
    // Typed storage observations retain live values and detached entry snapshots.
    stored_values: Arc<storage_effects::StoredValues>,
    pub(super) capture_sequence: usize,
    storage_write_sequence: usize,
    pub(super) normal_guarantees: Arc<Vec<(StructuralTerm, StructuralTerm)>>,
    // Keep scalar promises, completed-call results and mathematical data
    // identity distinct when reusing equations. Executable branch comparisons
    // remain ordinary normal-path proof facts.
    captured_result_equations: Arc<Vec<normal_returns::CapturedResultEquation>>,
    conditional_guarantees: Arc<Vec<conditional_guarantees::ConditionalGuarantees>>,
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
    case_facts: Arc<Vec<cases::CaseFact>>,
    // Keep raw, application-free equations separate from the legacy value
    // normalizer, which can unfold a name-selected call before storing it.
    // Copying its substitutions would already have lost callable identity.
    case_substitutions: Arc<Vec<(String, StructuralTerm)>>,
    pub(super) hypotheses_contradictory: bool,
    ring_licenses: Vec<RingLicense>,
    /// Tier-2: paired add/mul licenses with a conformed distributivity law.
    semiring_licenses: Vec<SemiringLicense>,
    /// Everything this judge's queries write while answering. Clones made
    /// inside one query share it; an independent query replaces it whole.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) query_state:
        QueryState<'program>,
}

/// The state one independent structural query writes while it answers, kept
/// in one value so that a judge for a new query gets fresh state by replacing
/// this value rather than by resetting a hand-kept list of fields.
///
/// Every clone of a judge made inside a query (one per case-arm path, per
/// resolution step) shares this by design: that sharing is what makes these
/// the query's memos. Nothing here may carry answers from one independent
/// query to another, so `StructuralJudge::with_fresh_query_state` gives each
/// one `QueryState::fresh()`.
#[derive(Clone)]
pub(in crate::proof_contracts::contract_entailment::structural_judgment) struct QueryState<'program>
{
    /// The returned-field dependency trace of the query that installed it.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) returned_field_trace:
        Option<Arc<std::sync::Mutex<returned_fields::ReturnedFieldTrace>>>,
    completed_resolutions:
        Option<Arc<std::sync::Mutex<resolution_queries::CompletedResolutions<'program>>>>,
    /// Frame resolution is program-pure: one cell, built on first use, serves
    /// every clone in the query instead of one build per probe site.
    frame_resolver: Arc<std::sync::OnceLock<Option<CallFrameResolver<'program>>>>,
    prepared_integer_premises_cache:
        Arc<std::sync::Mutex<integer_predicates::PreparedIntegerPremisesCache<'program>>>,
}

impl QueryState<'_> {
    /// The state a newly built judge's first query starts with.
    fn fresh() -> Self {
        Self {
            returned_field_trace: None,
            completed_resolutions: Some(Arc::new(std::sync::Mutex::new(Default::default()))),
            frame_resolver: Arc::new(std::sync::OnceLock::new()),
            prepared_integer_premises_cache: Arc::new(std::sync::Mutex::new(
                integer_predicates::PreparedIntegerPremisesCache::default(),
            )),
        }
    }
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
            returned_value_source: self.returned_value_source.clone(),
            completed_nominal_values: Arc::clone(&self.completed_nominal_values),
            returned_field_owners: Arc::clone(&self.returned_field_owners),
            body_invocations: Arc::clone(&self.body_invocations),
            runtime_induction: Arc::clone(&self.runtime_induction),
            induction_values: Arc::clone(&self.induction_values),
            active_application: self.active_application.clone(),
            resolution_observations: self.resolution_observations.clone(),
            normal_memberships: Arc::clone(&self.normal_memberships),
            normal_content_guarantees: Arc::clone(&self.normal_content_guarantees),
            invalidated_storage: Arc::clone(&self.invalidated_storage),
            storage_version: self.storage_version,
            stored_values: Arc::clone(&self.stored_values),
            capture_sequence: self.capture_sequence,
            storage_write_sequence: self.storage_write_sequence,
            normal_guarantees: Arc::clone(&self.normal_guarantees),
            captured_result_equations: Arc::clone(&self.captured_result_equations),
            conditional_guarantees: Arc::clone(&self.conditional_guarantees),
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
            query_state: self.query_state.clone(),
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
            && self.captured_result_equations.len() == base.captured_result_equations.len()
            && Arc::ptr_eq(&self.induction_values, &base.induction_values)
            && Arc::ptr_eq(&self.conditional_guarantees, &base.conditional_guarantees)
            && self.returned_field_owners.len() == base.returned_field_owners.len()
            && self.normal_memberships.len() == base.normal_memberships.len()
            && (Arc::ptr_eq(
                &self.completed_nominal_values,
                &base.completed_nominal_values,
            ) || (self.completed_nominal_values.len() == base.completed_nominal_values.len()
                && self
                    .completed_nominal_values
                    .iter()
                    .zip(base.completed_nominal_values.iter())
                    .all(|(current, original)| current.same_admission(original))))
            && self.normal_content_guarantees.len() == base.normal_content_guarantees.len()
            && self.case_facts.len() == base.case_facts.len()
    }

    /// True when `self` has added no non-boolean rewrites, non-boolean
    /// substitutions, or integer ordering/equality guarantees since `base`.
    pub(super) fn has_no_new_non_boolean_facts_since(&self, base: &Self) -> bool {
        if !Arc::ptr_eq(&self.conditional_guarantees, &base.conditional_guarantees)
            || !Arc::ptr_eq(&self.induction_values, &base.induction_values)
        {
            return false;
        }
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
                .captured_result_equations
                .get(base.captured_result_equations.len()..)
                .is_some_and(|added| added.iter().all(|equation| !equation.is_data_identity()))
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
        rejoined.induction_values = induction_values::common_induction_values(outcomes);
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
                if outcomes[1..].iter().all(|judge| {
                    judge
                        .stored_values
                        .values_at(&entry.coordinate)
                        .any(|stored| stored == entry)
                }) {
                    intersected.push(entry.clone());
                    continue;
                }
                let mut matched_read = entry.completed_read;
                let mut matched_case = entry.selected_case;
                let mut matched_current = entry.current.clone();
                let mut matched_entry = entry.entry.clone();
                let mut matched_backing = entry.backing.clone();
                // Equal values joined from several paths hold where every path
                // reaches one of their establishing versions.
                let mut matched_anchors = entry.current_anchors.clone();
                let mut all_have_coord = true;
                for judge in &outcomes[1..] {
                    let Some(other) = judge.stored_values.value_at(&entry.coordinate) else {
                        all_have_coord = false;
                        break;
                    };
                    if matched_backing != other.backing {
                        matched_backing = None;
                    }
                    if other.anchor_state != entry.anchor_state {
                        matched_anchors.clear();
                    } else {
                        for anchor in &other.current_anchors {
                            if !matched_anchors.contains(anchor) {
                                matched_anchors.push(*anchor);
                            }
                        }
                    }
                    if matched_case != other.selected_case {
                        matched_case = None;
                    }
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
                                statement: s1,
                                callable: c1,
                                result_type: t1,
                                integer_range: r1,
                                ..
                            }),
                            Some(StructuralTerm::CapturedValue {
                                expression: e2,
                                statement: s2,
                                callable: c2,
                                result_type: t2,
                                integer_range: r2,
                                ..
                            }),
                        ) = (matched_current.as_ref(), other.current.as_ref())
                            && e1 == e2
                            && s1 == s2
                            && c1 == c2
                            && t1 == t2
                            && r1 == r2
                        {
                            // Keep matched_current (same expression/callable/type).
                        } else {
                            matched_current = None;
                        }
                    }
                }
                if all_have_coord
                    && (matched_current.is_some()
                        || matched_entry.is_some()
                        || matched_case.is_some())
                {
                    if matched_current.is_none() {
                        matched_read = None;
                    }
                    intersected.push(storage_effects::CapturedStorageValue {
                        coordinate: entry.coordinate.clone(),
                        entry: matched_entry,
                        current: matched_current,
                        selected_case: matched_case,
                        completed_read: matched_read,
                        backing: matched_backing,
                        current_anchors: matched_anchors,
                        anchor_state: entry.anchor_state,
                    });
                }
            }
            rejoined.stored_values =
                Arc::new(storage_effects::StoredValues::from_values(intersected));
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
                    statement: s1,
                    callable: c1,
                    result_type: t1,
                    integer_range: r1,
                    ..
                } = val
                    && outcomes[1..].iter().all(|judge| {
                        judge.normal_results.iter().any(|(other_expr, other_val)| {
                            other_expr == expr
                                && matches!(
                                    other_val,
                                    StructuralTerm::CapturedValue {
                                        expression: e2,
                                statement: s2,
                                        callable: c2,
                                        result_type: t2,
                                        integer_range: r2,
                                        ..
                                    } if e1 == e2 && s1 == s2 && c1 == c2 && t1 == t2 && r1 == r2
                                )
                        })
                    })
                {
                    intersected.push((*expr, val.clone()));
                }
            }
            rejoined.normal_results = Arc::new(intersected);
        }
        rejoined.completed_nominal_values = Arc::new(
            first
                .completed_nominal_values
                .iter()
                .filter(|row| {
                    let Some(key) = row.value.observation_identity_key() else {
                        return false;
                    };
                    outcomes.iter().all(|judge| {
                        judge.completed_nominal_values.iter().any(|other| {
                            other.anchor == row.anchor
                                && other.reference == row.reference
                                && other.value.observation_identity_key().as_ref() == Some(&key)
                        })
                    }) && rejoined.normal_results.iter().any(|(anchor, value)| {
                        *anchor == row.anchor
                            && value.observation_identity_key().as_ref() == Some(&key)
                    })
                })
                .cloned()
                .collect(),
        );
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
        Self::from_runtime_declaration_requires(
            program,
            judged_machine,
            program.machine_states(judged_machine).first(),
            requires,
        )
    }

    /// Every arrival proves this declaration's own entry contract separately.
    /// Its prefix retains admitted input domains, never another invocation's
    /// entry values or unrelated enclosing requirements.
    pub(super) fn from_runtime_state_requires(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        requires: &[ExpressionHandle],
    ) -> Self {
        Self::from_runtime_declaration_requires(program, machine, Some(state), requires)
    }

    pub(super) fn from_specification_state_requires(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        requires: &[ExpressionHandle],
    ) -> Self {
        let mut judge = Self::from_runtime_state_requires(program, machine, state, requires);
        judge.specification_terms = true;
        judge
    }

    fn from_runtime_declaration_requires(
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        requires: &[ExpressionHandle],
    ) -> Self {
        // Install premises under the runtime value resolver. Installing them
        // in tag-only mode first can resolve the new equation through its own
        // case substitution and discard its scalar value consequence.
        let mut judge = Self::from_case_requires(program, machine, &[]);
        judge.runtime_body_values = true;
        judge.body_type_parameters = program.machine_type_parameters(machine);
        if let Some(state) = state {
            judge.declaration_state = state.symbol;
            judge.retain_entry_values(state);
            judge.retain_entry_scalar_parameters(
                machine,
                state,
                program.state_parameters(state).iter().map(|parameter| {
                    (
                        parameter.symbol,
                        StructuralTerm::Variable(parameter.name.to_string()),
                    )
                }),
            );
        }
        judge.premise_facts.extend_from_slice(requires);
        for fact in requires {
            judge.intake_proposition(program, *fact);
            judge.intake_entry_collection_predicate(*fact);
        }
        if let Some(state) = state {
            judge.intake_entry_domain_predicates(machine, state);
        }
        // Capture entry comparison truth before any prefix effect. A later
        // write retires current storage observations, while detached input
        // values and their predicates keep the original invocation identity.
        for fact in requires {
            judge.intake_preserved_predicate(*fact);
        }
        // Entry read checks need the admitted case and bounds facts above.
        // Retain each snapshot after those predicates are recorded; a scalar
        // observation must not establish its own formation premise.
        if let Some(state) = state {
            for fact in requires {
                judge.retain_entry_scalar_reads(state, *fact);
            }
            judge.retain_declared_entry_scalar_reads(machine, state);
        }
        judge
    }

    /// This judge's premises for an independent query, with the fresh
    /// `QueryState` a newly built judge starts with: no resolution, prepared
    /// premise, frame resolver or trace another query wrote is visible to it.
    pub(super) fn with_fresh_query_state(&self) -> Self {
        Self {
            query_state: QueryState::fresh(),
            ..self.clone()
        }
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
    pub(super) fn frame_resolver(&self) -> Option<&CallFrameResolver<'program>> {
        self.query_state
            .frame_resolver
            .get_or_init(|| CallFrameResolver::new(self.program))
            .as_ref()
    }

    /// Tag preservation does not preserve scalar premises or payload equations.
    pub(super) fn restrict_to_case_facts(&mut self) {
        self.case_facts_only = true;
        self.induction_values = Arc::new(Vec::new());
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
        let mut judge = Self::from_declaration_requires(
            program,
            judged_machine.symbol,
            program
                .machine_states(judged_machine)
                .first()
                .map(|entry| program.state_parameters(entry))
                .unwrap_or_default(),
            requires,
            resolve_applications,
        );
        for fact in requires {
            judge.retain_proposition_identity(*fact);
        }
        judge
    }

    pub(super) fn from_declaration_requires(
        program: &'program dyn crate::CheckingInput,
        owner: SymbolHandle,
        parameters: &[StateParameter],
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
            returned_value_source: None,
            completed_nominal_values: Arc::new(Vec::new()),
            returned_field_owners: Arc::new(Vec::new()),
            body_invocations: Arc::new(Vec::new()),
            runtime_induction: Arc::new(runtime_induction::RuntimeInduction::default()),
            induction_values: Arc::new(Vec::new()),
            active_application: None,
            resolution_observations: None,
            normal_memberships: Arc::new(Vec::new()),
            normal_content_guarantees: Arc::new(Vec::new()),
            invalidated_storage: Arc::new(Vec::new()),
            storage_version: None,
            stored_values: Arc::default(),
            capture_sequence: 0,
            storage_write_sequence: 0,
            normal_guarantees: Arc::new(Vec::new()),
            captured_result_equations: Arc::new(Vec::new()),
            conditional_guarantees: Arc::new(Vec::new()),
            resolve_applications,
            require_closed_computation: false,
            runtime_body_values: false,
            specification_terms: false,
            case_facts_only: false,
            substitutions: Arc::new(Vec::new()),
            rewrites: Arc::new(rewrite_table::RewriteTable::default()),
            disequalities: Arc::new(Vec::new()),
            query_state: QueryState::fresh(),
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

    fn runtime_declaration_scope(&self) -> Option<(&'program Machine, &'program State)> {
        if self.declaration_state.is_valid() {
            crate::machine_calls::calls::machine_state_by_symbol(
                self.program,
                self.declaration_state,
            )
        } else {
            let machine = self.program.machine(self.machine_symbol)?;
            Some((machine, self.program.machine_states(machine).first()?))
        }
    }

    fn declaration_scope(&self) -> Option<(SymbolHandle, &[StateParameter])> {
        // Declaration formation supplies its complete telescope. It can be a
        // non-entry state or a data/trait owner; substituting machine entry
        // parameters here would change the identity of its exact binders.
        if !self.runtime_body_values {
            return Some((self.machine_symbol, &self.declaration_parameters));
        }
        let Some((machine, state)) = self.runtime_declaration_scope() else {
            return Some((self.machine_symbol, &self.declaration_parameters));
        };
        Some((machine.symbol, self.program.state_parameters(state)))
    }

    fn declaration_term(&self, expression: ExpressionHandle) -> Option<StructuralTerm> {
        if self.requires_value_preservation() {
            let (machine, state) = self.runtime_declaration_scope()?;
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
            Some(&|expression| {
                let ExpressionNode::SavedSubject(saved) =
                    expression_table(self.program).expression(expression)
                else {
                    return None;
                };
                self.completed_match_subject(saved)
            }),
        )
    }

    /// Formation has already read these subjects in its immutable declaration
    /// frame. Retain their scoped anchors before intaking arm conditions; a
    /// capture never gains a premise from an arm that has not been selected.
    pub(super) fn bind_formed_match_subjects(
        &mut self,
        captures: &[(ExpressionHandle, StructuralTerm)],
    ) {
        Arc::make_mut(&mut self.normal_results).extend_from_slice(captures);
    }

    /// The ordinary predicate reader can also consume an executed prefix's
    /// exact local bindings. These bindings affect the question only; the
    /// judge's admitted premises and captured storage epochs remain intact.
    pub(super) fn predicate_term_in_scope(
        &self,
        scope: (&Machine, &State, TypeReferenceHandle),
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
        scope: (&Machine, &State, TypeReferenceHandle),
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        depth: usize,
    ) -> StructuralJudgment {
        if depth >= 32 {
            return StructuralJudgment::Unknown;
        }
        if let Some(binary) = expression_table(self.program).binary_node(expression)
            && matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
            && crate::has_builtin_decomposed_guard_meaning(
                self.program,
                scope.0,
                Some(scope.1),
                expression,
            )
        {
            let composed = StructuralJudgment::connective(
                binary.operator,
                self.judge_predicate_in_scope(scope, binary.left, environment, depth + 1),
                self.judge_predicate_in_scope(scope, binary.right, environment, depth + 1),
            );
            if !matches!(composed, StructuralJudgment::Unknown) {
                return composed;
            }
        }
        let Some(predicate) =
            self.predicate_term_in_scope(scope, expression, environment, depth + 1)
        else {
            return StructuralJudgment::Unknown;
        };
        self.judge_observed_predicate(predicate, depth)
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
                    expression_table(self.program).display_name(expression)
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
        if !self.case_facts_only && program.static_type_predicate_operands(fact).is_some() {
            if let Some(predicate) = self.declaration_term(fact) {
                self.intake_equation(predicate, value_predicates::boolean_term(true), 0);
            }
            return;
        }
        let Some(binary) = expression_table(program).binary_node(fact) else {
            return;
        };
        if super::structural_terms::is_case_observation(program, fact) {
            self.intake_case_condition(fact, true);
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

    /// A branch assumption is available only after its source operand was
    /// independently formed. This supplies truth under that scoped path; it
    /// never admits a pending nominal value or imports its default domain.
    pub(super) fn intake_formed_condition(&mut self, fact: ExpressionHandle, holds: bool) {
        if !holds && super::structural_terms::is_case_observation(self.program, fact) {
            self.intake_case_condition(fact, false);
            return;
        }
        if holds {
            self.intake(self.program, fact);
        }
        if self
            .declaration_expression_type(fact)
            .and_then(|reference| self.program.primitive_type_reference(reference))
            != Some(PrimitiveType::Bool)
        {
            return;
        }
        if let Some(term) = self.declaration_term(fact) {
            self.intake_equation(term, value_predicates::boolean_term(holds), 0);
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
        if let Some(binary) = expression_table(program).binary_node(fact)
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
        if let Some(binary) = expression_table(program).binary_node(fact)
            && binary.operator == BinaryOperator::And
            && self.has_boolean_connective_meaning(fact)
        {
            self.intake_preserved_predicate(binary.left);
            self.intake_preserved_predicate(binary.right);
        } else if let Some(term) = self.declaration_term(fact) {
            let predicate = term.is_checked_predicate()
                || match &term {
                    StructuralTerm::ScalarBinary { .. } | StructuralTerm::Application { .. } => {
                        self.declaration_expression_type(fact)
                            .and_then(|reference| program.primitive_type_reference(reference))
                            == Some(PrimitiveType::Bool)
                    }
                    StructuralTerm::Variable(_) => {
                        program.static_type_predicate_operands(fact).is_some()
                    }
                    _ => false,
                };
            if predicate {
                self.intake_equation(term, value_predicates::boolean_term(true), 0);
            }
        }
    }

    /// Follow variable substitutions to a fixpoint, depth-capped (a cyclic
    /// substitution chain resolves to wherever the cap lands, which only
    /// weakens judgments toward Unknown -- never unsound). Constructor
    /// fields resolve recursively under the same budget.
    /// Semantic term equality deliberately ignores observation authority and
    /// literal landing metadata. Resolution preserves those payloads, so an
    /// equal term is not an interchangeable memoized result. Completed queries
    /// retain full payload, immutable context, active stack and exact depth.
    pub(super) fn resolve(&self, term: StructuralTerm) -> StructuralTerm {
        self.resolve_at(term, self.application_resolution_depth())
    }

    fn resolve_at(&self, term: StructuralTerm, depth: usize) -> StructuralTerm {
        self.resolve_with_at(term, depth, ResolutionPurpose::General)
    }

    fn resolve_with_at(
        &self,
        term: StructuralTerm,
        depth: usize,
        purpose: ResolutionPurpose,
    ) -> StructuralTerm {
        let depth = depth.max(self.application_resolution_depth());
        // Mutable dependency collection cannot be replaced by a completed
        // answer, including when the collector belongs to an ancestor.
        if self.has_resolution_dependency_collector() {
            self.invalidate_resolution_observations();
            return self.resolve_uncached(term, depth, purpose);
        }
        if !matches!(
            term,
            StructuralTerm::Application { .. } | StructuralTerm::CallProjection { .. }
        ) {
            return self.resolve_uncached(term, depth, purpose);
        }
        // Both purposes normalize the exact same immutable input cut, but
        // numeric projection can retain a completed scalar where General
        // unfolds its description. The shared query owner keys those answers
        // separately while retaining its full context and dependency checks.
        match purpose {
            ResolutionPurpose::General => self.completed_resolution(term, depth, |judge, term| {
                judge.resolve_uncached(term, depth, purpose)
            }),
            ResolutionPurpose::IntegerProjection => {
                self.completed_integer_projection(term, depth, |judge, term| {
                    judge.resolve_uncached(term, depth, purpose)
                })
            }
        }
    }

    fn resolve_uncached(
        &self,
        term: StructuralTerm,
        depth: usize,
        purpose: ResolutionPurpose,
    ) -> StructuralTerm {
        self.resolve_uncached_with_budget(term, depth, purpose, 32)
    }

    fn resolve_uncached_with_budget(
        &self,
        mut term: StructuralTerm,
        depth: usize,
        purpose: ResolutionPurpose,
        rewrite_budget: usize,
    ) -> StructuralTerm {
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
        for remaining in (0..rewrite_budget).rev() {
            // A completed result's field contract names this exact snapshot.
            // Rewriting its enclosing value to a call description first would
            // discard the more precise, independently admitted field equation.
            if matches!(term, StructuralTerm::Projection { .. })
                && let Some((_, replacement)) = self.rewrites.find(&term)
            {
                term = replacement.clone();
                continue;
            }
            // A contract can acquire its returned capture only after result
            // substitution. Recover that capture's saved owned field before
            // a parent case rewrite replaces its payload by a fresh binder.
            if self.requires_value_preservation()
                && matches!(&term, StructuralTerm::Projection { subject, .. }
                    if !self.require_closed_computation || subject.is_closed_value())
                && let Some(saved) = self.completed_owned_field(&term)
                && !saved.same_resolution_payload(&term)
            {
                term = saved;
                continue;
            }
            // Public field guarantees belong to the captured value, before a
            // case refinement replaces its parent with fresh payload binders.
            // Read those exact completed equations without unfolding the call.
            if matches!(term, StructuralTerm::Projection { .. }) {
                let completed = self.completed_integer_call_description(&term);
                if !completed.same_resolution_payload(&term) {
                    term = completed;
                    continue;
                }
            }
            // Authenticate the original applied identity before the existing
            // canonical-result owner runs. Only the Application arm consumes
            // it, after current rewrites and public normal forms.
            let completed_integer = (purpose == ResolutionPurpose::IntegerProjection
                && matches!(&term, StructuralTerm::Application { .. }))
            .then(|| self.completed_integer_call(&term))
            .flatten()
            .map(|(captured, _, _)| captured);
            match term {
                StructuralTerm::CapturedValue { .. } | StructuralTerm::BoundValue(_) => {
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
                StructuralTerm::Constructor {
                    identity,
                    data,
                    case,
                    fields,
                } => {
                    return StructuralTerm::Constructor {
                        identity,
                        data,
                        case,
                        fields: fields
                            .into_iter()
                            .map(|(name, value)| {
                                (name, self.resolve_with_at(value, depth + 1, purpose))
                            })
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
                    let admitted_arguments = completed_integer.as_ref().map(|_| arguments.clone());
                    let arguments: Vec<StructuralTerm> = arguments
                        .into_iter()
                        .map(|argument| self.resolve_with_at(argument, depth + 1, purpose))
                        .collect();
                    let result_type = super::specification_calls::canonical_result_reference(
                        self.program,
                        result_type,
                    );
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
                        // Conditional normal-return guarantees are not proof
                        // that this evaluation completes. Compute its body
                        // using the same continuing invocation depth instead.
                        return self
                            .unfold_application(&resolved, depth + 1)
                            .map_or(resolved.clone(), |value| {
                                self.resolve_with_at(value, depth + 1, purpose)
                            });
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
                    if let Some(captured) = completed_integer
                        && let Some(admitted) = admitted_arguments
                        && let StructuralTerm::Application { arguments, .. } = &resolved
                        && admitted
                            .iter()
                            .zip(arguments)
                            .all(|(before, after)| before.same_resolution_payload(after))
                    {
                        // Target, selections and spelling were retained above;
                        // the result changed only through the existing exact
                        // canonical-result-reference owner. Changed arguments
                        // decline this optional leaf and keep ordinary replay.
                        return captured;
                    }
                    if let Some(unfolded) = self.unfold_application(&resolved, depth + 1) {
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
                        .map(|argument| self.resolve_with_at(argument, depth + 1, purpose))
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
                        purpose,
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
                        collection: Box::new(self.resolve_with_at(*collection, depth + 1, purpose)),
                        index: Box::new(self.resolve_with_at(*index, depth + 1, purpose)),
                    };
                    // A complete uniform value records every cell, including
                    // the zero representation supplied by construction. Select
                    // only a closed in-bounds coordinate; neither the desired
                    // predicate nor an empty array can supply a missing cell.
                    if let StructuralTerm::IndexedValue {
                        collection, index, ..
                    } = &resolved
                        && let Some(element) =
                            super::observed_projections::uniform_element(collection, index)
                    {
                        term = element;
                        continue;
                    }
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
                    // A forwarding identity names the completed descriptor
                    // before a case rewrite replaces its enclosing payload.
                    // Keep that subject first; the same metadata query below
                    // still handles descriptors produced by computation.
                    let collection = match self.collection_measure_identity(
                        measure,
                        (*collection).into_collection_descriptor(),
                        depth + 1,
                    ) {
                        StructuralTerm::CollectionMeasure { collection, .. } => *collection,
                        value => return value,
                    };
                    let collection = self
                        .resolve_with_at(collection, depth + 1, purpose)
                        .into_collection_descriptor();
                    let resolved = self.collection_measure_identity(measure, collection, depth + 1);
                    if !matches!(resolved, StructuralTerm::CollectionMeasure { .. }) {
                        return resolved;
                    }
                    if let Some((_, replacement)) = self.rewrites.find(&resolved) {
                        term = replacement.clone();
                        continue;
                    }
                    return resolved;
                }
                StructuralTerm::BoundProjection { .. } => {
                    // Header-field observations keep their declaration-rooted
                    // identity. An independently admitted scalar equality may
                    // relate them just like another completed projection.
                    if let Some((_, replacement)) = self.rewrites.find(&term) {
                        term = replacement.clone();
                        continue;
                    }
                    return term;
                }
                StructuralTerm::Projection { subject, path } => {
                    let subject = self.resolve_with_at(*subject, depth + 1, purpose);
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
                                self,
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
                term @ StructuralTerm::ScalarBinary { .. }
                    if logical_resolution::is_boolean_connective(&term) =>
                {
                    return self.resolve_logical_term(term, depth, purpose, remaining + 1);
                }
                StructuralTerm::ScalarBinary {
                    references,
                    operator,
                    meaning,
                    left,
                    right,
                } => {
                    let left = self.resolve_with_at(*left, depth + 1, purpose);
                    let right = self.resolve_with_at(*right, depth + 1, purpose);
                    let resolved = StructuralTerm::ScalarBinary {
                        references,
                        operator,
                        meaning,
                        left: Box::new(left.clone()),
                        right: Box::new(right.clone()),
                    };
                    match self.finish_scalar_resolution(resolved) {
                        logical_resolution::ScalarResolution::Finished(value) => return value,
                        logical_resolution::ScalarResolution::Rewrite(replacement) => {
                            term = replacement;
                            continue;
                        }
                    }
                }
                StructuralTerm::ScalarRepresentation {
                    source,
                    target,
                    value,
                } => {
                    let resolved = super::structural_terms::normalize_scalar_representation(
                        source,
                        target,
                        self.resolve_with_at(*value, depth + 1, purpose),
                    );
                    if let Some((_, replacement)) = self.rewrites.find(&resolved) {
                        term = replacement.clone();
                        continue;
                    }
                    return resolved;
                }
                StructuralTerm::IntegerEmbedding { reference, value } => {
                    let value = self.resolve_with_at(*value, depth + 1, purpose);
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
                        left: Box::new(self.resolve_with_at(*left, depth + 1, purpose)),
                        right: Box::new(self.resolve_with_at(*right, depth + 1, purpose)),
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
                // Variants that only carry substituted operands walk the
                // shared child table; the leaves come back unchanged. Every
                // variant whose rewrite depends on this judge's `rewrites`
                // table keeps its own arm above.
                _ => {
                    return term
                        .map_children(|child| self.resolve_with_at(child, depth + 1, purpose));
                }
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
        selections: &[StaticMachineArgument],
        result_type: TypeReferenceHandle,
        field: SymbolHandle,
        arguments: &[StructuralTerm],
        depth: usize,
        purpose: ResolutionPurpose,
    ) -> Option<StructuralTerm> {
        if depth >= 32 {
            return None;
        }
        let program = self.program;
        let (machine, state) =
            crate::machine_calls::calls::machine_state_by_symbol(program, target)?;
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
            self.record_returned_field_dependency(machine.symbol);
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
            let literal = expression_table(program).struct_literal_node(*value)?;
            let projected = expression_table(program)
                .struct_fields(literal.fields)
                .iter()
                .find(|candidate| candidate.field_symbol == field)?;
            // Nested calls still belong to the callee declaration. Their own
            // result query uses this frame; the field type supplies only the
            // retained landing for anonymous arithmetic.
            self.callee_term(
                projected.value,
                &environment,
                StaticObservations {
                    machine_bindings: &static_environment,
                    body_parameters: &[],
                },
                Some((machine, state, projected_field.type_reference)),
                depth + 1,
            )
        };
        if !self.require_closed_computation
            && let Some(value) = direct_literal_field()
        {
            return Some(value);
        }
        let application = StructuralTerm::Application {
            result_type,
            target,
            selections: selections.to_vec(),
            machine: machine.name.as_str().to_owned(),
            arguments: arguments.to_vec(),
        };
        let result = self.unfold_application(&application, depth + 1)?;
        let result = self.resolve_with_at(result, depth + 1, purpose);
        if self.require_closed_computation && !result.is_closed_value() {
            return None;
        }
        let StructuralTerm::Constructor {
            identity,
            case,
            fields,
            ..
        } = result
        else {
            return None;
        };
        // The checked return type fixes the owner; spelling only selects within
        // that owner's normalized value, after the exact field-handle join.
        if Some(identity) != ConstructorIdentity::at_reference(program, result_type, data, None)
            || !case.is_empty()
        {
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
        selections: &[StaticMachineArgument],
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
                ResolutionPurpose::General,
            )?,
            None => {
                let application = StructuralTerm::Application {
                    result_type,
                    target,
                    selections: selections.to_vec(),
                    machine: machine.name.as_str().to_owned(),
                    arguments: arguments.to_vec(),
                };
                judge.unfold_application(&application, 0)?
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
        application: &StructuralTerm,
        depth: usize,
    ) -> Option<StructuralTerm> {
        let StructuralTerm::Application {
            target,
            selections,
            machine: machine_name,
            arguments,
            ..
        } = application
        else {
            return None;
        };
        let target = *target;
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
                TypeParameterKind::Machine { .. }
                | TypeParameterKind::Const { .. }
                | TypeParameterKind::Value { .. } => {
                    static_environment.push((
                        parameter.symbol,
                        parameter.name.as_str().to_owned(),
                        selected.clone(),
                    ));
                }
                // The type actual is already the selected reference. It does
                // not become a value binder; the body is read under that
                // exact operand, and a missing or applied type stays closed.
                TypeParameterKind::Type
                    if super::specification_calls::exact_type_selection(program, selected) => {}
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
        // This is an admitted application body, not another runtime event.
        // Nested calls still pass specification_application_candidate; both
        // ordinary and normalized terminal values use that same admission.
        let mut specification = self.clone();
        specification.specification_terms = true;
        let specification = specification.resolving_application(application, depth)?;

        let mut environment = environment;
        specification.record_returned_field_dependency(machine.symbol);
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
                let term = specification.callee_term(
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
                    let completed = specification.resolve_at(term, depth + 1);
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
                        specification.callee_term(
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
                        let comparison = expression_table(program).binary_node(guard)?;
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
                            local_subject = specification
                                .resolve_at(environment[position].1.clone(), depth + 1);
                            &local_subject
                        };
                        let subject_term = specification.observed_case_value(subject_term)?;
                        let StructuralTerm::Constructor { identity, .. } = subject_term else {
                            // The matched argument is not (yet) a constructor:
                            // arm selection is undecidable, no unfold.
                            return None;
                        };
                        identity.matches_case(program, arm_data.symbol, arm_case.symbol)?
                    } else {
                        // Preserve the case-arm fast path above: resolving its
                        // already-completed constructor arguments again repeats
                        // recursive unfolding. Non-case scalar guards use the
                        // same captured environment and selected source meaning.
                        // Unknown never chooses an arm or assumes the goal.
                        match specification.judge_predicate_in_scope(
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
            return match program.statement_table.transition_target(transition.target) {
                TransitionTargetNode::Value(value) => specification.callee_term(
                    *value,
                    &environment,
                    StaticObservations {
                        machine_bindings: &static_environment,
                        body_parameters: &[],
                    },
                    Some((machine, state, state.return_type)),
                    depth + 1,
                ),
                TransitionTargetNode::Named {
                    path,
                    arguments,
                    static_machine_parameter,
                    evidence_arguments,
                    ..
                } if !static_machine_parameter.is_valid() && evidence_arguments.is_empty() => {
                    // The terminal target forwards this exact application's
                    // result. Its original checked selection and formation stay
                    // on the source transition; a fired guard supplies neither
                    // operational eligibility nor the target's preconditions.
                    let (target, result_type) =
                        crate::machine_calls::calls::terminal_callable_target(
                            program,
                            machine,
                            path.symbol,
                        )?;
                    let callee = self.entry_machines.machine(program, target)?;
                    if !program.machine_type_parameters(callee).is_empty() {
                        return None;
                    }
                    specification_application_candidate(
                        program,
                        machine.symbol,
                        target,
                        &[],
                        self.frame_resolver(),
                    )
                    .ok()?;
                    let arguments = program
                        .statement_table
                        .expression_handles(*arguments)
                        .iter()
                        .map(|argument| {
                            specification.callee_term(
                                *argument,
                                &environment,
                                StaticObservations {
                                    machine_bindings: &static_environment,
                                    body_parameters: &[],
                                },
                                Some((machine, state, state.return_type)),
                                depth + 1,
                            )
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Some(StructuralTerm::Application {
                        result_type,
                        target,
                        selections: Vec::new(),
                        machine: callee.name.as_str().to_owned(),
                        arguments,
                    })
                }
                _ => None,
            };
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
            let projected = super::observed_projections::project_declared_field(
                self.program,
                self,
                value,
                field_name,
            )?;
            // A constructor can move an observed owned value into this field.
            // Keep that input's identity without reading contained references.
            return Some(
                if super::structural_terms::owned_observation_type(self.program, &projected)
                    .is_some()
                {
                    self.completed_nominal_subject(&projected)
                } else {
                    projected
                },
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
        scope: Option<(&Machine, &State, TypeReferenceHandle)>,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        depth: usize,
    ) -> Option<StructuralTerm> {
        self.callee_term(
            expression,
            environment,
            StaticObservations {
                machine_bindings: &[],
                body_parameters: if scope.is_some_and(|(machine, _, _)| {
                    self.runtime_declaration_scope()
                        .is_some_and(|(owner, _)| machine.symbol == owner.symbol)
                }) {
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
        lexical_owner: SymbolHandle,
        call: &typed_trees::expression::TableCallExpression,
    ) -> bool {
        if !self.specification_terms {
            return false;
        }
        let closed = crate::machine_calls::denotational_calls::selected_requirement_authored_body(
            self.program,
            lexical_owner,
            call,
        );
        if closed.is_none()
            && let Some(super::specification_calls::SelectedCallContract::Signature(signature)) =
                super::specification_calls::selected_call_contract(
                    self.program,
                    lexical_owner,
                    call,
                )
            && crate::machine_calls::denotational_calls::signature_has_closed_value_contract(
                self.program,
                signature,
            )
        {
            // An abstract logical application consumes its exact public
            // effect/progress promise. It gains no body or result equation.
            return true;
        }
        let target = closed.map_or_else(
            || super::structural_terms::structural_call_target(self.program, lexical_owner, call),
            |(_, entry)| entry.symbol,
        );
        let candidate = specification_application_candidate(
            self.program,
            self.machine_symbol,
            target,
            if closed.is_some() {
                &[]
            } else {
                &call.machine_arguments
            },
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

    /// Long chains of && and || must not consume the recursion budget reserved
    /// for following declarations. Walk selected builtin operators in their
    /// original order; operand leaves keep the ordinary observation checks.
    fn callee_term(
        &self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        static_observations: StaticObservations<'_>,
        scope: Option<(&Machine, &State, TypeReferenceHandle)>,
        depth: usize,
    ) -> Option<StructuralTerm> {
        if depth >= 32 {
            return None;
        }
        enum Pending {
            Read(ExpressionHandle),
            Join(ExpressionHandle),
        }
        let mut pending = vec![Pending::Read(expression)];
        let mut values = Vec::new();
        while let Some(step) = pending.pop() {
            match step {
                Pending::Read(expression) => {
                    // The exact completed expression wins over describing its
                    // source again, just as in the ordinary leaf reader.
                    if let Some((_, value)) = self
                        .normal_results
                        .iter()
                        .find(|(source, _)| *source == expression)
                    {
                        values.push(value.clone());
                    } else if let Some(binary) =
                        expression_table(self.program).binary_node(expression)
                        && matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                        && scope.is_some_and(|(machine, state, _)| {
                            crate::has_builtin_binary_expression_meaning(
                                self.program,
                                machine,
                                Some(state),
                                expression,
                            )
                        })
                    {
                        pending.push(Pending::Join(expression));
                        pending.push(Pending::Read(binary.right));
                        pending.push(Pending::Read(binary.left));
                    } else {
                        values.push(self.callee_term_leaf(
                            expression,
                            environment,
                            static_observations,
                            scope,
                            depth,
                        )?);
                    }
                }
                Pending::Join(expression) => {
                    let right = values.pop()?;
                    let left = values.pop()?;
                    values.push(super::structural_terms::binary_term_in_scope(
                        self.program,
                        scope,
                        None,
                        expression,
                        left,
                        right,
                    )?);
                }
            }
        }
        values.pop()
    }

    fn callee_term_leaf(
        &self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        static_observations: StaticObservations<'_>,
        scope: Option<(&Machine, &State, TypeReferenceHandle)>,
        depth: usize,
    ) -> Option<StructuralTerm> {
        if depth >= 32 {
            return None;
        }
        if self.specification_terms
            && let Some(place) = crate::reserved_result_place(self.program, expression)
            && place.machine_symbol == self.machine_symbol
            && let Some(source) = self.returned_value_source.as_ref()
            && let Some(value) = self.completed_return_read(source, &place.segments)
        {
            return Some(value);
        }
        if let Some((_, value)) = self
            .normal_results
            .iter()
            .find(|(source, _)| *source == expression)
        {
            return Some(value.clone());
        }
        if let ExpressionNode::SavedSubject(saved) =
            expression_table(self.program).expression(expression)
            && expression_table(self.program).contains_value_capture(saved.capture)
            && matches!(
                expression_table(self.program)
                    .value_capture(saved.capture)
                    .owner,
                typed_trees::expression::ValueCaptureOwner::Match
            )
        {
            return self.completed_match_subject(saved);
        }
        if let Some(value) = self.completed_descriptor_place(scope, expression, environment) {
            return Some(value);
        }
        if let Some(value) = self.stored_value_observation(
            scope.map(|(machine, state, _)| (machine, state)),
            expression,
            |selector| {
                self.callee_term(selector, environment, static_observations, scope, depth + 1)
            },
        ) {
            return Some(value);
        }
        let program = self.program;
        macro_rules! child_term {
            ($expression:expr) => {
                self.callee_term(
                    $expression,
                    environment,
                    static_observations,
                    scope,
                    depth + 1,
                )
            };
        }
        if program.static_type_predicate_operands(expression).is_some() {
            // Types have no runtime storage observation. Read the same
            // declaration-owned proposition as contract term construction;
            // an open proposition remains symbolic until a path establishes it.
            return super::structural_terms::static_type_predicate_term(program, expression, &[]);
        }
        // A selected collection measure observes the completed descriptor, not
        // its retired referent. Resolve the receiver through the ordinary value
        // owner first: only an exact completed value or a separately proved
        // stable descriptor binding survives a write. Applying the referent
        // fence to `.len` itself loses that distinction after any earlier write
        // conservatively invalidated reference-bearing state parameters.
        // Ordinary fields (including a user field named `len`) still cross the
        // storage fence below; selection supplies no contents or bounds proof.
        if let ExpressionNode::Member(member) = expression_table(program).expression(expression)
            && let Some((machine, state, _)) = scope
            && let Some(reference) =
                crate::expression_result_type_reference(program, machine, state, member.receiver)
            && let Some(measure) =
                super::structural_terms::collection_measure_term(program, member, reference, || {
                    child_term!(member.receiver)
                })
        {
            return Some(measure);
        }
        if self.storage_observation_invalidated(expression) {
            if super::struct_trace() {
                eprintln!(
                    "STRUCT term-storage-refused caller={:?} expression={}",
                    self.machine_symbol,
                    expression_table(program).display_name(expression)
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
                child_term!(argument)?,
            );
        }
        if expression_table(program)
            .call_node(expression)
            .is_some_and(|call| {
                crate::mathematical_applications::is_mathematical_call(program, call)
            })
        {
            return super::structural_terms::mathematical_application_term(
                program,
                expression,
                |capture| child_term!(capture),
            );
        }
        // Closed anonymous arithmetic has the same exact value in a runtime
        // predicate as in its declaration. Read it before recursive operation
        // construction: the enclosing machine's return type is not the
        // comparison peer's landing, and must not erase a retained predicate.
        // The shared reader rejects runtime names, typed arithmetic policies
        // and unselected user operators; formation and storage fences above
        // remain independent obligations.
        if let Some(value) = super::proof_integer::anonymous_integer_value(program, expression) {
            return Some(StructuralTerm::Integer(value));
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::Match(dispatch) => self.specification_match_term(
                expression,
                dispatch,
                environment,
                static_observations,
                scope?,
                depth + 1,
            ),
            ExpressionNode::Name(path) => {
                // Static values use the resolved declaration, not the runtime
                // environment's spelling. A local may shadow a const binder;
                // that never substitutes the local for a retained static use.
                if let Some((_, _, selected)) = static_observations
                    .machine_bindings
                    .iter()
                    .find(|(symbol, _, _)| *symbol == path.symbol)
                    && let Some(parameter) = program.data_type_parameter_by_symbol(path.symbol)
                    && let TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } = parameter.kind
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
                                == Some(PrimitiveType::Bool) =>
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
                            && matches!(parameter.kind, TypeParameterKind::Const { .. })
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
                                    TypeParameterKind::Value { .. }
                                        | TypeParameterKind::Const { .. }
                                )
                        })
                }) {
                    return Some(StructuralTerm::BoundValue(path.symbol));
                }
                let members = expression_table(program).name_path_members(path.members);
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
                    expression_table(program).expression(member.receiver)
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
                    let arguments = expression_table(program)
                        .expression_handles(call.arguments)
                        .iter()
                        .map(|argument| child_term!(*argument))
                        .collect::<Option<Vec<_>>>()?;
                    let state =
                        crate::machine_calls::calls::signature_queries::unique_state_by_symbol(
                            program,
                            call.target_symbol,
                        )?;
                    let data_symbol = crate::value_custody::type_shapes::head_data_symbol(
                        program,
                        state.return_type,
                    )?;
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
                let receiver_term = child_term!(member.receiver)?;
                self.observed_field_term(receiver_term, member.member.as_str(), depth)
            }
            ExpressionNode::StructLiteral(literal) => {
                super::structural_terms::constructor_literal_term(
                    program,
                    expression,
                    literal,
                    |_, field| {
                        let field = field?;
                        child_term!(field.value)
                    },
                )
            }
            ExpressionNode::Unary(unary) => super::structural_terms::logical_not_term_in_scope(
                program,
                scope,
                None,
                expression,
                child_term!(unary.operand)?,
            ),
            ExpressionNode::Integer(value) => value.value_bignum().map(StructuralTerm::Integer),
            ExpressionNode::Cast(cast) => {
                let (machine, state, _) = scope?;
                let source =
                    crate::expression_result_type_reference(program, machine, state, cast.value);
                let value = child_term!(cast.value)?;
                super::structural_terms::conversion_term(program, cast, source, value)
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
                    && !self.specification_call_candidate(
                        scope.map_or(self.machine_symbol, |(machine, _, _)| machine.symbol),
                        call,
                    ) {
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
                            expression_table(program).display_name(expression),
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
                for argument in expression_table(program).expression_handles(call.arguments) {
                    arguments.push(child_term!(*argument)?);
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
                    target: realization.unwrap_or_else(|| {
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
            ExpressionNode::Borrow(borrow) => child_term!(borrow.target),
            ExpressionNode::Boolean(value) => Some(StructuralTerm::boolean(*value)),
            ExpressionNode::Indexed(indexed) => super::structural_terms::indexed_term_in_scope(
                program,
                scope,
                None,
                expression,
                child_term!(indexed.collection)?,
                child_term!(indexed.index)?,
            ),
            ExpressionNode::Binary(binary) => super::structural_terms::binary_term_in_scope(
                program,
                scope,
                None,
                expression,
                child_term!(binary.left)?,
                child_term!(binary.right)?,
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
        machine_map: &[(SymbolHandle, TypeReferenceHandle, StaticMachineArgument)],
    ) -> StructuralTerm {
        let substitute =
            |value: StructuralTerm| Self::substitute_call_term(&value, map, machine_map);
        match term {
            StructuralTerm::Variable(name) => map
                .iter()
                .find(|(variable, _)| variable == name)
                .map(|(_, replacement)| replacement.clone())
                .unwrap_or_else(|| term.clone()),
            StructuralTerm::Application { .. } | StructuralTerm::CallProjection { .. } => {
                let mut substituted = term.clone().map_children(substitute);
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
                        StructuralTerm::Identity { .. }
                        | StructuralTerm::ContentValue { .. }
                        | StructuralTerm::Integer(_)
                        | StructuralTerm::RepeatedArray { .. }
                        | StructuralTerm::Float(_)
                        | StructuralTerm::ScalarRepresentation { .. }
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
            // Every other variant carries its children through the shared
            // child table: substituting a leaf leaves it alone, and a shell or
            // composite rebuilds around its substituted operands.
            other => other.clone().map_children(substitute),
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

    /// Contracts supply proposition context to this expression and its logical
    /// children. Executable conditions use the observed-predicate entrance;
    /// their equality operation cannot acquire mathematical data identity here.
    pub(super) fn judge_proposition(
        &self,
        program: &dyn crate::CheckingInput,
        fact: ExpressionHandle,
    ) -> StructuralJudgment {
        if self.case_facts_only && !super::call_requirements::case_fact_only(program, fact) {
            return StructuralJudgment::Unknown;
        }
        if program.static_type_predicate_operands(fact).is_some() {
            return self
                .declaration_term(fact)
                .map(|term| self.resolve(term))
                .and_then(|term| value_predicates::boolean_value(term.semantic_value()))
                .map(|holds| {
                    if holds {
                        StructuralJudgment::Proven
                    } else {
                        StructuralJudgment::Refuted
                    }
                })
                .unwrap_or(StructuralJudgment::Unknown);
        }
        if expression_table(program)
            .call_node(fact)
            .is_some_and(|call| {
                crate::mathematical_applications::is_mathematical_call(program, call)
            })
        {
            // This engine consumes source entry observations rather than this
            // path's completed storage terms. Keep its separate epoch fence.
            if !self.entry_storage_observations_preserved(fact) {
                return StructuralJudgment::Unknown;
            }
            // Consume the checked calculus through the same completed-value
            // predicate reader as an inline contract. Integer-only capture
            // rebinding used to lose IEEE aliases despite retaining their
            // selected relations in the prepared artifact.
            return self
                .declaration_term(fact)
                .map_or(StructuralJudgment::Unknown, |predicate| {
                    self.judge_observed_predicate(predicate, 0)
                });
        }
        let Some(binary) = expression_table(program).binary_node(fact) else {
            return self
                .judge_value_predicate(fact)
                .unwrap_or(StructuralJudgment::Unknown);
        };
        if super::structural_terms::is_case_observation(program, fact) {
            if !self.is_case_membership(fact, binary) {
                return StructuralJudgment::Unknown;
            }
            let Some((definition, variant)) = cases::applied_case_classifier(
                program,
                binary.right,
                self.declaration_expression_type(binary.left),
            ) else {
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
                    self.judge_proposition(program, binary.left),
                    self.judge_proposition(program, binary.right),
                )
            }
            BinaryOperator::Or if self.has_boolean_connective_meaning(fact) => {
                self.judge_disjunction(binary.left, binary.right)
            }
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                let data_identity = self.declaration_scope().is_some_and(|(owner, parameters)| {
                    super::structural_terms::builtin_identity_operand_references(
                        program, owner, parameters, fact,
                    )
                    .is_some()
                });
                if data_identity {
                    // The authenticated owned primitive comparison has the
                    // same equality law as its value identity. This does not
                    // grant an aggregate or authored comparison an Id bridge.
                    let Some([left, right]) = self.declaration_data_identity_terms(fact) else {
                        return StructuralJudgment::Unknown;
                    };
                    if matches!(
                        self.judge_data_identity(left, right, 0),
                        StructuralJudgment::Proven
                    ) {
                        return if binary.operator == BinaryOperator::Equal {
                            StructuralJudgment::Proven
                        } else {
                            StructuralJudgment::Refuted
                        };
                    }
                } else {
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
                }
                if !self.has_identity_equality_meaning(fact) {
                    if super::struct_trace() {
                        eprintln!(
                            "STRUCT equality-refused caller={:?} fact={} reason=identity-authority",
                            self.machine_symbol,
                            expression_table(program).display_name(fact)
                        );
                    }
                    return StructuralJudgment::Unknown;
                }
                let Some(terms) = self.declaration_term_pair(binary) else {
                    if super::struct_trace() {
                        eprintln!(
                            "STRUCT equality-refused caller={:?} fact={} reason=term-formation",
                            self.machine_symbol,
                            expression_table(program).display_name(fact)
                        );
                    }
                    return StructuralJudgment::Unknown;
                };
                // No facts change between the authority check and judgment.
                // Reuse its exact resolution, including its depth budget; do
                // not resolve the result again or memoize by term equality,
                // which deliberately ignores some observation metadata.
                let [left, right] = terms.resolved;
                let [source_left, source_right] = terms.source;
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
                            // A case rewrite can hide saved field values behind a
                            // fresh payload binder. Select the original fields
                            // before normalizing their enclosing value.
                            let original = self.judge_equation(
                                self.record_fields_term(source_left, reference, &mut Vec::new()),
                                self.record_fields_term(source_right, reference, &mut Vec::new()),
                                0,
                            );
                            if !matches!(original, StructuralJudgment::Unknown) {
                                return original;
                            }
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
                identity: identity_l,
                fields: fields_l,
                ..
            },
            StructuralTerm::Constructor {
                identity: identity_r,
                fields: fields_r,
                ..
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
        if !identity_l.same_carrier(*identity_r) {
            return StructuralJudgment::Unknown;
        }
        if identity_l != identity_r {
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
                    identity, fields, ..
                } if Some(*identity)
                    == ConstructorIdentity::at_reference(self.program, reference, data, None) =>
                {
                    fields
                        .iter()
                        .find_map(|(field, value)| (field == name).then(|| value.clone()))
                }
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
        let Some(identity) = ConstructorIdentity::at_reference(self.program, reference, data, None)
        else {
            return term;
        };
        StructuralTerm::Constructor {
            identity,
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
        let StructuralTerm::Constructor {
            identity,
            case,
            fields,
            ..
        } = constructor
        else {
            return false;
        };
        if !case.is_empty() {
            return false;
        }
        let StructuralTerm::Variable(name) = variable else {
            return false;
        };
        let Some(definition) = identity
            .declaration()
            .and_then(|symbol| self.program.data_definition_by_symbol(symbol))
        else {
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
                    && match self.resolve(value.clone()) {
                        StructuralTerm::Opaque(opaque) => opaque == format!("{name}.{field_name}"),
                        StructuralTerm::Projection { subject, path } => {
                            matches!(*subject, StructuralTerm::Variable(root) if root == *name)
                                && path == *field_name
                        }
                        _ => false,
                    }
            })
        })
    }
}
