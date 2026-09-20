//! Header invariants for a cyclic machine whose guarantee relates the value
//! it returns to an invocation formal.
//!
//! A single-state machine re-entered by a named backedge is its own loop
//! header: the invocation is the implicit arrival and every backedge is a real
//! one. Its `ensures` speaks about the invocation formals, but a re-entered
//! parameter has no exact incoming origin, so no exit route can read the
//! guarantee at all. The producer already strengthens the Terminal header with
//! the guarantee transported through the exit's exact equations; this is the
//! source analog. For every exit returning `e` and every guarantee `G(result,
//! formals)` it proposes the header conjunct `G(e(header), formals)`, proves it
//! at the invocation arrival (where header and formals coincide, from the
//! machine `requires`) and at every backedge (from the proposed conjunction,
//! the re-established `requires`, and that arm's guard facts, under the
//! arrival's simultaneous parameter-to-argument substitution), and only then
//! discharges each exit guarantee by the conjunct that is literally that
//! guarantee at the header. The entry requirements generalized over the header
//! are the state's own arrival contract, re-proved at every backedge by the
//! call-requirement check; a uniformly forwarded parameter keeps its exact
//! origin through the flow. No prefix fact and no ranking premise is used: the
//! claim is checked against the loop, never inherited from a termination
//! certificate, and a failed arrival discards every proposal so the existing
//! diagnostic stays exactly as it was. Search is incomplete by design.
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
                    admitted: Header::new(program, facts, exit.machine_symbol)
                        .map(|header| header.admitted())
                        .unwrap_or_default(),
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

/// One named backedge: its simultaneous arguments and the facts alive at
/// that arm, each with the polarity the arm established.
struct Backedge {
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
    root: &'program State,
    state_flow: &'facts FlowStateFact,
    /// Immutable fixed-integer parameters, the only binders a proposition
    /// may name, with the arithmetic domain each one evaluates in. Anything
    /// else stays outside the language.
    roster: Vec<RosterParameter>,
    /// Present when the returned value is a fixed integer the synthetic
    /// `result` may denote, with its domain.
    result: Option<(TypeReferenceHandle, ArithmeticDomain)>,
    requires: Vec<ExpressionHandle>,
}

impl<'program, 'facts> Header<'program, 'facts> {
    fn new(
        program: &'program TypedTrees,
        facts: &'facts CheckFacts,
        machine_symbol: SymbolHandle,
    ) -> Option<Self> {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)?;
        // The free-loop form: the entry state is the header and every arrival
        // is either the invocation or a named backedge to it.
        let [root] = program.machine_states(machine) else {
            return None;
        };
        let (_, state_flow) = facts.flow.control.states.iter().find(|(_, state)| {
            state.machine_symbol == machine.symbol && state.state_symbol == root.symbol
        })?;
        let roster = program
            .state_parameters(root)
            .iter()
            .filter(|parameter| !parameter.is_mutable && !parameter.is_self && !parameter.is_const)
            .filter_map(|parameter| {
                let (primitive, domain) = fixed_integer(program, parameter.type_reference)?;
                Some(RosterParameter {
                    symbol: parameter.symbol,
                    type_reference: parameter.type_reference,
                    domain,
                    unsigned: is_unsigned(primitive),
                })
            })
            .collect();
        let result = root
            .return_type
            .is_valid()
            .then_some(root.return_type)
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
            root,
            state_flow,
            roster,
            result,
            requires,
        })
    }

    /// The exit guarantees the proved header conjunction discharges: all of
    /// the transported candidates, or none.
    fn admitted(&self) -> Vec<AdmittedGuarantee> {
        let candidates = self.candidates();
        if candidates.is_empty() {
            return Vec::new();
        }
        let Some(backedges) = self.backedges() else {
            return Vec::new();
        };
        if backedges.is_empty() {
            return Vec::new();
        }
        let requires = |roster: &dyn Fn() -> Vec<ScopedArithmeticBinding>| {
            self.requires
                .iter()
                .filter(|expression| self.hypothesis_is_admitted(**expression))
                .map(|expression| self.hypothesis(*expression, roster(), true))
                .collect::<Vec<_>>()
        };
        // Establishment: the invocation binds every header parameter from
        // its formal, so the conjunct is read with both rosters coinciding.
        let invocation = requires(&|| self.formal_roster());
        for candidate in &candidates {
            let goal = self.transported(candidate, self.formal_roster());
            if !self.proves(&invocation, &goal) {
                return Vec::new();
            }
        }
        // Preservation: from the conjunction at the current header, the
        // re-established requirements and the arm's own facts, each conjunct
        // holds under the backedge's simultaneous substitution.
        let mut header_facts = candidates
            .iter()
            .map(|candidate| ScopedArithmeticHypothesis {
                proposition: self.transported(candidate, self.header_roster()),
                holds: true,
            })
            .collect::<Vec<_>>();
        header_facts.extend(requires(&|| self.header_roster()));
        for backedge in &backedges {
            let mut hypotheses = header_facts.clone();
            hypotheses.extend(backedge.hypotheses.iter().map(|(expression, holds)| {
                self.hypothesis(*expression, self.header_roster(), *holds)
            }));
            let arrival = self.arrival_roster(&backedge.arguments);
            for candidate in &candidates {
                let goal = self.transported(candidate, arrival.clone());
                if !self.proves(&hypotheses, &goal) {
                    return Vec::new();
                }
            }
        }
        candidates
            .into_iter()
            .map(|candidate| candidate.exit)
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

    /// The named transitions re-entering the header. `None` when a backedge
    /// cannot be read: its arguments are then not a substitution the
    /// invariant can be checked under, so nothing may be admitted. A `self`
    /// transition forwards every immutable parameter unchanged and preserves
    /// the conjunction trivially, so it needs no obligation here.
    fn backedges(&self) -> Option<Vec<Backedge>> {
        let explicit = self
            .program
            .state_parameters(self.root)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
        let mut backedges = Vec::new();
        for call in self
            .facts
            .flow
            .control
            .calls
            .span_or_empty(self.state_flow.calls)
        {
            let Some(site) = crate::semantic_calls::find_call_site(
                self.program,
                self.machine.symbol,
                self.root.symbol,
                call.statement_index,
                call.call_ordinal,
            ) else {
                continue;
            };
            let crate::semantic_calls::CallSite::TransitionNamed {
                path,
                evidence_arguments,
                ..
            } = &site
            else {
                continue;
            };
            let reenters =
                |symbol: SymbolHandle| symbol == self.root.symbol || symbol == self.machine.symbol;
            if !reenters(path.symbol) || !reenters(call.target_symbol) {
                continue;
            }
            if !evidence_arguments.is_empty() {
                return None;
            }
            let arguments =
                crate::semantic_calls::call_site_argument_expressions(self.program, &site);
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
                    self.state_flow,
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
            backedges.push(Backedge {
                arguments: arguments.to_vec(),
                hypotheses,
            });
        }
        Some(backedges)
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

    fn atoms(&self, scope: &str) -> Vec<ScopedArithmeticBinding> {
        self.roster
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
        self.atoms("invocation")
    }

    /// The values bound at the current header arrival.
    fn header_roster(&self) -> Vec<ScopedArithmeticBinding> {
        self.atoms("header")
    }

    /// The backedge's simultaneous substitution: each header parameter
    /// denotes its argument read at the current header.
    fn arrival_roster(&self, arguments: &[ExpressionHandle]) -> Vec<ScopedArithmeticBinding> {
        self.program
            .state_parameters(self.root)
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
                    bindings: self.header_roster(),
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
                self.roster
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
