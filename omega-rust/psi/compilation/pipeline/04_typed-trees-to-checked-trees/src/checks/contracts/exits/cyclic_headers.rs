//! Header invariants for a cyclic machine whose guarantee relates the value
//! it returns to an invocation formal.
//!
//! Each state in a cyclic transition component has its own current parameter
//! frame; the invocation formals are a separate immutable frame even when the
//! entry state is reentered. Only the flow owner's exact all-path origin
//! solution can identify an immutable scalar with an invocation value; another
//! parameter changing on that same edge does not invalidate this identity. For
//! every exit returning `e`, propose its guarantee with `result` replaced by `e`
//! in that state's frame. The component owns the
//! conjunction of these state-local candidates. Every incoming edge proves the
//! destination conjunction under simultaneous actual-argument substitution.
//! An internal edge may assume only its caller's conjunction, while an external
//! edge must establish the destination without it. Invocation is an additional
//! initial arrival when the entry belongs to the component.
//!
//! Only after the entire component's arrivals check do its candidates discharge
//! their exits. Current state requirements and arm guards remain local to the
//! caller; invocation requirements remain about the immutable formals. Neither
//! ranking nor prefix facts grant an invariant. Unsupported substitutions and
//! any failed arrival retire the whole component proposal. Candidate discovery
//! remains incomplete: a state without an admitted exit contributes no conjunct.
//!
//! The binders are the state's immutable fixed-integer parameters in either
//! readable arithmetic domain. `Exact` terms read as mathematical integers
//! directly. A `Wrapping` binder reads as its residue representative: the
//! engine argues in integers, and an integer identity descends to Z/2^w, so
//! a residue binder may appear in an equality goal. An order or disequality
//! on a wrapped value does not descend, so propositions name residue terms
//! only under `==`. Facts fed to the engine must additionally read the
//! program exactly: `x < y` on wrapped atoms compares the residues, while
//! `x + y == z` would claim an integer equality stronger than the wrapped
//! congruence the program established — so a hypothesis names residue terms
//! only between bare binders and literals. `Saturating` and `Trapping`
//! stay outside the language.

use arena::Handle;
use checked_trees::{CheckFacts, ContractProofFactKind, FlowExitFact, FlowStateFact};
use facts::{ContractFactKind, FactOrigin, FactPayload};
use language_core::OperatorSpelling;
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::TransitionTargetHandle;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
    scoped_arithmetic_implication,
};

mod components;

use super::super::prover::has_builtin_operators;
use super::super::return_values::{exit_return_expression, is_result_reference};

/// Guarantees discharged by a proved header invariant, computed once per
/// machine the first time one of its exit guarantees asks.
pub(in crate::checks::contracts) struct CyclicHeaderInvariants {
    machines: Vec<MachineHeaderInvariants>,
}

struct MachineHeaderInvariants {
    machine: SymbolHandle,
    admitted: Vec<AdmittedGuarantee>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AdmittedGuarantee {
    state_symbol: SymbolHandle,
    statement_index: usize,
    transition_target: TransitionTargetHandle,
    contract: Handle<ProofFact>,
}

impl CyclicHeaderInvariants {
    pub(in crate::checks::contracts) fn new() -> Self {
        Self {
            machines: Vec::new(),
        }
    }

    pub(in crate::checks::contracts) fn proves(
        &mut self,
        program: &TypedTrees,
        facts: &CheckFacts,
        exit: &FlowExitFact,
        requirement: &facts::Fact,
    ) -> bool {
        let FactPayload::ContractBooleanExpression { fact: contract, .. } = requirement.payload
        else {
            return false;
        };
        let position = self
            .machines
            .iter()
            .position(|invariants| invariants.machine == exit.machine_symbol)
            .unwrap_or_else(|| {
                self.machines.push(MachineHeaderInvariants {
                    machine: exit.machine_symbol,
                    admitted: components::admitted(program, facts, exit.machine_symbol),
                });
                self.machines.len() - 1
            });
        self.machines[position]
            .admitted
            .contains(&AdmittedGuarantee {
                state_symbol: exit.state_symbol,
                statement_index: exit.statement_index,
                transition_target: exit.transition_target,
                contract,
            })
    }
}

/// One transported guarantee: `guarantee` over `result` at the exit that
/// returns `returned`.
struct Candidate {
    exit: AdmittedGuarantee,
    guarantee: ExpressionHandle,
    returned: ExpressionHandle,
}

/// One named arrival, retaining its caller identity and exact simultaneous
/// arguments. Component membership decides whether a caller invariant is live.
struct Arrival {
    caller: SymbolHandle,
    caller_atoms: Vec<ScopedArithmeticBinding>,
    arguments: Vec<ExpressionHandle>,
    hypotheses: Vec<(ExpressionHandle, bool)>,
}

struct RosterParameter {
    symbol: SymbolHandle,
    type_reference: TypeReferenceHandle,
    domain: ArithmeticDomain,
    unsigned: bool,
}

struct Header<'program, 'facts> {
    program: &'program TypedTrees,
    facts: &'facts CheckFacts,
    machine: &'program Machine,
    /// The destination state whose local invariant is being proposed.
    header: &'program State,
    /// The entry state; its parameters are the invocation formals.
    entry: &'program State,
    state_flow: &'facts FlowStateFact,
    /// Immutable fixed-integer parameters of the header state, the only
    /// binders a proposition may name there, with the arithmetic domain each
    /// one evaluates in. Anything else stays outside the language.
    roster: Vec<RosterParameter>,
    /// The same roster over the entry state — the formal scope the machine's
    /// `requires` and `ensures` are written in.
    formals: Vec<RosterParameter>,
    /// The union over every state's parameters, the lookup a term admission
    /// consults: a caller state's arm fact names the caller's own binders.
    terms: Vec<RosterParameter>,
    /// Present when the returned value is a fixed integer the synthetic
    /// `result` may denote, with its domain.
    result: Option<(TypeReferenceHandle, ArithmeticDomain)>,
    requires: Vec<ExpressionHandle>,
    origins: &'facts [Vec<(SymbolHandle, SymbolHandle)>],
}

impl<'program, 'facts> Header<'program, 'facts> {
    fn new(
        program: &'program TypedTrees,
        facts: &'facts CheckFacts,
        machine: &'program Machine,
        header: &'program State,
        origins: &'facts [Vec<(SymbolHandle, SymbolHandle)>],
    ) -> Option<Self> {
        let states = program.machine_states(machine);
        let entry = states.first()?;
        let state_flow = Self::state_flow(facts, machine, header)?;
        let params_of = |state: &State| {
            program
                .state_parameters(state)
                .iter()
                .filter(|parameter| {
                    !parameter.is_mutable && !parameter.is_self && !parameter.is_const
                })
                .filter_map(|parameter| {
                    let (primitive, domain) = fixed_integer(program, parameter.type_reference)?;
                    Some(RosterParameter {
                        symbol: parameter.symbol,
                        type_reference: parameter.type_reference,
                        domain,
                        unsigned: is_unsigned(primitive),
                    })
                })
                .collect::<Vec<_>>()
        };
        let roster = params_of(header);
        let formals = params_of(entry);
        let terms = states.iter().flat_map(params_of).collect();
        let result = header
            .return_type
            .is_valid()
            .then_some(header.return_type)
            .and_then(|type_reference| {
                fixed_integer(program, type_reference).map(|(_, domain)| (type_reference, domain))
            });
        let requires = program
            .machine_contracts(machine)
            .iter()
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .filter_map(|fact| match fact {
                ProofFact::Expression(expression) => Some(*expression),
                _ => None,
            })
            .collect();
        Some(Self {
            program,
            facts,
            machine,
            header,
            entry,
            state_flow,
            roster,
            formals,
            terms,
            result,
            requires,
            origins,
        })
    }

    fn state_flow<'a>(
        facts: &'a CheckFacts,
        machine: &Machine,
        state: &State,
    ) -> Option<&'a FlowStateFact> {
        facts
            .flow
            .control
            .states
            .iter()
            .find(|(_, flow)| {
                flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol
            })
            .map(|(_, flow)| flow)
    }

    /// A spelled or resolved target enters `header`: the header's own symbol
    /// for a named state, or the machine's symbol for an entry backedge.
    fn target_is_header(
        machine: &Machine,
        entry: &State,
        header: &State,
        symbol: SymbolHandle,
    ) -> bool {
        symbol == header.symbol || (header.symbol == entry.symbol && symbol == machine.symbol)
    }

    fn requirements(
        &self,
        bindings: Vec<ScopedArithmeticBinding>,
    ) -> Vec<ScopedArithmeticHypothesis> {
        self.requires
            .iter()
            .filter(|expression| self.hypothesis_is_admitted(**expression))
            .map(|expression| self.hypothesis(*expression, bindings.clone(), true))
            .collect()
    }

    fn proves(
        &self,
        hypotheses: &[ScopedArithmeticHypothesis],
        goal: &ScopedArithmeticExpression,
    ) -> bool {
        scoped_arithmetic_implication(self.program, self.machine, hypotheses, goal)
            == StrictArithmeticImplicationJudgment::Proven
    }

    /// Every expression guarantee of every exit of the header, transported
    /// through that exit's returned term. A guarantee or return outside the
    /// language is not proposed.
    fn candidates(&self) -> Vec<Candidate> {
        let mut candidates = Vec::new();
        for exit in self
            .facts
            .flow
            .control
            .exits
            .span_or_empty(self.state_flow.exits)
        {
            let returned = exit_return_expression(self.program, exit);
            if !self.term_is_admitted(returned) {
                continue;
            }
            for reference in self
                .facts
                .proof
                .contract_fact_refs
                .span_or_empty(exit.ensures)
            {
                let contract = self.facts.proof.contract_facts.get(reference.fact);
                if contract.kind != ContractProofFactKind::Ensures {
                    continue;
                }
                let ProofFact::Expression(guarantee) = self.program.proof_facts.get(contract.fact)
                else {
                    continue;
                };
                if !self.proposition_is_admitted(*guarantee) {
                    continue;
                }
                candidates.push(Candidate {
                    exit: AdmittedGuarantee {
                        state_symbol: exit.state_symbol,
                        statement_index: exit.statement_index,
                        transition_target: exit.transition_target,
                        contract: contract.fact,
                    },
                    guarantee: *guarantee,
                    returned,
                });
            }
        }
        candidates
    }

    /// Every named transition into the header, from the header itself or any
    /// other state. `None` when an arrival cannot be read: its arguments are
    /// then not a substitution the invariant can be checked under, so nothing
    /// may be admitted. A `self` transition forwards every immutable
    /// parameter unchanged and preserves the conjunction trivially, so it
    /// needs no obligation here.
    fn arrivals(&self) -> Option<Vec<Arrival>> {
        let explicit = self
            .program
            .state_parameters(self.header)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
        let mut arrivals = Vec::new();
        for caller in self.program.machine_states(self.machine) {
            let caller_flow = Self::state_flow(self.facts, self.machine, caller)?;
            for call in self
                .facts
                .flow
                .control
                .calls
                .span_or_empty(caller_flow.calls)
            {
                let Some(site) = crate::semantic::calls::find_call_site(
                    self.program,
                    self.machine.symbol,
                    caller.symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    continue;
                };
                let crate::semantic::calls::CallSite::TransitionNamed {
                    path,
                    evidence_arguments,
                    ..
                } = &site
                else {
                    continue;
                };
                let enters_header =
                    Self::target_is_header(self.machine, self.entry, self.header, path.symbol)
                        || Self::target_is_header(
                            self.machine,
                            self.entry,
                            self.header,
                            call.target_symbol,
                        );
                if !enters_header {
                    continue;
                }
                if !evidence_arguments.is_empty() {
                    return None;
                }
                let arguments =
                    crate::semantic::calls::call_site_argument_expressions(self.program, &site);
                if arguments.len() != explicit
                    || !arguments
                        .iter()
                        .all(|argument| self.term_is_admitted(*argument))
                {
                    return None;
                }
                let hypotheses = self
                    .facts
                    .flow
                    .state_call_entry_semantic_contexts(
                        caller_flow,
                        call.statement_index,
                        call.call_ordinal,
                        call.target_symbol,
                        call.receiver_symbol,
                    )
                    .flat_map(|context| {
                        self.facts
                            .semantic
                            .context_view(self.facts.semantic.contexts.get(context))
                            .facts()
                            .filter_map(|fact| self.arm_fact(fact))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                arrivals.push(Arrival {
                    caller: caller.symbol,
                    caller_atoms: self.caller_atoms(caller),
                    arguments: arguments.to_vec(),
                    hypotheses,
                });
            }
        }
        Some(arrivals)
    }

    /// A fact alive at the arm that the header roster can read: the state's
    /// own requirements and this iteration's guard values. Imported call
    /// contracts are not this machine's facts, and a prefix fact from an
    /// earlier iteration never survives the state's arrival contract.
    fn arm_fact(&self, fact: &facts::Fact) -> Option<(ExpressionHandle, bool)> {
        if matches!(
            fact.origin,
            FactOrigin::CallRequires | FactOrigin::CallEnsures
        ) {
            return None;
        }
        let (expression, holds) = match fact.payload {
            FactPayload::ContractBooleanExpression {
                kind: ContractFactKind::Requires,
                expression,
                instantiated,
                ..
            } if !instantiated.is_valid() => (expression, true),
            FactPayload::BooleanValue { expression, value } => (expression, value),
            _ => return None,
        };
        let (expression, holds) = self.folded_polarity(expression, holds);
        self.hypothesis_is_admitted(expression)
            .then_some((expression, holds))
    }

    /// A guard retained as `comparison == true` / `== false` is the
    /// comparison with the literal's polarity folded in.
    fn folded_polarity(
        &self,
        expression: ExpressionHandle,
        holds: bool,
    ) -> (ExpressionHandle, bool) {
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return (expression, holds);
        };
        if binary.operator != BinaryOperator::Equal
            || !has_builtin_operators(self.program, &self.facts.operators, expression)
        {
            return (expression, holds);
        }
        match self.program.expression_table.expression(binary.right) {
            ExpressionNode::Boolean(literal) => (binary.left, holds == *literal),
            _ => (expression, holds),
        }
    }

    fn atoms_of(
        &self,
        parameters: &[RosterParameter],
        scope: String,
    ) -> Vec<ScopedArithmeticBinding> {
        parameters
            .iter()
            .map(|parameter| ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value: ScopedArithmeticValue::Atom {
                    identity: format!("\0{scope}:{:?}", parameter.symbol),
                    unsigned: parameter.unsigned,
                },
            })
            .collect()
    }

    /// The invocation's values, immutable for the whole run.
    fn formal_roster(&self) -> Vec<ScopedArithmeticBinding> {
        self.atoms_of(&self.formals, "invocation".to_string())
    }

    /// `bindings` plus an invocation-atom binding for every formal symbol the
    /// scope does not already bind: formals are immutable machine-level
    /// binders, visible inside every state.
    fn with_formals(
        &self,
        mut bindings: Vec<ScopedArithmeticBinding>,
    ) -> Vec<ScopedArithmeticBinding> {
        let formal = self
            .formal_roster()
            .into_iter()
            .filter(|binding| {
                !bindings
                    .iter()
                    .any(|existing| existing.binder == binding.binder)
            })
            .collect::<Vec<_>>();
        bindings.extend(formal);
        bindings
    }

    /// Current parameters start with fresh atoms. The flow owner's exact
    /// all-path origin solution can identify one immutable scalar with its
    /// invocation value independently of other parameters drifting. Missing,
    /// changed or ambiguous origins grant no equality, and a reference's
    /// identity can never turn its mutable contents into an entry snapshot.
    fn caller_atoms(&self, caller: &State) -> Vec<ScopedArithmeticBinding> {
        let parameters = self
            .program
            .state_parameters(caller)
            .iter()
            .filter(|parameter| !parameter.is_mutable && !parameter.is_self && !parameter.is_const)
            .filter_map(|parameter| {
                self.terms
                    .iter()
                    .find(|term| term.symbol == parameter.symbol)
                    .map(|term| RosterParameter {
                        symbol: term.symbol,
                        type_reference: term.type_reference,
                        domain: term.domain,
                        unsigned: term.unsigned,
                    })
            })
            .collect::<Vec<_>>();
        let mut bindings = self.atoms_of(&parameters, format!("arrival:{:?}", caller.symbol));
        let origins = self
            .program
            .machine_states(self.machine)
            .iter()
            .position(|state| state.symbol == caller.symbol)
            .and_then(|index| self.origins.get(index));
        let formals = self.formal_roster();
        if let Some(origins) = origins {
            for (parameter, binding) in parameters.iter().zip(&mut bindings) {
                let Some((origin, _)) = origins
                    .iter()
                    .find(|(_, current)| *current == parameter.symbol)
                else {
                    continue;
                };
                let Some(formal) = self.formals.iter().find(|formal| formal.symbol == *origin)
                else {
                    continue;
                };
                if formal.domain != parameter.domain
                    || self.program.primitive_type_reference(formal.type_reference)
                        != self
                            .program
                            .primitive_type_reference(parameter.type_reference)
                {
                    continue;
                }
                if let Some(invocation) = formals
                    .iter()
                    .find(|entry| entry.binder == ScopedArithmeticBinder::Symbol(*origin))
                {
                    binding.value = invocation.value.clone();
                }
            }
        }
        self.with_formals(bindings)
    }

    /// An arrival's simultaneous substitution: each header parameter denotes
    /// its argument read at the caller's scope.
    fn arrival_roster(
        &self,
        arguments: &[ExpressionHandle],
        caller: Vec<ScopedArithmeticBinding>,
    ) -> Vec<ScopedArithmeticBinding> {
        self.program
            .state_parameters(self.header)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .zip(arguments)
            .filter(|(parameter, _)| {
                self.roster
                    .iter()
                    .any(|bound| bound.symbol == parameter.symbol)
            })
            .map(|(parameter, argument)| ScopedArithmeticBinding {
                binder: ScopedArithmeticBinder::Symbol(parameter.symbol),
                value: ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                    expression: *argument,
                    bindings: caller.clone(),
                }),
            })
            .collect()
    }

    /// The candidate's guarantee over the formals, with `result` denoting the
    /// exit's returned term read under `returned_roster`.
    fn transported(
        &self,
        candidate: &Candidate,
        returned_roster: Vec<ScopedArithmeticBinding>,
    ) -> ScopedArithmeticExpression {
        let mut bindings = self.formal_roster();
        bindings.push(ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Result,
            value: ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                expression: candidate.returned,
                bindings: returned_roster,
            }),
        });
        ScopedArithmeticExpression {
            expression: candidate.guarantee,
            bindings,
        }
    }

    fn hypothesis(
        &self,
        expression: ExpressionHandle,
        bindings: Vec<ScopedArithmeticBinding>,
        holds: bool,
    ) -> ScopedArithmeticHypothesis {
        ScopedArithmeticHypothesis {
            proposition: ScopedArithmeticExpression {
                expression,
                bindings,
            },
            holds,
        }
    }

    /// Conjunctions of builtin integer comparisons over admitted terms —
    /// the guarantees this check proposes and discharges. A residue-domain
    /// term may appear only under `==`: the engine argues in mathematical
    /// integers, an integer identity descends to Z/2^w, but an order or a
    /// disequality on a wrapped value does not.
    fn proposition_is_admitted(&self, expression: ExpressionHandle) -> bool {
        self.comparison_is_admitted(expression, false)
    }

    /// A program-true fact the encoding may feed the engine. A residue
    /// binder compares faithfully only through its own representative: every
    /// side of a residue-domain comparison must be a bare binder or literal,
    /// since `x < y` on wrapped atoms reads the residues exactly while
    /// `x + y == z` would claim an integer equality stronger than the wrapped
    /// congruence the program established.
    fn hypothesis_is_admitted(&self, expression: ExpressionHandle) -> bool {
        self.comparison_is_admitted(expression, true)
    }

    fn comparison_is_admitted(&self, expression: ExpressionHandle, leaf_only: bool) -> bool {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return false;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => self.comparison_is_admitted(borrow.target, leaf_only),
            ExpressionNode::Binary(binary) => {
                let spelling = match binary.operator {
                    BinaryOperator::And => {
                        return has_builtin_operators(
                            self.program,
                            &self.facts.operators,
                            expression,
                        ) && self.comparison_is_admitted(binary.left, leaf_only)
                            && self.comparison_is_admitted(binary.right, leaf_only);
                    }
                    BinaryOperator::Equal => OperatorSpelling::Equal,
                    BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
                    BinaryOperator::Less => OperatorSpelling::Less,
                    BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
                    BinaryOperator::Greater => OperatorSpelling::Greater,
                    BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
                    _ => return false,
                };
                let (Some(left), Some(right)) = (
                    self.term_reference(binary.left),
                    self.term_reference(binary.right),
                ) else {
                    return false;
                };
                if !self.builtin_meaning(expression, spelling, left.binder_type, right.binder_type)
                {
                    return false;
                }
                if left.domain == ArithmeticDomain::Exact && right.domain == ArithmeticDomain::Exact
                {
                    return true;
                }
                if leaf_only {
                    self.is_leaf_term(binary.left) && self.is_leaf_term(binary.right)
                } else {
                    binary.operator == BinaryOperator::Equal
                }
            }
            _ => false,
        }
    }

    /// A bare binder or literal: its value is its own residue
    /// representative, so a comparison on it reads the program fact exactly.
    fn is_leaf_term(&self, expression: ExpressionHandle) -> bool {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => self.is_leaf_term(borrow.target),
            ExpressionNode::Name(_) | ExpressionNode::Integer(_) => {
                self.term_reference(expression).is_some()
            }
            _ => false,
        }
    }

    fn term_is_admitted(&self, expression: ExpressionHandle) -> bool {
        self.term_reference(expression).is_some()
    }

    /// `Some` for a term the rosters can read: the retained binder type of a
    /// direct parameter or result (`None` for a literal or a compound whose
    /// builtin result carries no source type), and the arithmetic domain the
    /// term evaluates in — `Wrapping` when any leaf is residue arithmetic.
    fn term_reference(&self, expression: ExpressionHandle) -> Option<TermReference> {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                if is_result_reference(self.program, self.machine, expression) {
                    return self.result.map(|(type_reference, domain)| TermReference {
                        binder_type: Some(type_reference),
                        domain,
                    });
                }
                if !path.symbol.is_valid()
                    || path.head_symbol != path.symbol
                    || self
                        .program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        != 1
                {
                    return None;
                }
                self.terms
                    .iter()
                    .find(|parameter| parameter.symbol == path.symbol)
                    .map(|parameter| TermReference {
                        binder_type: Some(parameter.type_reference),
                        domain: parameter.domain,
                    })
            }
            ExpressionNode::Integer(literal) => literal
                .landing()
                .is_none_or(|landing| {
                    matches!(
                        landing.domain,
                        ArithmeticDomain::Exact | ArithmeticDomain::Wrapping
                    )
                })
                .then_some(TermReference {
                    binder_type: None,
                    domain: ArithmeticDomain::Exact,
                }),
            ExpressionNode::Borrow(borrow) => self.term_reference(borrow.target),
            ExpressionNode::Binary(binary) => {
                let spelling = match binary.operator {
                    BinaryOperator::Add => OperatorSpelling::Add,
                    BinaryOperator::Subtract => OperatorSpelling::Subtract,
                    BinaryOperator::Multiply => OperatorSpelling::Multiply,
                    _ => return None,
                };
                let left = self.term_reference(binary.left)?;
                let right = self.term_reference(binary.right)?;
                self.builtin_meaning(expression, spelling, left.binder_type, right.binder_type)
                    .then_some(TermReference {
                        binder_type: None,
                        domain: if left.domain == ArithmeticDomain::Wrapping
                            || right.domain == ArithmeticDomain::Wrapping
                        {
                            ArithmeticDomain::Wrapping
                        } else {
                            ArithmeticDomain::Exact
                        },
                    })
            }
            _ => None,
        }
    }

    /// No checked row selected a non-builtin operator, and no declared or
    /// trait meaning applies to these operand types.
    fn builtin_meaning(
        &self,
        expression: ExpressionHandle,
        spelling: OperatorSpelling,
        left: Option<TypeReferenceHandle>,
        right: Option<TypeReferenceHandle>,
    ) -> bool {
        has_builtin_operators(self.program, &self.facts.operators, expression)
            && typed_trees::operator::has_builtin_spelled_expression_meaning(
                self.program,
                self.machine.symbol,
                expression,
                spelling,
                &[left, right],
            )
    }
}

/// One admitted term: its binder type (`None` for a literal or compound)
/// and the domain its evaluation follows.
struct TermReference {
    binder_type: Option<TypeReferenceHandle>,
    domain: ArithmeticDomain,
}

/// A fixed-width integer in a domain the strict arithmetic reading can
/// carry: `Exact` terms are mathematical integers, and a `Wrapping` term
/// reads as its residue representative. `Saturating` and `Trapping` stay
/// outside the language.
fn fixed_integer(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(PrimitiveType, ArithmeticDomain)> {
    let primitive = program.primitive_type_reference(type_reference)?;
    if !matches!(
        primitive,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    let domain = program.arithmetic_domain_for_type_reference(type_reference);
    matches!(domain, ArithmeticDomain::Exact | ArithmeticDomain::Wrapping)
        .then_some((primitive, domain))
}

fn is_unsigned(primitive: PrimitiveType) -> bool {
    matches!(
        primitive,
        PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64
    )
}

#[cfg(test)]
mod tests {
    use crate::CheckingRequest;
    use crate::lower_typed_trees;

    fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
        lower_typed_trees(
            crate::tests::front_end::typed_program_with_core_service(&format!(
                "boundary trait MachineControl {{}}\nboundary trait PortIo {{}}\n{source}"
            )),
            &CheckingRequest::settled(),
        )
    }

    /// A named cyclic state as its own header: the carried `p` relates the
    /// exit's returned value to the invocation's `previous`, and the guard
    /// supplies the `n <= p` arm fact the preservation step needs.
    fn carried(guard: &str, edge: &str) -> String {
        format!(
            r#"
machine descend(n: u64 [0..=1000], previous: u64 [0..=1000]) -> u64
terminates by n -> Nat::Descending;
ensures result <= previous
{{
    transition {{ _ -> loop(n, previous) }}
    state loop(n: u64 [0..=1000], p: u64 [0..=1000]) -> u64 {{
        transition {guard} {{
            true -> loop({edge})
            false -> p
        }}
    }}
}}
"#
        )
    }

    #[test]
    fn a_named_state_header_discharges_the_guarantee() {
        // The conjunct `p <= previous@invocation` is established at the
        // `entry -> loop` edge (the entry parameters read the formals) and
        // preserved at the self-edge: `p := n` drifts, but the arm fact
        // `n <= p` chains `n <= p <= previous@invocation`. The drift means no
        // exact-origin route can answer — the header conjunct must.
        check(&carried("n > 0 && n <= p", "n - 1, n"))
            .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    }

    #[test]
    fn an_unrelated_carried_value_fails_preservation() {
        // Without the `n <= p` arm fact nothing relates `n` to `p` at the
        // header, so `n <= previous@invocation` is unprovable.
        let diagnostics = check(&carried("n > 0", "n - 1, n"))
            .expect_err("a carried value without a relating arm fact must not be admitted");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.message
                == "cannot prove ensures contract for exit from descend at statement 1: result <= previous; no exact incoming reference origin for previous"),
            "{diagnostics:#?}"
        );
    }

    #[test]
    fn sequential_components_reuse_only_exact_parameter_origins() {
        // The first component changes n but preserves previous exactly.
        // The second's base may use that value identity, independently of
        // either component's invariant proposals.
        let source = r#"
machine descend(n: u64 [0..=1000], previous: u64 [0..=1000]) -> u64
terminates by n -> Nat::Descending;
ensures result <= previous
{
    transition n > 0 {
        true -> descend(n - 1, previous)
        false -> loop(n, previous)
    }
    state loop(n: u64 [0..=1000], p: u64 [0..=1000]) -> u64 {
        transition n > 0 && n <= p {
            true -> loop(n - 1, n)
            false -> p
        }
    }
}
"#;
        check(source).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    }
}

#[cfg(test)]
mod component_tests;

#[cfg(test)]
mod origin_tests;
