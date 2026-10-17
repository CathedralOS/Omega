use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Machine, StatementNode, SymbolHandle,
    TransitionGuardNode, TransitionTargetNode, is_arm_pattern_marker, structural_call_machine_name,
    term_contains,
};
use std::sync::Arc;
use symbols::JoinedPath;
use typed_trees::types::TypeReferenceHandle;

mod integer_predicates;
mod licenses;
pub(super) mod normal_returns;
mod selected_contracts;
mod storage_effects;
mod value_predicates;

pub(crate) use licenses::{EntryMachines, LicenseCandidates, declared_index_algebra_laws};
use licenses::{
    RingLicense, SemiringLicense, additive_addends, fold_scalar_binary, polynomial_normal_form,
    sorted_multiset_subtract, term_uses_application,
};

pub(super) enum StructuralJudgment {
    Proven,
    Refuted,
    Unknown,
}

/// Observation authority is established on resolved terms. Judgment consumes
/// those exact results; equation intake instead needs the original terms,
/// because installing a case equation can change subsequent resolution.
struct ObservedEqualityTerms {
    source: [StructuralTerm; 2],
    resolved: [StructuralTerm; 2],
}

#[derive(Clone)]
pub(super) struct CaseGuarantee {
    subject: StructuralTerm,
    data: SymbolHandle,
    case: SymbolHandle,
}

#[derive(Clone)]
pub(super) enum StructuralTerm {
    Variable(String),
    /// A frame-authenticated scalar observation of an input referent. The
    /// wrapped value retains its original identity; this declared type records
    /// content authority, not reference descriptor identity or output origin.
    ObservedValue {
        observed_type: TypeReferenceHandle,
        value: Box<StructuralTerm>,
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

    pub(super) fn into_semantic_value(self) -> Self {
        match self {
            Self::ObservedValue { value, .. } => (*value).into_semantic_value(),
            other => other,
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

pub(super) struct StructuralJudge<'program> {
    program: &'program dyn crate::CheckingInput,
    entry_machines: std::sync::Arc<EntryMachines>,
    machine_symbol: SymbolHandle,
    // Runtime prefix facts belong to the current state invocation. Keep its
    // declaration frame separate from the machine's dependency identity.
    declaration_state: SymbolHandle,
    premise_facts: Vec<ExpressionHandle>,
    // Term-bearing fact vectors are Arc-shared so the per-leaf judge clone
    // (one per recognized case-arm path) costs refcount bumps instead of
    // deep StructuralTerm copies; writers go through Arc::make_mut and pay
    // one copy only for the field they actually change.
    normal_results: Arc<Vec<(ExpressionHandle, StructuralTerm)>>,
    invalidated_storage: Arc<Vec<SymbolHandle>>,
    // Typed storage observations retain live values and detached entry snapshots.
    stored_values: Arc<Vec<storage_effects::CapturedStorageValue>>,
    capture_sequence: usize,
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
    case_facts_only: bool,
    pub(super) substitutions: Arc<Vec<(String, StructuralTerm)>>,
    /// Application REWRITES (`add_zero_right(prev) -> prev`): hypothesis
    /// equations with an application side orient REDUCING -- the inductive
    /// hypothesis rewrites the self-application away instead of expanding a
    /// variable into it, which also serves asymmetric goals.
    rewrites: Arc<rewrite_table::RewriteTable>,
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
            premise_facts: self.premise_facts.clone(),
            normal_results: Arc::clone(&self.normal_results),
            invalidated_storage: Arc::clone(&self.invalidated_storage),
            stored_values: Arc::clone(&self.stored_values),
            capture_sequence: self.capture_sequence,
            storage_write_sequence: self.storage_write_sequence,
            normal_guarantees: Arc::clone(&self.normal_guarantees),
            captured_result_equations: Arc::clone(&self.captured_result_equations),
            resolve_applications: self.resolve_applications,
            require_closed_computation: self.require_closed_computation,
            runtime_body_values: self.runtime_body_values,
            case_facts_only: self.case_facts_only,
            substitutions: Arc::clone(&self.substitutions),
            rewrites: Arc::clone(&self.rewrites),
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
        judge.premise_facts.extend_from_slice(requires);
        for fact in requires {
            judge.intake(program, *fact);
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
        judge.premise_facts.extend_from_slice(requires);
        for fact in requires {
            judge.intake(program, *fact);
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
            requires,
            resolve_applications,
        )
    }

    pub(super) fn from_declaration_requires(
        program: &'program dyn crate::CheckingInput,
        owner: SymbolHandle,
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
            premise_facts: requires.to_vec(),
            normal_results: Arc::new(Vec::new()),
            invalidated_storage: Arc::new(Vec::new()),
            stored_values: Arc::new(Vec::new()),
            capture_sequence: 0,
            storage_write_sequence: 0,
            normal_guarantees: Arc::new(Vec::new()),
            captured_result_equations: Arc::new(Vec::new()),
            resolve_applications,
            require_closed_computation: false,
            runtime_body_values: false,
            case_facts_only: false,
            substitutions: Arc::new(Vec::new()),
            rewrites: Arc::new(rewrite_table::RewriteTable::default()),
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

    fn declaration_term(&self, expression: ExpressionHandle) -> Option<StructuralTerm> {
        if let Some(definition) = self
            .program
            .mathematical_definitions()
            .iter()
            .find(|definition| definition.symbol == self.machine_symbol)
        {
            return super::structural_terms::structural_term(
                self.program,
                None,
                Some((
                    definition.symbol,
                    &crate::mathematical_applications::ordinary_parameters(
                        self.program,
                        definition,
                    ),
                )),
                expression,
                None,
            );
        }
        let machine = self.program.machine(self.machine_symbol)?;
        let state = if self.declaration_state.is_valid() {
            self.program
                .state_in_machine(machine, self.declaration_state)?
        } else {
            self.program.machine_states(machine).first()?
        };
        super::structural_terms::structural_term(
            self.program,
            None,
            Some((machine.symbol, self.program.state_parameters(state))),
            expression,
            None,
        )
    }

    pub(super) fn application_is_independent(
        &self,
        callee: SymbolHandle,
        selections: &[typed_trees::expression::StaticMachineArgument],
    ) -> bool {
        selected_contracts::independent_application(
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
        if let Some(definition) = self
            .program
            .mathematical_definitions()
            .iter()
            .find(|definition| definition.symbol == self.machine_symbol)
        {
            return crate::parameter_expression_result_type_reference(
                self.program,
                definition.symbol,
                &crate::mathematical_applications::ordinary_parameters(self.program, definition),
                expression,
            );
        }
        let machine = self.program.machine(self.machine_symbol)?;
        let state = if self.declaration_state.is_valid() {
            self.program
                .state_in_machine(machine, self.declaration_state)?
        } else {
            self.program.machine_states(machine).first()?
        };
        crate::parameter_expression_result_type_reference(
            self.program,
            machine.symbol,
            self.program.state_parameters(state),
            expression,
        )
    }

    fn declaration_term_pair(
        &self,
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<ObservedEqualityTerms> {
        let left = self.declaration_term(binary.left)?;
        let right = self.declaration_term(binary.right)?;
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
            let resolved = self.resolve(term.clone());
            if !super::structural_terms::scalar_operand_is_observed(
                self.program,
                reference,
                &resolved,
            ) {
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
        if let Some(definition) = self
            .program
            .mathematical_definitions()
            .iter()
            .find(|definition| definition.symbol == self.machine_symbol)
        {
            return super::structural_terms::has_identity_equality_meaning(
                self.program,
                definition.symbol,
                &crate::mathematical_applications::ordinary_parameters(self.program, definition),
                expression,
            );
        }
        let Some(machine) = self.program.machine(self.machine_symbol) else {
            return false;
        };
        let state = if self.declaration_state.is_valid() {
            self.program
                .state_in_machine(machine, self.declaration_state)
        } else {
            self.program.machine_states(machine).first()
        };
        let Some(state) = state else {
            return false;
        };
        super::structural_terms::has_identity_equality_meaning(
            self.program,
            machine.symbol,
            self.program.state_parameters(state),
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
            && binary.operator == BinaryOperator::And
            && self.has_boolean_connective_meaning(fact)
        {
            self.intake_preserved_predicate(binary.left);
            self.intake_preserved_predicate(binary.right);
        } else if let Some(term) = self.declaration_term(fact) {
            let predicate = match &term {
                StructuralTerm::MathematicalBinary { .. } => term.is_mathematical_predicate(),
                StructuralTerm::ScalarBinary { .. } => {
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
                } => {
                    return StructuralTerm::ObservedValue {
                        observed_type,
                        value: Box::new(self.resolve_at(*value, depth + 1)),
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
                        && let Some(value) = path
                            .split('.')
                            .try_fold(subject, super::observed_projections::project_field)
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
                    if let Some((_, replacement)) = self
                        .rewrites
                        .iter()
                        .find(|(pattern, _)| pattern == &resolved)
                    {
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
        let matches = program
            .machines()
            .iter()
            .flat_map(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .filter(move |state| state.symbol == target)
                    .map(move |state| (machine, state))
            })
            .collect::<Vec<_>>();
        let [(machine, state)] = matches.as_slice() else {
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
            let machine_environment = machine_parameters
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
                &machine_environment,
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

    /// COMPUTE-MODE unfolding (N3): apply a single-state proof machine of
    /// the case-arm shape to structural arguments. The desugared arm guard
    /// is `subject == Data::Case` (membership lowers to that exact Binary at
    /// parse/lowering time), so arm selection reads the guard directly: the
    /// matched argument must RESOLVE to a constructor, the arm whose case
    /// matches fires, and its value expression converts to a term under an
    /// environment of callee params -> argument terms (payload bindings are
    /// case-tagged member reads off the subject and resolve to the
    /// constructor's field terms). Any name outside the environment aborts
    /// the unfold -- callee-scope names must never leak into caller-scope
    /// judgments. `None` = no unfold (never unsound; the application just
    /// stays opaque).
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
        let machine_parameters: Vec<&typed_trees::data::TypeParameter> =
            program.machine_kind_type_parameters(machine).collect();
        if machine_parameters.len() != selections.len() {
            return None;
        }
        let machine_environment: Vec<(
            SymbolHandle,
            String,
            typed_trees::expression::StaticMachineArgument,
        )> = machine_parameters
            .iter()
            .zip(selections)
            .map(|(parameter, selected)| {
                (
                    parameter.symbol,
                    parameter.name.as_str().to_owned(),
                    selected.clone(),
                )
            })
            .collect();
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
                    &machine_environment,
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
                            &machine_environment,
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
                    let (arm_data, arm_case) = super::structural_terms::case_guard_classifier(
                        program,
                        machine,
                        Some(state),
                        guard,
                    )?;
                    let position = environment
                        .iter()
                        .position(|(name, _)| name == &subject_name)?;
                    let local_subject;
                    let subject_term = if position < parameter_count {
                        &environment[position].1
                    } else {
                        local_subject = self.resolve_at(environment[position].1.clone(), depth + 1);
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
                &machine_environment,
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
        super::observed_projections::project_field(value, field_name)
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
        self.callee_term(expression, environment, &[], scope, depth)
    }

    fn callee_term(
        &self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        machine_environment: &[(
            SymbolHandle,
            String,
            typed_trees::expression::StaticMachineArgument,
        )],
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
        if let Some(value) = self.stored_value_observation(
            scope.map(|(machine, state, _)| (machine, state)),
            expression,
        ) {
            return Some(value);
        }
        if self.storage_observation_invalidated(expression) {
            return None;
        }
        let program = self.program;
        if let Some((_, argument)) =
            crate::proof_contracts::proof_embeddings::integer_embedding_argument(
                program, expression,
            )
        {
            return super::structural_terms::integer_embedding_term(
                program,
                expression,
                self.callee_term(argument, environment, machine_environment, scope, depth + 1)?,
            );
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                if path.symbol.is_valid()
                    && (program.symbols.get(path.symbol).kind == symbols::SymbolKind::Const
                        || program.symbols.builtin_type_member(path.symbol)
                            == Some(symbols::BuiltinTypeMember::AddressBound))
                {
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
                        || super::structural_terms::static_call_receiver(program, call))
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
                                machine_environment,
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
                            machine_environment,
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
                    machine_environment,
                    scope,
                    depth + 1,
                )?;
                self.observed_field_term(receiver_term, member.member.as_str(), depth)
            }
            ExpressionNode::StructLiteral(literal) => {
                super::structural_terms::constructor_literal_term(program, literal, |field| {
                    self.callee_term(
                        field.value,
                        environment,
                        machine_environment,
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
                    machine_environment,
                    scope,
                    depth + 1,
                )?,
            ),
            ExpressionNode::Integer(value) => value.value_bignum().map(StructuralTerm::Integer),
            ExpressionNode::Float(value) => Some(StructuralTerm::Float(value.clone())),
            ExpressionNode::ZeroValue(type_reference) => {
                super::structural_terms::zero_value_structural_term(program, *type_reference)
            }
            ExpressionNode::Call(call) => {
                if (self.runtime_body_values
                    && !normal_returns::call_has_mathematical_denotation(program, call))
                    || (call.receiver.is_valid()
                        && !super::structural_terms::static_call_receiver(program, call))
                    || !call.evidence_arguments.is_empty()
                    || call.static_requirement_dispatch.is_some()
                {
                    return None;
                }
                let selected_target = machine_environment
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
                        machine_environment,
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
                Some(StructuralTerm::Application {
                    result_type,
                    target: selected_target.map_or_else(
                        || super::structural_terms::structural_call_target(program, call),
                        |selected| selected.symbol,
                    ),
                    selections: call
                        .machine_arguments
                        .iter()
                        .map(|argument| {
                            machine_environment
                                .iter()
                                .find(|(symbol, _, _)| *symbol == argument.symbol)
                                .map_or_else(
                                    || argument.clone(),
                                    |(_, _, selected)| selected.clone(),
                                )
                        })
                        .collect(),
                    machine: structural_call_machine_name(
                        call.target.as_str(),
                        &call.machine_arguments,
                        machine_environment,
                    ),
                    arguments,
                })
            }
            // A checked borrow selects its referent; it does not create a new
            // case-bearing value. Preserve aliases to the same saved subject.
            ExpressionNode::Borrow(borrow) => self.callee_term(
                borrow.target,
                environment,
                machine_environment,
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
                    machine_environment,
                    scope,
                    depth + 1,
                )?,
                self.callee_term(
                    indexed.index,
                    environment,
                    machine_environment,
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
                    machine_environment,
                    scope,
                    depth + 1,
                )?,
                self.callee_term(
                    binary.right,
                    environment,
                    machine_environment,
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
            } => StructuralTerm::ObservedValue {
                observed_type: *observed_type,
                value: Box::new(Self::substitute_call_term(value, map, machine_map)),
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
        match term {
            StructuralTerm::IndexedValue {
                collection, index, ..
            } => {
                Self::self_applications(collection, selected_target, found);
                Self::self_applications(index, selected_target, found);
            }
            StructuralTerm::Projection { subject, .. }
            | StructuralTerm::IntegerEmbedding { value: subject, .. } => {
                Self::self_applications(subject, selected_target, found)
            }
            StructuralTerm::ScalarBinary { left, right, .. }
            | StructuralTerm::MathematicalBinary { left, right, .. } => {
                Self::self_applications(left, selected_target, found);
                Self::self_applications(right, selected_target, found);
            }
            StructuralTerm::Application {
                result_type: _,
                target,
                selections: _,
                machine: _,
                arguments,
            } => {
                if *target == selected_target {
                    found.push(term);
                }
                for argument in arguments {
                    Self::self_applications(argument, selected_target, found);
                }
            }
            StructuralTerm::CallProjection { arguments, .. } => {
                for argument in arguments {
                    Self::self_applications(argument, selected_target, found);
                }
            }
            StructuralTerm::Constructor { fields, .. } => {
                for (_, value) in fields {
                    Self::self_applications(value, selected_target, found);
                }
            }
            _ => {}
        }
    }

    pub(super) fn judge(
        &self,
        program: &dyn crate::CheckingInput,
        fact: ExpressionHandle,
    ) -> StructuralJudgment {
        if self.requires_value_preservation() {
            let mut expressions = Vec::new();
            crate::expression_traversal::collect_expression_tree(program, fact, &mut expressions);
            if expressions
                .iter()
                .any(|expression| self.storage_observation_invalidated(*expression))
            {
                // Entry equations describe the old contents. A source-level
                // exit predicate cannot reinterpret them after a write;
                // captured return values remain independent observations.
                return StructuralJudgment::Unknown;
            }
        }
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
            // Mathematical facts consume their prepared term in the same
            // judgment entrance, including proof-only Strict results. A result
            // carrier or source call shape does not select another body walk.
            let mut engine =
                super::arithmetic_judgment::Engine::for_proof_integer_formation(self.program);
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
                match (
                    self.judge(program, binary.left),
                    self.judge(program, binary.right),
                ) {
                    (StructuralJudgment::Proven, StructuralJudgment::Proven) => {
                        StructuralJudgment::Proven
                    }
                    (StructuralJudgment::Refuted, _) | (_, StructuralJudgment::Refuted) => {
                        StructuralJudgment::Refuted
                    }
                    _ => StructuralJudgment::Unknown,
                }
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
                if let Some(holds) = crate::proof_contracts::float_projection_bindings::semantic_values::closed_float_meaning_equality(
                    program, self.machine_symbol, fact,
                ) {
                    return if holds { StructuralJudgment::Proven } else { StructuralJudgment::Refuted };
                }
                if !self.has_identity_equality_meaning(fact) {
                    return StructuralJudgment::Unknown;
                }
                let Some(terms) = self.declaration_term_pair(binary) else {
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
            let mut definitions = self
                .program
                .data_definitions()
                .iter()
                .filter(|definition| definition.name.as_str() == data);
            let Some(definition) = definitions.next() else {
                return StructuralJudgment::Unknown;
            };
            if definitions.next().is_some() || definition.symbol != data_symbol {
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
            } => Some(StructuralTerm::ObservedValue {
                observed_type,
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
        if active.contains(&reference) || active.len() >= 32 {
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
            let mut frontier: Vec<Vec<StructuralTerm>> = vec![left_addends.clone()];
            for _depth in 0..2 {
                let mut next: Vec<Vec<StructuralTerm>> = Vec::new();
                for current in &frontier {
                    for (pattern, replacement) in self.rewrites.iter() {
                        for (from, to) in [(pattern, replacement), (replacement, pattern)] {
                            let mut from_addends = Vec::new();
                            additive_addends(from, op, &mut from_addends);
                            let mut to_addends = Vec::new();
                            additive_addends(to, op, &mut to_addends);
                            from_addends.sort_by_key(|term| format!("{term:?}"));
                            let Some(mut candidate) =
                                sorted_multiset_subtract(current, &from_addends)
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
