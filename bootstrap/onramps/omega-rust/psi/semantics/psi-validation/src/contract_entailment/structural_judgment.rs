use super::*;
use std::cell::Cell;

pub(super) enum StructuralJudgment {
    Proven,
    Refuted,
    Unknown,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum StructuralTerm {
    Variable(String),
    /// data name, case name, named payload fields (sorted by field name;
    /// empty for a nullary classifier like `Nat::Zero`). Payload-carrying
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
    /// machine selections are encoded in `machine`, so `f<A>` and `f<B>`
    /// remain distinct terms and generic unfolding can alpha-substitute them.
    /// Resolution UNFOLDS it when the callee is a single-state proof
    /// machine of the case-arm shape and the matched argument resolves to
    /// a constructor -- the compute-mode of N3's operator routing.
    Application {
        machine: String,
        arguments: Vec<StructuralTerm>,
    },
    /// Anything else, compared by canonical display name only.
    Opaque(String),
}

/// REARRANGE-MODE license (settle 2026-07-18, rung C): a carrier EARNS ring
/// canonicalization over an op through EXPLICIT conformance, never
/// scope-sniffing. A license exists for op machine `add_machine` when some
/// trait declares an op slot with BOTH a commutativity law and an
/// associativity law over it (detected by SHAPE, not by name -- `R(x, y) ==
/// R(y, x)` and `R(R(x, y), z) == R(x, R(y, z))` with distinct requirement
/// params), the op slot is conformed by `add_machine`, and BOTH law slots
/// have satisfiers for the same carrier (whose proofs rung B already
/// machine-checked against the declared laws).
#[derive(Clone, Debug)]
struct RingLicense {
    add_machine: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProvedIndexAlgebra {
    pub trait_symbol: SymbolHandle,
    pub requirement: String,
    pub alias: Option<String>,
}

/// Exact proved associative/commutative algebra instances supplied by one
/// operation provider. PDI3 records one of these beside the selected public
/// operator contract; an absent or plural result is not normalization
/// authority.
pub(crate) fn proved_index_algebras_for_provider(
    program: &TypedTrees,
    provider: &Machine,
) -> Vec<ProvedIndexAlgebra> {
    let Some(entry) = program.machine_states(provider).first() else {
        return Vec::new();
    };
    let carrier = program
        .state_parameters(entry)
        .first()
        .map(|parameter| parameter.type_reference)
        .unwrap_or(entry.return_type);
    let mut algebras = Vec::new();

    for conformance in program.machine_trait_conformances(provider) {
        let Some(requirement) = conformance.requirement.as_ref() else {
            continue;
        };
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == conformance.symbol)
        else {
            continue;
        };
        let mut comm_laws = Vec::new();
        let mut assoc_laws = Vec::new();
        for law in program.trait_machine_signatures(trait_definition) {
            let parameters = program
                .state_signature_parameters(law)
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect::<Vec<_>>();
            for contract in program.state_signature_contracts(law) {
                if contract.kind != SignatureContractKind::Ensures {
                    continue;
                }
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Expression(expression) = fact else {
                        continue;
                    };
                    let mut conjuncts = Vec::new();
                    collect_equality_conjuncts(program, *expression, &mut conjuncts);
                    for conjunct in conjuncts {
                        let ExpressionNode::Binary(binary) =
                            program.expression_table.expression(conjunct)
                        else {
                            continue;
                        };
                        let (Some(left), Some(right)) = (
                            structural_term(program, binary.left),
                            structural_term(program, binary.right),
                        ) else {
                            continue;
                        };
                        if commutativity_shape(&left, &right, &parameters).as_deref()
                            == Some(requirement.as_str())
                        {
                            comm_laws.push(law.name.as_str().to_owned());
                        }
                        if associativity_shape(&left, &right, &parameters).as_deref()
                            == Some(requirement.as_str())
                        {
                            assoc_laws.push(law.name.as_str().to_owned());
                        }
                    }
                }
            }
        }
        let alias = conformance.alias.as_ref().map(|alias| alias.as_str());
        let licensed = comm_laws.iter().any(|law| {
            slot_satisfier_exists_for_alias(program, trait_definition, law, carrier, alias)
        }) && assoc_laws.iter().any(|law| {
            slot_satisfier_exists_for_alias(program, trait_definition, law, carrier, alias)
        });
        if licensed {
            algebras.push(ProvedIndexAlgebra {
                trait_symbol: trait_definition.symbol,
                requirement: requirement.as_str().to_owned(),
                alias: alias.map(str::to_owned),
            });
        }
    }
    algebras.sort_by(|left, right| {
        (
            left.trait_symbol.arena_index(),
            &left.requirement,
            &left.alias,
        )
            .cmp(&(
                right.trait_symbol.arena_index(),
                &right.requirement,
                &right.alias,
            ))
    });
    algebras.dedup();
    algebras
}

/// Tier-2 (full polynomial): the PAIRED license -- an add op and a mul op
/// each carrying comm+assoc, connected by a conformed DISTRIBUTIVITY law.
#[derive(Clone)]
struct SemiringLicense {
    add_machine: String,
    mul_machine: String,
}

pub(super) struct StructuralJudge<'program> {
    program: &'program TypedTrees,
    pub(super) substitutions: Vec<(String, StructuralTerm)>,
    /// Application REWRITES (`add_zero_right(prev) -> prev`): hypothesis
    /// equations with an application side orient REDUCING -- the inductive
    /// hypothesis rewrites the self-application away instead of expanding a
    /// variable into it, which also serves asymmetric goals.
    rewrites: Vec<(StructuralTerm, StructuralTerm)>,
    pub(super) hypotheses_contradictory: bool,
    ring_licenses: Vec<RingLicense>,
    /// Tier-2: paired add/mul licenses with a conformed distributivity law.
    semiring_licenses: Vec<SemiringLicense>,
    /// The last exact source-ordered machine selected for structural unfolding.
    /// Every hit rechecks the complete name/attachment predicate; a miss still
    /// scans from the beginning, so this hint cannot change first-match order.
    unfold_machine_hint: Cell<Option<usize>>,
}

impl Clone for StructuralJudge<'_> {
    fn clone(&self) -> Self {
        Self {
            program: self.program,
            substitutions: self.substitutions.clone(),
            rewrites: self.rewrites.clone(),
            hypotheses_contradictory: self.hypotheses_contradictory,
            ring_licenses: self.ring_licenses.clone(),
            semiring_licenses: self.semiring_licenses.clone(),
            unfold_machine_hint: Cell::new(self.unfold_machine_hint.get()),
        }
    }
}

impl<'program> StructuralJudge<'program> {
    pub(super) fn from_requires(
        program: &'program TypedTrees,
        judged_machine: &Machine,
        requires: &[ExpressionHandle],
    ) -> Self {
        let mut judge = Self {
            program,
            substitutions: Vec::new(),
            rewrites: Vec::new(),
            hypotheses_contradictory: false,
            ring_licenses: compute_ring_licenses(program, judged_machine),
            semiring_licenses: compute_semiring_licenses(program, judged_machine),
            unfold_machine_hint: Cell::new(None),
        };
        for fact in requires {
            judge.intake(program, *fact);
        }
        judge
    }

    pub(super) fn intake(&mut self, program: &TypedTrees, fact: ExpressionHandle) {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
            return;
        };
        match binary.operator {
            BinaryOperator::And => {
                self.intake(program, binary.left);
                self.intake(program, binary.right);
            }
            BinaryOperator::Equal => {
                let (Some(left), Some(right)) = (
                    structural_term(program, binary.left),
                    structural_term(program, binary.right),
                ) else {
                    return;
                };
                self.intake_equation(left, right, 0);
            }
            _ => {}
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
        if depth >= 32 {
            return;
        }
        let left = self.resolve(left);
        let right = self.resolve(right);
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
            (StructuralTerm::Application { .. }, _) => {
                if !term_contains(&right, &left) {
                    self.rewrites.push((left, right));
                }
            }
            (_, StructuralTerm::Application { .. }) => {
                if !term_contains(&left, &right) {
                    self.rewrites.push((right, left));
                }
            }
            (StructuralTerm::Variable(name), _) => {
                if left != right {
                    self.substitutions.push((name.clone(), right));
                }
            }
            (_, StructuralTerm::Variable(name)) => {
                self.substitutions.push((name.clone(), left));
            }
            _ => {}
        }
    }

    /// Follow variable substitutions to a fixpoint, depth-capped (a cyclic
    /// substitution chain resolves to wherever the cap lands, which only
    /// weakens judgments toward Unknown -- never unsound). Constructor
    /// fields resolve recursively under the same budget.
    pub(super) fn resolve(&self, term: StructuralTerm) -> StructuralTerm {
        self.resolve_at(term, 0)
    }

    fn resolve_at(&self, mut term: StructuralTerm, depth: usize) -> StructuralTerm {
        if depth >= 32 {
            return term;
        }
        for _ in 0..32 {
            match term {
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
                StructuralTerm::Application { machine, arguments } => {
                    let arguments: Vec<StructuralTerm> = arguments
                        .into_iter()
                        .map(|argument| self.resolve_at(argument, depth + 1))
                        .collect();
                    let resolved = StructuralTerm::Application { machine, arguments };
                    // Hypothesis rewrites first (the inductive hypothesis
                    // reduces the self-application), then unfolding.
                    if let Some((_, replacement)) = self
                        .rewrites
                        .iter()
                        .find(|(pattern, _)| pattern == &resolved)
                    {
                        term = replacement.clone();
                        continue;
                    }
                    let StructuralTerm::Application { machine, arguments } = &resolved else {
                        unreachable!();
                    };
                    if let Some(unfolded) = self.unfold_application(machine, arguments, depth + 1) {
                        term = unfolded;
                        continue;
                    }
                    return resolved;
                }
                StructuralTerm::Opaque(_) => return term,
            }
        }
        term
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
        machine_name: &str,
        arguments: &[StructuralTerm],
        depth: usize,
    ) -> Option<StructuralTerm> {
        if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
            eprintln!("STRUCT unfold? {machine_name} args {arguments:?} depth {depth}");
        }
        if depth >= 32 {
            return None;
        }
        let program = self.program;
        let (machine_name, selected_machines) = split_structural_machine_name(machine_name);
        let machines = program.machines();
        let matches = |machine: &Machine| {
            machine.attached_data.is_none() && machine.name.as_str() == machine_name
        };
        let machine_index = self
            .unfold_machine_hint
            .get()
            .filter(|index| machines.get(*index).is_some_and(matches))
            .or_else(|| machines.iter().position(matches))?;
        self.unfold_machine_hint.set(Some(machine_index));
        let machine = &machines[machine_index];
        let machine_parameters: Vec<&psi_typed_trees::data::TypeParameter> = program
            .machine_type_parameters(machine)
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind,
                    psi_typed_trees::data::TypeParameterKind::Machine { .. }
                )
            })
            .collect();
        if machine_parameters.len() != selected_machines.len() {
            return None;
        }
        let machine_environment: Vec<(String, String)> = machine_parameters
            .iter()
            .zip(selected_machines)
            .map(|(parameter, selected)| (parameter.name.as_str().to_owned(), selected.to_owned()))
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

        // CITE the callee's proven ensures first (extraction into consumer
        // proofs): a lemma with a functional `ensures result == <term>`
        // abstracts its body, so instantiating that ensures under the call
        // environment yields the result directly -- and it is the ONLY route
        // for an INDUCTIVE lemma whose body never finitely unfolds for a
        // symbolic argument (`add_zero_right(a) == a`). Sound because the
        // callee's ensures is proven in the same validation batch (a false
        // one raises its own error, so no compiling program cites an
        // unproven fact). Prefer it over body unfolding. REQUIRES-bearing
        // callees are EXCLUDED: their ensures is conditional and this path
        // has no site to discharge the condition at -- injecting it
        // unconditioned would be unsound (probed 2026-07-16). Their BODY
        // still unfolds below (computation is unconditional).
        let requires_bearing = program.machine_contracts(machine).iter().any(|contract| {
            matches!(
                contract.kind,
                psi_typed_trees::signature::SignatureContractKind::Requires
            ) && !program.proof_facts.span_or_empty(contract.facts).is_empty()
        });
        if !requires_bearing {
            for contract in program.machine_contracts(machine) {
                if !matches!(
                    contract.kind,
                    psi_typed_trees::signature::SignatureContractKind::Ensures
                ) {
                    continue;
                }
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Expression(expression) = fact else {
                        continue;
                    };
                    if let Some(term) = self.functional_ensures_result(
                        *expression,
                        &environment,
                        &machine_environment,
                        depth + 1,
                    ) {
                        return Some(term);
                    }
                }
            }
        }

        let mut environment = environment;
        for statement in program.statement_table.statements(state.statement_nodes) {
            // A `let` (spelled, or the lowering's __hoist_N of a call-valued
            // terminal -- e.g. a definitional wrapper like
            // `snoc(s, x) = (append(s, [x]))`) BINDS: its initializer
            // termifies under the environment built so far and the local
            // joins it, so the terminal's name resolves. Mirrors the
            // sole-arm and case-arm recognizers.
            if is_arm_pattern_marker(statement) {
                continue; // exhaustiveness carrier, not shape
            }
            if let StatementNode::LocalData(local) = statement {
                let term = self.callee_term_with_machines(
                    local.initial_value,
                    &environment,
                    &machine_environment,
                    depth + 1,
                )?;
                environment.push((local.name.as_str().to_owned(), term));
                continue;
            }
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            if transition.continuation.is_valid() {
                return None;
            }
            let fires = match transition.guard {
                TransitionGuardNode::Always => true,
                TransitionGuardNode::When(guard) => {
                    let ExpressionNode::Binary(comparison) =
                        program.expression_table.expression(guard)
                    else {
                        return None;
                    };
                    if comparison.operator != BinaryOperator::Equal {
                        return None;
                    }
                    let subject = structural_term(program, comparison.left)?;
                    let StructuralTerm::Variable(subject_name) = subject else {
                        return None;
                    };
                    let case = structural_term(program, comparison.right)?;
                    let StructuralTerm::Constructor {
                        case: arm_case,
                        fields: arm_fields,
                        ..
                    } = case
                    else {
                        return None;
                    };
                    if !arm_fields.is_empty() {
                        return None;
                    }
                    let (_, subject_term) =
                        environment.iter().find(|(name, _)| name == &subject_name)?;
                    let StructuralTerm::Constructor { case: got_case, .. } =
                        self.resolve_at(subject_term.clone(), depth + 1)
                    else {
                        // The matched argument is not (yet) a constructor:
                        // arm selection is undecidable, no unfold.
                        return None;
                    };
                    got_case == arm_case
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
            return self.callee_term_with_machines(
                *value,
                &environment,
                &machine_environment,
                depth + 1,
            );
        }
        None
    }

    /// If `ensures_fact` is exactly `result == <term>` (either orientation),
    /// convert `<term>` under the call environment -- the functional-result
    /// abstraction of a lemma. `None` for any other ensures shape.
    fn functional_ensures_result(
        &self,
        ensures_fact: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        machine_environment: &[(String, String)],
        depth: usize,
    ) -> Option<StructuralTerm> {
        let program = self.program;
        let ExpressionNode::Binary(binary) = program.expression_table.expression(ensures_fact)
        else {
            return None;
        };
        if binary.operator != BinaryOperator::Equal {
            return None;
        }
        let is_result = |handle: ExpressionHandle| {
            matches!(
                program.expression_table.expression(handle),
                ExpressionNode::Name(path)
                    if matches!(
                        program.expression_table.name_path_members(path.members),
                        [only] if only.as_str() == RESULT_BINDER
                    )
            )
        };
        let value = if is_result(binary.left) {
            binary.right
        } else if is_result(binary.right) {
            binary.left
        } else {
            return None;
        };
        self.callee_term_with_machines(value, environment, machine_environment, depth)
    }

    /// Convert a callee-body expression to a term under the call
    /// environment. Names must be callee parameters; case-tagged member
    /// reads (`a.prev`) index the bound constructor's fields; case literals
    /// and nested free calls recurse. Anything else aborts (None).
    pub(super) fn callee_term(
        &self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        depth: usize,
    ) -> Option<StructuralTerm> {
        self.callee_term_with_machines(expression, environment, &[], depth)
    }

    fn callee_term_with_machines(
        &self,
        expression: ExpressionHandle,
        environment: &[(String, StructuralTerm)],
        machine_environment: &[(String, String)],
        depth: usize,
    ) -> Option<StructuralTerm> {
        if depth >= 32 {
            return None;
        }
        let program = self.program;
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                match members {
                    [single] => environment
                        .iter()
                        .find(|(name, _)| name == single.as_str())
                        .map(|(_, term)| term.clone()),
                    [first, second] => program
                        .data_definitions()
                        .iter()
                        .any(|definition| definition.name.as_str() == first.as_str())
                        .then(|| StructuralTerm::Constructor {
                            data: first.as_str().to_owned(),
                            case: second.as_str().to_owned(),
                            fields: Vec::new(),
                        }),
                    _ => None,
                }
            }
            ExpressionNode::Member(member) => {
                let receiver_term = self.callee_term_with_machines(
                    member.receiver,
                    environment,
                    machine_environment,
                    depth + 1,
                )?;
                match self.resolve_at(receiver_term, depth + 1) {
                    StructuralTerm::Constructor { fields, .. } => fields
                        .iter()
                        .find(|(name, _)| name == member.member.as_str())
                        .map(|(_, term)| term.clone()),
                    // A field read off a SYMBOLIC receiver names the caller's
                    // possibly nested place in the shared Opaque vocabulary --
                    // exactly how the caller-side termifier spells `a.num.neg`
                    // (display name), so citations over the same place line up.
                    StructuralTerm::Variable(name) => Some(StructuralTerm::Opaque(format!(
                        "{name}.{}",
                        member.member.as_str()
                    ))),
                    StructuralTerm::Opaque(inner) => Some(StructuralTerm::Opaque(format!(
                        "{inner}.{}",
                        member.member.as_str()
                    ))),
                    StructuralTerm::Application { .. } => None,
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                // Records (no case name) term as empty-case constructors,
                // mirroring the caller-side termifier.
                let case = literal
                    .case_name
                    .as_ref()
                    .map(|case| case.as_str())
                    .unwrap_or("");
                let mut fields: Vec<(String, StructuralTerm)> = Vec::new();
                for field in program.expression_table.struct_fields(literal.fields) {
                    fields.push((
                        field.name.as_str().to_owned(),
                        self.callee_term_with_machines(
                            field.value,
                            environment,
                            machine_environment,
                            depth + 1,
                        )?,
                    ));
                }
                fields.sort_by(|(left, _), (right, _)| left.cmp(right));
                Some(StructuralTerm::Constructor {
                    data: literal.type_name.as_str().to_owned(),
                    case: case.to_owned(),
                    fields,
                })
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    return None;
                }
                let mut arguments = Vec::new();
                for argument in program.expression_table.expression_handles(call.arguments) {
                    arguments.push(self.callee_term_with_machines(
                        *argument,
                        environment,
                        machine_environment,
                        depth + 1,
                    )?);
                }
                Some(StructuralTerm::Application {
                    machine: structural_call_machine_name(
                        call.target.as_str(),
                        &call.machine_arguments,
                        machine_environment,
                    ),
                    arguments,
                })
            }
            ExpressionNode::Boolean(value) => Some(StructuralTerm::Constructor {
                data: "bool".to_owned(),
                case: value.to_string(),
                fields: Vec::new(),
            }),
            // A structural theorem may recurse on an ordinary scalar
            // measure (`build(n - 1)`) while its result lives in proof data.
            // The structural judge does not interpret that scalar algebra;
            // retain it as an opaque operand so the self-application has the
            // correct arity and identity.  The separate arithmetic recursion
            // validator is solely responsible for proving the edge decreases.
            ExpressionNode::Binary(_) | ExpressionNode::Integer(_) => Some(StructuralTerm::Opaque(
                program.expression_table.display_name(expression),
            )),
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
        match term {
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
                    .map(|(name, value)| (name.clone(), Self::substitute_term(value, map)))
                    .collect(),
            },
            StructuralTerm::Application { machine, arguments } => StructuralTerm::Application {
                machine: machine.clone(),
                arguments: arguments
                    .iter()
                    .map(|argument| Self::substitute_term(argument, map))
                    .collect(),
            },
            StructuralTerm::Opaque(display) => {
                // Symbolic record member places currently share the Opaque
                // vocabulary (`p.den`). Citation instantiation must still
                // alpha-substitute their exact root parameter; otherwise a
                // cited Rat law leaks callee names into the caller frame.
                // Restrict the rewrite to an exact `<parameter>.` prefix.
                // A symbolic static-machine application is also a legitimate
                // place root (`Middle(index).den`); retain that projection in
                // the Opaque place vocabulary.  A concrete constructor can be
                // projected structurally.  Arbitrary opaque arithmetic never
                // gains substring-rewrite semantics.
                for (parameter, replacement) in map {
                    let prefix = format!("{parameter}.");
                    let Some(suffix) = display.strip_prefix(&prefix) else {
                        continue;
                    };
                    return match replacement {
                        StructuralTerm::Variable(root) | StructuralTerm::Opaque(root) => {
                            StructuralTerm::Opaque(format!("{root}.{suffix}"))
                        }
                        StructuralTerm::Application { .. } => StructuralTerm::Opaque(format!(
                            "{}.{suffix}",
                            display_structural_term(replacement)
                        )),
                        StructuralTerm::Constructor { fields, .. } => fields
                            .iter()
                            .find(|(name, _)| name == suffix)
                            .map(|(_, value)| value.clone())
                            .unwrap_or_else(|| term.clone()),
                    };
                }
                term.clone()
            }
        }
    }

    /// Collect every self-application (calls to `machine_name`) in a term.
    pub(super) fn self_applications<'term>(
        term: &'term StructuralTerm,
        machine_name: &str,
        found: &mut Vec<&'term StructuralTerm>,
    ) {
        match term {
            StructuralTerm::Application { machine, arguments } => {
                if machine == machine_name {
                    found.push(term);
                }
                for argument in arguments {
                    Self::self_applications(argument, machine_name, found);
                }
            }
            StructuralTerm::Constructor { fields, .. } => {
                for (_, value) in fields {
                    Self::self_applications(value, machine_name, found);
                }
            }
            _ => {}
        }
    }

    pub(super) fn judge(&self, program: &TypedTrees, fact: ExpressionHandle) -> StructuralJudgment {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(fact) else {
            // A boolean-valued proof call is itself a proposition. Resolve a
            // closed checked application exactly as `call == true`; N6
            // equivalence laws use this ordinary contract shape.
            let Some(term) = structural_term(program, fact) else {
                return StructuralJudgment::Unknown;
            };
            return match self.resolve(term) {
                StructuralTerm::Constructor { data, case, fields }
                    if data == "bool" && case == "true" && fields.is_empty() =>
                {
                    StructuralJudgment::Proven
                }
                StructuralTerm::Constructor { data, case, fields }
                    if data == "bool" && case == "false" && fields.is_empty() =>
                {
                    StructuralJudgment::Refuted
                }
                _ => StructuralJudgment::Unknown,
            };
        };
        match binary.operator {
            BinaryOperator::And => {
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
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                let (Some(left), Some(right)) = (
                    structural_term(program, binary.left),
                    structural_term(program, binary.right),
                ) else {
                    return StructuralJudgment::Unknown;
                };
                let equality = self.judge_equation(self.resolve(left), self.resolve(right), 0);
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
            _ => StructuralJudgment::Unknown,
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
        if depth >= 32 {
            return StructuralJudgment::Unknown;
        }
        if left == right {
            return StructuralJudgment::Proven;
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
        let Some(definition) = self
            .program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == data.as_str())
        else {
            return false;
        };
        let declared: Vec<&str> = self
            .program
            .data_members(definition)
            .iter()
            .filter_map(|member| match member {
                psi_typed_trees::data::DataMember::Field(field) => Some(field.name.as_str()),
                psi_typed_trees::data::DataMember::Variant(_) => None,
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
    /// else is an atom by canonical display) and compare. At least two
    /// addends must appear -- a single atom has nothing to rearrange.
    fn ring_rearranged_equal(&self, left: &StructuralTerm, right: &StructuralTerm) -> bool {
        for license in &self.ring_licenses {
            let op = license.add_machine.as_str();
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
            left_addends.sort();
            right_addends.sort();
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
            let mut frontier: Vec<Vec<String>> = vec![left_addends.clone()];
            for _depth in 0..2 {
                let mut next: Vec<Vec<String>> = Vec::new();
                for current in &frontier {
                    for (pattern, replacement) in &self.rewrites {
                        for (from, to) in [(pattern, replacement), (replacement, pattern)] {
                            let mut from_addends = Vec::new();
                            additive_addends(from, op, &mut from_addends);
                            let mut to_addends = Vec::new();
                            additive_addends(to, op, &mut to_addends);
                            from_addends.sort();
                            let Some(mut candidate) =
                                sorted_multiset_subtract(current, &from_addends)
                            else {
                                continue;
                            };
                            candidate.extend(to_addends.iter().cloned());
                            candidate.sort();
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
            left_poly.sort();
            right_poly.sort();
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
            let mut atoms: Vec<String> = left_poly
                .iter()
                .chain(right_poly.iter())
                .flatten()
                .cloned()
                .collect();
            atoms.sort();
            atoms.dedup();
            let mut scales: Vec<Vec<String>> = vec![Vec::new()];
            scales.extend(atoms.into_iter().map(|atom| vec![atom]));
            let mut hypothesis_polys: Vec<(Vec<Vec<String>>, Vec<Vec<String>>)> = Vec::new();
            for (pattern, replacement) in &self.rewrites {
                if let (Some(mut hl), Some(mut hr)) = (
                    polynomial_normal_form(pattern, license),
                    polynomial_normal_form(replacement, license),
                ) {
                    hl.sort();
                    hr.sort();
                    hypothesis_polys.push((hl, hr));
                }
            }
            let scaled = |poly: &[Vec<String>], scale: &[String]| -> Vec<Vec<String>> {
                poly.iter()
                    .map(|monomial| {
                        let mut product = monomial.clone();
                        product.extend(scale.iter().cloned());
                        product.sort();
                        product
                    })
                    .collect()
            };
            let mut frontier: Vec<Vec<Vec<String>>> = vec![left_poly.clone()];
            for _depth in 0..2 {
                let mut next: Vec<Vec<Vec<String>>> = Vec::new();
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
                                candidate.sort();
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

/// Distribute the licensed mul through the licensed add: a term becomes a
/// list of MONOMIALS (sorted factor lists). `None` past the size cap (the
/// cross product is quadratic; a runaway form refuses into the ordinary
/// path rather than stalling).
fn polynomial_normal_form(
    term: &StructuralTerm,
    license: &SemiringLicense,
) -> Option<Vec<Vec<String>>> {
    const MONOMIAL_CAP: usize = 64;
    if let StructuralTerm::Application { machine, arguments } = term
        && arguments.len() == 2
    {
        if *machine == license.add_machine {
            let mut left = polynomial_normal_form(&arguments[0], license)?;
            let right = polynomial_normal_form(&arguments[1], license)?;
            left.extend(right);
            return (left.len() <= MONOMIAL_CAP).then_some(left);
        }
        if *machine == license.mul_machine {
            let left = polynomial_normal_form(&arguments[0], license)?;
            let right = polynomial_normal_form(&arguments[1], license)?;
            let mut product = Vec::new();
            for left_monomial in &left {
                for right_monomial in &right {
                    let mut monomial = left_monomial.clone();
                    monomial.extend(right_monomial.iter().cloned());
                    monomial.sort();
                    product.push(monomial);
                }
            }
            return (product.len() <= MONOMIAL_CAP).then_some(product);
        }
    }
    Some(vec![vec![display_structural_term(term)]])
}

/// `left - from` as multisets; `None` when `from` is not a sub-multiset of
/// `left` (the exchange does not apply). Generic over the element (tier-1
/// addend displays, tier-2 monomial factor lists).
fn sorted_multiset_subtract<T: Clone + PartialEq>(left: &[T], from: &[T]) -> Option<Vec<T>> {
    let mut remaining = left.to_vec();
    for item in from {
        let index = remaining.iter().position(|candidate| candidate == item)?;
        remaining.remove(index);
    }
    Some(remaining)
}

/// Flatten nested applications of the licensed op into its addend list; any
/// other term is one addend, compared by canonical display (the Opaque
/// discipline).
fn additive_addends(term: &StructuralTerm, op: &str, out: &mut Vec<String>) {
    if let StructuralTerm::Application { machine, arguments } = term {
        if machine == op && arguments.len() == 2 {
            additive_addends(&arguments[0], op, out);
            additive_addends(&arguments[1], op, out);
            return;
        }
    }
    out.push(display_structural_term(term));
}

fn term_uses_application(term: &StructuralTerm, op: &str) -> bool {
    match term {
        StructuralTerm::Application { machine, arguments } => {
            machine == op
                || arguments
                    .iter()
                    .any(|argument| term_uses_application(argument, op))
        }
        StructuralTerm::Constructor { fields, .. } => fields
            .iter()
            .any(|(_, value)| term_uses_application(value, op)),
        _ => false,
    }
}

/// Compute the program's REARRANGE licenses (settle 2026-07-18): for every
/// trait, find op slots carrying BOTH a commutativity law and an
/// associativity law (matched by SHAPE over the trait's own requirement
/// names), then license each conforming op machine whose carrier also has
/// satisfiers for both law slots. Conformance is the license -- rung B
/// machine-checked those satisfiers against the declared laws, so the
/// closure the canonicalizer assumes is exactly what the carrier proved.
///
/// NO CIRCULAR LICENSING: a machine that itself binds a comm/assoc LAW slot
/// of a trait gets NO licenses from that trait -- the axiom base always
/// proves ring-free. This kills self-licensing (add_comm rearranging its own
/// goal into triviality) AND multi-machine cycles (two comm satisfiers each
/// licensed by the other's conformance, none carrying a real proof).
fn compute_ring_licenses(program: &TypedTrees, judged_machine: &Machine) -> Vec<RingLicense> {
    let mut licenses = Vec::new();

    for trait_definition in program.traits() {
        // Op slot name -> (has commutativity law named, has associativity law
        // named): the LAW requirement names matter later (their satisfiers
        // must exist for the carrier).
        let mut comm_laws: Vec<(String, String)> = Vec::new(); // (op, law requirement)
        let mut assoc_laws: Vec<(String, String)> = Vec::new();

        for requirement in program.trait_machine_signatures(trait_definition) {
            let parameters: Vec<String> = program
                .state_signature_parameters(requirement)
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect();
            for contract in program.state_signature_contracts(requirement) {
                if contract.kind != SignatureContractKind::Ensures {
                    continue;
                }
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Expression(expression) = fact else {
                        continue;
                    };
                    let mut conjuncts = Vec::new();
                    collect_equality_conjuncts(program, *expression, &mut conjuncts);
                    for conjunct in conjuncts {
                        let ExpressionNode::Binary(binary) =
                            program.expression_table.expression(conjunct)
                        else {
                            continue;
                        };
                        let (Some(left), Some(right)) = (
                            structural_term(program, binary.left),
                            structural_term(program, binary.right),
                        ) else {
                            continue;
                        };
                        if let Some(op) = commutativity_shape(&left, &right, &parameters) {
                            comm_laws.push((op, requirement.name.as_str().to_owned()));
                        }
                        if let Some(op) = associativity_shape(&left, &right, &parameters) {
                            assoc_laws.push((op, requirement.name.as_str().to_owned()));
                        }
                    }
                }
            }
        }

        // PER-LICENSE circularity break (refined 2026-07-16 from the old
        // trait-wide skip): the judged machine is excluded only from
        // licenses it ITSELF underpins -- the ones whose comm/assoc law
        // slots it binds FOR THE SAME CARRIER. A law lemma's goal is
        // exactly the law shape over its own op, so no other carrier's
        // license can rearrange it -- per-carrier exclusion breaks every
        // cycle while letting IntPair's mul_comm keep using NAT's earned
        // licenses (the trait-wide skip wrongly stripped those).
        let judged_bound_laws: Vec<String> = program
            .machine_trait_conformances(judged_machine)
            .iter()
            .filter(|conformance| conformance.symbol == trait_definition.symbol)
            .filter_map(|conformance| {
                conformance
                    .requirement
                    .as_ref()
                    .map(|name| name.as_str().to_owned())
                    .or_else(|| {
                        judged_machine
                            .attached_data
                            .is_none()
                            .then(|| judged_machine.name.as_str().to_owned())
                    })
            })
            .collect();
        let judged_carrier = program.machine_states(judged_machine).first().map(|entry| {
            program
                .state_parameters(entry)
                .first()
                .map(|parameter| parameter.type_reference)
                .unwrap_or(entry.return_type)
        });

        for (op_slot, comm_law) in &comm_laws {
            let Some((_, assoc_law)) = assoc_laws.iter().find(|(op, _)| op == op_slot) else {
                continue;
            };
            // Every machine conforming the op slot is a candidate license --
            // provided its carrier also conformed BOTH law slots.
            for candidate in program.machines() {
                for conformance in program.machine_trait_conformances(candidate) {
                    if conformance.symbol != trait_definition.symbol {
                        continue;
                    }
                    let bound_requirement = conformance
                        .requirement
                        .as_ref()
                        .map(|name| name.as_str().to_owned())
                        .or_else(|| {
                            candidate
                                .attached_data
                                .is_none()
                                .then(|| candidate.name.as_str().to_owned())
                        });
                    if bound_requirement.as_deref() != Some(op_slot.as_str()) {
                        continue;
                    }
                    let Some(candidate_entry) = program.machine_states(candidate).first() else {
                        continue;
                    };
                    let carrier = program
                        .state_parameters(candidate_entry)
                        .first()
                        .map(|parameter| parameter.type_reference)
                        .unwrap_or(candidate_entry.return_type);
                    let judged_underpins_this_license = judged_bound_laws
                        .iter()
                        .any(|law| law == comm_law || law == assoc_law)
                        && judged_carrier.is_some_and(|judged| {
                            crate::type_references::type_references_match(program, judged, carrier)
                        });
                    if judged_underpins_this_license {
                        continue;
                    }
                    if slot_satisfier_exists(program, trait_definition, comm_law, carrier)
                        && slot_satisfier_exists(program, trait_definition, assoc_law, carrier)
                    {
                        licenses.push(RingLicense {
                            add_machine: candidate.name.as_str().to_owned(),
                        });
                    }
                }
            }
        }
    }

    licenses
}

/// Whether SOME machine conforms `(trait, requirement)` for this carrier.
fn slot_satisfier_exists(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    requirement_name: &str,
    carrier: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    program.machines().iter().any(|candidate| {
        program
            .machine_trait_conformances(candidate)
            .iter()
            .any(|conformance| {
                if conformance.symbol != trait_definition.symbol {
                    return false;
                }
                let bound_requirement = conformance
                    .requirement
                    .as_ref()
                    .map(|name| name.as_str().to_owned())
                    .or_else(|| {
                        candidate
                            .attached_data
                            .is_none()
                            .then(|| candidate.name.as_str().to_owned())
                    });
                if bound_requirement.as_deref() != Some(requirement_name) {
                    return false;
                }
                let Some(candidate_entry) = program.machine_states(candidate).first() else {
                    return false;
                };
                let candidate_carrier = program
                    .state_parameters(candidate_entry)
                    .first()
                    .map(|parameter| parameter.type_reference)
                    .unwrap_or(candidate_entry.return_type);
                crate::type_references::type_references_match(program, candidate_carrier, carrier)
            })
    })
}

fn slot_satisfier_exists_for_alias(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    requirement_name: &str,
    carrier: psi_typed_trees::types::TypeReferenceHandle,
    alias: Option<&str>,
) -> bool {
    program.machines().iter().any(|candidate| {
        program
            .machine_trait_conformances(candidate)
            .iter()
            .any(|conformance| {
                if conformance.symbol != trait_definition.symbol
                    || conformance.requirement.as_ref().map(|name| name.as_str())
                        != Some(requirement_name)
                    || conformance.alias.as_ref().map(|name| name.as_str()) != alias
                {
                    return false;
                }
                let Some(entry) = program.machine_states(candidate).first() else {
                    return false;
                };
                let candidate_carrier = program
                    .state_parameters(entry)
                    .first()
                    .map(|parameter| parameter.type_reference)
                    .unwrap_or(entry.return_type);
                crate::type_references::type_references_match(program, candidate_carrier, carrier)
            })
    })
}

/// `R(x, y) == R(y, x)` with `x`/`y` DISTINCT requirement parameters -> the
/// op slot `R` is declared commutative by this law.
/// Tier-2: recognize `mul(a, add(b, c)) == add(mul(a, b), mul(a, c))` up
/// to parameter naming -- returns (mul_op, add_op).
fn distributivity_shape(
    left: &StructuralTerm,
    right: &StructuralTerm,
    parameters: &[String],
) -> Option<(String, String)> {
    // left = mul(a, add(b, c))
    let StructuralTerm::Application {
        machine: mul_op,
        arguments: mul_args,
    } = left
    else {
        return None;
    };
    let [
        StructuralTerm::Variable(a),
        StructuralTerm::Application {
            machine: add_op,
            arguments: add_args,
        },
    ] = mul_args.as_slice()
    else {
        return None;
    };
    let [StructuralTerm::Variable(b), StructuralTerm::Variable(c)] = add_args.as_slice() else {
        return None;
    };
    if mul_op == add_op {
        return None;
    }
    for name in [a, b, c] {
        if !parameters.contains(name) {
            return None;
        }
    }
    // right = add(mul(a, b), mul(a, c))
    let StructuralTerm::Application {
        machine: outer_add,
        arguments: outer_args,
    } = right
    else {
        return None;
    };
    if outer_add != add_op {
        return None;
    }
    let [
        StructuralTerm::Application {
            machine: left_mul,
            arguments: left_args,
        },
        StructuralTerm::Application {
            machine: right_mul,
            arguments: right_args,
        },
    ] = outer_args.as_slice()
    else {
        return None;
    };
    if left_mul != mul_op || right_mul != mul_op {
        return None;
    }
    let (
        [StructuralTerm::Variable(la), StructuralTerm::Variable(lb)],
        [StructuralTerm::Variable(ra), StructuralTerm::Variable(rc)],
    ) = (left_args.as_slice(), right_args.as_slice())
    else {
        return None;
    };
    (la == a && lb == b && ra == a && rc == c).then(|| (mul_op.clone(), add_op.clone()))
}

/// Tier-2 licensing: a trait carrying comm+assoc for BOTH an add op and a
/// mul op, plus a DISTRIBUTIVITY law connecting them, licenses each carrier
/// that conformed ALL FIVE law slots. Same no-circularity rule: the judged
/// machine binding ANY involved law slot gets nothing from this trait.
fn compute_semiring_licenses(
    program: &TypedTrees,
    judged_machine: &Machine,
) -> Vec<SemiringLicense> {
    let mut licenses = Vec::new();
    for trait_definition in program.traits() {
        let mut comm_laws: Vec<(String, String)> = Vec::new();
        let mut assoc_laws: Vec<(String, String)> = Vec::new();
        let mut dist_laws: Vec<(String, String, String)> = Vec::new(); // (mul, add, law)
        for requirement in program.trait_machine_signatures(trait_definition) {
            let parameters: Vec<String> = program
                .state_signature_parameters(requirement)
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect();
            for contract in program.state_signature_contracts(requirement) {
                if contract.kind != SignatureContractKind::Ensures {
                    continue;
                }
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Expression(expression) = fact else {
                        continue;
                    };
                    let mut conjuncts = Vec::new();
                    collect_equality_conjuncts(program, *expression, &mut conjuncts);
                    for conjunct in conjuncts {
                        let ExpressionNode::Binary(binary) =
                            program.expression_table.expression(conjunct)
                        else {
                            continue;
                        };
                        let (Some(left), Some(right)) = (
                            structural_term(program, binary.left),
                            structural_term(program, binary.right),
                        ) else {
                            continue;
                        };
                        if let Some(op) = commutativity_shape(&left, &right, &parameters) {
                            comm_laws.push((op, requirement.name.as_str().to_owned()));
                        }
                        if let Some(op) = associativity_shape(&left, &right, &parameters) {
                            assoc_laws.push((op, requirement.name.as_str().to_owned()));
                        }
                        if let Some((mul_op, add_op)) =
                            distributivity_shape(&left, &right, &parameters)
                        {
                            dist_laws.push((mul_op, add_op, requirement.name.as_str().to_owned()));
                        }
                    }
                }
            }
        }
        for (mul_op, add_op, dist_law) in &dist_laws {
            let Some((_, add_comm)) = comm_laws.iter().find(|(op, _)| op == add_op) else {
                continue;
            };
            let Some((_, add_assoc)) = assoc_laws.iter().find(|(op, _)| op == add_op) else {
                continue;
            };
            let Some((_, mul_comm)) = comm_laws.iter().find(|(op, _)| op == mul_op) else {
                continue;
            };
            let Some((_, mul_assoc)) = assoc_laws.iter().find(|(op, _)| op == mul_op) else {
                continue;
            };
            let law_slots = [add_comm, add_assoc, mul_comm, mul_assoc, dist_law];
            // PER-LICENSE circularity break (refined 2026-07-16, mirroring
            // compute_ring_licenses): the judged machine is excluded only
            // from paired licenses it underpins -- binding one of the five
            // law slots FOR THE SAME CARRIER. A law lemma's goal is the law
            // shape over its own carrier's ops, so other carriers' licenses
            // cannot rearrange it.
            let judged_bound_laws: Vec<String> = program
                .machine_trait_conformances(judged_machine)
                .iter()
                .filter(|conformance| conformance.symbol == trait_definition.symbol)
                .filter_map(|conformance| {
                    conformance
                        .requirement
                        .as_ref()
                        .map(|name| name.as_str().to_owned())
                        .or_else(|| {
                            judged_machine
                                .attached_data
                                .is_none()
                                .then(|| judged_machine.name.as_str().to_owned())
                        })
                })
                .filter(|name| law_slots.iter().any(|law| law.as_str() == name))
                .collect();
            let judged_carrier = program.machine_states(judged_machine).first().map(|entry| {
                program
                    .state_parameters(entry)
                    .first()
                    .map(|parameter| parameter.type_reference)
                    .unwrap_or(entry.return_type)
            });
            // Each carrier conforming BOTH op slots with all five law slots
            // satisfied earns the paired license.
            for add_candidate in program.machines() {
                for conformance in program.machine_trait_conformances(add_candidate) {
                    if conformance.symbol != trait_definition.symbol {
                        continue;
                    }
                    let bound = conformance
                        .requirement
                        .as_ref()
                        .map(|name| name.as_str().to_owned())
                        .or_else(|| {
                            add_candidate
                                .attached_data
                                .is_none()
                                .then(|| add_candidate.name.as_str().to_owned())
                        });
                    if bound.as_deref() != Some(add_op.as_str()) {
                        continue;
                    }
                    let Some(entry) = program.machine_states(add_candidate).first() else {
                        continue;
                    };
                    let carrier = program
                        .state_parameters(entry)
                        .first()
                        .map(|parameter| parameter.type_reference)
                        .unwrap_or(entry.return_type);
                    if !judged_bound_laws.is_empty()
                        && judged_carrier.is_some_and(|judged| {
                            crate::type_references::type_references_match(program, judged, carrier)
                        })
                    {
                        continue;
                    }
                    if !law_slots
                        .iter()
                        .all(|law| slot_satisfier_exists(program, trait_definition, law, carrier))
                    {
                        continue;
                    }
                    if let Some(mul_machine) =
                        op_slot_satisfier(program, trait_definition, mul_op, carrier)
                    {
                        licenses.push(SemiringLicense {
                            add_machine: add_candidate.name.as_str().to_owned(),
                            mul_machine,
                        });
                    }
                }
            }
        }
    }
    licenses
}

/// The NAME of the machine conforming `op_slot` for the given carrier.
fn op_slot_satisfier(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    op_slot: &str,
    carrier: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<String> {
    for candidate in program.machines() {
        for conformance in program.machine_trait_conformances(candidate) {
            if conformance.symbol != trait_definition.symbol {
                continue;
            }
            let bound = conformance
                .requirement
                .as_ref()
                .map(|name| name.as_str().to_owned())
                .or_else(|| {
                    candidate
                        .attached_data
                        .is_none()
                        .then(|| candidate.name.as_str().to_owned())
                });
            if bound.as_deref() != Some(op_slot) {
                continue;
            }
            let Some(entry) = program.machine_states(candidate).first() else {
                continue;
            };
            let candidate_carrier = program
                .state_parameters(entry)
                .first()
                .map(|parameter| parameter.type_reference)
                .unwrap_or(entry.return_type);
            if crate::type_references::type_references_match(program, candidate_carrier, carrier) {
                return Some(candidate.name.as_str().to_owned());
            }
        }
    }
    None
}

fn commutativity_shape(
    left: &StructuralTerm,
    right: &StructuralTerm,
    parameters: &[String],
) -> Option<String> {
    let StructuralTerm::Application {
        machine: op_l,
        arguments: args_l,
    } = left
    else {
        return None;
    };
    let StructuralTerm::Application {
        machine: op_r,
        arguments: args_r,
    } = right
    else {
        return None;
    };
    if op_l != op_r || args_l.len() != 2 || args_r.len() != 2 {
        return None;
    }
    let [StructuralTerm::Variable(x), StructuralTerm::Variable(y)] = args_l.as_slice() else {
        return None;
    };
    let [StructuralTerm::Variable(rx), StructuralTerm::Variable(ry)] = args_r.as_slice() else {
        return None;
    };
    let is_parameter = |name: &String| parameters.iter().any(|parameter| parameter == name);
    (x != y && rx == y && ry == x && is_parameter(x) && is_parameter(y)).then(|| op_l.clone())
}

/// `R(R(x, y), z) == R(x, R(y, z))` (either orientation) with distinct
/// requirement parameters -> the op slot `R` is declared associative.
fn associativity_shape(
    left: &StructuralTerm,
    right: &StructuralTerm,
    parameters: &[String],
) -> Option<String> {
    for (first, second) in [(left, right), (right, left)] {
        let StructuralTerm::Application {
            machine: op_outer,
            arguments: outer_args,
        } = first
        else {
            continue;
        };
        if outer_args.len() != 2 {
            continue;
        }
        let StructuralTerm::Application {
            machine: op_inner,
            arguments: inner_args,
        } = &outer_args[0]
        else {
            continue;
        };
        if op_inner != op_outer || inner_args.len() != 2 {
            continue;
        }
        let (StructuralTerm::Variable(x), StructuralTerm::Variable(y), StructuralTerm::Variable(z)) =
            (&inner_args[0], &inner_args[1], &outer_args[1])
        else {
            continue;
        };
        let StructuralTerm::Application {
            machine: op_right,
            arguments: right_args,
        } = second
        else {
            continue;
        };
        if op_right != op_outer || right_args.len() != 2 {
            continue;
        }
        let StructuralTerm::Variable(rx) = &right_args[0] else {
            continue;
        };
        let StructuralTerm::Application {
            machine: op_right_inner,
            arguments: right_inner_args,
        } = &right_args[1]
        else {
            continue;
        };
        if op_right_inner != op_outer || right_inner_args.len() != 2 {
            continue;
        }
        let (StructuralTerm::Variable(ry), StructuralTerm::Variable(rz)) =
            (&right_inner_args[0], &right_inner_args[1])
        else {
            continue;
        };
        let is_parameter = |name: &String| parameters.iter().any(|parameter| parameter == name);
        let distinct = x != y && y != z && x != z;
        if distinct
            && rx == x
            && ry == y
            && rz == z
            && is_parameter(x)
            && is_parameter(y)
            && is_parameter(z)
        {
            return Some(op_outer.clone());
        }
    }
    None
}
