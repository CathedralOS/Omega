/*
    Exit checks: at each normal return, the machine's `ensures` clauses and
    the declared domains of its result and of its exclusive borrowed referents
    must be proved from the facts at that exit.

    `contracts::check_flow_call_contracts` creates one
    `CyclicHeaderInvariants` for the pass and, for each state, calls:

    - for each call, `check_state_transfer_result_domains` (`result_domains`);
    - for each exit, in order, `check_scalar_result_domains`,
      `check_exit_ensures`, `check_result_field_domains` and
      `check_exclusive_referent_field_domains`.

    `check_exit_ensures`, defined here, checks each `ensures` fact of an exit.
    It skips an owner-authorized boundary result, which is established at the
    admitted crossing. Otherwise the fact holds when the machine's exit
    entailment proves the expression, when it is a named witness-bearing
    `ensures` that the evidence-forwarding proof pass discharges, or, when no
    entry parameter it names lacks an exact incoming origin, when the entry
    contexts prove it or another exit prover does: result domains
    (`result_domains`), scalar returns (`scalars`), content preservation,
    embedded literals, transparent propositions, collection-predicate premises of a
    helper theorem or checked store handback, or an authorized domain
    establishment route. A proved loop header invariant (`cyclic_headers`)
    also satisfies it. A fact nothing proves is reported with the entry
    parameters that lack an origin.

    `cases` observes which case an exit returns, for `scalars` and
    `result_domains`. `result_domains` also exports `exact_scalar_membership`
    to the nominal input and field write checks, and domain queries used by
    flow facts, fact construction, content checks and proof.
*/

use crate::checks::content::proves_composed_exit;
use crate::flow::active_context_handles;
use checked_trees::{CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use super::prover::semantic_contexts_prove_contract_fact;
use crate::published::labels::{machine_name, semantic_fact_requirement_label};

mod bindings {
    /*
        Prove each exit obligation in the declaration that authored it.

        An ordinary implementation can owe several public contracts with distinct
        parameter and result symbols. The retained conformance supplies their exact
        correspondence; the flow graph still supplies every original-to-current
        value mapping. Neither the public goal nor a same-spelled parameter proves
        anything about the completed result.
    */
    use checked_trees::{CheckFacts, ContractProofFactKind, ContractProofFactOwner, FlowExitFact};
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::StateParameter;
    use typed_trees::type_identity::TypeIdentityRequest;
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::{machine::Machine, state::State};

    #[allow(clippy::too_many_arguments)]
    pub(super) fn proves(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        exit: &FlowExitFact,
        machine: &Machine,
        state: &State,
        source_fact: arena::Handle<typed_trees::domain::ProofFact>,
        expression: ExpressionHandle,
        origins: &[(symbols::SymbolHandle, symbols::SymbolHandle)],
    ) -> bool {
        if program.proof_facts.get(source_fact).as_expression() != Some(&expression) {
            return false;
        }
        let Some(entry) = program.machine_states(machine).first() else {
            return false;
        };
        let returned = super::super::return_values::exit_return_expression(program, exit);
        let mut inherited = false;
        for (_, contract) in facts.proof.contract_facts.iter().filter(|(_, contract)| {
            contract.kind == ContractProofFactKind::Ensures
                && contract.fact == source_fact
                && contract.inherited_scope.is_some()
                && matches!(contract.owner, ContractProofFactOwner::MachineState { machine_symbol, .. }
                    if machine_symbol == machine.symbol)
        }) {
            inherited = true;
            let Some((declaration, requirement, supplying, scope)) =
                crate::published::semantic::facts::inherited_contract_scope_parts(
                    program,
                    &facts.proof,
                    contract,
                )
            else {
                return false;
            };
            if scope.conformance_machine != machine.symbol || supplying.symbol != entry.symbol {
                return false;
            }
            let Some(applied) = crate::published::semantic::facts::applied_inherited_requirement(
                program,
                requirement,
                scope,
            ) else {
                return false;
            };
            let Some(actual) = validation::inherited_satisfier_parameters(
                program,
                declaration,
                requirement,
                supplying,
            ) else {
                return false;
            };
            let declared = program.state_signature_parameters(requirement);
            let selected = program.state_signature_parameters(applied);
            if declared.len() != selected.len()
                || selected.len() != actual.len()
                || !same_type(program, applied.return_type, supplying.return_type)
                || !selected.iter().zip(actual).all(|(formal, actual)| {
                    formal.is_const == actual.is_const
                        && formal.is_mutable == actual.is_mutable
                        && formal.relevance == actual.relevance
                        && same_type(program, formal.type_reference, actual.type_reference)
                })
            {
                return false;
            }
            let parameters = declared
                .iter()
                .zip(selected)
                .map(|(declared, selected)| StateParameter {
                    type_reference: selected.type_reference,
                    ..declared.clone()
                })
                .collect::<Vec<_>>();
            let type_bindings = program
                .trait_type_parameters(declaration)
                .iter()
                .zip(&scope.trait_arguments)
                .map(|(formal, actual)| (formal.symbol, *actual))
                .collect::<Vec<_>>();
            if !validation::normal_exit_proves(
                program,
                machine,
                state,
                exit.statement_index,
                returned,
                validation::NormalExitContract {
                    expression,
                    callable: requirement.symbol,
                    parameters: &parameters,
                    supplying_parameters: actual,
                    type_bindings: &type_bindings,
                },
                origins,
                &facts.termination.rank_arrivals,
            ) {
                return false;
            }
        }
        inherited
            || validation::normal_exit_proves(
                program,
                machine,
                state,
                exit.statement_index,
                returned,
                validation::NormalExitContract {
                    expression,
                    callable: machine.symbol,
                    parameters: program.state_parameters(entry),
                    supplying_parameters: program.state_parameters(entry),
                    type_bindings: &[],
                },
                origins,
                &facts.termination.rank_arrivals,
            )
    }

    fn same_type(
        program: &dyn validation::CheckingInput,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
    ) -> bool {
        left.is_valid()
            && right.is_valid()
            && program.type_identity(TypeIdentityRequest::ordinary(left))
                == program.type_identity(TypeIdentityRequest::ordinary(right))
    }
}
mod cases {
    /*
        Result classification observes the exact normal return, not value equality
        against a fabricated case payload. Constructors provide their nominal tag;
        saved places consume only live flow predicates. Initializers and calls are
        never replayed to recover a tag after their evaluation point.
    */
    use crate::published::places::place_at;
    use validation::CanonicalPlace;

    use checked_trees::{CheckFacts, ContractProofFactKind, ContractProofFactOwner, FlowExitFact};
    use facts::{FactContextHandle, FactPayload, PlaceRoot, PlaceSegment};
    use symbols::SymbolHandle;
    use typed_trees::{
        expression::{ExpressionHandle, ExpressionNode},
        machine::Machine,
        state::State,
    };

    use super::super::return_values::exit_return_expression;

    impl<'a> CaseObservation<'a> {
        pub(super) fn for_requirement(
            program: &'a dyn ::validation::CheckingInput,
            facts: &'a CheckFacts,
            exit: &'a FlowExitFact,
            contexts: &'a [FactContextHandle],
            requirement: &facts::Fact,
            call_frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Option<Self> {
            let FactPayload::ContractBooleanExpression {
                fact, expression, ..
            } = requirement.payload
            else {
                return None;
            };
            if !matches!(program.proof_facts.get(fact), typed_trees::domain::ProofFact::Expression(source) if *source == expression)
            {
                return None;
            }
            if !facts
                .proof
                .contract_fact_refs
                .span_or_empty(exit.ensures)
                .iter()
                .any(|reference| {
                    let contract = facts.proof.contract_facts.get(reference.fact);
                    contract.fact == fact
                        && contract.kind == ContractProofFactKind::Ensures
                        && match contract.owner {
                            ContractProofFactOwner::Machine { machine_symbol } => {
                                machine_symbol == exit.machine_symbol
                            }
                            ContractProofFactOwner::MachineState {
                                machine_symbol,
                                state_symbol,
                            } => {
                                machine_symbol == exit.machine_symbol
                                    && state_symbol == exit.state_symbol
                            }
                            _ => false,
                        }
                })
            {
                return None;
            }
            Self::for_exit(program, facts, exit, contexts, call_frames)
        }

        pub(super) fn for_exit(
            program: &'a dyn ::validation::CheckingInput,
            facts: &'a CheckFacts,
            exit: &'a FlowExitFact,
            contexts: &'a [FactContextHandle],
            call_frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Option<Self> {
            let machine = program.machine(exit.machine_symbol)?;
            let state = program.state_in_machine(machine, exit.state_symbol)?;
            let returned = exit_return_expression(program, exit);
            let mut returned_nodes = Vec::new();
            validation::expression_traversal::collect_expression_tree(
                program,
                returned,
                &mut returned_nodes,
            );
            let has_unframed_writes = returned_nodes.iter().any(|expression| {
                matches!(
                    program.expression_table.expression(*expression),
                    ExpressionNode::Atomic(_)
                )
            }) || facts.operators.uses.iter().any(|(_, operator)| {
                operator.status != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                    && returned_nodes.contains(&operator.expression)
            });
            Some(CaseObservation {
                program,
                facts,
                exit,
                contexts,
                machine,
                state,
                return_has_no_writes: !has_unframed_writes
                    && call_frames.is_some_and(|frames| {
                        frames
                            .expression_write_frame(machine, returned)
                            .into_complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                    }),
            })
        }
    }

    pub(super) struct CaseObservation<'a> {
        program: &'a dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        exit: &'a FlowExitFact,
        contexts: &'a [FactContextHandle],
        machine: &'a Machine,
        state: &'a State,
        return_has_no_writes: bool,
    }

    impl CaseObservation<'_> {
        /*
            An absent payload owes no field membership. Unknown tags still owe the
            conditional field contract; neither a missing fact nor a failed
            projection establishes that its case is absent.
        */
        pub(super) fn result_path_is_inactive(&self, path: &[PlaceSegment]) -> bool {
            let returned = exit_return_expression(self.program, self.exit);
            path.iter().enumerate().any(|(position, segment)| {
                let PlaceSegment::Case { variant } = segment else {
                    return false;
                };
                self.returned_case(returned, &path[..position], *variant, false, true, 0)
            })
        }

        pub(super) fn observe(&self, expression: ExpressionHandle) -> Option<bool> {
            let binary = self.program.expression_table.binary_node(expression)?;
            let case = validation::applied_case_membership_variant(
                self.program,
                self.machine,
                Some(self.state),
                expression,
                binary,
            )?;
            let subject = validation::reserved_result_place(self.program, binary.left)?;
            if subject.machine_symbol != self.machine.symbol {
                return None;
            }
            let returned = exit_return_expression(self.program, self.exit);
            if self.returned_case(returned, &subject.segments, case, true, true, 0) {
                Some(true)
            } else if self.returned_case(returned, &subject.segments, case, false, true, 0) {
                Some(false)
            } else {
                None
            }
        }

        fn returned_case(
            &self,
            expression: ExpressionHandle,
            projection: &[PlaceSegment],
            case: SymbolHandle,
            required: bool,
            can_read_live: bool,
            depth: usize,
        ) -> bool {
            if self
                .program
                .expression_table
                .expression_at_depth(expression, depth)
                .is_none()
            {
                return false;
            }
            if let Some(dispatch) = self.program.expression_table.match_node(expression) {
                return !self
                    .program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .is_empty()
                    && self
                        .program
                        .expression_table
                        .match_arms_all(dispatch.arms, |_, value| {
                            self.returned_case(
                                value,
                                projection,
                                case,
                                required,
                                can_read_live,
                                depth + 1,
                            )
                        });
            }
            if let Some((segment, rest)) = projection.split_first() {
                if let Some(literal) = self
                    .program
                    .expression_table
                    .struct_literal_node(expression)
                {
                    if let PlaceSegment::Case { variant } = segment {
                        let declared = self.program.data_definitions().iter().any(|data| {
                            data.symbol == literal.type_symbol
                                && self.program.data_members(data).iter().any(|member| {
                                    matches!(member, typed_trees::data::DataMember::Variant(candidate)
                                        if candidate.symbol == *variant)
                                })
                        });
                        return declared
                            && literal.case_symbol == Some(*variant)
                            && self.returned_case(
                                expression,
                                rest,
                                case,
                                required,
                                can_read_live,
                                depth + 1,
                            );
                    }
                    let PlaceSegment::Field { symbol } = segment else {
                        return false;
                    };
                    let field_owner = literal.case_symbol.unwrap_or(literal.type_symbol);
                    if !symbol.is_valid()
                        || self.program.symbols.get(*symbol).kind != symbols::SymbolKind::Field
                        || self.program.symbols.get(*symbol).parent != field_owner
                    {
                        return false;
                    }
                    let mut fields = self
                        .program
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .filter(|field| field.field_symbol == *symbol);
                    let Some(field) = fields.next() else {
                        return false;
                    };
                    if fields.next().is_some() {
                        return false;
                    }
                    return self.returned_case(
                        field.value,
                        rest,
                        case,
                        required,
                        can_read_live && self.return_has_no_writes,
                        depth + 1,
                    );
                }
                if let Some(elements) = self.program.expression_table.array_literal_node(expression)
                {
                    let PlaceSegment::FixedIndex { index } = segment else {
                        return false;
                    };
                    let Some(element) = self
                        .program
                        .expression_table
                        .expression_handles(*elements)
                        .get(*index)
                    else {
                        return false;
                    };
                    return self.returned_case(
                        *element,
                        rest,
                        case,
                        required,
                        can_read_live && self.return_has_no_writes,
                        depth + 1,
                    );
                }
                return can_read_live
                    && self.live_case(expression, projection, case, required, depth);
            }
            let owner = self.program.symbols.get(case).parent;
            let Some(data) = self.program.data_definition_by_symbol(owner) else {
                return false;
            };
            match self.program.expression_table.expression(expression) {
                ExpressionNode::StructLiteral(literal) => {
                    let Some(actual) = literal.case_symbol else {
                        return false;
                    };
                    let declared = self.program.data_members(data).iter().any(|member| {
                        matches!(member, typed_trees::data::DataMember::Variant(variant) if variant.symbol == actual)
                    });
                    literal.type_symbol == owner && declared && (actual == case) == required
                }
                ExpressionNode::Name(path)
                    if self.program.symbols.get(path.symbol).kind
                        == symbols::SymbolKind::Variant =>
                {
                    let Some(actual_owner) =
                        validation::exact_case_reference_owner(self.program, expression)
                    else {
                        return false;
                    };
                    let payload_free = self.program.data_members(data).iter().any(|member| {
                        matches!(member, typed_trees::data::DataMember::Variant(variant) if variant.symbol == path.symbol && variant.payload.is_empty())
                    });
                    actual_owner.symbol == owner
                        && payload_free
                        && (path.symbol == case) == required
                }
                _ => can_read_live && self.live_case(expression, &[], case, required, depth),
            }
        }

        fn live_case(
            &self,
            expression: ExpressionHandle,
            projection: &[PlaceSegment],
            case: SymbolHandle,
            required: bool,
            _depth: usize,
        ) -> bool {
            let Some(mut subject) = self.stable_place(expression) else {
                return false;
            };
            if !crate::flow::place_cases_are_selected(
                self.program,
                &self.facts.semantic,
                self.contexts,
                self.machine.symbol,
                self.state.symbol,
                self.exit.statement_index,
                &subject,
            ) {
                return false;
            }
            subject.segments.extend_from_slice(projection);
            crate::flow::place_case_has_value(
                self.program,
                &self.facts.semantic,
                self.contexts,
                self.machine.symbol,
                self.state.symbol,
                self.exit.statement_index,
                &subject,
                case,
                required,
            )
        }

        fn stable_place(&self, expression: ExpressionHandle) -> Option<CanonicalPlace> {
            let place = place_at(
                self.program,
                self.state.symbol,
                self.exit.statement_index,
                expression,
            )?;
            (matches!(place.root, PlaceRoot::Symbol(symbol) if symbol.is_valid())
                && place.segments.iter().all(|segment| {
                    matches!(segment, PlaceSegment::Field { symbol } if symbol.is_valid())
                        || matches!(segment, PlaceSegment::Case { variant } if variant.is_valid())
                        || matches!(segment, PlaceSegment::FixedIndex { .. })
                }))
            .then_some(place)
        }
    }
}
mod cyclic_headers {
    /*
        Header invariants for a cyclic machine whose guarantee relates the value
        it returns to an invocation formal.

        Each state in a cyclic transition component has its own current parameter
        frame; the invocation formals are a separate immutable frame even when the
        entry state is reentered. Only the flow owner's exact all-path origin
        solution can identify an immutable scalar with an invocation value; another
        parameter changing on that same edge does not invalidate this identity. For
        every exit returning `e`, propose its guarantee with `result` replaced by `e`
        in that state's frame. The component owns the
        conjunction of these state-local candidates. Every incoming edge proves the
        destination conjunction under simultaneous actual-argument substitution.
        An internal edge may assume only its caller's conjunction. An external edge
        can use a preceding component's relation only after that component's complete
        induction has checked. Invocation is an additional
        initial arrival when the entry belongs to the component.

        Only after the entire component's arrivals check do its candidates discharge
        their exits. Current state requirements and arm guards remain local to the
        caller; invocation requirements remain about the immutable formals. Neither
        ranking nor prefix facts grant an invariant. Unsupported substitutions and
        any failed arrival retire the whole component proposal. Candidate discovery
        remains incomplete: demands propagate backwards along state-simple paths,
        including exitless intermediaries and acyclic bridges, and stop before a
        state repeats. They never unroll a cycle to invent further predicates.

        The binders are the state's immutable fixed-integer parameters in either
        readable arithmetic domain. `Exact` terms read as mathematical integers
        directly. A `Wrapping` binder reads as its residue representative: the
        engine argues in integers, and an integer identity descends to Z/2^w, so
        a residue binder may appear in an equality goal. An order or disequality
        on a wrapped value does not descend, so propositions name residue terms
        only under `==`. Facts fed to the engine must additionally read the
        program exactly: `x < y` on wrapped atoms compares the residues, while
        `x + y == z` would claim an integer equality stronger than the wrapped
        congruence the program established — so a hypothesis names residue terms
        only between bare binders and literals. `Saturating` and `Trapping`
        stay outside the language.
    */
    use crate::checks::unsigned;
    use arena::Handle;
    use checked_trees::{CheckFacts, ContractProofFactKind, FlowExitFact, FlowStateFact};
    use facts::{ContractFactKind, FactOrigin, FactPayload};
    use language_core::OperatorSpelling;
    use numerics::arithmetic::ArithmeticDomain;
    use symbols::SymbolHandle;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::TransitionTargetHandle;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
    use validation::{
        ScopedArithmeticBinder as ArithmeticBinder, ScopedArithmeticBinding,
        ScopedArithmeticExpression, ScopedArithmeticHypothesis as ArithmeticHypothesis,
        ScopedArithmeticValue, StrictArithmeticImplicationJudgment, scoped_arithmetic_implication,
    };

    mod completions {
        /*
            Prove a completion property from all of its actual result producers.

            A terminal invocation forwards its result unchanged. Following these edges
            can cross machine boundaries or revisit a cycle without assuming that
            terminal callee's postcondition. Every finite normal completion must reach a
            value producer; each producer must independently establish the property. Unknown
            dispatch, missing flow and an unproved producer refuse the whole closure.

            Result-only properties need no input substitution. Current exclusive
            referents follow the call's exact storage correspondence, with the complete
            projected goal retained at each node. Entry snapshots remain separate. This
            traversal grants no recursive citation, termination, argument admission or
            reference authority.
        */
        use super::super::super::return_values::exit_return_call_in_state;
        use super::{
            AdmittedGuarantee, Header, exit_return_expression, fixed_integer, is_result_reference,
        };
        use arena::Handle;
        use checked_trees::{CheckFacts, CheckedScalarExpression, FlowExitFact};
        use numerics::arithmetic::ArithmeticDomain;
        use symbols::SymbolHandle;
        use typed_trees::{
            domain::ProofFact,
            expression::{ExpressionHandle, ExpressionNode},
            machine::Machine,
            state::State,
            statement::{StatementNode, TransitionExit, TransitionTargetNode},
        };
        use validation::BigIntInterval;
        use validation::{
            ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
            ScopedArithmeticValue,
        };

        mod referents {
            /*
                Check guarantees about borrowed data after a terminal call.
                Each node retains the original proposition's complete projected subjects.
                Leaves read independently established current values, with the ordinary
                dependency checks for completed helper calls. Entry snapshots and the
                terminal callee's own postcondition cannot close its proof.
            */
            use crate::checks::current_scalar_values::CurrentScalarValues;
            use checked_trees::{CheckFacts, FlowCallFact, FlowExitFact};
            use facts::{PlaceRoot, PlaceSegment};
            use symbols::SymbolHandle;
            use typed_trees::{
                expression::{ExpressionHandle, ExpressionNode},
                machine::Machine,
                state::State,
            };
            use validation::{
                CanonicalPlace, ScopedArithmeticBinder, ScopedArithmeticBinding,
                ScopedArithmeticExpression,
            };

            pub(super) struct ReferentGoal {
                owner: SymbolHandle,
                expression: ExpressionHandle,
                reads: Vec<ExpressionHandle>,
                pub(super) initial: Vec<Vec<CanonicalPlace>>,
            }

            impl ReferentGoal {
                pub(super) fn new(
                    program: &dyn validation::CheckingInput,
                    facts: &CheckFacts,
                    machine: &Machine,
                    expression: ExpressionHandle,
                    remaining: &mut usize,
                ) -> Option<Self> {
                    let owner = machine.symbol;
                    let trace = ::validation::trace_flags::struct_trace();
                    let entry = program.machine_states(machine).first()?;
                    if !crate::checks::contracts::prover::has_builtin_operators(
                        program,
                        &facts.operators,
                        expression,
                    ) {
                        if trace {
                            eprintln!(
                                "STRUCT terminal-referent declined owner={} reason=operator-meaning",
                                machine.name
                            );
                        }
                        return None;
                    }
                    let mut tree = Vec::new();
                    validation::expression_traversal::collect_expression_tree(
                        program, expression, &mut tree,
                    );
                    if tree.iter().any(|operand| {
                        matches!(
                            program.expression_table.expression(*operand),
                            ExpressionNode::Call(_)
                        ) || super::super::is_result_reference(program, machine, *operand)
                    }) {
                        if trace {
                            eprintln!(
                                "STRUCT terminal-referent declined owner={} reason=call-or-result",
                                machine.name
                            );
                        }
                        return None;
                    }
                    let mut reads = Vec::new();
                    let mut places = Vec::new();
                    for operand in tree {
                        super::super::demands::charge(remaining)?;
                        if !matches!(
                            program.expression_table.expression(operand),
                            ExpressionNode::Name(_) | ExpressionNode::Member(_)
                        ) {
                            continue;
                        }
                        let Some(place) =
                            validation::canonical_place_from_expression(program, operand)
                        else {
                            continue;
                        };
                        let Some(parameter) =
                            program.state_parameters(entry).iter().find(|parameter| {
                                place.root == PlaceRoot::Symbol(parameter.symbol)
                                    && validation::reference_shell(
                                        program,
                                        parameter.type_reference,
                                    )
                                    .is_some_and(|shell| shell.access.is_exclusive())
                            })
                        else {
                            continue;
                        };
                        if !place
                            .segments
                            .iter()
                            .all(|segment| matches!(segment, PlaceSegment::Field { .. }))
                        {
                            return None;
                        }
                        let Some(reference) =
                            validation::parameter_expression_result_type_reference(
                                program,
                                entry.symbol,
                                program.state_parameters(entry),
                                operand,
                            )
                        else {
                            continue;
                        };
                        if super::super::fixed_integer(program, reference).is_none() {
                            continue;
                        }
                        if !validation::state_reference_parameter_binding_is_stable(
                            program,
                            machine,
                            entry,
                            parameter.symbol,
                        ) {
                            if trace {
                                eprintln!(
                                    "STRUCT terminal-referent declined owner={} reason=unstable-reference parameter={:?}",
                                    machine.name, parameter.symbol
                                );
                            }
                            return None;
                        }
                        reads.push(operand);
                        places.push(place);
                    }
                    if reads.is_empty() {
                        if trace {
                            eprintln!(
                                "STRUCT terminal-referent declined owner={} reason=no-scalar-reference parameters={:?}",
                                machine.name,
                                program.state_parameters(entry)
                            );
                        }
                        return None;
                    }
                    let origins = crate::flow::activation_parameter_origins(program, machine);
                    let mut initial = Vec::new();
                    for state in program.machine_states(machine) {
                        super::super::demands::charge(remaining)?;
                        let origins =
                            crate::flow::state_parameter_origins(program, machine, state, &origins);
                        let mapped = places
                            .iter()
                            .map(|place| {
                                *remaining =
                                    remaining.checked_sub(place.segments.len().checked_add(1)?)?;
                                let source = place.root.as_symbol()?;
                                let mut matches =
                                    origins.iter().filter(|(entry, _)| *entry == source);
                                let (_, destination) = matches.next()?;
                                if matches.next().is_some() {
                                    return None;
                                }
                                if !validation::state_reference_parameter_binding_is_stable(
                                    program,
                                    machine,
                                    state,
                                    *destination,
                                ) {
                                    return None;
                                }
                                Some(CanonicalPlace {
                                    root: PlaceRoot::Symbol(*destination),
                                    segments: place.segments.clone(),
                                })
                            })
                            .collect::<Option<Vec<_>>>();
                        let Some(places) = mapped else {
                            if trace {
                                eprintln!(
                                    "STRUCT terminal-referent declined owner={} reason=initial-subjects state={:?} origins={origins:?}",
                                    machine.name, state.symbol
                                );
                            }
                            return None;
                        };
                        initial.push(places);
                    }

                    if trace {
                        eprintln!(
                            "STRUCT terminal-referent prepared owner={} reads={reads:?} initial={initial:?}",
                            machine.name
                        );
                    }
                    Some(Self {
                        owner,
                        expression,
                        reads,
                        initial,
                    })
                }

                pub(super) fn forward<'program>(
                    program: &'program dyn validation::CheckingInput,
                    frames: &validation::CallFrameResolver<'program>,
                    machine: &'program Machine,
                    state: &State,
                    call: &FlowCallFact,
                    places: &[CanonicalPlace],
                    remaining: &mut usize,
                ) -> Option<Vec<CanonicalPlace>> {
                    let target = validation::call_target_state(program, call.target_symbol)?;
                    let correspondence = frames.normal_return_reference_correspondence(
                        machine,
                        state,
                        call,
                        program.state_parameters(target),
                    );
                    if ::validation::trace_flags::struct_trace() {
                        eprintln!(
                            "STRUCT terminal-referent forward caller={} target={:?} writes_complete={} subjects={places:?} correspondence={correspondence:?}",
                            machine.name, target.symbol, call.normal_return_writes_complete
                        );
                    }
                    let correspondence = correspondence?;
                    *remaining = remaining.checked_sub(correspondence.len())?;
                    let comparison_work = correspondence
                        .iter()
                        .try_fold(1usize, |work, (_, source)| {
                            work.checked_add(source.segments.len().checked_add(1)?)
                        })?;
                    *remaining =
                        remaining.checked_sub(comparison_work.checked_mul(places.len())?)?;
                    let places = places
                        .iter()
                        .map(|place| {
                            *remaining =
                                remaining.checked_sub(place.segments.len().checked_add(1)?)?;
                            let mut suppliers =
                                correspondence.iter().filter_map(|(parameter, source)| {
                                    if source.root != place.root {
                                        return None;
                                    }
                                    let suffix =
                                        place.segments.strip_prefix(source.segments.as_slice())?;
                                    Some(CanonicalPlace {
                                        root: PlaceRoot::Symbol(*parameter),
                                        segments: suffix.to_vec(),
                                    })
                                });
                            let place = suppliers.next()?;
                            suppliers.next().is_none().then_some(place)
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Some(places)
                }

                pub(super) fn proves_leaf<'program>(
                    &self,
                    program: &'program dyn validation::CheckingInput,
                    frames: &validation::CallFrameResolver<'program>,
                    machine: &'program Machine,
                    state: &'program State,
                    exit: &FlowExitFact,
                    places: &[CanonicalPlace],
                ) -> bool {
                    let Some(mut values) = CurrentScalarValues::at(
                        program,
                        machine,
                        state,
                        exit.statement_index,
                        exit.transition_target,
                        Some(frames),
                    ) else {
                        return false;
                    };
                    let returned = super::super::exit_return_expression(program, exit);
                    if returned.is_valid() {
                        values.retire_operand_effects(returned);
                    }
                    let bindings = self
                        .reads
                        .iter()
                        .zip(places)
                        .map(|(read, place)| {
                            Some(ScopedArithmeticBinding {
                                binder: ScopedArithmeticBinder::Occurrence(*read),
                                value: values.scalar_value_at_place(place)?,
                            })
                        })
                        .collect::<Option<Vec<_>>>();
                    let Some(bindings) = bindings else {
                        if ::validation::trace_flags::struct_trace() {
                            eprintln!(
                                "STRUCT terminal-referent leaf-unavailable owner={} state={:?} subjects={places:?}",
                                machine.name, state.symbol
                            );
                        }
                        return false;
                    };
                    let goal = ScopedArithmeticExpression {
                        owner: self.owner,
                        expression: self.expression,
                        bindings,
                    };
                    let proven = values.proves(program, machine, &goal);
                    if ::validation::trace_flags::struct_trace() {
                        eprintln!(
                            "STRUCT terminal-referent leaf owner={} state={:?} goal={goal:?} proven={proven}",
                            machine.name, state.symbol
                        );
                    }
                    proven
                }
            }
        }

        pub(super) fn proves(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            owner: SymbolHandle,
            contract: Handle<ProofFact>,
            admitted: &[AdmittedGuarantee],
        ) -> bool {
            let Some(machine) = program.machine(owner) else {
                return false;
            };
            let Some(entry) = program.machine_states(machine).first() else {
                return false;
            };
            let Some(guarantee) = program.proof_facts.get(contract).as_expression() else {
                return false;
            };
            let origins = crate::flow::activation_parameter_origins(program, machine);
            let origins = program
                .machine_states(machine)
                .iter()
                .map(|state| {
                    crate::flow::state_parameter_origins(program, machine, state, &origins)
                })
                .collect::<Vec<_>>();
            let Some(header) = Header::new(program, facts, machine, entry, &origins) else {
                return false;
            };
            let mut expressions = Vec::new();
            validation::expression_traversal::collect_expression_tree(
                program,
                *guarantee,
                &mut expressions,
            );
            let result_only = header.proposition_is_admitted(*guarantee)
                && expressions.iter().all(|expression| {
                    !matches!(
                        program.expression_table.expression(*expression),
                        ExpressionNode::Name(_)
                    ) || is_result_reference(program, machine, *expression)
                });
            let mut remaining = super::demands::TRANSPORT_WORK;
            let referent_goal =
                referents::ReferentGoal::new(program, facts, machine, *guarantee, &mut remaining);
            let frames = referent_goal
                .as_ref()
                .and_then(|_| validation::CallFrameResolver::new(program));
            let mut pending = program
                .machine_states(machine)
                .iter()
                .enumerate()
                .map(|(index, state)| {
                    (
                        owner,
                        state.symbol,
                        referent_goal
                            .as_ref()
                            .map(|goal| goal.initial[index].clone()),
                    )
                })
                .collect::<Vec<_>>();
            let mut visited = std::collections::HashSet::new();
            let mut observed_return = false;
            while let Some((machine_symbol, state_symbol, subjects)) = pending.pop() {
                let work = subjects.as_ref().map_or(Some(1usize), |subjects| {
                    subjects.iter().try_fold(1usize, |work, subject| {
                        work.checked_add(subject.segments.len().checked_add(1)?)
                    })
                });
                let Some(left) = work.and_then(|work| remaining.checked_sub(work)) else {
                    return false;
                };
                remaining = left;
                if !visited.insert((machine_symbol, state_symbol, subjects.clone())) {
                    continue;
                }
                let Some((current_machine, state)) =
                    program.state_flow_site(machine_symbol, state_symbol)
                else {
                    return false;
                };
                let Some(state_flow) = Header::state_flow(facts, current_machine, state) else {
                    return false;
                };
                let mut outgoing_found = false;
                for outgoing in facts
                    .flow
                    .control
                    .outgoing
                    .span_or_empty(state_flow.outgoing)
                {
                    if super::demands::charge(&mut remaining).is_none() {
                        return false;
                    }
                    outgoing_found = true;
                    if let Some(StatementNode::Transition(transition)) = program
                        .statement_table
                        .statement_at(state, outgoing.statement_index)
                        && matches!(transition.exit, TransitionExit::Crash(_))
                        && outgoing.transition_target == transition.target
                    {
                        continue;
                    }
                    if outgoing.transition_target.is_valid()
                        && let TransitionTargetNode::Named { path, .. } = program
                            .statement_table
                            .transition_target(outgoing.transition_target)
                        && let Some(destination) = validation::activation_target_state_index(
                            program,
                            current_machine,
                            path.symbol,
                        )
                    {
                        let next_subjects = if referent_goal.is_some() {
                            let Some(frames) = &frames else {
                                return false;
                            };
                            let mut calls = facts
                                .flow
                                .control
                                .calls
                                .span_or_empty(state_flow.calls)
                                .iter()
                                .filter(|call| {
                                    call.statement_index == outgoing.statement_index
                                        && validation::transition_call_target(
                                            program,
                                            current_machine,
                                            state,
                                            call.statement_index,
                                            call.call_ordinal,
                                        ) == Some(outgoing.transition_target)
                                });
                            let Some(call) = calls.next() else {
                                return false;
                            };
                            if calls.next().is_some() {
                                return false;
                            }
                            let Some(subjects) = subjects.as_deref().and_then(|subjects| {
                                referents::ReferentGoal::forward(
                                    program,
                                    frames,
                                    current_machine,
                                    state,
                                    call,
                                    subjects,
                                    &mut remaining,
                                )
                            }) else {
                                return false;
                            };
                            Some(subjects)
                        } else {
                            None
                        };
                        pending.push((
                            machine_symbol,
                            program.machine_states(current_machine)[destination].symbol,
                            next_subjects,
                        ));
                        continue;
                    }
                    let mut exits = facts
                        .flow
                        .control
                        .exits
                        .span_or_empty(state_flow.exits)
                        .iter()
                        .filter(|exit| {
                            exit.statement_index == outgoing.statement_index
                                && exit.transition_target == outgoing.transition_target
                                && exit.returned_value == outgoing.returned_value
                        });
                    let Some(exit) = exits.next() else {
                        return false;
                    };
                    if exits.next().is_some() {
                        return false;
                    }
                    if let Some(call) = exit_return_call_in_state(program, facts, state_flow, exit)
                    {
                        let Some(callee) = program.machine_holding_state(call.target_symbol) else {
                            return false;
                        };
                        let Some(callee_entry) = program.machine_states(callee).first() else {
                            return false;
                        };
                        if call.target_symbol != callee_entry.symbol {
                            return false;
                        }
                        let next_subjects = if referent_goal.is_some() {
                            let Some(frames) = &frames else {
                                return false;
                            };
                            let Some(subjects) = subjects.as_deref().and_then(|subjects| {
                                referents::ReferentGoal::forward(
                                    program,
                                    frames,
                                    current_machine,
                                    state,
                                    call,
                                    subjects,
                                    &mut remaining,
                                )
                            }) else {
                                return false;
                            };
                            Some(subjects)
                        } else {
                            if (!result_only && callee.symbol != owner)
                                || fixed_integer(program, entry.return_type).is_none()
                                || fixed_integer(program, entry.return_type)
                                    != fixed_integer(program, callee_entry.return_type)
                            {
                                return false;
                            }
                            None
                        };
                        pending.push((callee.symbol, callee_entry.symbol, next_subjects));
                        continue;
                    }
                    if referent_goal.is_none()
                        && machine_symbol == owner
                        && admitted.contains(&AdmittedGuarantee {
                            state_symbol,
                            statement_index: exit.statement_index,
                            transition_target: exit.transition_target,
                            contract,
                        })
                    {
                        observed_return = true;
                        continue;
                    }
                    let proven = if let Some(goal) = &referent_goal {
                        frames.as_ref().zip(subjects.as_deref()).is_some_and(
                            |(frames, subjects)| {
                                goal.proves_leaf(
                                    program,
                                    frames,
                                    current_machine,
                                    state,
                                    exit,
                                    subjects,
                                )
                            },
                        )
                    } else {
                        result_only
                            && proves_value(
                                program,
                                facts,
                                owner,
                                *guarantee,
                                current_machine,
                                state,
                                exit,
                            )
                    };
                    if !proven {
                        return false;
                    }
                    observed_return = true;
                }
                if !outgoing_found {
                    return false;
                }
            }
            observed_return
        }

        fn proves_value(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            owner: SymbolHandle,
            guarantee: ExpressionHandle,
            machine: &Machine,
            state: &State,
            exit: &FlowExitFact,
        ) -> bool {
            let origins = crate::flow::activation_parameter_origins(program, machine);
            let origins = program
                .machine_states(machine)
                .iter()
                .map(|state| {
                    crate::flow::state_parameter_origins(program, machine, state, &origins)
                })
                .collect::<Vec<_>>();
            let Some(header) = Header::new(program, facts, machine, state, &origins) else {
                return false;
            };
            let returned = exit_return_expression(program, exit);
            let bindings = header.caller_atoms(state);
            let Some(value) = return_value(&header, state, exit, returned, &bindings) else {
                return false;
            };
            let mut hypotheses = header.requirements(header.formal_roster());
            if state.symbol == header.entry.symbol {
                hypotheses.extend(header.requirements(bindings.clone()));
            }
            for context in facts
                .flow
                .contexts
                .semantic_context_refs
                .span_or_empty(exit.entry_semantic_contexts)
            {
                for fact in facts
                    .semantic
                    .context_view(facts.semantic.contexts.get(context.context))
                    .facts()
                {
                    for (expression, holds) in header.arm_facts(fact) {
                        hypotheses.push(header.hypothesis(expression, bindings.clone(), holds));
                    }
                }
            }
            let goal = ScopedArithmeticExpression {
                owner,
                expression: guarantee,
                bindings: vec![ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Result,
                    value,
                }],
            };
            header.proves(&hypotheses, &goal)
        }

        /*
            Exact terms retain their symbolic relationships. Other selected arithmetic
            contributes only independently established bounds, never an integer rewrite
            of residue operations. Both routes read immutable scalar parameters only:
            no replayed local initializer, borrowed storage or imported call guarantee
            can justify a cyclic completion.
        */
        fn return_value(
            header: &Header<'_, '_>,
            state: &State,
            exit: &FlowExitFact,
            returned: ExpressionHandle,
            bindings: &[ScopedArithmeticBinding],
        ) -> Option<ScopedArithmeticValue> {
            if header
                .term_reference(returned)
                .is_some_and(|term| term.domain == ArithmeticDomain::Exact)
            {
                return Some(ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                    owner: header.machine.symbol,
                    expression: returned,
                    bindings: bindings.to_vec(),
                }));
            }
            let (primitive, _) = fixed_integer(header.program, state.return_type)?;
            let selected = crate::values::lower_unit_scalar_argument_for_places(
                header.program,
                &header.facts.operators,
                state,
                exit.statement_index,
                returned,
                primitive,
                |place, primitive_type| {
                    let facts::PlaceRoot::Symbol(symbol) = place.root else {
                        return None;
                    };
                    (place.segments.is_empty()
                        && header
                            .roster
                            .iter()
                            .any(|parameter| parameter.symbol == symbol))
                    .then_some(CheckedScalarExpression::StorageRead {
                        symbol,
                        primitive_type,
                    })
                },
            )?;
            let mut values = crate::values::LivePlaceIntervals {
                program: header.program,
                semantic: &header.facts.semantic,
                contexts: &[],
                parameters: header.program.state_parameters(state),
                symbols: &[],
                state: state.symbol,
                contents: crate::values::CurrentIntegerBounds::new(
                    header.program,
                    &header.facts.operators,
                    &header.facts.semantic,
                    &[],
                    state.symbol,
                    facts::ProgramPoint::Exit {
                        machine_symbol: exit.machine_symbol,
                        state_symbol: exit.state_symbol,
                        statement_index: exit.statement_index,
                        transition_target: exit.transition_target,
                    },
                ),
            };
            let bounds = validation::selected_scalar_interval(&selected, &mut values)?;
            Some(ScopedArithmeticValue::BoundedInteger {
                value: Box::new(ScopedArithmeticValue::Atom {
                    identity: format!("completion:{:?}:{returned:?}", state.symbol),
                    unsigned: super::unsigned(primitive),
                }),
                interval: BigIntInterval {
                    low: Some(bounds.minimum),
                    high: Some(bounds.maximum),
                },
            })
        }
    }
    mod components {
        /*
            Discover demands along state-simple backward paths, then prove components.
            A destination's exact exit predicate can demand a relation from a preceding
            component with no exit of its own. Transport retains scoped substitutions,
            not new source expressions or guessed parameter correspondence. Internal
            paths stop before revisiting a state: discovery can reach an exitless
            intermediary inside a cycle, but never unrolls that cycle. Cycle edges still
            require induction proofs, independently of how their demands were found.

            Each component proves all arrivals and invocation establishment before its
            predicates become available to later components. Acyclic singleton bridges
            follow the same rule. Within a component only its caller-state conjunction
            is assumed; externally only already proved predicates may be imported.
        */
        use super::demands;
        use super::{AdmittedGuarantee, Arrival, Header};
        use crate::checks::termination::strongly_connected_components;
        use checked_trees::CheckFacts;
        use symbols::SymbolHandle;
        use validation::{ScopedArithmeticExpression, ScopedArithmeticHypothesis};

        pub(super) fn admitted(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine_symbol: SymbolHandle,
        ) -> Vec<AdmittedGuarantee> {
            let Some(machine) = program.machine(machine_symbol) else {
                return Vec::new();
            };
            let states = program.machine_states(machine);
            let solved = crate::flow::activation_parameter_origins(program, machine);
            let origins = states
                .iter()
                .map(|state| crate::flow::state_parameter_origins(program, machine, state, &solved))
                .collect::<Vec<_>>();
            let Some(headers) = states
                .iter()
                .map(|state| Header::new(program, facts, machine, state, &origins))
                .collect::<Option<Vec<_>>>()
            else {
                return Vec::new();
            };
            let Some(arrivals) = headers
                .iter()
                .map(Header::arrivals)
                .collect::<Option<Vec<_>>>()
            else {
                return Vec::new();
            };
            let mut adjacency = vec![Vec::new(); headers.len()];
            for (destination, incoming) in arrivals.iter().enumerate() {
                for arrival in incoming {
                    if let Some(caller) = headers
                        .iter()
                        .position(|header| header.header.symbol == arrival.caller)
                        && !adjacency[caller].contains(&destination)
                    {
                        adjacency[caller].push(destination);
                    }
                }
            }
            let components = strongly_connected_components(&adjacency);
            let candidates = headers.iter().map(Header::candidates).collect::<Vec<_>>();
            let mut demands = headers
                .iter()
                .zip(&candidates)
                .map(|(header, candidates)| {
                    candidates
                        .iter()
                        .map(|candidate| header.transported(candidate, header.current_roster()))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            discover(&headers, &arrivals, &mut demands);

            let mut proved = vec![false; states.len()];
            let mut admitted = Vec::new();
            for component in components.iter().rev() {
                if proves_component(&headers, component, &arrivals, &demands, &proved) {
                    for &index in component {
                        proved[index] = true;
                        admitted.extend(candidates[index].iter().map(|candidate| candidate.exit));
                    }
                }
            }
            admitted
        }

        /*
            A demand occurrence keeps its own path: another path reaching the same
            predicate can have a different unvisited predecessor. Predicate sharing in
            the roster must not suppress that distinct discovery route.
        */
        struct PendingDemand {
            state: usize,
            predicate: usize,
            visited: Vec<usize>,
        }

        fn discovery_work(remaining: &mut usize, amount: usize) -> Option<()> {
            *remaining = remaining.checked_sub(amount)?;
            Some(())
        }

        fn discover(
            headers: &[Header<'_, '_>],
            arrivals: &[Vec<Arrival>],
            demands: &mut [Vec<ScopedArithmeticExpression>],
        ) {
            let mut remaining = demands::TRANSPORT_WORK;
            let mut pending = Vec::new();
            for (state, predicates) in demands.iter().enumerate() {
                for predicate in 0..predicates.len() {
                    if discovery_work(&mut remaining, 1).is_none() {
                        return;
                    }
                    pending.push(PendingDemand {
                        state,
                        predicate,
                        visited: vec![state],
                    });
                }
            }
            let mut cursor = 0;
            while cursor < pending.len() {
                let item = &pending[cursor];
                if discovery_work(&mut remaining, item.visited.len() + 1).is_none() {
                    return;
                }
                let destination = item.state;
                let predicate = item.predicate;
                let visited = item.visited.clone();
                cursor += 1;
                for arrival in &arrivals[destination] {
                    if discovery_work(&mut remaining, headers.len() + visited.len()).is_none() {
                        return;
                    }
                    let Some(caller) = headers
                        .iter()
                        .position(|header| header.header.symbol == arrival.caller)
                    else {
                        continue;
                    };
                    if visited.contains(&caller) {
                        continue;
                    }
                    let replacements = headers[destination]
                        .arrival_roster(&arrival.arguments, headers[caller].current_roster());
                    let Some(transported) = demands::substitute(
                        &demands[destination][predicate],
                        &headers[destination],
                        &replacements,
                        &mut remaining,
                    ) else {
                        return;
                    };
                    let predicate = match demands[caller]
                        .iter()
                        .position(|existing| existing == &transported)
                    {
                        Some(index) => index,
                        None => {
                            demands[caller].push(transported);
                            demands[caller].len() - 1
                        }
                    };
                    if discovery_work(&mut remaining, visited.len() + 1).is_none() {
                        return;
                    }
                    let mut next_visited = visited.clone();
                    next_visited.push(caller);
                    pending.push(PendingDemand {
                        state: caller,
                        predicate,
                        visited: next_visited,
                    });
                }
            }
        }

        fn proves_component(
            headers: &[Header<'_, '_>],
            component: &[usize],
            arrivals: &[Vec<Arrival>],
            demands: &[Vec<ScopedArithmeticExpression>],
            proved: &[bool],
        ) -> bool {
            let mut remaining = demands::TRANSPORT_WORK;
            for &destination in component {
                let header = &headers[destination];
                if arrivals[destination].is_empty() && header.header.symbol != header.entry.symbol {
                    return false;
                }
                for arrival in &arrivals[destination] {
                    let mut hypotheses = header.requirements(header.formal_roster());
                    if let Some(caller) = headers
                        .iter()
                        .position(|header| header.header.symbol == arrival.caller)
                        && (component.contains(&caller) || proved[caller])
                    {
                        for demand in &demands[caller] {
                            let Some(proposition) = demands::substitute(
                                demand,
                                &headers[caller],
                                &arrival.caller_atoms,
                                &mut remaining,
                            ) else {
                                return false;
                            };
                            hypotheses.push(ScopedArithmeticHypothesis {
                                proposition,
                                holds: true,
                            });
                        }
                    }
                    if arrival.caller == header.entry.symbol {
                        hypotheses.extend(header.requirements(arrival.caller_atoms.clone()));
                    }
                    hypotheses.extend(arrival.hypotheses.iter().map(|(expression, holds)| {
                        header.hypothesis(*expression, arrival.caller_atoms.clone(), *holds)
                    }));
                    let replacements =
                        header.arrival_roster(&arrival.arguments, arrival.caller_atoms.clone());
                    for demand in &demands[destination] {
                        let Some(goal) =
                            demands::substitute(demand, header, &replacements, &mut remaining)
                        else {
                            return false;
                        };
                        if !header.proves(&hypotheses, &goal) {
                            return false;
                        }
                    }
                }
                if header.header.symbol == header.entry.symbol {
                    let invocation = header.requirements(header.formal_roster());
                    for demand in &demands[destination] {
                        let Some(goal) = demands::substitute(
                            demand,
                            header,
                            &header.formal_roster(),
                            &mut remaining,
                        ) else {
                            return false;
                        };
                        if !header.proves(&invocation, &goal) {
                            return false;
                        }
                    }
                }
            }
            true
        }
    }
    mod demands {
        /*
            Backward demand transport substitutes private current-frame atoms only.
            Invocation atoms stay immutable even when their source symbol also names a
            reentered parameter. An actual argument is read in the pre-edge caller's
            roster, and its replacement is never substituted again on the same edge.
            These are bounded search expressions, not facts or new typed source nodes.
        */
        use super::{
            Header, ScopedArithmeticBinding, ScopedArithmeticExpression, ScopedArithmeticValue,
        };

        /*
            Bounds all nested substitution and cloning work across one machine's
            state graph, including alternative incoming state-simple paths.
        */
        pub(super) const TRANSPORT_WORK: usize = 16_384;
        const SUBSTITUTION_DEPTH: usize = 64;

        pub(super) fn charge(remaining: &mut usize) -> Option<()> {
            *remaining = remaining.checked_sub(1)?;
            Some(())
        }

        pub(super) fn substitute(
            demand: &ScopedArithmeticExpression,
            header: &Header<'_, '_>,
            replacements: &[ScopedArithmeticBinding],
            remaining: &mut usize,
        ) -> Option<ScopedArithmeticExpression> {
            let current = header.current_atoms();
            let substitutions = current
                .iter()
                .filter_map(|source| {
                    replacements
                        .iter()
                        .find(|replacement| replacement.binder == source.binder)
                        .map(|replacement| (&source.value, &replacement.value))
                })
                .collect::<Vec<_>>();
            expression(demand, &substitutions, remaining, SUBSTITUTION_DEPTH)
        }

        fn expression(
            source: &ScopedArithmeticExpression,
            substitutions: &[(&ScopedArithmeticValue, &ScopedArithmeticValue)],
            remaining: &mut usize,
            depth: usize,
        ) -> Option<ScopedArithmeticExpression> {
            charge(remaining)?;
            let next_depth = depth.checked_sub(1)?;
            let mut bindings = Vec::new();
            for binding in &source.bindings {
                charge(remaining)?;
                bindings.push(ScopedArithmeticBinding {
                    binder: binding.binder.clone(),
                    value: value(&binding.value, substitutions, remaining, next_depth)?,
                });
            }
            Some(ScopedArithmeticExpression {
                owner: source.owner,
                expression: source.expression,
                bindings,
            })
        }

        fn value(
            source: &ScopedArithmeticValue,
            substitutions: &[(&ScopedArithmeticValue, &ScopedArithmeticValue)],
            remaining: &mut usize,
            depth: usize,
        ) -> Option<ScopedArithmeticValue> {
            charge(remaining)?;
            if let ScopedArithmeticValue::Atom { .. } = source {
                for (current, replacement) in substitutions {
                    charge(remaining)?;
                    if source == *current {
                        return value(replacement, &[], remaining, depth);
                    }
                }
            }
            Some(match source {
                ScopedArithmeticValue::TypeReference(_)
                | ScopedArithmeticValue::OwnedSnapshot(_)
                | ScopedArithmeticValue::CapturedInteger(_)
                | ScopedArithmeticValue::Atom { .. }
                | ScopedArithmeticValue::Integer(_) => source.clone(),
                ScopedArithmeticValue::BoundedInteger {
                    value: inner,
                    interval,
                } => ScopedArithmeticValue::BoundedInteger {
                    value: Box::new(value(
                        inner,
                        substitutions,
                        remaining,
                        depth.checked_sub(1)?,
                    )?),
                    interval: interval.clone(),
                },
                ScopedArithmeticValue::IntegerOperation {
                    operator,
                    left,
                    right,
                } => {
                    let next_depth = depth.checked_sub(1)?;
                    ScopedArithmeticValue::IntegerOperation {
                        operator: *operator,
                        left: Box::new(value(left, substitutions, remaining, next_depth)?),
                        right: Box::new(value(right, substitutions, remaining, next_depth)?),
                    }
                }
                ScopedArithmeticValue::Term(term) => {
                    ScopedArithmeticValue::Term(expression(term, substitutions, remaining, depth)?)
                }
                ScopedArithmeticValue::CollectionLength(term) => {
                    ScopedArithmeticValue::CollectionLength(expression(
                        term,
                        substitutions,
                        remaining,
                        depth,
                    )?)
                }
                ScopedArithmeticValue::AdmittedIntegerRemainder(term) => {
                    ScopedArithmeticValue::AdmittedIntegerRemainder(expression(
                        term,
                        substitutions,
                        remaining,
                        depth,
                    )?)
                }
            })
        }
    }

    use super::super::prover::has_builtin_operators;
    use super::super::return_values::{exit_return_expression, is_result_reference};

    /*
        Guarantees discharged by a proved header invariant, computed once per
        machine the first time one of its exit guarantees asks.
    */
    pub(in crate::checks::contracts) struct CyclicHeaderInvariants {
        machines: Vec<MachineHeaderInvariants>,
    }

    struct MachineHeaderInvariants {
        machine: SymbolHandle,
        admitted: Vec<AdmittedGuarantee>,
        completions: Vec<(Handle<ProofFact>, bool)>,
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
            program: &dyn ::validation::CheckingInput,
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
                        completions: Vec::new(),
                    });
                    self.machines.len() - 1
                });
            if self.machines[position]
                .admitted
                .contains(&AdmittedGuarantee {
                    state_symbol: exit.state_symbol,
                    statement_index: exit.statement_index,
                    transition_target: exit.transition_target,
                    contract,
                })
            {
                return true;
            }
            if let Some((_, proved)) = self.machines[position]
                .completions
                .iter()
                .find(|(candidate, _)| *candidate == contract)
            {
                return *proved;
            }
            let proved = completions::proves(
                program,
                facts,
                exit.machine_symbol,
                contract,
                &self.machines[position].admitted,
            );
            self.machines[position].completions.push((contract, proved));
            proved
        }
    }

    /*
        One transported guarantee: `guarantee` over `result` at the exit that
        returns `returned`.
    */
    struct Candidate {
        exit: AdmittedGuarantee,
        guarantee: ExpressionHandle,
        returned: ExpressionHandle,
    }

    /*
        One named arrival, retaining its caller identity and exact simultaneous
        arguments. Component membership decides whether a caller invariant is live.
    */
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
        program: &'program dyn ::validation::CheckingInput,
        facts: &'facts CheckFacts,
        machine: &'program Machine,
        /* The destination state whose local invariant is being proposed. */
        header: &'program State,
        /* The entry state; its parameters are the invocation formals. */
        entry: &'program State,
        state_flow: &'facts FlowStateFact,
        /*
            Immutable fixed-integer parameters of the header state, the only
            binders a proposition may name there, with the arithmetic domain each
            one evaluates in. Anything else stays outside the language.
        */
        roster: Vec<RosterParameter>,
        /*
            The same roster over the entry state — the formal scope the machine's
            `requires` and `ensures` are written in.
        */
        formals: Vec<RosterParameter>,
        /*
            The union over every state's parameters, the lookup a term admission
            consults: a caller state's arm fact names the caller's own binders.
        */
        terms: Vec<RosterParameter>,
        /*
            Present when the returned value is a fixed integer the synthetic
            `result` may denote, with its domain.
        */
        result: Option<(TypeReferenceHandle, ArithmeticDomain)>,
        requires: Vec<ExpressionHandle>,
        origins: &'facts [Vec<(SymbolHandle, SymbolHandle)>],
    }

    impl<'program, 'facts> Header<'program, 'facts> {
        fn new(
            program: &'program dyn ::validation::CheckingInput,
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
                    .filter(|parameter| (!parameter.is_mutable) && !parameter.is_const)
                    .filter_map(|parameter| {
                        let (primitive, domain) = fixed_integer(program, parameter.type_reference)?;
                        Some(RosterParameter {
                            symbol: parameter.symbol,
                            type_reference: parameter.type_reference,
                            domain,
                            unsigned: unsigned(primitive),
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
                    fixed_integer(program, type_reference)
                        .map(|(_, domain)| (type_reference, domain))
                });
            let requires = program
                .contract_expressions(
                    program.machine_contracts(machine).iter(),
                    typed_trees::signature::SignatureContractKind::Requires,
                )
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
            facts.flow.control.state(machine.symbol, state.symbol)
        }

        /*
            A spelled or resolved target enters `header`: the header's own symbol
            for a named state, or the machine's symbol for an entry backedge.
        */
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
        ) -> Vec<ArithmeticHypothesis> {
            self.requires
                .iter()
                .flat_map(|expression| self.hypothesis_parts(*expression, true))
                .map(|(expression, holds)| self.hypothesis(expression, bindings.clone(), holds))
                .collect()
        }

        fn proves(
            &self,
            hypotheses: &[ArithmeticHypothesis],
            goal: &ScopedArithmeticExpression,
        ) -> bool {
            scoped_arithmetic_implication(self.program, self.machine, hypotheses, goal)
                == StrictArithmeticImplicationJudgment::Proven
        }

        /*
            Every expression guarantee of every exit of the header, transported
            through that exit's returned term. A guarantee or return outside the
            language is not proposed.
        */
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
                    let Some(guarantee) =
                        self.program.proof_facts.get(contract.fact).as_expression()
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

        /*
            Every named transition into the header, from the header itself or any
            other state. `None` when an arrival cannot be read: its arguments are
            then not a substitution the invariant can be checked under, so nothing
            may be admitted. A `self` transition forwards every immutable
            parameter unchanged and preserves the conjunction trivially, so it
            needs no obligation here.
        */
        fn arrivals(&self) -> Option<Vec<Arrival>> {
            let explicit = self.program.state_parameters(self.header).iter().count();
            let mut arrivals = Vec::new();
            for caller in self.program.machine_states(self.machine) {
                let caller_flow = Self::state_flow(self.facts, self.machine, caller)?;
                for call in crate::flow::call_flows(&self.facts.flow.control, caller_flow.calls) {
                    let Some(site) = validation::find_call_site(
                        self.program,
                        self.machine.symbol,
                        caller.symbol,
                        call.statement_index,
                        call.call_ordinal,
                    ) else {
                        continue;
                    };
                    let validation::CallSite::TransitionNamed {
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
                    let arguments = validation::call_site_argument_expressions(self.program, &site);
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
                                .flat_map(|fact| self.arm_facts(fact))
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

        /*
            A fact alive at the arm that the header roster can read: the state's
            own requirements and this iteration's guard values. Imported call
            contracts are not this machine's facts, and a prefix fact from an
            earlier iteration never survives the state's arrival contract.
        */
        fn arm_facts(&self, fact: &facts::Fact) -> Vec<(ExpressionHandle, bool)> {
            if matches!(
                fact.origin,
                FactOrigin::CallRequires | FactOrigin::CallEnsures
            ) {
                return Vec::new();
            }
            let (expression, holds) = match fact.payload {
                FactPayload::ContractBooleanExpression {
                    kind: ContractFactKind::Requires,
                    expression,
                    instantiated,
                    ..
                } if !instantiated.is_valid() => (expression, true),
                FactPayload::BooleanValue { expression, value } => (expression, value),
                _ => return Vec::new(),
            };
            self.hypothesis_parts(expression, holds)
        }

        /*
            A true builtin conjunction supplies each conjunct independently. The
            arithmetic fragment may retain a scalar conjunct even when a sibling
            describes a record or another unsupported term. This projects premises,
            never goals: a false conjunction supplies neither false conjunct, and
            an unsupported disjunction supplies neither alternative.
        */
        fn hypothesis_parts(
            &self,
            expression: ExpressionHandle,
            holds: bool,
        ) -> Vec<(ExpressionHandle, bool)> {
            let (expression, holds) = self.folded_polarity(expression, holds);
            if holds
                && let Some(binary) = self.program.expression_table.binary_node(expression)
                && binary.operator == BinaryOperator::And
                && has_builtin_operators(self.program, &self.facts.operators, expression)
            {
                let mut parts = self.hypothesis_parts(binary.left, true);
                parts.extend(self.hypothesis_parts(binary.right, true));
                return parts;
            }
            if self.comparison_is_admitted(expression, true) {
                vec![(expression, holds)]
            } else {
                Vec::new()
            }
        }

        /*
            A guard retained as `comparison == true` / `== false` is the
            comparison with the literal's polarity folded in.
        */
        fn folded_polarity(
            &self,
            expression: ExpressionHandle,
            holds: bool,
        ) -> (ExpressionHandle, bool) {
            let Some(binary) = self.program.expression_table.binary_node(expression) else {
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
                    binder: ArithmeticBinder::Symbol(parameter.symbol),
                    value: ScopedArithmeticValue::Atom {
                        identity: format!("\0{scope}:{:?}", parameter.symbol),
                        unsigned: parameter.unsigned,
                    },
                })
                .collect()
        }

        /* The invocation's values, immutable for the whole run. */
        fn formal_roster(&self) -> Vec<ScopedArithmeticBinding> {
            self.atoms_of(&self.formals, "invocation".to_string())
        }

        /*
            `bindings` plus an invocation-atom binding for every formal symbol the
            scope does not already bind: formals are immutable machine-level
            binders, visible inside every state.
        */
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

        /*
            Template placeholders for this state's current values. Keep these
            distinct even when exact origins later identify a parameter with an
            invocation value: only a proof-site instantiation applies that equality.
        */
        fn current_atoms(&self) -> Vec<ScopedArithmeticBinding> {
            self.atoms_of(&self.roster, format!("arrival:{:?}", self.header.symbol))
        }

        fn current_roster(&self) -> Vec<ScopedArithmeticBinding> {
            self.with_formals(self.current_atoms())
        }

        /*
            Current parameters start with fresh atoms. The flow owner's exact
            all-path origin solution can identify one immutable scalar with its
            invocation value independently of other parameters drifting. Missing,
            changed or ambiguous origins grant no equality, and a reference's
            identity can never turn its mutable contents into an entry snapshot.
        */
        fn caller_atoms(&self, caller: &State) -> Vec<ScopedArithmeticBinding> {
            let parameters = self
                .program
                .state_parameters(caller)
                .iter()
                .filter(|parameter| (!parameter.is_mutable) && !parameter.is_const)
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
            let origins = typed_trees::state::state_position(
                self.program.machine_states(self.machine),
                caller.symbol,
            )
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
                        .find(|entry| entry.binder == ArithmeticBinder::Symbol(*origin))
                    {
                        binding.value = invocation.value.clone();
                    }
                }
            }
            self.with_formals(bindings)
        }

        /*
            An arrival's simultaneous substitution: each header parameter denotes
            its argument read at the caller's scope.
        */
        fn arrival_roster(
            &self,
            arguments: &[ExpressionHandle],
            caller: Vec<ScopedArithmeticBinding>,
        ) -> Vec<ScopedArithmeticBinding> {
            self.program
                .state_parameters(self.header)
                .iter()
                .zip(arguments)
                .filter(|(parameter, _)| {
                    self.roster
                        .iter()
                        .any(|bound| bound.symbol == parameter.symbol)
                })
                .map(|(parameter, argument)| ScopedArithmeticBinding {
                    binder: ArithmeticBinder::Symbol(parameter.symbol),
                    value: ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                        owner: self.machine.symbol,
                        expression: *argument,
                        bindings: caller.clone(),
                    }),
                })
                .collect()
        }

        /*
            The candidate's guarantee over the formals, with `result` denoting the
            exit's returned term read under `returned_roster`.
        */
        fn transported(
            &self,
            candidate: &Candidate,
            returned_roster: Vec<ScopedArithmeticBinding>,
        ) -> ScopedArithmeticExpression {
            let mut bindings = self.formal_roster();
            bindings.push(ScopedArithmeticBinding {
                binder: ArithmeticBinder::Result,
                value: ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                    owner: self.machine.symbol,
                    expression: candidate.returned,
                    bindings: returned_roster,
                }),
            });
            ScopedArithmeticExpression {
                owner: self.machine.symbol,
                expression: candidate.guarantee,
                bindings,
            }
        }

        fn hypothesis(
            &self,
            expression: ExpressionHandle,
            bindings: Vec<ScopedArithmeticBinding>,
            holds: bool,
        ) -> ArithmeticHypothesis {
            ArithmeticHypothesis {
                proposition: ScopedArithmeticExpression {
                    owner: self.machine.symbol,
                    expression,
                    bindings,
                },
                holds,
            }
        }

        /*
            Conjunctions of builtin integer comparisons over admitted terms —
            the guarantees this check proposes and discharges. A residue-domain
            term may appear only under `==`: the engine argues in mathematical
            integers, an integer identity descends to Z/2^w, but an order or a
            disequality on a wrapped value does not.
        */
        fn proposition_is_admitted(&self, expression: ExpressionHandle) -> bool {
            self.comparison_is_admitted(expression, false)
        }

        /*
            A program-true fact the encoding may feed the engine. A residue
            binder compares faithfully only through its own representative: every
            side of a residue-domain comparison must be a bare binder or literal,
            since `x < y` on wrapped atoms reads the residues exactly while
            `x + y == z` would claim an integer equality stronger than the wrapped
            congruence the program established.
        */
        fn comparison_is_admitted(&self, expression: ExpressionHandle, leaf_only: bool) -> bool {
            if !self
                .program
                .expression_table
                .expression_is_valid(expression)
            {
                return false;
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Borrow(borrow) => {
                    self.comparison_is_admitted(borrow.target, leaf_only)
                }
                ExpressionNode::Binary(binary) => {
                    if binary.operator == BinaryOperator::And {
                        return has_builtin_operators(
                            self.program,
                            &self.facts.operators,
                            expression,
                        ) && self.comparison_is_admitted(binary.left, leaf_only)
                            && self.comparison_is_admitted(binary.right, leaf_only);
                    }
                    let Some(spelling) = validation::comparison_operator_spelling(binary.operator)
                    else {
                        return false;
                    };
                    let (Some(left), Some(right)) = (
                        self.term_reference(binary.left),
                        self.term_reference(binary.right),
                    ) else {
                        return false;
                    };
                    if !self.builtin_meaning(
                        expression,
                        spelling,
                        left.binder_type,
                        right.binder_type,
                    ) {
                        return false;
                    }
                    if left.domain == ArithmeticDomain::Exact
                        && right.domain == ArithmeticDomain::Exact
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

        /*
            A bare binder or literal: its value is its own residue
            representative, so a comparison on it reads the program fact exactly.
        */
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

        /*
            `Some` for a term the rosters can read: the retained binder type of a
            direct parameter or result (`None` for a literal or a compound whose
            builtin result carries no source type), and the arithmetic domain the
            term evaluates in — `Wrapping` when any leaf is residue arithmetic.
        */
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
                    if !self
                        .program
                        .expression_table
                        .name_path_is_single_member(path)
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
                    let spelling = crate::checks::arithmetic_spelling(binary.operator)?;
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

        /*
            No checked row selected a non-builtin operator, and no declared or
            trait meaning applies to these operand types.
        */
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

    /*
        One admitted term: its binder type (`None` for a literal or compound)
        and the domain its evaluation follows.
    */
    struct TermReference {
        binder_type: Option<TypeReferenceHandle>,
        domain: ArithmeticDomain,
    }

    /*
        A fixed-width integer in a domain the strict arithmetic reading can
        carry: `Exact` terms are mathematical integers, and a `Wrapping` term
        reads as its residue representative. `Saturating` and `Trapping` stay
        outside the language.
    */
    fn fixed_integer(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<(PrimitiveType, ArithmeticDomain)> {
        let primitive = program.primitive_type_reference(type_reference)?;
        if !primitive.is_fixed_integer() {
            return None;
        }
        let domain = program.arithmetic_domain_for_type_reference(type_reference);
        matches!(domain, ArithmeticDomain::Exact | ArithmeticDomain::Wrapping)
            .then_some((primitive, domain))
    }
}
mod result_domains {
    /*
        Declared domain obligations at a machine's normal return. A result
        consumes live membership with its exact domain instance or must independently
        establish the declared predicates and any authorized provenance. Its result
        annotation is never an input fact. An `ensures`
        domain fact authored over the reserved `result` name is discharged against
        the exact returned expression's live place; a nominal return type
        additionally makes every declared field qualification an obligation at each
        value-returning exit, and a readable reference return owes them on
        the returned place. The return is also a default-domain consumption point
        for every exclusive borrowed referent the machine received: declared root
        and field facts assumed on entry must hold again when
        the referent is handed back. All of these consume only live evidence: a
        write that retired a field's fact fails the proof, so a nominal
        annotation alone restores nothing.
    */
    mod proofs {
        /*
            proofs slice of result_domains.
        */
        use super::super::super::float_predicate_domains::expression_establishes_domain_predicate;
        use super::super::super::prover::evaluate_scalar;
        use super::super::super::prover::prove_domain_at_place;
        use super::super::super::prover::prove_indexed_domain_at_place;
        use super::super::super::prover::scalar_value_at_place;
        use super::super::super::return_values::exit_return_expression;
        use super::{domain_predicate_meaning, exact_integer_reference};
        use crate::flow::canonical_place_from_semantic_place;
        use crate::published::places::place_at;
        use checked_trees::{CheckFacts, FlowExitFact};
        use facts::{ContractFactKind, Fact, FactContextHandle, FactOrigin, FactPayload};
        use language_semantics::SemanticDomainId;
        use numerics::arithmetic::ArithmeticDomain;
        use numerics::bignum::BigInt;
        use symbols::SymbolHandle;
        use typed_trees::domain::DomainDefinition;
        use typed_trees::domain::domain_subject_reference;
        use typed_trees::domain::index_parameters;
        use typed_trees::domain::indexed_domain_instance_name;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::{DomainConstraint, TypeReferenceNode};
        use validation::ScopedArithmeticBinder;
        use validation::ScopedArithmeticBinding;
        use validation::ScopedArithmeticExpression;
        use validation::ScopedArithmeticHypothesis;
        use validation::ScopedArithmeticValue;
        use validation::StrictArithmeticImplicationJudgment;
        use validation::completed_value_proves_domain_predicate;
        use validation::declared_place_type_raw;
        use validation::expression_result_type_reference;
        use validation::find_state_for_exit;
        use validation::has_builtin_bound_expression_meaning;
        use validation::land_integer_value;
        use validation::reserved_result_owner;
        use validation::scoped_arithmetic_implication;
        use validation::validate_closed_const_argument;

        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(super) enum ScalarPredicateJudgment {
            Proven,
            Refuted,
            Unknown,
        }

        impl ScalarPredicateJudgment {
            fn join(self, next: Self) -> Self {
                match (self, next) {
                    (Self::Refuted, _) | (_, Self::Refuted) => Self::Refuted,
                    (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
                    (Self::Proven, Self::Proven) => Self::Proven,
                }
            }
        }

        pub(super) fn establishes_scalar_predicates(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            exit: &FlowExitFact,
            contexts: &[FactContextHandle],
            domain: &DomainDefinition,
            constraint: &DomainConstraint,
        ) -> ScalarPredicateJudgment {
            establishes_scalar_predicates_inner(
                program,
                facts,
                exit,
                contexts,
                domain,
                constraint,
                &mut Vec::new(),
            )
        }

        /*
            `domain A::B` desugars B's theory to include the same-subject membership
            `self in A`; an authored `self in Other` conjunct is the same obligation.
            Discharge it by establishing this returned value in that exact parent
            instance — the recursion reuses this entry's evidence and proof frame, so a
            different subject's membership stays an obligation and a cyclic authored
            theory fails closed at the visited roster.
        */
        fn establishes_scalar_predicates_inner(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            exit: &FlowExitFact,
            contexts: &[FactContextHandle],
            domain: &DomainDefinition,
            constraint: &DomainConstraint,
            visited: &mut Vec<(SymbolHandle, SemanticDomainId)>,
        ) -> ScalarPredicateJudgment {
            if domain.alias.is_some() || visited.contains(&(domain.symbol, domain.semantic_id)) {
                return ScalarPredicateJudgment::Unknown;
            }
            visited.push((domain.symbol, domain.semantic_id));
            let result = establishes_scalar_predicates_visited(
                program, facts, exit, contexts, domain, constraint, visited,
            );
            visited.pop();
            result
        }

        fn establishes_scalar_predicates_visited(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            exit: &FlowExitFact,
            contexts: &[FactContextHandle],
            domain: &DomainDefinition,
            constraint: &DomainConstraint,
            visited: &mut Vec<(SymbolHandle, SemanticDomainId)>,
        ) -> ScalarPredicateJudgment {
            let Some(indices) = closed_domain_indices(program, domain, constraint) else {
                return ScalarPredicateJudgment::Unknown;
            };
            let Some(machine) = program.machine(exit.machine_symbol) else {
                return ScalarPredicateJudgment::Unknown;
            };
            let Some(state) = find_state_for_exit(program, exit) else {
                return ScalarPredicateJudgment::Unknown;
            };
            let proof = ScalarReturnProof {
                program,
                facts,
                exit,
                contexts,
                machine,
                state,
            };
            let mut hypotheses = contexts
                .iter()
                .flat_map(|context| {
                    facts
                        .semantic
                        .context_view(facts.semantic.contexts.get(*context))
                        .facts()
                })
                .filter_map(|fact| {
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
                        FactPayload::BooleanExpression(expression) => (expression, true),
                        FactPayload::BooleanValue { expression, value } => (expression, value),
                        _ => return None,
                    };
                    Some(ScopedArithmeticHypothesis {
                        proposition: proof.current_expression(expression)?,
                        holds,
                    })
                })
                .collect::<Vec<_>>();
            for context in contexts {
                for fact in facts
                    .semantic
                    .context_view(facts.semantic.contexts.get(*context))
                    .facts()
                {
                    if matches!(
                        fact.origin,
                        FactOrigin::CallRequires | FactOrigin::CallEnsures
                    ) {
                        continue;
                    }
                    let Some((_, _, domain_symbol, semantic_domain)) =
                        fact.payload.domain_membership()
                    else {
                        continue;
                    };
                    let Some(candidate) = program.domain_definition(domain_symbol) else {
                        continue;
                    };
                    if !semantic_domain.is_valid()
                        || semantic_domain != candidate.semantic_id
                        || !index_parameters(program, candidate).is_empty()
                    {
                        continue;
                    }
                    let Some(place) = fact.place.as_place() else {
                        continue;
                    };
                    let Some(subject) = canonical_place_from_semantic_place(
                        program,
                        &facts.semantic,
                        facts.semantic.places.get(place),
                    ) else {
                        continue;
                    };
                    let Some(primitive) = program
                        .primitive_type_reference(candidate.target_type)
                        .filter(|primitive| primitive.accepts_integer_literal())
                    else {
                        continue;
                    };
                    for predicate in program.proof_facts.span_or_empty(candidate.facts) {
                        let Some(expression) = predicate.as_expression() else {
                            continue;
                        };
                        let mut subjects = Vec::new();
                        if domain_predicate_meaning(
                            program,
                            candidate,
                            *expression,
                            &mut subjects,
                            0,
                        )
                        .is_none()
                        {
                            continue;
                        }
                        hypotheses.push(ScopedArithmeticHypothesis {
                            proposition: ScopedArithmeticExpression {
                                owner: candidate.symbol,
                                expression: *expression,
                                bindings: vec![ScopedArithmeticBinding {
                                    binder: ScopedArithmeticBinder::Symbol(
                                        candidate.subject_symbol,
                                    ),
                                    value: ScopedArithmeticValue::Atom {
                                        identity: format!("return-place:{subject:?}"),
                                        unsigned: !primitive.is_signed_integer(),
                                    },
                                }],
                            },
                            holds: true,
                        });
                    }
                }
            }
            let mut judgment = ScalarPredicateJudgment::Proven;
            for fact in program.proof_facts.span_or_empty(domain.facts) {
                let next = if let Some(membership) = fact.as_membership() {
                    let parent = program.domain_definition_for_instance(
                        membership.domain_symbol,
                        membership.semantic_domain,
                    );
                    if let Some(parent) = parent
                        && domain_subject_reference(program, domain, membership.value)
                    {
                        let constraint = DomainConstraint {
                            symbol: membership.domain_symbol,
                            semantic_id: membership.semantic_domain,
                            arguments: program
                                .type_reference_table
                                .type_reference_handles(membership.domain_arguments)
                                .to_vec(),
                            ..Default::default()
                        };
                        establishes_scalar_predicates_inner(
                            program,
                            facts,
                            exit,
                            contexts,
                            parent,
                            &constraint,
                            visited,
                        )
                    } else {
                        ScalarPredicateJudgment::Unknown
                    }
                } else if let Some(expression) = fact.as_expression() {
                    let returned = exit_return_expression(program, exit);
                    if expression_establishes_domain_predicate(
                        program,
                        facts,
                        contexts,
                        proof.machine,
                        proof.state,
                        exit.statement_index,
                        returned,
                        domain,
                        *expression,
                    ) {
                        ScalarPredicateJudgment::Proven
                    } else {
                        let mut subjects = Vec::new();
                        let has_meaning = domain_predicate_meaning(
                            program,
                            domain,
                            *expression,
                            &mut subjects,
                            0,
                        )
                        .is_some();
                        if has_meaning {
                            let known = proof.current_value(returned);
                            let evaluated = evaluate_scalar(
                                program,
                                *expression,
                                &mut |leaf| {
                                    if subjects.contains(&leaf) {
                                        return known.clone();
                                    }
                                    let typed_trees::expression::ExpressionNode::Name(path) =
                                        program.expression_table.expression(leaf)
                                    else {
                                        return None;
                                    };
                                    indices
                                        .iter()
                                        .find(|index| index.symbol == path.symbol)
                                        .map(|index| index.value.clone())
                                },
                                &|_, _| true,
                            );
                            if evaluated
                                == Some(super::super::super::prover::ScalarValue::Boolean(true))
                            {
                                ScalarPredicateJudgment::Proven
                            } else if evaluated
                                == Some(super::super::super::prover::ScalarValue::Boolean(false))
                            {
                                ScalarPredicateJudgment::Refuted
                            } else if let Some(returned_term) = proof.current_expression(returned) {
                                let mut bindings = Vec::new();
                                let mut indices_ok = true;
                                for index in &indices {
                                    let super::super::super::prover::ScalarValue::Integer(value) =
                                        &index.value
                                    else {
                                        indices_ok = false;
                                        break;
                                    };
                                    bindings.push(ScopedArithmeticBinding {
                                        binder: ScopedArithmeticBinder::Symbol(index.symbol),
                                        value: ScopedArithmeticValue::Integer(value.clone()),
                                    });
                                }
                                if indices_ok {
                                    bindings.push(ScopedArithmeticBinding {
                                        binder: ScopedArithmeticBinder::Symbol(
                                            domain.subject_symbol,
                                        ),
                                        value: ScopedArithmeticValue::Term(returned_term),
                                    });
                                    let goal = ScopedArithmeticExpression {
                                        owner: domain.symbol,
                                        expression: *expression,
                                        bindings,
                                    };
                                    if scoped_arithmetic_implication(
                                        program,
                                        machine,
                                        &hypotheses,
                                        &goal,
                                    ) == StrictArithmeticImplicationJudgment::Proven
                                        || completed_value_proves_domain_predicate(
                                            program,
                                            machine,
                                            state,
                                            exit.statement_index,
                                            returned,
                                            domain.symbol,
                                            *expression,
                                            &[],
                                        )
                                    {
                                        ScalarPredicateJudgment::Proven
                                    } else {
                                        ScalarPredicateJudgment::Unknown
                                    }
                                } else {
                                    ScalarPredicateJudgment::Unknown
                                }
                            } else if completed_value_proves_domain_predicate(
                                program,
                                machine,
                                state,
                                exit.statement_index,
                                returned,
                                domain.symbol,
                                *expression,
                                &[],
                            ) {
                                ScalarPredicateJudgment::Proven
                            } else {
                                ScalarPredicateJudgment::Unknown
                            }
                        } else if completed_value_proves_domain_predicate(
                            program,
                            machine,
                            state,
                            exit.statement_index,
                            returned,
                            domain.symbol,
                            *expression,
                            &[],
                        ) {
                            ScalarPredicateJudgment::Proven
                        } else {
                            ScalarPredicateJudgment::Unknown
                        }
                    }
                } else {
                    ScalarPredicateJudgment::Unknown
                };
                judgment = judgment.join(next);
                if judgment == ScalarPredicateJudgment::Refuted {
                    break;
                }
            }
            judgment
        }

        pub(super) struct ScalarDomainIndex {
            symbol: SymbolHandle,
            value: super::super::super::prover::ScalarValue,
        }

        /*
            A predicate instance is reconstructed from the retained actual arguments,
            not decoded from an identity label. Ordinary const admission establishes
            each argument's declared carrier/range before its canonical scalar payload
            becomes a mathematical substitution. The complete tuple must reproduce the
            exact semantic instance the result type promises.
        */
        pub(super) fn closed_domain_indices(
            program: &dyn ::validation::CheckingInput,
            domain: &DomainDefinition,
            constraint: &DomainConstraint,
        ) -> Option<Vec<ScalarDomainIndex>> {
            use super::super::super::prover::ScalarValue;
            use language_semantics::const_value::{
                CanonicalConstValue, DecodedCanonicalConstValue,
            };
            let parameters = index_parameters(program, domain);
            if parameters.len() != constraint.arguments.len() || !constraint.semantic_id.is_valid()
            {
                return None;
            }
            let identity =
                indexed_domain_instance_name(program, domain, parameters, &constraint.arguments)
                    .ok()?;
            if program.semantic_domains.lookup(&identity) != Some(constraint.semantic_id) {
                return None;
            }
            let mut indices = Vec::with_capacity(parameters.len());
            for (parameter, argument) in parameters.iter().zip(&constraint.arguments) {
                validate_closed_const_argument(
                    program,
                    domain.name.as_str(),
                    parameter,
                    *argument,
                    parameters,
                    &constraint.arguments,
                )
                .ok()?;
                let TypeReferenceNode::Named { name, .. } =
                    program.type_reference_table.type_reference(*argument)
                else {
                    return None;
                };
                let value = if let Some(canonical) = CanonicalConstValue::from_atom(name.as_str()) {
                    match canonical.decode_encoding()? {
                        DecodedCanonicalConstValue::Integer { value, .. } => {
                            ScalarValue::Integer(BigInt::from_i128(value))
                        }
                        DecodedCanonicalConstValue::Boolean(value) => ScalarValue::Boolean(value),
                        _ => return None,
                    }
                } else {
                    ScalarValue::Integer(BigInt::from_i128(name.as_str().parse::<i128>().ok()?))
                };
                indices.push(ScalarDomainIndex {
                    symbol: parameter.symbol,
                    value,
                });
            }
            Some(indices)
        }

        pub(super) struct ScalarReturnProof<'a> {
            program: &'a dyn ::validation::CheckingInput,
            facts: &'a CheckFacts,
            exit: &'a FlowExitFact,
            contexts: &'a [FactContextHandle],
            machine: &'a Machine,
            state: &'a State,
        }

        impl ScalarReturnProof<'_> {
            /*
                An exact value conversion within one primitive carrier preserves its
                value. This establishes no target qualification: the predicate is
                proved again over the source in the same live context. Anonymous
                integer literals additionally undergo the ordinary exact landing check.
            */
            pub(super) fn value_preserving_source(
                &self,
                mut expression: ExpressionHandle,
            ) -> Option<ExpressionHandle> {
                for _ in 0..128 {
                    let Some(cast) = self.program.expression_table.cast_node(expression) else {
                        return Some(expression);
                    };
                    if cast.form.is_recast() || cast.domain != ArithmeticDomain::Exact {
                        return None;
                    }
                    let target = self.program.primitive_type_reference(cast.target_type)?;
                    let source = expression_result_type_reference(
                        self.program,
                        self.machine,
                        self.state,
                        cast.value,
                    )
                    .and_then(|reference| self.program.primitive_type_reference(reference));
                    if source != Some(target) {
                        let literal = self.program.expression_table.integer_node(cast.value)?;
                        if literal.landing().is_some()
                            || land_integer_value(&literal.value_bignum()?, target).is_none()
                        {
                            return None;
                        }
                    }
                    expression = cast.value;
                }
                None
            }

            /*
                Mathematical evaluation is permitted only for exact integer
                computations. Comparisons can still observe policy-bearing values;
                their operands are values, not permission to replace wrapping or
                saturating operations by unbounded arithmetic.
            */
            pub(super) fn exact_computations(
                &self,
                expression: ExpressionHandle,
                depth: usize,
            ) -> bool {
                use typed_trees::expression::{BinaryOperator, ExpressionNode, UnaryOperator};
                if self
                    .program
                    .expression_table
                    .expression_at_depth(expression, depth)
                    .is_none()
                {
                    return false;
                }
                let exact_operand = |operand| {
                    expression_result_type_reference(
                        self.program,
                        self.machine,
                        self.state,
                        operand,
                    )
                    .map(|reference| exact_integer_reference(self.program, reference))
                    .unwrap_or_else(|| {
                        matches!(self.program.expression_table.expression(operand),
                            ExpressionNode::Integer(literal) if literal.landing().is_none())
                    })
                };
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Binary(binary) => {
                        let observes_values = matches!(
                            binary.operator,
                            BinaryOperator::Equal
                                | BinaryOperator::NotEqual
                                | BinaryOperator::Less
                                | BinaryOperator::LessOrEqual
                                | BinaryOperator::Greater
                                | BinaryOperator::GreaterOrEqual
                                | BinaryOperator::And
                                | BinaryOperator::Or
                        );
                        (observes_values
                            || (exact_operand(binary.left) && exact_operand(binary.right)))
                            && self.exact_computations(binary.left, depth + 1)
                            && self.exact_computations(binary.right, depth + 1)
                    }
                    ExpressionNode::Unary(unary) => {
                        unary.operator == UnaryOperator::LogicalNot
                            && self.exact_computations(unary.operand, depth + 1)
                    }
                    ExpressionNode::Name(_)
                    | ExpressionNode::Integer(_)
                    | ExpressionNode::Boolean(_) => true,
                    _ => false,
                }
            }

            pub(super) fn current_expression(
                &self,
                expression: ExpressionHandle,
            ) -> Option<ScopedArithmeticExpression> {
                let expression = self.value_preserving_source(expression)?;
                if !self.exact_computations(expression, 0)
                    || !has_builtin_bound_expression_meaning(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    )
                {
                    return None;
                }
                let mut bindings = Vec::new();
                self.current_bindings(expression, &mut bindings, 0)?;
                Some(ScopedArithmeticExpression {
                    owner: self.state.symbol,
                    expression,
                    bindings,
                })
            }

            pub(super) fn current_bindings(
                &self,
                expression: ExpressionHandle,
                bindings: &mut Vec<ScopedArithmeticBinding>,
                depth: usize,
            ) -> Option<()> {
                use typed_trees::expression::ExpressionNode;
                if depth >= 128
                    || !self
                        .program
                        .expression_table
                        .expression_is_valid(expression)
                {
                    return None;
                }
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
                    ExpressionNode::Name(path)
                        if path.symbol.is_valid() && path.head_symbol == path.symbol =>
                    {
                        let reference = declared_place_type_raw(
                            self.program,
                            self.machine,
                            Some(self.state),
                            expression,
                        )?;
                        let primitive = self.program.primitive_type_reference(reference)?;
                        if !primitive.accepts_integer_literal() {
                            return None;
                        }
                        let subject = place_at(
                            self.program,
                            self.exit.state_symbol,
                            self.exit.statement_index,
                            expression,
                        )?;
                        let value = crate::values::literal_at_place(
                            self.program,
                            &self.facts.semantic,
                            self.contexts
                                .iter()
                                .map(|context| self.facts.semantic.contexts.get(*context)),
                            &subject,
                        )
                        .map(|literal| {
                            ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                                owner: self.state.symbol,
                                expression: literal,
                                bindings: Vec::new(),
                            })
                        })
                        .unwrap_or_else(|| ScopedArithmeticValue::Atom {
                            identity: format!("return-place:{subject:?}"),
                            unsigned: !primitive.is_signed_integer(),
                        });
                        bindings.push(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Symbol(path.symbol),
                            value,
                        });
                    }
                    ExpressionNode::Binary(binary) => {
                        self.current_bindings(binary.left, bindings, depth + 1)?;
                        self.current_bindings(binary.right, bindings, depth + 1)?;
                    }
                    ExpressionNode::Unary(unary) => {
                        self.current_bindings(unary.operand, bindings, depth + 1)?
                    }
                    _ => return None,
                }
                Some(())
            }

            pub(super) fn current_value(
                &self,
                expression: ExpressionHandle,
            ) -> Option<super::super::super::prover::ScalarValue> {
                let expression = self.value_preserving_source(expression)?;
                if !self.exact_computations(expression, 0)
                    || !has_builtin_bound_expression_meaning(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    )
                {
                    return None;
                }
                evaluate_scalar(
                    self.program,
                    expression,
                    &mut |leaf| {
                        let subject = place_at(
                            self.program,
                            self.exit.state_symbol,
                            self.exit.statement_index,
                            leaf,
                        )?;
                        scalar_value_at_place(
                            self.program,
                            &self.facts.semantic,
                            self.contexts
                                .iter()
                                .map(|context| self.facts.semantic.contexts.get(*context)),
                            &subject,
                        )
                    },
                    &|_, _| true,
                )
            }
        }

        /* Explain a refused returned membership after its ordinary check has failed. */
        pub(in crate::checks::contracts::exits) fn completed_result_domain_diagnostic(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            exit: &FlowExitFact,
            requirement: &Fact,
            refusal: Option<validation::completed_storage::CompletedMembershipRefusal>,
        ) -> Option<String> {
            requirement.payload.domain_membership()?;
            let place = requirement.place.as_place()?;
            if !facts.semantic.places.is_valid(place) {
                return None;
            }
            let place = facts.semantic.places.get(place);
            let root = place.root.as_expression()?;
            if !program.expression_table.expression_is_valid(root)
                || reserved_result_owner(program, root)
                    .is_none_or(|(owner, _)| owner != exit.machine_symbol)
            {
                return None;
            }
            Some(
                refusal
                    .map_or(
                        "the returned value does not establish this domain",
                        |refusal| refusal.reason,
                    )
                    .to_owned(),
            )
        }

        /*
            Rebase an ensures fact rooted at the reserved `result` occurrence onto the
            returned expression's canonical place, keeping the fact's own declared
            field coordinates. `result.bytes` on a `rows[0]` return is exactly
            `rows[0].bytes` at this statement -- the same place mutation invalidation
            retires, so corrupted evidence cannot satisfy it.
        */
        pub(in crate::checks::contracts::exits) fn proves_result_domain(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            exit: &FlowExitFact,
            contexts: &[FactContextHandle],
            requirement: &Fact,
            refusal: &mut Option<validation::completed_storage::CompletedMembershipRefusal>,
        ) -> bool {
            let Some((_, _, domain_symbol, semantic_domain)) =
                requirement.payload.domain_membership()
            else {
                return false;
            };
            let Some(place) = requirement.place.as_place() else {
                return false;
            };
            let place = facts.semantic.places.get(place);
            let Some(root) = place.root.as_expression() else {
                return false;
            };
            if reserved_result_owner(program, root)
                .is_none_or(|(owner, _)| owner != exit.machine_symbol)
            {
                return false;
            }
            if validation::completed_storage::completed_membership_with_refusal(
                program,
                facts,
                exit,
                facts.semantic.place_segments.span_or_empty(place.segments),
                domain_symbol,
                semantic_domain,
                refusal,
            ) {
                return true;
            }
            let returned = exit_return_expression(program, exit);
            if !returned.is_valid() {
                return false;
            }
            let Some(mut subject) =
                place_at(program, exit.state_symbol, exit.statement_index, returned)
            else {
                return false;
            };
            if !crate::flow::place_cases_are_selected(
                program,
                &facts.semantic,
                contexts,
                exit.machine_symbol,
                exit.state_symbol,
                exit.statement_index,
                &subject,
            ) {
                return false;
            }
            subject.extend_segments(facts.semantic.place_segments.span_or_empty(place.segments));
            if program
                .domain_definition(domain_symbol)
                .is_some_and(|domain| !index_parameters(program, domain).is_empty())
            {
                return prove_indexed_domain_at_place(
                    program,
                    &facts.semantic,
                    contexts,
                    &subject,
                    domain_symbol,
                    semantic_domain,
                );
            }
            prove_domain_at_place(program, &facts.semantic, contexts, &subject, domain_symbol)
        }
    }
    mod scalar_exits {
        /*
            scalar exits slice of result_domains.
        */
        use validation::CanonicalPlace;

        use super::super::super::return_values::{exit_return_call, exit_return_expression};
        use super::proofs::*;
        use crate::flow::{active_context_handles, canonical_place_from_semantic_place};
        use crate::published::places::place_at;
        use checked_trees::{CheckFacts, FlowExitFact};
        use diagnostics::Diagnostic;
        use facts::FactContextHandle;
        use typed_trees::types::TypeReferenceNode;

        /*
            Shared by the exit census and its consumer: a scalar domain annotation
            needs a live return context even when there is no authored `ensures`.
        */
        pub(crate) fn scalar_result_domains(
            program: &dyn ::validation::CheckingInput,
            return_type: typed_trees::types::TypeReferenceHandle,
        ) -> Vec<&typed_trees::types::DomainConstraint> {
            let carrier = program
                .type_reference_table
                .unconstrained_type_reference(return_type);
            if matches!(
                program.type_reference_table.type_reference(carrier),
                TypeReferenceNode::Reference { .. }
            ) || program.primitive_type_reference(carrier).is_none()
            {
                return Vec::new();
            }
            let mut domains = Vec::new();
            let mut reference = return_type;
            while let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program.type_reference_table.type_reference(reference)
            {
                domains.extend(
                    program
                        .type_reference_table
                        .constraints(*constraints)
                        .iter()
                        .filter_map(|constraint| match constraint {
                            typed_trees::types::TypeConstraintNode::Domain(domain)
                                if domain.symbol.is_valid() =>
                            {
                                Some(domain)
                            }
                            _ => None,
                        }),
                );
                reference = *base_type;
            }
            domains
        }

        /*
            A scalar signature promises membership to callers, but supplies no evidence
            to its own body. Consume the returned subject's live exit facts before
            attempting fresh establishment. In particular, a declaration symbol alone
            cannot identify an indexed domain application, and predicate truth alone
            cannot establish a routed qualification.
        */
        pub(in crate::checks::contracts) fn check_scalar_result_domains(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            _state_flow: &checked_trees::FlowStateFact,
            exit: &FlowExitFact,
            decisions: &mut Vec<checked_trees::CheckedScalarReturnQualification>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let Some(machine) = program.machine(exit.machine_symbol) else {
                return;
            };
            let domains = scalar_exit_domains(program, machine, exit.state_symbol);
            if domains.is_empty() {
                return;
            }
            let returned = exit_return_expression(program, exit);
            let contexts =
                active_context_handles(&facts.flow.contexts, exit.entry_semantic_contexts);
            let subject = returned
                .is_valid()
                .then(|| place_at(program, exit.state_symbol, exit.statement_index, returned))
                .flatten();
            if subject.as_ref().is_some_and(|subject| {
                !crate::flow::place_cases_are_selected(
                    program,
                    &facts.semantic,
                    &contexts,
                    exit.machine_symbol,
                    exit.state_symbol,
                    exit.statement_index,
                    subject,
                )
            }) {
                diagnostics.push(Diagnostic::error(
                    "cannot prove selected case for qualified scalar return",
                ));
                return;
            }
            for constraint in domains {
                let domain_symbol = constraint.symbol;
                let semantic_domain = constraint.semantic_id;
                if crate::published::facts::domain_is_vacuous(
                    program,
                    domain_symbol,
                    &mut Vec::new(),
                ) {
                    continue;
                }
                let returned_value = returned.is_valid().then(|| CanonicalPlace {
                    root: facts::PlaceRoot::Expression(returned),
                    segments: Vec::new(),
                });
                let transported = subject.iter().chain(returned_value.iter()).any(|subject| {
                    exact_scalar_membership(
                        program,
                        facts.view(),
                        &contexts,
                        subject,
                        domain_symbol,
                        semantic_domain,
                    )
                });
                let supplied_by_return =
                    exit_return_call(program, facts, exit).is_some_and(|call| {
                        call_result_has_domain(program, call.target_symbol, constraint)
                    });
                let origin = if supplied_by_return || (returned.is_valid() && transported) {
                    Some(checked_trees::CheckedScalarReturnOrigin::Forwarded)
                } else {
                    program.domain_definition(domain_symbol).and_then(|domain| {
                        if !returned.is_valid()
                            || (!typed_trees::domain::index_parameters(program, domain).is_empty()
                                && !domain.establishment_routes.is_empty())
                        {
                            return None;
                        }
                        let origin =
                            if domain.establishment_routes.is_empty() {
                                checked_trees::CheckedScalarReturnOrigin::Predicate
                            } else {
                                checked_trees::CheckedScalarReturnOrigin::Established(
                                crate::published::facts::qualification_evidence::machine_domain_establishment_route(
                                    program, machine, domain_symbol,
                                )?,
                            )
                            };
                        if let Some(validation::StrictArithmeticImplicationJudgment::Proven) =
                            literal_result_domain_judgment(
                                program,
                                facts,
                                exit,
                                returned,
                                domain_symbol,
                                semantic_domain,
                                &constraint.arguments,
                            )
                        {
                            return Some(origin);
                        }
                        matches!(
                            establishes_scalar_predicates(
                                program, facts, exit, &contexts, domain, constraint,
                            ),
                            ScalarPredicateJudgment::Proven,
                        )
                        .then_some(origin)
                    })
                };
                if let Some(origin) = origin {
                    decisions.push(checked_trees::CheckedScalarReturnQualification {
                        machine: exit.machine_symbol,
                        state: exit.state_symbol,
                        statement_index: exit.statement_index,
                        transition_target: exit.transition_target,
                        is_continuation: exit.is_continuation,
                        expression: returned,
                        semantic_domain,
                        domain: domain_symbol,
                        origin,
                    });
                } else if (program
                    .domain_definition(domain_symbol)
                    .is_some_and(|domain| {
                        matches!(
                            establishes_scalar_predicates(
                                program, facts, exit, &contexts, domain, constraint,
                            ),
                            ScalarPredicateJudgment::Refuted,
                        )
                    })
                    || literal_result_domain_judgment(
                        program,
                        facts,
                        exit,
                        returned,
                        domain_symbol,
                        semantic_domain,
                        &constraint.arguments,
                    ) == Some(validation::StrictArithmeticImplicationJudgment::Refuted))
                    && crate::published::labels::machine_name(program, exit.machine_symbol)
                        .starts_with("@const-qualification-")
                {
                    let full = crate::published::labels::symbol_name(program, domain_symbol);
                    let short = full
                        .split_once("::")
                        .map(|(_, rest)| rest.to_owned())
                        .unwrap_or(full.clone());
                    diagnostics.push(Diagnostic::error(format!(
                        "domain constraint `{short}` for the const leaf value is false; failed domain `{full}`",
                    )));
                } else {
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot prove scalar result domain for return from {} at statement {}: {}",
                        crate::published::labels::machine_name(program, exit.machine_symbol),
                        exit.statement_index,
                        crate::published::labels::symbol_name(program, domain_symbol),
                    )));
                }
            }
        }

        /*
            Both the machine's external result and the current state's result are
            obligations. Identical instances need only one establishment judgment.
        */
        pub(super) fn scalar_exit_domains<'program>(
            program: &'program dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state_symbol: symbols::SymbolHandle,
        ) -> Vec<&'program typed_trees::types::DomainConstraint> {
            let mut domains = Vec::new();
            for state in program.machine_states(machine).iter().filter(|state| {
                state.symbol == state_symbol
                    || program
                        .machine_states(machine)
                        .first()
                        .is_some_and(|entry| entry.symbol == state.symbol)
            }) {
                for constraint in scalar_result_domains(program, state.return_type) {
                    if !domains
                        .iter()
                        .any(|existing: &&typed_trees::types::DomainConstraint| {
                            existing.symbol == constraint.symbol
                                && existing.semantic_id == constraint.semantic_id
                        })
                    {
                        domains.push(constraint);
                    }
                }
            }
            domains
        }

        /*
            A local state transfer keeps the machine invocation alive, but cannot erase
            a stronger result qualification promised by its source state's signature.
            Consume its retained call occurrence and checked result contract; it has
            no authored value expression to manufacture for the ordinary exit prover.
            Transfers within this machine preserve additional state promises through
            the selected target contract. The common machine promise is checked at leaf
            exits, so intermediate states need not repeat it in their annotations.
        */
        pub(in crate::checks::contracts) fn check_state_transfer_result_domains(
            program: &dyn ::validation::CheckingInput,
            state_flow: &checked_trees::FlowStateFact,
            call: &checked_trees::FlowCallFact,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if call.authored_expression.is_valid() {
                return;
            }
            let Some(machine) = program.machine(state_flow.machine_symbol) else {
                return;
            };
            let Some(state) = program.state_in_machine(machine, state_flow.state_symbol) else {
                return;
            };
            if !matches!(program.statement_table.statement_at(state, call.statement_index),
                Some(typed_trees::statement::StatementNode::Transition(transition))
                if transition.exit == typed_trees::statement::TransitionExit::Ordinary)
            {
                return;
            }
            let Some(validation::CallSite::TransitionNamed { path, .. }) =
                validation::find_call_site(
                    program,
                    state_flow.machine_symbol,
                    state_flow.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                )
            else {
                return;
            };
            let selected = crate::published::proof::contract_target_from_state_symbol(
                program,
                call.target_symbol,
            );
            let exact_invocation = selected.is_some()
                && crate::published::proof::contract_target_from_state_symbol(program, path.symbol)
                    == selected;
            let local_transfer =
                exact_invocation && selected.is_some_and(|(owner, _)| owner == machine.symbol);
            if !local_transfer {
                return;
            }
            let common_domains = program
                .machine_states(machine)
                .first()
                .map(|entry| scalar_result_domains(program, entry.return_type))
                .unwrap_or_default();
            for required in scalar_exit_domains(program, machine, state.symbol) {
                if common_domains.iter().any(|common| {
                    common.symbol == required.symbol && common.semantic_id == required.semantic_id
                }) {
                    continue;
                }
                if crate::published::facts::domain_is_vacuous(
                    program,
                    required.symbol,
                    &mut Vec::new(),
                ) {
                    continue;
                }
                if exact_invocation && call_result_has_domain(program, call.target_symbol, required)
                {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove scalar result domain for tail return from {} at statement {}: {}",
                    crate::published::labels::machine_name(program, machine.symbol),
                    call.statement_index,
                    crate::published::labels::symbol_name(program, required.symbol),
                )));
            }
        }

        pub(super) fn call_result_has_domain(
            program: &dyn ::validation::CheckingInput,
            target: symbols::SymbolHandle,
            required: &typed_trees::types::DomainConstraint,
        ) -> bool {
            let target =
                crate::published::proof::contract_target_from_state_symbol(program, target)
                    .map_or(target, |(_, state)| state);
            validation::call_target_return_type(program, target).is_some_and(|return_type| {
                required.semantic_id.is_valid()
                    && scalar_result_domains(program, return_type)
                        .iter()
                        .any(|provided| {
                            provided.symbol == required.symbol
                                && provided.semantic_id == required.semantic_id
                        })
            })
        }

        /*
            An integer literal returned into a scalar domain. Inherited membership
            reuses this same integer, so `3` in `Nested::Inherited` is `3 < 5` and
            `9` is not.
        */
        fn literal_result_domain_judgment(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            exit: &FlowExitFact,
            returned: typed_trees::expression::ExpressionHandle,
            domain_symbol: symbols::SymbolHandle,
            semantic_domain: language_semantics::SemanticDomainId,
            arguments: &[typed_trees::types::TypeReferenceHandle],
        ) -> Option<validation::StrictArithmeticImplicationJudgment> {
            let expression = if returned.is_valid() {
                returned
            } else if facts.flow.control.operands.is_valid(exit.returned_value) {
                facts
                    .flow
                    .control
                    .operands
                    .get(exit.returned_value)
                    .expression
            } else {
                return None;
            };
            let integer = integer_result_leaf(program, expression)?;
            Some(validation::integer_domain_membership(
                program,
                domain_symbol,
                arguments,
                semantic_domain,
                &validation::ScopedArithmeticValue::Integer(integer),
            ))
        }

        /*
            The integer a qualified return actually denotes. A destination cast keeps
            the same number; the domain judgment still has to prove that number.
        */
        fn integer_result_leaf(
            program: &dyn ::validation::CheckingInput,
            mut expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<numerics::bignum::BigInt> {
            for _ in 0..8 {
                if let Some(literal) = program.expression_table.integer_node(expression) {
                    return literal.decimal_value_bignum();
                }
                let cast = program.expression_table.cast_node(expression)?;
                if cast.form.is_recast()
                    || cast.domain != numerics::arithmetic::ArithmeticDomain::Exact
                {
                    return None;
                }
                expression = cast.value;
            }
            None
        }

        pub(crate) fn exact_scalar_membership(
            program: &dyn ::validation::CheckingInput,
            facts: checked_trees::CheckFactsRef<'_>,
            contexts: &[FactContextHandle],
            subject: &CanonicalPlace,
            domain_symbol: symbols::SymbolHandle,
            semantic_domain: language_semantics::SemanticDomainId,
        ) -> bool {
            semantic_domain.is_valid()
                && contexts.iter().any(|context| {
                    facts
                        .semantic
                        .context_view(facts.semantic.contexts.get(*context))
                        .facts()
                        .any(|fact| {
                            let Some((_, _, candidate, instance)) =
                                fact.payload.domain_membership()
                            else {
                                return false;
                            };
                            let Some(place) = fact.place.as_place() else {
                                return false;
                            };
                            candidate == domain_symbol
                                && instance == semantic_domain
                                && canonical_place_from_semantic_place(
                                    program,
                                    facts.semantic,
                                    facts.semantic.places.get(place),
                                )
                                .is_some_and(|candidate| {
                                    candidate.root == subject.root
                                        && crate::checks::contracts::prover::segments_cover_subject(
                                            program,
                                            &candidate.segments,
                                            &subject.segments,
                                        )
                                })
                        })
                })
        }
    }
    pub(in crate::checks::contracts) use field_domains::{
        check_exclusive_referent_field_domains, check_result_field_domains,
    };
    pub(crate) use field_domains::{
        exclusive_referent_type, is_readable_mutable_reference, value_provable_domain,
    };
    pub(in crate::checks::contracts::exits) use proofs::{
        completed_result_domain_diagnostic, proves_result_domain,
    };
    pub(crate) use scalar_exits::exact_scalar_membership;
    pub(in crate::checks::contracts) use scalar_exits::{
        check_scalar_result_domains, check_state_transfer_result_domains,
    };
    mod field_domains {
        /*
            Returned storage owes both root qualifications and declared field domains.

            An executable predicate-only structural root is no less a promise than a
            scalar or a routed root. Checking only its fields would let a byte-field copy
            claim a stronger encoding on return. Value establishment and return admission
            share the occurrence-bound qualification judgment; annotations supply obligations,
            not membership. Field requirements still prove at their exact live paths.
            A borrowed view's root predicate names its completed descriptor and exact
            extent: UTF-8 on a prefix does not certify the complete backing array.
            Root qualification therefore uses the existing occurrence-bound value
            judgment even when origin reconstruction also identifies live referents.
            Projected field requirements continue to use those current referents.
            Proof-only predicate roots remain outside this storage judgment, as before;
            mathematical constructor entailment is not occurrence-bound storage evidence.
        */
        use validation::CanonicalPlace;

        use super::super::super::return_values::exit_return_expression;
        use super::scalar_exits::*;
        use crate::flow::{active_context_handles, canonical_place_from_semantic_place};
        use crate::published::places::place_at;
        use checked_trees::{CheckFacts, FlowExitFact, FlowStateFact};
        use diagnostics::Diagnostic;
        use facts::{FactOrigin, FactPayload, FactPlace, PlaceRoot, PlaceSegment, ProgramPoint};
        use typed_trees::types::TypeReferenceNode;

        /*
            Every declared field predicate of an owned nominal (or fixed-array nominal
            element) return type is an obligation on the returned value at each
            value-returning exit -- the return-position dual of the declared-field
            requirements a call's nominal input imposes on its actuals
            (checks/contracts/nominal_inputs.rs). A readable reference return owns no
            result storage but hands its caller the referent, so the same predicates
            are obligations on the returned place: the caller reads the referent's
            declared fields through the reference, and a source write that retired
            them must have been repaired before the return.
        */
        pub(in crate::checks::contracts) fn check_result_field_domains(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            state_flow: &FlowStateFact,
            exit: &FlowExitFact,
            call_frames: Option<&validation::CallFrameResolver<'_>>,
            value_qualifications: &arena::Arena<checked_trees::CheckedValueQualification>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let Some(machine) = program.machine(exit.machine_symbol) else {
                return;
            };
            let Some(entry) = program.machine_states(machine).first() else {
                return;
            };
            let mut paths = validation::field_domains::declared_result_field_domain_identities(
                program,
                result_domain_type(program, entry.return_type),
            );
            if program
                .primitive_type_reference(entry.return_type)
                .is_none()
            {
                let proof_only = validation::proof_only_classification(program);
                paths.extend(
                    validation::field_domains::domain_constraint_identities(
                        program,
                        entry.return_type,
                    )
                    .into_iter()
                    .filter(|(domain, _)| {
                        validation::field_domains::domain_requires_provenance(program, *domain)
                            || !proof_only.is_proof_machine(program, machine)
                    })
                    .map(|(domain, identity)| (Vec::new(), domain, identity)),
                );
            }
            if paths.is_empty() {
                return;
            }
            let mut result_memberships = crate::flow::CapturedResultMemberships::new(facts);
            let returned = exit_return_expression(program, exit);
            if !returned.is_valid() {
                return;
            }
            let entry_contexts =
                active_context_handles(&facts.flow.contexts, exit.entry_semantic_contexts);
            let base = place_at(program, exit.state_symbol, exit.statement_index, returned);
            let readable_reference = validation::reference_shell(program, entry.return_type)
                .is_some_and(|shell| shell.access.is_readable());
            let returned_storage = readable_reference
                .then(|| {
                    if !facts.flow.control.operands.is_valid(exit.returned_value) {
                        return None;
                    }
                    let operand = facts.flow.control.operands.get(exit.returned_value);
                    if operand.expression != returned
                        || operand.state_symbol != exit.state_symbol
                        || operand.statement_index != exit.statement_index
                    {
                        return None;
                    }
                    let referents = facts
                        .flow
                        .control
                        .operand_referents
                        .span(operand.referents)?;
                    if referents.is_empty() {
                        return None;
                    }
                    referents
                        .iter()
                        .map(|referent| {
                            crate::checks::places::place_in(
                                &facts.flow.control.operand_referent_segments,
                                referent.root,
                                referent.segments,
                            )
                        })
                        .collect::<Option<Vec<_>>>()
                })
                .flatten();
            let cases = std::cell::OnceCell::new();
            for (path, domain_symbol, semantic_domain) in paths {
                if path
                    .iter()
                    .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
                    && cases
                        .get_or_init(|| {
                            super::super::cases::CaseObservation::for_exit(
                                program,
                                facts,
                                exit,
                                &entry_contexts,
                                call_frames,
                            )
                        })
                        .as_ref()
                        .is_some_and(|cases| cases.result_path_is_inactive(&path))
                {
                    continue;
                }
                let proves_in = |contexts: &[facts::FactContextHandle],
                                 subject: &CanonicalPlace| {
                    if exact_scalar_membership(
                        program,
                        facts.view(),
                        contexts,
                        subject,
                        domain_symbol,
                        semantic_domain,
                    ) {
                        true
                    } else if !validation::field_domains::domain_requires_provenance(
                        program,
                        domain_symbol,
                    ) {
                        super::super::super::prover::prove_indexed_domain_at_place(
                            program,
                            &facts.semantic,
                            contexts,
                            subject,
                            domain_symbol,
                            semantic_domain,
                        ) || super::super::super::prover::prove_domain_at_place(
                            program,
                            &facts.semantic,
                            contexts,
                            subject,
                            domain_symbol,
                        )
                    } else {
                        false
                    }
                };
                let proves = |subject: &CanonicalPlace| proves_in(&entry_contexts, subject);
                let established_root = path.is_empty()
                    && program.domain_definition(domain_symbol).is_some_and(|_| {
                        validation::field_domains::domain_constraint_identities(program,entry.return_type)
                            .contains(&(domain_symbol,semantic_domain))
                            && crate::published::facts::qualification_evidence::machine_has_checked_domain_establishment(
                                program, machine, domain_symbol,
                            )
                            && super::super::super::writes::value_satisfies_domain_predicates_in_contexts(
                                program, facts, state_flow, exit.statement_index, returned,
                                domain_symbol, &entry_contexts, &mut Vec::new(),
                                facts.flow.control.operands.is_valid(exit.returned_value)
                                    .then(|| facts.flow.control.operands.get(exit.returned_value)),
                                call_frames,
                            )
                    });
                let qualified_root = path.is_empty()
                    && value_qualifications.iter().any(|(_, decision)| {
                        decision.machine == state_flow.machine_symbol
                            && decision.state == state_flow.state_symbol
                            && decision.statement_index as usize == exit.statement_index
                            && decision.expression == returned
                            && !decision.structural_value.is_valid()
                            && decision.domain_symbol == domain_symbol
                            && decision.semantic_domain == semantic_domain
                    });
                let root_satisfied = qualified_root
                    || established_root
                    || (path.is_empty()
                        && super::super::super::writes::value_proves_qualification_in_contexts(
                            program,
                            facts,
                            state_flow,
                            exit.statement_index,
                            returned,
                            domain_symbol,
                            semantic_domain,
                            &entry_contexts,
                            call_frames,
                        ));
                if root_satisfied {
                    continue;
                }
                let result_membership = crate::flow::CapturedOperand::recorded(
                    &facts.flow.control,
                    exit.returned_value,
                )
                .and_then(|capture| {
                    crate::flow::captured_operand_values(
                        program,
                        facts.flow.view(),
                        &facts.semantic,
                        state_flow,
                        capture,
                        entry.return_type,
                        &path,
                    )
                })
                .is_some_and(|values| {
                    !values.is_empty()
                        && values.iter().all(|value| {
                            facts.flow.control.operands.is_valid(exit.returned_value)
                                && result_memberships.contains(
                                    program,
                                    value,
                                    facts.flow.control.operands.get(exit.returned_value),
                                    domain_symbol,
                                    semantic_domain,
                                )
                        })
                });
                if result_membership {
                    continue;
                }
                let satisfied = if let Some(subjects) = returned_storage.as_ref() {
                    {
                        subjects.iter().all(|storage| {
                            crate::flow::place_cases_are_selected(
                                program,
                                &facts.semantic,
                                &entry_contexts,
                                exit.machine_symbol,
                                exit.state_symbol,
                                exit.statement_index,
                                storage,
                            ) && crate::flow::project_captured_view_place(storage.clone(), &path)
                                .is_some_and(|subject| {
                                    crate::flow::place_case_path_is_inactive(
                                        program,
                                        &facts.semantic,
                                        &entry_contexts,
                                        exit.machine_symbol,
                                        exit.state_symbol,
                                        exit.statement_index,
                                        &subject,
                                    ) || proves(&subject)
                                })
                        })
                    }
                } else {
                    let captured_satisfies = |selected: &[facts::PlaceSegment]| {
                        let Some(capture) = crate::flow::CapturedOperand::recorded(
                            &facts.flow.control,
                            exit.returned_value,
                        ) else {
                            return false;
                        };
                        crate::flow::captured_operand_values(
                            program,
                            facts.flow.view(),
                            &facts.semantic,
                            state_flow,
                            capture,
                            entry.return_type,
                            selected,
                        )
                        .is_some_and(|values| {
                            !values.is_empty()
                                && values.iter().all(|captured| {
                                    let (source_state, source_statement) = captured
                                        .producer_frontier(exit.state_symbol, exit.statement_index);
                                    let contexts = crate::flow::operand_contexts(
                                        program,
                                        &facts.flow.contexts,
                                        &facts.flow.control,
                                        &facts.semantic,
                                        captured.operand.row(),
                                        captured.type_reference,
                                        captured.producer_invocation(exit.entry_constraints),
                                    );
                                    crate::flow::captured_value_places(
                                        program,
                                        facts.flow.view(),
                                        captured,
                                    )
                                    .is_some_and(|places| {
                                        !places.is_empty()
                                            && places.iter().all(|place| {
                                                crate::flow::place_cases_are_selected(
                                                    program,
                                                    &facts.semantic,
                                                    &contexts,
                                                    exit.machine_symbol,
                                                    source_state,
                                                    source_statement,
                                                    &place.storage,
                                                ) && (crate::flow::place_case_path_is_inactive(
                                                    program,
                                                    &facts.semantic,
                                                    &contexts,
                                                    exit.machine_symbol,
                                                    source_state,
                                                    source_statement,
                                                    &place.projected,
                                                ) || proves_in(&contexts, &place.projected))
                                            })
                                    })
                                })
                        })
                    };
                    let completed_satisfies = || {
                        !validation::field_domains::domain_requires_provenance(program, domain_symbol)
                            && program
                                .domain_definition_for_instance(domain_symbol, semantic_domain)
                                .is_some_and(|domain| {
                                    !program.proof_facts(domain).is_empty()
                                        && program.proof_facts(domain).iter().all(|fact| {
                                            fact.as_expression().is_some_and(|predicate| {
                                                program
                                                    .state_in_machine(machine, exit.state_symbol)
                                                    .is_some_and(|state| {
                                                        validation::completed_value_proves_bound_domain_predicate(
                                                            program,
                                                            machine,
                                                            state,
                                                            exit.statement_index,
                                                            returned,
                                                            domain,
                                                            *predicate,
                                                            &path,
                                                        )
                                                    })
                                            })
                                        })
                                })
                    };
                    crate::checks::content::completed_membership(
                        program,
                        facts,
                        exit.returned_value,
                        &path,
                        domain_symbol,
                        semantic_domain,
                    ) || captured_satisfies(&path)
                        || completed_satisfies()
                };
                if !satisfied {
                    let (root, mut segments) = base
                        .as_ref()
                        .map(|base| (base.root, base.segments.clone()))
                        .unwrap_or((PlaceRoot::Expression(returned), Vec::new()));
                    segments.extend_from_slice(&path);
                    // Same wording as the declared-place refusal below: a machine
                    // exits from its single entry state, so naming that state
                    // (usually the generated `entry`) adds nothing the reader wrote.
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot prove default-domain field requirement for return from {} at statement {}: {} requires {}",
                        crate::published::labels::machine_name(program, exit.machine_symbol),
                        exit.statement_index,
                        facts::canonical_place_label_from_parts(program, root, &segments),
                        crate::published::labels::symbol_name(program, domain_symbol),
                    )));
                }
            }
        }

        /*
            The type whose declared fields a return owes: the owned result type
            itself, or the referent behind a readable reference return. A write-only
            reference exposes no readable storage and owes nothing.
        */
        fn result_domain_type(
            program: &dyn ::validation::CheckingInput,
            return_type: typed_trees::types::TypeReferenceHandle,
        ) -> typed_trees::types::TypeReferenceHandle {
            let mut reference = return_type;
            while reference.is_valid() {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    TypeReferenceNode::Reference {
                        referee, access, ..
                    } if access.is_readable() => return *referee,
                    _ => return reference,
                }
            }
            return_type
        }

        /*
            Whether a declared field domain is re-provable from the field's value.
            An establishment-gated domain (`established by ..`) mints membership only
            through its listed routes, so its rows are custody evidence transported
            by the carry machinery rather than an invariant window a return can
            close from predicates alone. Routed return obligations instead consume
            exact live membership; predicate-only call reseeding must exclude them.
        */
        pub(crate) fn value_provable_domain(
            program: &dyn ::validation::CheckingInput,
            domain_symbol: symbols::SymbolHandle,
        ) -> bool {
            !validation::field_domains::domain_requires_provenance(program, domain_symbol)
        }

        /*
            Whether a declared reference supplies readable mutable alias authority.
            This is narrower than the normal-return validity obligation: write-only
            references owe validity too, but cannot supply a readable alias.
        */
        pub(crate) fn is_readable_mutable_reference(
            program: &dyn ::validation::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> bool {
            let mut reference = type_reference;
            while reference.is_valid() {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    TypeReferenceNode::Reference { access, .. } => {
                        return *access == language_semantics::ReferenceAccess::Mutable;
                    }
                    _ => return false,
                }
            }
            false
        }

        /*
            The referent whose validity an exclusive borrow owes on normal return.
            Write-only authority changes which operations may establish that validity,
            not the requirement to return a valid value. Declared field/default domains
            remain invariant premises; additional input qualifications are preconditions.
            This query grants no observation or readable alias.
        */
        pub(crate) fn exclusive_referent_type(
            program: &dyn ::validation::CheckingInput,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> typed_trees::types::TypeReferenceHandle {
            let mut reference = type_reference;
            while reference.is_valid() {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                    TypeReferenceNode::Reference {
                        referee,
                        access:
                            language_semantics::ReferenceAccess::Mutable
                            | language_semantics::ReferenceAccess::WriteOnly,
                        ..
                    } => {
                        return *referee;
                    }
                    _ => break,
                }
            }
            typed_trees::types::TypeReferenceHandle::invalid()
        }

        /*
            The default-domain field facts a state assumes on entry for its exclusive
            referents: each such parameter's `StateParameterDomain` field facts.
            These are exactly the rows the state-transition arrival check re-proves
            (checks/contracts/arrivals.rs); no second obligation vocabulary exists.
        */
        pub(crate) fn exclusive_referent_field_requirements(
            program: &dyn ::validation::CheckingInput,
            semantic: &facts::FactPlan,
            machine_symbol: symbols::SymbolHandle,
            state_symbol: symbols::SymbolHandle,
        ) -> Vec<facts::Fact> {
            let Some(state) =
                validation::find_state_in_machine(program, machine_symbol, state_symbol)
            else {
                return Vec::new();
            };
            let mut referents = Vec::new();
            for parameter in program.state_parameters(state) {
                let referee = exclusive_referent_type(program, parameter.type_reference);
                if !referee.is_valid() {
                    continue;
                }
                let Some(carrier) =
                    crate::published::semantic::facts::declared_field_domain_carrier(
                        program, referee,
                    )
                else {
                    continue;
                };
                let rows: Vec<_> =
                    validation::field_domains::declared_type_domain_rows(program, carrier)
                        .into_iter()
                        .filter(|row| !row.transfer && !row.path.is_empty())
                        .map(|row| (row.path, row.domain_symbol, row.semantic_domain))
                        .collect();
                referents.push((parameter.symbol, rows));
            }
            let mut requirements = Vec::new();
            if !referents.is_empty() {
                requirements.extend(
                    semantic
                        .contexts_at_point(ProgramPoint::State {
                            machine_symbol,
                            state_symbol,
                        })
                        .flat_map(|context| context.facts())
                        .filter(|fact| {
                            matches!(
                                fact.origin,
                                FactOrigin::StateParameterDomain {
                                    machine_symbol: origin_machine,
                                    state_symbol: origin_state,
                                } if origin_machine == machine_symbol && origin_state == state_symbol
                            ) && match (fact.place, fact.payload) {
                                (
                                    FactPlace::Place(place),
                                    FactPayload::DomainMembership {
                                        domain_symbol,
                                        semantic_domain,
                                        ..
                                    },
                                ) => {
                                    let place = semantic.places.get(place);
                                    let path = semantic.place_segments.span_or_empty(place.segments);
                                    referents.iter().any(|(root, rows)| {
                                        place.root == PlaceRoot::Symbol(*root)
                                            && rows.iter().any(|(declared, domain, identity)| {
                                                path == declared
                                                    && domain_symbol == *domain
                                                    && semantic_domain == *identity
                                            })
                                    })
                                }
                                _ => false,
                            }
                        })
                        .cloned(),
                );
            }
            requirements
        }

        /*
            A machine's normal return is a default-domain consumption point for every
            mutable or write-only referent it received: its declared field/default facts
            assumed on entry must be provable again from the live evidence at the
            exit. A write through a bare alias (`&mut [u8; 4]` onto a `Utf8` field)
            retires the field's fact without establishing anything, so returning
            without repairing it rejects here with the exact place, the same way a
            call or transition would refuse the corrupted referent.
        */
        pub(in crate::checks::contracts) fn check_exclusive_referent_field_domains(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            exit: &FlowExitFact,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let requirements = exclusive_referent_field_requirements(
                program,
                &facts.semantic,
                exit.machine_symbol,
                exit.state_symbol,
            );
            let entry_contexts =
                active_context_handles(&facts.flow.contexts, exit.entry_semantic_contexts);
            let memberships = std::cell::OnceCell::new();
            let membership = || {
                memberships.get_or_init(|| {
                    crate::flow::MembershipEvidence::prepare(
                        program,
                        &facts.semantic,
                        &entry_contexts,
                    )
                })
            };
            let cases = std::cell::OnceCell::new();
            for requirement in &requirements {
                let (
                    FactPlace::Place(place),
                    FactPayload::DomainMembership {
                        domain_symbol,
                        semantic_domain,
                        ..
                    },
                ) = (requirement.place, requirement.payload)
                else {
                    continue;
                };
                let place = facts.semantic.places.get(place);
                if canonical_place_from_semantic_place(program, &facts.semantic, place).is_some_and(
                    |subject| {
                        subject
                            .segments
                            .iter()
                            .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
                            && cases
                                .get_or_init(|| {
                                    crate::flow::CaseEvidence::prepare(
                                        program,
                                        &facts.semantic,
                                        &entry_contexts,
                                        exit.machine_symbol,
                                        exit.state_symbol,
                                        exit.statement_index,
                                    )
                                })
                                .as_ref()
                                .is_some_and(|cases| cases.path_is_inactive(&subject))
                    },
                ) {
                    continue;
                }
                let satisfied = if validation::field_domains::domain_requires_provenance(
                    program,
                    domain_symbol,
                ) {
                    canonical_place_from_semantic_place(program, &facts.semantic, place)
                        .is_some_and(|subject| {
                            membership().contains(&subject, domain_symbol, semantic_domain)
                        })
                } else {
                    super::super::super::prover::semantic_contexts_prove_contract_fact(
                        program,
                        facts,
                        facts::ProgramPoint::Statement {
                            machine_symbol: exit.machine_symbol,
                            state_symbol: exit.state_symbol,
                            statement_index: exit.statement_index,
                        },
                        &entry_contexts,
                        requirement,
                    )
                };
                if satisfied {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove default-domain field requirement for return from {} at statement {}: {} requires {}",
                    crate::published::labels::machine_name(program, exit.machine_symbol),
                    exit.statement_index,
                    facts::canonical_place_label_from_parts(
                        program,
                        place.root,
                        facts.semantic.place_segments.span_or_empty(place.segments),
                    ),
                    crate::published::labels::symbol_name(program, domain_symbol),
                )));
            }
        }
    }

    fn exact_integer_reference(
        program: &dyn ::validation::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        program
            .primitive_type_reference(reference)
            .is_some_and(|primitive| primitive.accepts_integer_literal())
            && program.arithmetic_domain_for_type_reference(reference)
                == numerics::arithmetic::ArithmeticDomain::Exact
    }

    /*
        Traverse only the actual retained domain fact, preserving its declaration
        owner for every operator query. The optional inner type is an anonymous
        literal, not permission to borrow another operand's type. This traversal
        establishes meaning and exact self occurrences; the shared evaluator and
        implication engine, rather than this code, judge predicate truth.
    */
    fn domain_predicate_meaning(
        program: &dyn ::validation::CheckingInput,
        domain: &typed_trees::domain::DomainDefinition,
        expression: typed_trees::expression::ExpressionHandle,
        subjects: &mut Vec<typed_trees::expression::ExpressionHandle>,
        depth: usize,
    ) -> Option<Option<typed_trees::types::TypeReferenceHandle>> {
        use typed_trees::expression::{BinaryOperator, ExpressionNode};
        if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let boolean_type = || {
            let symbol = program
                .symbols
                .child_handles(program.symbols.root())?
                .find(|symbol| {
                    program.symbols.builtin_type_atom(*symbol)
                        == Some(symbols::BuiltinTypeAtom::Bool)
                })?;
            program
                .type_reference_table
                .find_named_type_reference(symbol)
        };
        if let Some(reference) = validation::exact_domain_subject_type(program, domain, expression)
        {
            crate::shared::collections::push_unique(&mut *subjects, expression);
            return Some(Some(reference));
        }
        Some(match program.expression_table.expression(expression) {
            ExpressionNode::Integer(_) => {
                validation::landed_integer_literal_type_reference(program, expression)
            }
            ExpressionNode::Boolean(_) => boolean_type(),
            ExpressionNode::Name(path)
                if validation::is_single_segment_name_path(program, path) =>
            {
                let parameter = typed_trees::domain::index_parameters(program, domain)
                    .iter()
                    .find(|parameter| parameter.symbol == path.symbol)?;
                let typed_trees::data::TypeParameterKind::Const { type_reference } = parameter.kind
                else {
                    return None;
                };
                Some(type_reference)
            }
            ExpressionNode::Binary(binary) => {
                let left =
                    domain_predicate_meaning(program, domain, binary.left, subjects, depth + 1)?;
                let right =
                    domain_predicate_meaning(program, domain, binary.right, subjects, depth + 1)?;
                let spelling = typed_trees::operator::binary_operator_spelling(binary.operator);
                if binary.operator == BinaryOperator::CaseMembership {
                    return None;
                }
                if spelling.is_some_and(|spelling| {
                    !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        domain.symbol,
                        expression,
                        spelling,
                        &[left, right],
                    )
                }) {
                    return None;
                }
                match binary.operator {
                    BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::And
                    | BinaryOperator::Or => boolean_type(),
                    _ => {
                        let exact_operand = |operand,
                                             reference: Option<
                            typed_trees::types::TypeReferenceHandle,
                        >| {
                            reference
                                .map(|reference| exact_integer_reference(program, reference))
                                .unwrap_or_else(|| {
                                    matches!(program.expression_table.expression(operand),
                                    ExpressionNode::Integer(literal) if literal.landing().is_none())
                                })
                        };
                        if !exact_operand(binary.left, left) || !exact_operand(binary.right, right)
                        {
                            return None;
                        }
                        let left_result = match left {
                            Some(reference) => Some(validation::arithmetic_result_type_reference(
                                program, reference,
                            )?),
                            None => None,
                        };
                        let right_result = match right {
                            Some(reference) => Some(validation::arithmetic_result_type_reference(
                                program, reference,
                            )?),
                            None => None,
                        };
                        left_result.or(right_result)
                    }
                }
            }
            ExpressionNode::Unary(unary)
                if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
            {
                domain_predicate_meaning(program, domain, unary.operand, subjects, depth + 1)?
            }
            _ => return None,
        })
    }
}
pub(crate) use result_domains::exact_scalar_membership;
mod scalars {
    /*
        Scalar postconditions consume exact return and live place values. Entry
        parameter names are substituted only through the retained exit origin map.
    */
    use super::super::current_values::ExitValueCache;
    use super::super::current_values::proves_exit_expression;
    use super::cases::CaseObservation;
    use checked_trees::{CheckFacts, CheckedScalarExpressionRole, FlowExitFact};
    use facts::{FactContextHandle, FactPayload, PlaceRoot, PlaceSegment};
    use typed_trees::{expression::ExpressionHandle, machine::Machine};

    use super::super::{
        prover::{
            ScalarValue, evaluate_checked_scalar, evaluate_for_atoms, evaluate_scalar,
            has_builtin_operators, semantic_contexts_prove_boolean_expression,
        },
        return_values::{exit_return_expression, is_result_reference},
    };
    use crate::flow::canonical_place_from_symbol;
    use crate::flow::expressions;
    use crate::published::places::place_at;

    mod admitted_domains {
        /*
            Admission authorizes a route, not a contradiction. Exact applied provider
            obligations stay separate from body assumptions and direct-call guarantees.
            Completed values and live storage observations may refute their predicates;
            unknown predicates grant neither a veto nor evidence of consistency.
        */
        use super::*;
        use crate::flow::active_context_handles;
        use crate::published::facts::boolean_predicates;
        use crate::published::places::place_at;

        pub(in crate::checks::contracts) struct BoundaryRefutations {
            machines: Vec<(
                symbols::SymbolHandle,
                Vec<validation::BoundaryQualificationObligation>,
                crate::flow::MachineParameterOrigins,
            )>,
        }

        impl BoundaryRefutations {
            pub(in crate::checks::contracts) fn new(
                program: &dyn validation::CheckingInput,
            ) -> Self {
                let machines = program
                    .machines()
                    .iter()
                    .filter_map(|machine| {
                        let obligations = validation::boundary_provider_qualification_obligations(
                            program, machine,
                        );
                        if obligations.is_empty() {
                            return None;
                        }
                        Some((
                            machine.symbol,
                            obligations,
                            crate::flow::machine_parameter_origins(program, machine),
                        ))
                    })
                    .collect();
                Self { machines }
            }

            pub(in crate::checks::contracts) fn check<'program>(
                &self,
                program: &'program dyn validation::CheckingInput,
                facts: &CheckFacts,
                exit: &FlowExitFact,
                call_frames: Option<&validation::CallFrameResolver<'program>>,
                diagnostics: &mut Vec<diagnostics::Diagnostic>,
            ) {
                let Some((_, obligations, origins)) = self
                    .machines
                    .iter()
                    .find(|(machine, _, _)| *machine == exit.machine_symbol)
                else {
                    return;
                };
                let Some((machine, state)) =
                    program.state_flow_site(exit.machine_symbol, exit.state_symbol)
                else {
                    return;
                };
                let Some(entry) = program.machine_states(machine).first() else {
                    return;
                };
                let origins =
                    crate::flow::state_parameter_origins(program, machine, state, origins);
                let contexts =
                    active_context_handles(&facts.flow.contexts, exit.entry_semantic_contexts);
                let retained = boolean_predicates::retained(
                    program,
                    &facts.operators,
                    &facts.semantic,
                    &contexts,
                    facts::ProgramPoint::Statement {
                        machine_symbol: exit.machine_symbol,
                        state_symbol: exit.state_symbol,
                        statement_index: exit.statement_index,
                    },
                );
                let evaluator = ExitScalars {
                    program,
                    facts,
                    machine,
                    exit,
                    contexts: &contexts,
                    contract: Default::default(),
                    call_frames,
                };
                let returned = exit_return_expression(program, exit);
                for obligation in obligations {
                    let Some(domain) = program.domain_definition(obligation.domain) else {
                        continue;
                    };
                    let completed_refutation = !obligation.parameter.is_valid()
                        && validation::completed_value_refutes_domain(
                            program,
                            machine,
                            state,
                            exit.statement_index,
                            returned,
                            obligation.domain,
                        );
                    let subject = if !obligation.parameter.is_valid() {
                        place_at(program, state.symbol, exit.statement_index, returned)
                    } else if obligation.realization_state == entry.symbol {
                        origins
                            .iter()
                            .find(|(source, _)| *source == obligation.parameter)
                            .and_then(|(_, target)| canonical_place_from_symbol(*target))
                    } else if obligation.realization_state == state.symbol
                        && validation::state_reference_parameter_binding_is_stable(
                            program,
                            machine,
                            state,
                            obligation.parameter,
                        )
                    {
                        canonical_place_from_symbol(obligation.parameter)
                    } else {
                        None
                    };
                    let live_refutation = subject
                        .and_then(|subject| {
                            boolean_predicates::domain_predicates_for_subject(
                                program,
                                &facts.operators,
                                domain,
                                &subject,
                            )
                        })
                        .is_some_and(|predicates| {
                            predicates.iter().any(|predicate| {
                                boolean_predicates::truth(&retained, predicate, |place| {
                                    evaluator.value_at_place(place)
                                }) == Some(false)
                            })
                        });
                    if completed_refutation || live_refutation {
                        diagnostics.push(diagnostics::Diagnostic::error(format!(
                            "boundary qualification is refuted at exit of {} in state `{}` for requirement `{}`: {}",
                            crate::published::labels::machine_name(program, machine.symbol), state.name,
                            program.symbols.name(obligation.requirement), domain.name,
                        )));
                    }
                }
            }
        }
    }
    mod deferred_recipes {
        /*
            Exact return substitution transports a predicate over closed recipe terms.

            This is conditional congruence, never a guessed endpoint value. Immutable
            invocation origins establish the returned scalar; identical closed source
            applications supply paired recipe demands. Every pair independently forms,
            evaluates, and agrees under the owning target before publication. A failed
            conjunction/candidate publishes no partial debt or proof.
        */
        use super::{ExitScalars, is_result_reference};
        use checked_trees::{
            CheckFacts, ConstantRecipeObligation, ConstantRecipeRequirement, FlowExitFact,
        };
        use facts::{FactContextHandle, FactPayload};
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

        pub(in super::super) fn proves_with_recipe_conditions<'program>(
            program: &'program dyn validation::CheckingInput,
            facts: &CheckFacts,
            exit: &FlowExitFact,
            contexts: &[FactContextHandle],
            requirement: &facts::Fact,
            call_frames: Option<&validation::CallFrameResolver<'program>>,
        ) -> Option<Vec<ConstantRecipeObligation>> {
            let FactPayload::ContractBooleanExpression {
                fact: contract,
                expression,
                ..
            } = requirement.payload
            else {
                return None;
            };
            let machine = program.machine(exit.machine_symbol)?;
            let entry = program.machine_states(machine).first()?;
            let evaluator = ExitScalars {
                program,
                facts,
                machine,
                exit,
                contexts,
                contract,
                call_frames,
            };
            let mut hypotheses = Vec::new();
            for expression in program.contract_expressions(
                program
                    .machine_contracts(machine)
                    .iter()
                    .chain(program.state_contracts(entry).iter()),
                typed_trees::signature::SignatureContractKind::Requires,
            ) {
                conjuncts(&evaluator, expression, &mut hypotheses)?;
            }
            let mut conditions = Vec::new();
            let mut used_result = false;
            prove(
                &evaluator,
                expression,
                &hypotheses,
                &mut conditions,
                &mut used_result,
            )?;
            (used_result && !conditions.is_empty()).then_some(conditions)
        }

        /*
            A returned evaluated range may prove a scalar guarantee conditionally. The
            existing range closure owns every endpoint observation and publication debt.
        */
        pub(in super::super) fn proves_with_range_conditions<'program>(
            program: &'program dyn validation::CheckingInput,
            facts: &CheckFacts,
            exit: &FlowExitFact,
            contexts: &[FactContextHandle],
            requirement: &facts::Fact,
            call_frames: Option<&validation::CallFrameResolver<'program>>,
        ) -> Option<Vec<checked_trees::IntegerRangeObligation>> {
            let FactPayload::ContractBooleanExpression {
                fact: contract,
                expression,
                ..
            } = requirement.payload
            else {
                return None;
            };
            let machine = program.machine(exit.machine_symbol)?;
            let evaluator = ExitScalars {
                program,
                facts,
                machine,
                exit,
                contexts,
                contract,
                call_frames,
            };
            let mut pending = vec![expression];
            let mut conditions = Vec::new();
            while let Some(expression) = pending.pop() {
                if !builtin(&evaluator, expression) {
                    return None;
                }
                if let Some(binary) = program.expression_table.binary_node(expression)
                    && binary.operator == BinaryOperator::And
                {
                    pending.extend([binary.left, binary.right]);
                    continue;
                }
                conditions.push(evaluator.result_range_condition(expression)?);
            }
            (!conditions.is_empty()).then_some(conditions)
        }

        fn builtin(evaluator: &ExitScalars<'_, '_>, expression: ExpressionHandle) -> bool {
            super::has_builtin_operators(evaluator.program, &evaluator.facts.operators, expression)
                && validation::has_builtin_bound_expression_meaning(
                    evaluator.program,
                    evaluator.machine,
                    evaluator.program.machine_states(evaluator.machine).first(),
                    expression,
                )
        }

        fn conjuncts(
            evaluator: &ExitScalars<'_, '_>,
            expression: ExpressionHandle,
            output: &mut Vec<ExpressionHandle>,
        ) -> Option<()> {
            if let Some(binary) = evaluator.program.expression_table.binary_node(expression)
                && binary.operator == BinaryOperator::And
                && builtin(evaluator, expression)
            {
                conjuncts(evaluator, binary.left, output)?;
                conjuncts(evaluator, binary.right, output)?;
            } else {
                output.push(expression);
            }
            Some(())
        }

        fn prove(
            evaluator: &ExitScalars<'_, '_>,
            goal: ExpressionHandle,
            hypotheses: &[ExpressionHandle],
            conditions: &mut Vec<ConstantRecipeObligation>,
            used_result: &mut bool,
        ) -> Option<()> {
            if let Some(binary) = evaluator.program.expression_table.binary_node(goal)
                && binary.operator == BinaryOperator::And
                && builtin(evaluator, goal)
            {
                prove(evaluator, binary.left, hypotheses, conditions, used_result)?;
                return prove(evaluator, binary.right, hypotheses, conditions, used_result);
            }
            for hypothesis in hypotheses {
                let mut candidate = Vec::new();
                let mut candidate_result = false;
                if congruent(
                    evaluator,
                    *hypothesis,
                    goal,
                    &mut candidate,
                    &mut candidate_result,
                    0,
                ) {
                    conditions.extend(candidate);
                    *used_result |= candidate_result;
                    return Some(());
                }
            }
            None
        }

        fn congruent(
            evaluator: &ExitScalars<'_, '_>,
            hypothesis: ExpressionHandle,
            goal: ExpressionHandle,
            conditions: &mut Vec<ConstantRecipeObligation>,
            used_result: &mut bool,
            depth: usize,
        ) -> bool {
            if depth >= 64 {
                return false;
            }
            let program = evaluator.program;
            let expressions = &program.expression_table;
            if is_result_reference(program, evaluator.machine, goal) {
                let Some(name) = expressions.name_path_node(hypothesis) else {
                    return false;
                };
                if expressions.name_path_is_single_member(name)
                    && evaluator.returned_immutable_parameter(name.symbol)
                {
                    *used_result = true;
                    return true;
                }
                return false;
            }
            let left = program.constant_recipe_for_contract_expression(hypothesis);
            let right = program.constant_recipe_for_contract_expression(goal);
            if left.is_valid() && right.is_valid() {
                if !closed_terms_identical(program, hypothesis, goal, depth + 1) {
                    return false;
                }
                if left != right {
                    conditions.push(
                        ConstantRecipeObligation::retain(
                            evaluator.machine.symbol,
                            semantic_vocabulary::ScalarConditionRole::Result,
                            left,
                            ConstantRecipeRequirement::EqualRecipe(right),
                            expressions.source_span(goal),
                            "normal return preserves its requires predicate under exact scalar identity"
                                .into(),
                        )
                        .with_recipe_owner(evaluator.machine.symbol),
                    );
                }
                return true;
            }
            match (
                expressions.expression(hypothesis),
                expressions.expression(goal),
            ) {
                (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
                    expressions.name_path_is_single_member(left)
                        && expressions.name_path_is_single_member(right)
                        && left.symbol == right.symbol
                        && program
                            .machine_states(evaluator.machine)
                            .first()
                            .and_then(|entry| program.state_parameter_by_symbol(entry, left.symbol))
                            .is_some_and(|parameter| {
                                !parameter.is_mutable
                                    && !parameter.is_const
                                    && program
                                        .primitive_type_reference(parameter.type_reference)
                                        .is_some_and(|primitive| {
                                            primitive.is_integer()
                                                || primitive
                                                    == typed_trees::types::PrimitiveType::Bool
                                        })
                            })
                }
                (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
                (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
                (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
                    left.operator == right.operator
                        && builtin(evaluator, hypothesis)
                        && builtin(evaluator, goal)
                        && congruent(
                            evaluator,
                            left.left,
                            right.left,
                            conditions,
                            used_result,
                            depth + 1,
                        )
                        && congruent(
                            evaluator,
                            left.right,
                            right.right,
                            conditions,
                            used_result,
                            depth + 1,
                        )
                }
                (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
                    left.operator == right.operator
                        && builtin(evaluator, hypothesis)
                        && builtin(evaluator, goal)
                        && congruent(
                            evaluator,
                            left.operand,
                            right.operand,
                            conditions,
                            used_result,
                            depth + 1,
                        )
                }
                _ => false,
            }
        }

        /*
            Ordinary expression equality deliberately ignores generic call metadata in
            some source-reuse queries. Contract denotation identity cannot: retain the
            whole applied selector and every nested actual in the same operand order.
        */
        fn closed_terms_identical(
            program: &dyn validation::CheckingInput,
            left: ExpressionHandle,
            right: ExpressionHandle,
            depth: usize,
        ) -> bool {
            if depth >= 64 {
                return false;
            }
            let expressions = &program.expression_table;
            if !expressions.expressions_structurally_equal(left, right) {
                return false;
            }
            match (expressions.expression(left), expressions.expression(right)) {
                (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
                    left.target_symbol.is_valid()
                        && left.target_symbol == right.target_symbol
                        && left.machine_arguments == right.machine_arguments
                        && left.authored_machine_application == right.authored_machine_application
                        && left.requirement_application == right.requirement_application
                        && left.static_machine_parameter == right.static_machine_parameter
                        && left.static_requirement_dispatch == right.static_requirement_dispatch
                        && left.evidence_arguments == right.evidence_arguments
                        && left.quotient_operation == right.quotient_operation
                        && left.private_layout_operation == right.private_layout_operation
                        && left.installed_interrupt_operation == right.installed_interrupt_operation
                        && left.layout_query == right.layout_query
                        && left.native_code_layout == right.native_code_layout
                        && left.build_selection == right.build_selection
                        && expressions
                            .expression_handles(left.arguments)
                            .iter()
                            .zip(expressions.expression_handles(right.arguments))
                            .all(|(left, right)| {
                                closed_terms_identical(program, *left, *right, depth + 1)
                            })
                }
                (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
                    closed_terms_identical(program, left.left, right.left, depth + 1)
                        && closed_terms_identical(program, left.right, right.right, depth + 1)
                }
                (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
                    closed_terms_identical(program, left.operand, right.operand, depth + 1)
                }
                (ExpressionNode::Integer(_), ExpressionNode::Integer(_))
                | (ExpressionNode::Boolean(_), ExpressionNode::Boolean(_)) => true,
                _ => left == right,
            }
        }

        #[cfg(test)]
        mod tests {
            use super::closed_terms_identical;
            use numerics::literals::IntegerLiteral;
            use typed_trees::{
                TypedTrees,
                expression::{ExpressionNode, StaticMachineArgument, TableCallExpression},
            };

            fn call() -> TableCallExpression {
                TableCallExpression {
                    authored_call_selection: None,
                    authored_call_binding: None,
                    build_selection: Default::default(),
                    authored_machine_application: Default::default(),
                    exact_coercion_target: Default::default(),
                    requirement_application: Default::default(),
                    receiver: Default::default(),
                    target_symbol: symbols::SymbolHandle::from_arena_index(1),
                    static_machine_parameter: Default::default(),
                    target: typed_trees::name::Identifier::generated("closed"),
                    static_requirement_dispatch: None,
                    machine_arguments: Box::new([]),
                    quotient_operation: None,
                    private_layout_operation: None,
                    installed_interrupt_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                    arguments: Default::default(),
                    evidence_arguments: Box::new([]),
                    operational_acknowledgement: Default::default(),
                }
            }

            /*
                Runtime call shape omits proof-static arguments. It therefore cannot prove
                equal denotation, either before specialization or after operands are erased
                into their retained static application identities.
            */
            #[test]
            fn identical_runtime_calls_do_not_identify_distinct_static_denotations() {
                let mut program = TypedTrees::default();
                let mut first = call();
                first.machine_arguments = vec![StaticMachineArgument {
                    const_literal: Some(IntegerLiteral::from_value(8)),
                    ..Default::default()
                }]
                .into_boxed_slice();
                let mut second = first.clone();
                second.machine_arguments[0].const_literal = Some(IntegerLiteral::from_value(6));
                let first = program.expression_table.insert(ExpressionNode::Call(first));
                let second = program
                    .expression_table
                    .insert(ExpressionNode::Call(second));
                assert!(
                    program
                        .expression_table
                        .expressions_structurally_equal(first, second)
                );
                assert!(!closed_terms_identical(&program, first, second, 0));

                let mut first = call();
                first.authored_machine_application = arena::Handle::from_arena_index(1);
                let mut second = first.clone();
                second.authored_machine_application = arena::Handle::from_arena_index(2);
                let same = program
                    .expression_table
                    .insert(ExpressionNode::Call(first.clone()));
                let first = program.expression_table.insert(ExpressionNode::Call(first));
                let second = program
                    .expression_table
                    .insert(ExpressionNode::Call(second));
                assert!(
                    program
                        .expression_table
                        .expressions_structurally_equal(first, second)
                );
                assert!(!closed_terms_identical(&program, first, second, 0));
                assert!(closed_terms_identical(&program, first, same, 0));
            }
        }
    }
    pub(in crate::checks::contracts) use admitted_domains::BoundaryRefutations;
    pub(super) use deferred_recipes::{
        proves_with_range_conditions, proves_with_recipe_conditions,
    };
    pub(crate) mod calls {
        /*
            Call operands transported through an exact normal-return guarantee.
            No source arithmetic, callee body, or current argument storage is replayed.
        */
        use super::{
            CheckedScalarExpressionRole, ExpressionHandle, FactPayload, Machine, ScalarValue,
        };
        use crate::checks::contracts::exits::scalars::ExitScalars;
        use crate::checks::contracts::return_values::is_result_reference;
        use crate::flow::state_statements;
        use crate::published::places::place_at;

        use crate::values::evaluate_checked_scalar;
        use crate::values::scalar_value_at_place;
        use checked_trees::{
            CheckedScalarComputationKind, ContractProofFactKind, ContractProofFactOwner,
        };
        use typed_trees::expression::{BinaryOperator, ExpressionNode};

        #[derive(Clone, Copy)]
        pub(super) struct NormalReturnCall<'a> {
            pub state: symbols::SymbolHandle,
            pub fact: &'a checked_trees::FlowCallFact,
            pub callee: &'a Machine,
            pub entry: &'a typed_trees::state::State,
            pub arguments: &'a [ExpressionHandle],
        }

        impl ExitScalars<'_, '_> {
            /*
                Callee predicate spelling is not transport: the returned occurrence,
                declared result/formal equation, and actual immutable origin supply it.
            */
            pub(super) fn call_returns_immutable_symbol(
                &self,
                expression: ExpressionHandle,
                symbol: symbols::SymbolHandle,
            ) -> bool {
                let call_expression = if matches!(
                    self.program.expression_table.expression(expression),
                    ExpressionNode::Call(_)
                ) {
                    Some(expression)
                } else {
                    place_at(
                        self.program,
                        self.exit.state_symbol,
                        self.exit.statement_index,
                        expression,
                    )
                    .and_then(|place| self.assigned_call_at_place(&place))
                };
                let Some(call) =
                    call_expression.and_then(|expression| self.normal_return_call(expression))
                else {
                    return false;
                };
                if call.state != self.exit.state_symbol
                    || call.fact.statement_index > self.exit.statement_index
                {
                    return false;
                }
                self.normal_call_guarantees(call).any(|guarantee| {
                    let Some(position) = self.normal_call_result_parameter(call, guarantee) else {
                        return false;
                    };
                    matches!(self.program.expression_table.expression(call.arguments[position]),
                        ExpressionNode::Name(path)
                            if self.program.expression_table.name_path_is_single_member(path)
                                && path.symbol == symbol)
                })
            }

            fn normal_call_result_parameter(
                &self,
                call: NormalReturnCall<'_>,
                guarantee: ExpressionHandle,
            ) -> Option<usize> {
                let binary = self.program.expression_table.binary_node(guarantee)?;
                if binary.operator != BinaryOperator::Equal {
                    return None;
                }
                let formal = if is_result_reference(self.program, call.callee, binary.left) {
                    binary.right
                } else if is_result_reference(self.program, call.callee, binary.right) {
                    binary.left
                } else {
                    return None;
                };
                let path = self.program.expression_table.name_path_node(formal)?;
                let (position, parameter) = self
                    .program
                    .state_parameters(call.entry)
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| {
                        parameter.symbol.is_valid()
                            && path.symbol == parameter.symbol
                            && path.head_symbol == parameter.symbol
                    })?;
                let result_type = self
                    .program
                    .primitive_type_reference(call.entry.return_type)?;
                (!parameter.is_mutable
                    && self
                        .program
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        == 1
                    && self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        == Some(result_type)
                    && result_type.accepts_integer_literal()
                    && typed_trees::operator::has_builtin_spelled_expression_meaning(
                        self.program,
                        call.callee.symbol,
                        guarantee,
                        language_core::OperatorSpelling::Equal,
                        &[Some(call.entry.return_type), Some(parameter.type_reference)],
                    ))
                .then_some(position)
            }

            pub(super) fn value_at_place(
                &self,
                subject: &validation::CanonicalPlace,
            ) -> Option<ScalarValue> {
                if let Some(value) = scalar_value_at_place(
                    self.program,
                    &self.facts.semantic,
                    self.contexts
                        .iter()
                        .map(|context| self.facts.semantic.contexts.get(*context)),
                    subject,
                ) {
                    return Some(value);
                }
                self.closed_call_value(self.assigned_call_at_place(subject)?)
            }

            pub(super) fn assigned_call_at_place(
                &self,
                subject: &validation::CanonicalPlace,
            ) -> Option<ExpressionHandle> {
                let mut source = None;
                for context in self.contexts {
                    for fact in self
                        .facts
                        .semantic
                        .context_view(self.facts.semantic.contexts.get(*context))
                        .facts()
                    {
                        if !matches!(
                            fact.payload,
                            FactPayload::AssignedValue { .. }
                                | FactPayload::AssignedScalarValue { .. }
                        ) {
                            continue;
                        }
                        let Some(place) = fact.place.as_place() else {
                            continue;
                        };
                        let Some(candidate) = crate::flow::canonical_place_from_semantic_place(
                            self.program,
                            &self.facts.semantic,
                            self.facts.semantic.places.get(place),
                        ) else {
                            continue;
                        };
                        if !crate::flow::canonical_places_agree(self.program, &candidate, subject) {
                            continue;
                        }
                        let FactPayload::AssignedValue { value } = fact.payload else {
                            return None;
                        };
                        if !matches!(
                            self.program.expression_table.expression(value),
                            ExpressionNode::Call(_)
                        ) || source.is_some_and(|prior| prior != value)
                        {
                            return None;
                        }
                        source = Some(value);
                    }
                }
                source
            }

            pub(super) fn normal_return_call(
                &self,
                expression: ExpressionHandle,
            ) -> Option<NormalReturnCall<'_>> {
                let authored = self.program.expression_table.call_node(expression)?;
                if !authored.machine_arguments.is_empty() {
                    return None;
                }
                self.normal_return_call_target(expression)
            }

            pub(super) fn normal_return_call_target(
                &self,
                expression: ExpressionHandle,
            ) -> Option<NormalReturnCall<'_>> {
                let authored = self.program.expression_table.call_node(expression)?;
                if authored.receiver.is_valid()
                    || !authored.evidence_arguments.is_empty()
                    || !authored.selects_only_nominal_route()
                {
                    return None;
                }
                let mut occurrences = self
                    .facts
                    .flow
                    .control
                    .machine_states(self.machine.symbol)
                    .into_iter()
                    .map(|state| ((), state))
                    .flat_map(|(_, state)| {
                        self.facts
                            .flow
                            .control
                            .calls
                            .span_or_empty(state.calls)
                            .iter()
                            .map(move |call| (state, call))
                    })
                    .filter(|(_, call)| call.authored_expression == expression);
                let (state, call) = occurrences.next()?;
                let authored_target = validation::call_target_symbol(self.program, authored);
                let entered_target = validation::call_target_state(self.program, authored_target)
                    .map_or(authored_target, |state| state.symbol);
                if occurrences.next().is_some() || call.target_symbol != entered_target {
                    return None;
                }
                let Some(validation::CallSite::Expression {
                    expression: actual,
                    call: selected,
                }) = validation::find_call_site(
                    self.program,
                    self.machine.symbol,
                    state.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                )
                else {
                    return None;
                };
                if actual != expression || !std::ptr::eq(selected, authored) {
                    return None;
                }
                if !validation::independent_call_application(
                    self.program,
                    self.machine.symbol,
                    selected.target_symbol,
                    typed_trees::references::StaticArgumentCall::Expression(expression, selected),
                ) {
                    return None;
                }
                let (callee, entry) =
                    validation::find_machine_by_entry_state(self.program, call.target_symbol)?;
                let parameters = self.program.state_parameters(entry);
                if parameters.iter().any(|parameter| parameter.is_const) {
                    return None;
                }
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(authored.arguments);
                if arguments.len() != parameters.len() {
                    return None;
                }
                Some(NormalReturnCall {
                    state: state.state_symbol,
                    fact: call,
                    callee,
                    entry,
                    arguments,
                })
            }

            pub(super) fn normal_call_guarantees<'a>(
                &'a self,
                call: NormalReturnCall<'a>,
            ) -> impl Iterator<Item = ExpressionHandle> + 'a {
                self.facts
                    .proof
                    .contract_fact_refs
                    .span_or_empty(call.fact.ensures)
                    .iter()
                    .filter_map(move |reference| {
                    let guarantee = self.facts.proof.contract_facts.get(reference.fact);
                    if guarantee.kind != ContractProofFactKind::Ensures
                        || !matches!(guarantee.owner,
                            ContractProofFactOwner::Machine { machine_symbol } if machine_symbol == call.callee.symbol)
                            && !matches!(guarantee.owner,
                                ContractProofFactOwner::MachineState { machine_symbol, state_symbol }
                                    if machine_symbol == call.callee.symbol && state_symbol == call.entry.symbol)
                    {
                        return None;
                    }
                    let expression = self.program.proof_facts.get(guarantee.fact).as_expression()?;
                    Some(*expression)
                })
            }

            pub(super) fn closed_call_value(
                &self,
                expression: ExpressionHandle,
            ) -> Option<ScalarValue> {
                let selected = self.normal_return_call(expression)?;
                let NormalReturnCall {
                    state,
                    fact: call,
                    entry,
                    arguments,
                    ..
                } = selected;
                let result_type = self.program.primitive_type_reference(entry.return_type)?;
                if !result_type.is_fixed_integer() {
                    return None;
                }
                let mut retained = None;
                for guarantee_expression in self.normal_call_guarantees(selected) {
                    let Some(position) =
                        self.normal_call_result_parameter(selected, guarantee_expression)
                    else {
                        continue;
                    };
                    let argument = arguments[position];
                    let value = self.closed_argument_value(
                        state,
                        call,
                        expression,
                        argument,
                        position,
                        result_type,
                    )?;
                    if retained.as_ref().is_some_and(|prior| prior != &value) {
                        return None;
                    }
                    retained = Some(value);
                }
                retained
            }

            fn closed_argument_value(
                &self,
                state: symbols::SymbolHandle,
                call: &checked_trees::FlowCallFact,
                call_expression: ExpressionHandle,
                argument: ExpressionHandle,
                position: usize,
                expected_type: typed_trees::types::PrimitiveType,
            ) -> Option<ScalarValue> {
                let statement = u32::try_from(call.statement_index).ok()?;
                let position = u32::try_from(position).ok()?;
                let caller_state =
                    validation::find_state_in_machine(self.program, self.machine.symbol, state)?;
                let statements = state_statements(self.program, caller_state.statement_nodes);
                let direct_binding_ordinal = match statements.get(call.statement_index)? {
                    typed_trees::statement::StatementNode::LocalData(local)
                        if !local.is_mutable && local.initial_value == call_expression =>
                    {
                        u32::try_from(statements[..call.statement_index].iter().filter(|statement| {
                            matches!(statement, typed_trees::statement::StatementNode::LocalData(local)
                                if !local.is_mutable && self.program.primitive_type_reference(local.type_reference).is_some())
                        }).count()).ok()
                    }
                    _ => None,
                };
                let plans = &self.facts.values.scalar_expressions;
                let mut values = Vec::new();
                for (_, binding) in plans.source_bindings.iter() {
                    let matches_role = match binding.role {
                        CheckedScalarExpressionRole::CallArgument {
                            binding_ordinal,
                            argument_ordinal,
                        } => {
                            direct_binding_ordinal == Some(binding_ordinal)
                                && argument_ordinal == position
                        }
                        CheckedScalarExpressionRole::UnitCallArgument {
                            call_ordinal,
                            argument_ordinal,
                        } => {
                            usize::try_from(call_ordinal).ok() == Some(call.call_ordinal)
                                && argument_ordinal == position
                        }
                        _ => false,
                    };
                    if binding.state == state
                        && binding.statement_ordinal == statement
                        && binding.expression == argument
                        && matches_role
                    {
                        values.push(plans.expression_at(
                            binding.state,
                            binding.statement_ordinal,
                            binding.role,
                        )?);
                    }
                }
                let computations = &self.facts.values.scalar_computations;
                for (_, node) in computations.nodes.iter() {
                    if node.authored_root != call_expression {
                        continue;
                    }
                    let CheckedScalarComputationKind::Call {
                        source_call,
                        callee: checked_trees::CheckedScalarCallee::Body { .. },
                        arguments,
                        ..
                    } = &node.kind
                    else {
                        continue;
                    };
                    let Some(source_call) = source_call.statement() else {
                        continue;
                    };
                    if self.facts.flow.control.calls.get(source_call) != call {
                        continue;
                    }
                    let parameters =
                        validation::call_target_parameters(self.program, call.target_symbol)?;
                    let source_position = usize::try_from(position).ok()?;
                    if self
                        .program
                        .primitive_type_reference(parameters.get(source_position)?.type_reference)?
                        != expected_type
                    {
                        return None;
                    }
                    let scalar_position = parameters[..source_position]
                        .iter()
                        .filter(|parameter| {
                            self.program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                        })
                        .count();
                    let operand = computations
                        .operands
                        .span_or_empty(*arguments)
                        .get(scalar_position)?;
                    let CheckedScalarComputationKind::Value(value) =
                        &computations.nodes.get(*operand).kind
                    else {
                        return None;
                    };
                    values.push(value);
                }
                let mut retained = None;
                for expression in values {
                    if crate::values::scalar_expression_type(expression) != Some(expected_type) {
                        return None;
                    }
                    let value = evaluate_checked_scalar(expression, &mut |_| None)?;
                    if retained.as_ref().is_some_and(|prior| prior != &value) {
                        return None;
                    }
                    retained = Some(value);
                }
                retained
            }
        }
    }
    mod predicates {
        /*
            Live exit predicates and completed return values share captured-value transport.
            A result is bound only by its authenticated outgoing occurrence. Sources keep
            their read epochs; contract inputs observe storage after return evaluation.
        */
        use super::{
            ExitScalars, ExpressionHandle, PlaceRoot, ScalarValue,
            semantic_contexts_prove_boolean_expression,
        };
        use crate::checks::contracts::{captured_predicates, prover, return_values};
        use crate::flow::captured_observation_places;
        use facts::ProgramPoint;
        use typed_trees::types::PrimitiveType;
        use validation::CanonicalPlace;
        use validation::expression_result_type_reference;

        impl ExitScalars<'_, '_> {
            pub(super) fn current_predicate(&self, expression: ExpressionHandle) -> Option<bool> {
                if !self
                    .contract_origins()
                    .any(|origin| origin.entry_parameter != origin.state_parameter)
                    && self
                        .state()
                        .and_then(|state| {
                            expression_result_type_reference(
                                self.program,
                                self.machine,
                                state,
                                expression,
                            )
                        })
                        .and_then(|reference| self.program.primitive_type_reference(reference))
                        == Some(PrimitiveType::Bool)
                    && semantic_contexts_prove_boolean_expression(
                        self.program,
                        &self.facts.semantic,
                        self.contexts,
                        ProgramPoint::Statement {
                            machine_symbol: self.machine.symbol,
                            state_symbol: self.exit.state_symbol,
                            statement_index: self.exit.statement_index,
                        },
                        expression,
                    )
                {
                    return Some(true);
                }
                let caller = self
                    .facts
                    .flow
                    .control
                    .state(self.exit.machine_symbol, self.exit.state_symbol);
                if let Some(frames) = self.call_frames
                    && let Some(caller) = caller
                    && let Some(entry) = self.program.machine_states(self.machine).first()
                    && prover::call_guarantees::proves_current(
                        self.program,
                        self.facts,
                        caller,
                        self.exit.statement_index,
                        self.contexts,
                        frames,
                        prover::call_guarantees::callable::Callable::Machine {
                            machine: self.machine,
                            state: entry,
                        },
                        expression,
                        &|expression| {
                            let parameters = self.program.state_parameters(entry);
                            let (position, path, _) = crate::values::structural_parameter_place(
                                self.program,
                                parameters,
                                expression,
                            )?;
                            let (root, segments, _, _) =
                                crate::values::resolve_structural_parameter_path(
                                    self.program,
                                    parameters,
                                    position,
                                    &path,
                                )?;
                            let place = CanonicalPlace {
                                root: PlaceRoot::Symbol(root),
                                segments,
                            };
                            let place = self.contract_place_at_exit(place)?;
                            crate::flow::rebase_exact_local_place(
                                self.program,
                                self.exit.state_symbol,
                                self.exit.statement_index,
                                place,
                                Some(frames),
                            )
                        },
                    )
                {
                    return Some(true);
                }
                let expression = crate::values::lower_scalar_contract_predicate(
                    self.program,
                    &self.facts.operators,
                    self.machine,
                    expression,
                    true,
                    &mut 4096,
                )?;
                let entry = self.program.machine_states(self.machine).first()?;
                let completed =
                    return_values::completed_return_operand(self.program, self.facts, self.exit);
                let result = completed.map(|operand| CanonicalPlace {
                    root: PlaceRoot::Expression(operand.expression),
                    segments: Vec::new(),
                });
                let mut required = crate::published::facts::boolean_predicates::bind_parameters(
                    self.program,
                    self.machine.symbol,
                    entry.symbol,
                    self.program.state_parameters(entry),
                    result.as_ref().map(|place| (place, entry.return_type)),
                    expression,
                )?;
                let mut captures = Vec::new();
                for position in 0..required.bindings.len() {
                    let binding = &mut required.bindings[position];
                    if result
                        .as_ref()
                        .is_some_and(|result| binding.place.root == result.root)
                    {
                        let operand = crate::flow::CapturedOperand::recorded(
                            &self.facts.flow.control,
                            self.exit.returned_value,
                        )?;
                        let state = caller?;
                        let values = crate::flow::captured_operand_values(
                            self.program,
                            self.facts.flow.view(),
                            &self.facts.semantic,
                            state,
                            operand,
                            entry.return_type,
                            &binding.place.segments,
                        )?;
                        let observed = captured_observation_places(
                            self.program,
                            self.facts.flow.view(),
                            values.first()?,
                        )?;
                        let [observed] = observed.as_slice() else {
                            return None;
                        };
                        if !values.iter().all(|value| {
                            captured_observation_places(self.program, self.facts.flow.view(), value)
                                .is_some_and(|places| {
                                    places.len() == 1 && places[0].projected == observed.projected
                                })
                        }) {
                            return None;
                        }
                        binding.place = observed.projected.clone();
                        for value in &values {
                            captured_predicates::retain_bindings(
                                self.program,
                                self.facts,
                                value,
                                operand.row(),
                                std::slice::from_ref(binding),
                                position,
                                &mut captures,
                            );
                        }
                    } else {
                        binding.place = self.contract_place_at_exit(binding.place.clone())?;
                        if let Some(operand) = completed {
                            captures.push(captured_predicates::CapturedBinding::observation(
                                position,
                                binding.place.clone(),
                                operand,
                            ));
                        }
                    }
                }
                let mut predicates = crate::published::facts::boolean_predicates::retained(
                    self.program,
                    &self.facts.operators,
                    &self.facts.semantic,
                    self.contexts,
                    ProgramPoint::Statement {
                        machine_symbol: self.machine.symbol,
                        state_symbol: self.exit.state_symbol,
                        statement_index: self.exit.statement_index,
                    },
                );
                if let Some(frames) = self.call_frames
                    && let Some(state) = caller
                {
                    predicates.extend(prover::call_guarantees::current_predicates(
                        self.program,
                        self.facts,
                        state,
                        prover::call_guarantees::GuaranteeUse::StatementEntry(
                            self.exit.statement_index + usize::from(completed.is_some()),
                        ),
                        self.contexts,
                        frames,
                    ));
                }
                if let Some(state) = caller {
                    let transported = captured_predicates::transport(
                        self.program,
                        self.facts,
                        state,
                        &mut required,
                        &captures,
                        &predicates,
                        self.call_frames,
                    );
                    if transported.proves_required {
                        return Some(true);
                    }
                    predicates.extend(transported.predicates);
                }
                let mut ordinary = |place: &CanonicalPlace| {
                    prover::boolean_value_at_place(
                        self.program,
                        &self.facts.operators,
                        &self.facts.semantic,
                        self.contexts,
                        ProgramPoint::Statement {
                            machine_symbol: self.machine.symbol,
                            state_symbol: self.exit.state_symbol,
                            statement_index: self.exit.statement_index,
                        },
                        place,
                    )
                    .map(ScalarValue::Boolean)
                    .or_else(|| {
                        crate::values::scalar_value_at_place(
                            self.program,
                            &self.facts.semantic,
                            self.contexts
                                .iter()
                                .map(|context| self.facts.semantic.contexts.get(*context)),
                            place,
                        )
                    })
                };
                crate::published::facts::boolean_predicates::truth(
                    &predicates,
                    &required,
                    &mut ordinary,
                )
                .or_else(|| {
                    prover::call_guarantees::proves_captured_predicates(
                        self.program,
                        &self.facts.semantic,
                        self.contexts,
                        &predicates,
                        &required,
                        ordinary,
                    )
                    .then_some(true)
                })
            }
        }
    }
    mod result_bounds {
        /*
            Postconditions compare immutable return bounds or live exit-storage snapshots.
            The shared bounds owner checks selected arithmetic, including overflow policy;
            no initializer or stale mutable read is replayed to reconstruct a value.
        */
        use super::super::super::return_values::completed_return_operand;
        use super::{
            ExitScalars, exit_return_expression, has_builtin_operators, is_result_reference,
        };
        use numerics::{arithmetic::ArithmeticDomain, bignum::BigInt};
        use typed_trees::{
            expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
            types::{PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode},
        };

        mod guarded_returns {
            /*
                Bounds on an exact selected return, including its taken-arm guard.

                Selected reads share a value only across a proven unchanged storage interval.
                Captured predicates retain their earlier selected producers; complete write
                frames preserve the collection and selector through the taken-arm return.
            */
            use super::super::is_scalar_roster_local;
            use super::{
                BigInt, ExitScalars, ExpressionHandle, exact_integer_carrier,
                exit_return_expression, interval_comparison, is_result_reference,
            };
            use crate::flow::state_statements;
            use checked_trees::{
                CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression,
                CheckedScalarExpressionRole,
            };
            use facts::IntegerRange;
            use symbols::SymbolHandle;
            use typed_trees::statement::{StatementNode, TransitionGuardNode};

            impl ExitScalars<'_, '_> {
                pub(super) fn guarded_return_bounds_comparison(
                    &self,
                    expression: ExpressionHandle,
                ) -> Option<bool> {
                    let binary = self.program.expression_table.binary_node(expression)?;
                    let (argument, result_on_left) =
                        if is_result_reference(self.program, self.machine, binary.left) {
                            (binary.right, true)
                        } else if is_result_reference(self.program, self.machine, binary.right) {
                            (binary.left, false)
                        } else {
                            return None;
                        };
                    let entry = self.program.machine_states(self.machine).first()?;
                    let primitive = exact_integer_carrier(self.program, entry.return_type)?;
                    let argument_type = validation::expression_result_type_reference(
                        self.program,
                        self.machine,
                        entry,
                        argument,
                    );
                    if argument_type.is_some_and(|reference| {
                        exact_integer_carrier(self.program, reference) != Some(primitive)
                    }) {
                        return None;
                    }
                    let endpoint = validation::closed_integer_range_bound(self.program, argument)?;
                    typed_trees::closed_numeric::land_integer(&endpoint, primitive)?;
                    let returned = exit_return_expression(self.program, self.exit);
                    if !self.return_expression_is_stable(returned) {
                        return None;
                    }
                    let (selected, symbols) = self.selected_return_expression(returned)?;
                    if selected.primitive_type() != Some(primitive) {
                        return None;
                    }
                    let mut bounds = validation::carrier_integer_range(primitive)?;
                    if let CheckedScalarExpression::IntegerLiteral { literal } = selected {
                        let value = literal.value_bignum()?;
                        typed_trees::closed_numeric::land_integer(&value, primitive)?;
                        bounds = IntegerRange {
                            minimum: value.clone(),
                            maximum: value,
                        };
                    }
                    let state = self.state()?;
                    if let Some(StatementNode::Transition(transition)) = self
                        .program
                        .statement_table
                        .statement_at(state, self.exit.statement_index)
                        && self.exit.transition_target.is_valid()
                        && self.exit.transition_target == transition.target
                        && let TransitionGuardNode::When(guard) = transition.guard
                        && self.return_expression_is_stable(guard)
                        && let Some((CheckedScalarExpression::Boolean(predicate), guard_symbols)) =
                            self.selected_scalar_expression(
                                u32::try_from(self.exit.statement_index).ok()?,
                                CheckedScalarExpressionRole::Guard,
                                guard,
                            )
                        && guard_symbols == symbols
                    {
                        self.refine_guarded_return(
                            predicate,
                            guard_symbols,
                            u32::try_from(self.exit.statement_index).ok()?,
                            selected,
                            &mut bounds,
                            &mut 4096,
                        );
                    }
                    let argument = IntegerRange {
                        minimum: endpoint.clone(),
                        maximum: endpoint,
                    };
                    if result_on_left {
                        interval_comparison(binary.operator, &bounds, &argument)
                    } else {
                        interval_comparison(binary.operator, &argument, &bounds)
                    }
                }
            }

            impl ExitScalars<'_, '_> {
                /*
                    A captured guard denotes its selected initializer, not a reread of the
                    source syntax. Earlier rosters are exact prefixes of the exit roster;
                    complete write frames keep every captured storage read current to exit.
                */
                fn captured_scalar_local(
                    &self,
                    position: usize,
                    symbols: &[SymbolHandle],
                    before_statement: u32,
                    remaining: &mut usize,
                ) -> Option<(&CheckedScalarExpression, &[SymbolHandle], u32)> {
                    *remaining = remaining.checked_sub(1)?;
                    let symbol = *symbols.get(position)?;
                    let state = self.state()?;
                    let statements = state_statements(self.program, state.statement_nodes);
                    let mut locals = statements
                        .iter()
                        .enumerate()
                        .filter_map(|(statement, node)| {
                            let local = node.as_local_data()?;
                            is_scalar_roster_local(self.program, local)
                                .then_some((statement, local))
                        })
                        .enumerate()
                        .filter(|(_, (_, local))| local.symbol == symbol);
                    let (binding_ordinal, (statement, local)) = locals.next()?;
                    if locals.next().is_some() || statement >= before_statement as usize {
                        return None;
                    }
                    let statement_ordinal = u32::try_from(statement).ok()?;
                    let role = CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: u32::try_from(binding_ordinal).ok()?,
                    };
                    let mut bindings = self
                        .facts
                        .values
                        .scalar_expressions
                        .source_bindings
                        .iter()
                        .filter(|(_, binding)| {
                            binding.state == self.exit.state_symbol && binding.destination == symbol
                        });
                    let (_, binding) = bindings.next()?;
                    if bindings.next().is_some()
                        || binding.statement_ordinal != statement_ordinal
                        || binding.role != role
                        || binding.expression != local.initial_value
                    {
                        return None;
                    }
                    let (selected, operands) = self.selected_scalar_expression(
                        statement_ordinal,
                        role,
                        local.initial_value,
                    )?;
                    if !symbols.starts_with(operands)
                        || selected.primitive_type()
                            != self.program.primitive_type_reference(local.type_reference)
                    {
                        return None;
                    }
                    let frames = self.call_frames?;
                    for node in statements.get(statement..self.exit.statement_index)? {
                        *remaining = remaining.checked_sub(1)?;
                        if !frames
                            .statement_value_write_frame(self.machine, node)
                            .into_complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                        {
                            return None;
                        }
                        let stable = match node {
                            StatementNode::LocalData(local) => {
                                self.return_expression_is_stable(local.initial_value)
                            }
                            StatementNode::Expression(expression) => {
                                self.return_expression_is_stable(*expression)
                            }
                            StatementNode::Assignment(assignment) => {
                                self.return_expression_is_stable(assignment.target)
                                    && self.return_expression_is_stable(assignment.value)
                                    && frames
                                        .assignment_write_frame(self.machine, node)
                                        .into_complete_paths()
                                        .is_some_and(|paths| paths.is_empty())
                            }
                            StatementNode::Call(call) => {
                                self.program
                                    .statement_table
                                    .expression_handles(call.arguments)
                                    .iter()
                                    .all(|argument| self.return_expression_is_stable(*argument))
                                    && frames
                                        .may_write_frame(self.machine, call)
                                        .into_complete_paths()
                                        .is_some_and(|paths| paths.is_empty())
                            }
                            StatementNode::Transition(_)
                            | StatementNode::Assembly(_)
                            | StatementNode::AssemblyFact(_) => false,
                        };
                        if !stable {
                            return None;
                        }
                    }
                    Some((selected, operands, statement_ordinal))
                }

                fn uncaptured_scalar<'selected>(
                    &'selected self,
                    mut expression: &'selected CheckedScalarExpression,
                    mut symbols: &'selected [SymbolHandle],
                    mut before_statement: u32,
                    remaining: &mut usize,
                ) -> Option<&'selected CheckedScalarExpression> {
                    while let CheckedScalarExpression::Local {
                        position,
                        primitive_type,
                    } = expression
                    {
                        let (selected, operands, statement) = self.captured_scalar_local(
                            *position,
                            symbols,
                            before_statement,
                            remaining,
                        )?;
                        if selected.primitive_type() != Some(*primitive_type) {
                            return None;
                        }
                        expression = selected;
                        symbols = operands;
                        before_statement = statement;
                    }
                    Some(expression)
                }

                fn refine_guarded_return(
                    &self,
                    predicate: &CheckedBooleanExpression,
                    symbols: &[SymbolHandle],
                    before_statement: u32,
                    returned: &CheckedScalarExpression,
                    bounds: &mut IntegerRange,
                    remaining: &mut usize,
                ) {
                    let Some(next) = remaining.checked_sub(1) else {
                        return;
                    };
                    *remaining = next;
                    match predicate {
                        CheckedBooleanExpression::Local { position } => {
                            let Some((
                                CheckedScalarExpression::Boolean(predicate),
                                operands,
                                statement,
                            )) = self.captured_scalar_local(
                                *position,
                                symbols,
                                before_statement,
                                remaining,
                            )
                            else {
                                return;
                            };
                            self.refine_guarded_return(
                                predicate, operands, statement, returned, bounds, remaining,
                            );
                        }
                        CheckedBooleanExpression::And { left, right } => {
                            self.refine_guarded_return(
                                left,
                                symbols,
                                before_statement,
                                returned,
                                bounds,
                                remaining,
                            );
                            self.refine_guarded_return(
                                right,
                                symbols,
                                before_statement,
                                returned,
                                bounds,
                                remaining,
                            );
                        }
                        CheckedBooleanExpression::Equal { left, right } => {
                            if matches!(left.as_ref(), CheckedBooleanExpression::Constant(true)) {
                                self.refine_guarded_return(
                                    right,
                                    symbols,
                                    before_statement,
                                    returned,
                                    bounds,
                                    remaining,
                                );
                            } else if matches!(
                                right.as_ref(),
                                CheckedBooleanExpression::Constant(true)
                            ) {
                                self.refine_guarded_return(
                                    left,
                                    symbols,
                                    before_statement,
                                    returned,
                                    bounds,
                                    remaining,
                                );
                            }
                        }
                        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                            let Some(left) =
                                self.uncaptured_scalar(left, symbols, before_statement, remaining)
                            else {
                                return;
                            };
                            let Some(right) =
                                self.uncaptured_scalar(right, symbols, before_statement, remaining)
                            else {
                                return;
                            };
                            let (endpoint, returned_on_left) = if left == returned {
                                (right, true)
                            } else if right == returned {
                                (left, false)
                            } else {
                                return;
                            };
                            let CheckedScalarExpression::IntegerLiteral { literal } = endpoint
                            else {
                                return;
                            };
                            let Some(value) = literal.value_bignum() else {
                                return;
                            };
                            let one = BigInt::from_i64(1);
                            match (kind, returned_on_left) {
                                (CheckedIntegerComparisonKind::Equal, _) => {
                                    bounds.minimum = bounds.minimum.clone().max(value.clone());
                                    bounds.maximum = bounds.maximum.clone().min(value);
                                }
                                (CheckedIntegerComparisonKind::LessThan, true) => {
                                    bounds.maximum = bounds.maximum.clone().min(value.sub(&one));
                                }
                                (CheckedIntegerComparisonKind::LessOrEqual, true) => {
                                    bounds.maximum = bounds.maximum.clone().min(value);
                                }
                                (CheckedIntegerComparisonKind::LessThan, false) => {
                                    bounds.minimum = bounds.minimum.clone().max(value.add(&one));
                                }
                                (CheckedIntegerComparisonKind::LessOrEqual, false) => {
                                    bounds.minimum = bounds.minimum.clone().max(value);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        mod reconstruction {
            /*
                Truncating reconstruction window for `(dividend / divisor) * divisor`.

                Independent intervals forget that the product and the dividend are the
                same value. For truncating division toward zero, a nonnegative dividend
                and a divisor of at least one leave a remainder in `0 .. divisor`, so

                `dividend - (divisor - 1) <= (dividend / divisor) * divisor <= dividend`.

                Comparisons against that window are decided by cancelling the shared
                linear terms, then reading only the residual against parameter bounds or
                an unwritten field's `where` facts.
                A negative dividend does not have this window: the product can sit
                strictly above the dividend. Wrapping or saturating carriers are not
                exact truncating division and stay on the interval path.
            */
            use crate::flow::state_statements;
            use arena::HandleSpan;
            use symbols::SymbolHandle;
            use typed_trees::state::State;
            use typed_trees::types::TypeReferenceHandle;
            use typed_trees::{
                expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
                statement::StatementNode,
            };
            use validation::call_target_state;
            use validation::state_parameter_integer_interval;
            use validation::stored_integer_bounds;

            use super::super::ExitScalars;

            const EXPANSION_LIMIT: usize = 8;

            enum LinearShape {
                Integer(i64),
                Name(SymbolHandle),
                Member(ExpressionHandle, SymbolHandle),
                Binary(ExpressionHandle, ExpressionHandle, BinaryOperator),
                Other,
            }

            #[derive(Clone, Copy)]
            enum ReconstructionRelation {
                AtLeast,
                AtMost,
                Above,
                Below,
                Equal,
            }

            /*
                A linear unknown. Two field spellings of the same root and member are one
                atom even when their expression handles differ, so cancellation still sees
                them. The retained place is only how the field's `where` interval is read.
            */
            #[derive(Clone, Copy)]
            enum LinearAtom {
                Parameter(SymbolHandle),
                Field {
                    root: SymbolHandle,
                    field: SymbolHandle,
                    place: ExpressionHandle,
                },
            }

            impl PartialEq for LinearAtom {
                fn eq(&self, other: &Self) -> bool {
                    match (self, other) {
                        (Self::Parameter(left), Self::Parameter(right)) => left == right,
                        (
                            Self::Field {
                                root: left_root,
                                field: left_field,
                                ..
                            },
                            Self::Field {
                                root: right_root,
                                field: right_field,
                                ..
                            },
                        ) => left_root == right_root && left_field == right_field,
                        _ => false,
                    }
                }
            }

            impl Eq for LinearAtom {}

            impl LinearAtom {
                fn sort_key(self) -> (u8, u32, u32, u32, u32) {
                    match self {
                        Self::Parameter(symbol) => {
                            (0, symbol.arena_index(), symbol.generation(), 0, 0)
                        }
                        Self::Field { root, field, .. } => (
                            1,
                            root.arena_index(),
                            root.generation(),
                            field.arena_index(),
                            field.generation(),
                        ),
                    }
                }
            }

            /*
                Coefficients are kept in arena order so form equality does not depend on
                the order the expression spelled its parameters.
            */
            #[derive(Clone, PartialEq, Eq)]
            struct LinearForm {
                terms: Vec<(LinearAtom, i64)>,
                constant: i64,
            }

            impl LinearForm {
                fn constant(constant: i64) -> Self {
                    Self {
                        terms: Vec::new(),
                        constant,
                    }
                }

                fn parameter(symbol: SymbolHandle) -> Self {
                    Self {
                        terms: vec![(LinearAtom::Parameter(symbol), 1)],
                        constant: 0,
                    }
                }

                fn field(root: SymbolHandle, field: SymbolHandle, place: ExpressionHandle) -> Self {
                    Self {
                        terms: vec![(LinearAtom::Field { root, field, place }, 1)],
                        constant: 0,
                    }
                }

                fn coefficient(&self, atom: LinearAtom) -> i64 {
                    self.terms
                        .iter()
                        .find(|(term, _)| *term == atom)
                        .map(|(_, coefficient)| *coefficient)
                        .unwrap_or(0)
                }

                fn set_coefficient(&mut self, atom: LinearAtom, coefficient: i64) {
                    if let Some(existing) = self.terms.iter_mut().find(|(term, _)| *term == atom) {
                        existing.1 = coefficient;
                        if let LinearAtom::Field { place, .. } = atom
                            && let LinearAtom::Field {
                                place: existing_place,
                                ..
                            } = &mut existing.0
                        {
                            *existing_place = place;
                        }
                    } else if coefficient != 0 {
                        self.terms.push((atom, coefficient));
                    }
                    self.terms.retain(|(_, coefficient)| *coefficient != 0);
                    self.terms.sort_by_key(|(atom, _)| atom.sort_key());
                }

                fn add(&self, other: &Self) -> Option<Self> {
                    let mut sum = self.clone();
                    sum.constant = self.constant.checked_add(other.constant)?;
                    for (atom, coefficient) in &other.terms {
                        let combined = self.coefficient(*atom).checked_add(*coefficient)?;
                        sum.set_coefficient(*atom, combined);
                    }
                    Some(sum)
                }

                fn subtract(&self, other: &Self) -> Option<Self> {
                    self.add(&other.scale(-1)?)
                }

                fn scale(&self, factor: i64) -> Option<Self> {
                    let mut terms = Vec::new();
                    for (atom, coefficient) in &self.terms {
                        let scaled = coefficient.checked_mul(factor)?;
                        if scaled != 0 {
                            terms.push((*atom, scaled));
                        }
                    }
                    Some(Self {
                        terms,
                        constant: self.constant.checked_mul(factor)?,
                    })
                }

                /*
                    Smallest value of this form over the exit state's immutable parameters.
                    A missing endpoint that the coefficient's sign needs is not a proof.
                */
                fn minimum(&self, scalars: &ExitScalars<'_, '_>, state: &State) -> Option<i64> {
                    let mut minimum = self.constant;
                    for (atom, coefficient) in &self.terms {
                        if *coefficient == 0 {
                            continue;
                        }
                        let (lower, upper) = match *atom {
                            LinearAtom::Parameter(symbol) => state_parameter_integer_interval(
                                scalars.program,
                                scalars.machine,
                                state,
                                symbol,
                            )?,
                            LinearAtom::Field { place, .. } => stored_integer_bounds(
                                scalars.program,
                                scalars.machine,
                                state,
                                place,
                            )?,
                        };
                        let endpoint = if *coefficient > 0 { lower } else { upper };
                        let contribution = coefficient.checked_mul(endpoint?)?;
                        minimum = minimum.checked_add(contribution)?;
                    }
                    Some(minimum)
                }
            }

            impl ExitScalars<'_, '_> {
                pub(super) fn truncating_reconstruction_proves(
                    &self,
                    operator: BinaryOperator,
                    returned: ExpressionHandle,
                    bound: ExpressionHandle,
                    result_on_left: bool,
                ) -> bool {
                    let Some(relation) = reconstruction_relation(operator, result_on_left) else {
                        return false;
                    };
                    let Some(state) = self.state() else {
                        return false;
                    };
                    let Some((dividend, divisor)) =
                        self.reconstruction_forms(returned, state, &[], 0)
                    else {
                        return false;
                    };
                    let dividend_floor = dividend.minimum(self, state);
                    let divisor_floor = divisor.minimum(self, state);
                    if dividend_floor.is_none_or(|floor| floor < 0)
                        || divisor_floor.is_none_or(|floor| floor < 1)
                    {
                        return false;
                    }
                    let Some(bound) = self.linear_form(bound, state, &[], &mut Vec::new(), 0)
                    else {
                        return false;
                    };
                    let Some(window_floor) = dividend
                        .subtract(&divisor)
                        .and_then(|shifted| shifted.add(&LinearForm::constant(1)))
                    else {
                        return false;
                    };
                    window_entails(relation, &window_floor, &dividend, &bound, self, state)
                }

                fn reconstruction_forms(
                    &self,
                    expression: ExpressionHandle,
                    state: &State,
                    substitution: &[(SymbolHandle, LinearForm)],
                    depth: usize,
                ) -> Option<(LinearForm, LinearForm)> {
                    if depth > EXPANSION_LIMIT {
                        return None;
                    }
                    let peeled = self
                        .program
                        .expression_table
                        .peeled_expression_handle(expression);
                    let named = match self.program.expression_table.expression(peeled) {
                        ExpressionNode::Name(path) => path.resolved_symbol(),
                        _ => None,
                    };
                    if let Some(symbol) = named
                        && substitution
                            .iter()
                            .all(|(parameter, _)| *parameter != symbol)
                        && let Some(initializer) = self.local_initializer(symbol, state)
                    {
                        return self.reconstruction_forms(
                            initializer,
                            state,
                            substitution,
                            depth + 1,
                        );
                    }
                    let call = match self.program.expression_table.expression(peeled) {
                        ExpressionNode::Call(call) => Some(CopiedCall {
                            target_symbol: call.target_symbol,
                            receiver: call.receiver,
                            arguments: call.arguments,
                            blocked: call.quotient_operation.is_some()
                                || call.layout_query.is_some()
                                || call.private_layout_operation.is_some()
                                || !call.machine_arguments.is_empty()
                                || !call.evidence_arguments.is_empty(),
                        }),
                        _ => None,
                    };
                    if let Some(call) = call {
                        return self.call_reconstruction(call, state, substitution, depth);
                    }
                    let (left, right, operator) = {
                        let binary = self.program.expression_table.binary_node(peeled)?;
                        (binary.left, binary.right, binary.operator)
                    };
                    if operator != BinaryOperator::Multiply {
                        return None;
                    }
                    self.matched_reconstruction(left, right, state, substitution, depth)
                        .or_else(|| {
                            self.matched_reconstruction(right, left, state, substitution, depth)
                        })
                }

                /* `dividend_over_divisor` is one multiply operand and `multiplier` the other. */
                fn matched_reconstruction(
                    &self,
                    dividend_over_divisor: ExpressionHandle,
                    multiplier: ExpressionHandle,
                    state: &State,
                    substitution: &[(SymbolHandle, LinearForm)],
                    depth: usize,
                ) -> Option<(LinearForm, LinearForm)> {
                    let divided = self
                        .program
                        .expression_table
                        .peeled_expression_handle(dividend_over_divisor);
                    let (dividend, divisor) = {
                        let binary = self.program.expression_table.binary_node(divided)?;
                        if binary.operator != BinaryOperator::Divide {
                            return None;
                        }
                        (binary.left, binary.right)
                    };
                    let mut expanding = Vec::new();
                    let divisor_form =
                        self.linear_form(divisor, state, substitution, &mut expanding, depth)?;
                    expanding.clear();
                    let multiplier_form =
                        self.linear_form(multiplier, state, substitution, &mut expanding, depth)?;
                    if divisor_form != multiplier_form {
                        return None;
                    }
                    expanding.clear();
                    let dividend_form =
                        self.linear_form(dividend, state, substitution, &mut expanding, depth)?;
                    Some((dividend_form, divisor_form))
                }

                fn call_reconstruction(
                    &self,
                    call: CopiedCall,
                    state: &State,
                    substitution: &[(SymbolHandle, LinearForm)],
                    depth: usize,
                ) -> Option<(LinearForm, LinearForm)> {
                    if call.receiver.is_valid() || call.blocked {
                        return None;
                    }
                    let entry = call_target_state(self.program, call.target_symbol)?;
                    if !exact_return(self, entry.return_type) {
                        return None;
                    }
                    let parameters = self.program.state_parameters(entry);
                    if parameters
                        .iter()
                        .any(|parameter| parameter.is_const || parameter.is_mutable)
                    {
                        return None;
                    }
                    let arguments = self
                        .program
                        .expression_table
                        .expression_handles(call.arguments);
                    if parameters.len() != arguments.len() {
                        return None;
                    }
                    let mut callee_substitution = Vec::with_capacity(parameters.len());
                    for (parameter, argument) in parameters.iter().zip(arguments) {
                        let mut expanding = Vec::new();
                        let form = self.linear_form(
                            *argument,
                            state,
                            substitution,
                            &mut expanding,
                            depth,
                        )?;
                        callee_substitution.push((parameter.symbol, form));
                    }
                    let returned = crate::flow::state_returned_value(self.program, entry)?;
                    self.reconstruction_forms(returned, entry, &callee_substitution, depth + 1)
                }

                fn linear_form(
                    &self,
                    expression: ExpressionHandle,
                    state: &State,
                    substitution: &[(SymbolHandle, LinearForm)],
                    expanding: &mut Vec<SymbolHandle>,
                    depth: usize,
                ) -> Option<LinearForm> {
                    if depth > EXPANSION_LIMIT {
                        return None;
                    }
                    let peeled = self
                        .program
                        .expression_table
                        .peeled_expression_handle(expression);
                    let shape = match self.program.expression_table.expression(peeled) {
                        ExpressionNode::Integer(literal) => {
                            LinearShape::Integer(literal.value_i64()?)
                        }
                        ExpressionNode::Name(path) => LinearShape::Name(path.resolved_symbol()?),
                        ExpressionNode::Member(member) => {
                            LinearShape::Member(member.receiver, member.member_symbol)
                        }
                        ExpressionNode::Binary(binary) => {
                            LinearShape::Binary(binary.left, binary.right, binary.operator)
                        }
                        _ => LinearShape::Other,
                    };
                    match shape {
                        LinearShape::Integer(value) => Some(LinearForm::constant(value)),
                        LinearShape::Name(symbol) => {
                            if let Some((_, form)) = substitution
                                .iter()
                                .find(|(parameter, _)| *parameter == symbol)
                            {
                                return Some(form.clone());
                            }
                            if let Some(parameter) = self
                                .program
                                .state_parameters(state)
                                .iter()
                                .find(|parameter| parameter.symbol == symbol)
                            {
                                if parameter.is_const || parameter.is_mutable {
                                    return None;
                                }
                                if !exact_return(self, parameter.type_reference) {
                                    return None;
                                }
                                return Some(LinearForm::parameter(symbol));
                            }
                            self.local_form(symbol, state, substitution, expanding, depth)
                        }
                        LinearShape::Member(receiver, field) => {
                            self.field_form(peeled, receiver, field, state)
                        }
                        LinearShape::Binary(left, right, operator) => {
                            let left =
                                self.linear_form(left, state, substitution, expanding, depth + 1)?;
                            let right =
                                self.linear_form(right, state, substitution, expanding, depth + 1)?;
                            match operator {
                                BinaryOperator::Add => left.add(&right),
                                BinaryOperator::Subtract => left.subtract(&right),
                                BinaryOperator::Multiply => scale_product(&left, &right),
                                _ => None,
                            }
                        }
                        LinearShape::Other => None,
                    }
                }

                /*
                    One field of a parameter, unread by this state. Its `where` facts are
                    what every store preserves, so they bound the field here without
                    replaying a mutable arrival.
                */
                fn field_form(
                    &self,
                    place: ExpressionHandle,
                    receiver: ExpressionHandle,
                    field: SymbolHandle,
                    state: &State,
                ) -> Option<LinearForm> {
                    let root = self
                        .program
                        .expression_table
                        .name_path_node(receiver)
                        .and_then(|path| path.resolved_symbol())?;
                    let parameter = self
                        .program
                        .state_parameters(state)
                        .iter()
                        .find(|parameter| parameter.symbol == root)?;
                    if parameter.is_const || self.field_is_assigned(state, root, field) {
                        return None;
                    }
                    let (lower, upper) =
                        stored_integer_bounds(self.program, self.machine, state, place)?;
                    if lower.is_none() && upper.is_none() {
                        return None;
                    }
                    Some(LinearForm::field(root, field, place))
                }

                fn field_is_assigned(
                    &self,
                    state: &State,
                    root: SymbolHandle,
                    field: SymbolHandle,
                ) -> bool {
                    let statements = state_statements(self.program, state.statement_nodes);
                    statements.iter().any(|statement| {
                        let StatementNode::Assignment(assignment) = statement else {
                            return false;
                        };
                        let target = self
                            .program
                            .expression_table
                            .peeled_expression_handle(assignment.target);
                        let Some((member_field, member_receiver)) =
                            (match self.program.expression_table.expression(target) {
                                ExpressionNode::Member(member) => {
                                    Some((member.member_symbol, member.receiver))
                                }
                                _ => None,
                            })
                        else {
                            return false;
                        };
                        member_field == field
                            && self
                                .program
                                .expression_table
                                .name_path_node(member_receiver)
                                .and_then(|path| path.resolved_symbol())
                                == Some(root)
                    })
                }

                fn local_form(
                    &self,
                    symbol: SymbolHandle,
                    state: &State,
                    substitution: &[(SymbolHandle, LinearForm)],
                    expanding: &mut Vec<SymbolHandle>,
                    depth: usize,
                ) -> Option<LinearForm> {
                    if expanding.contains(&symbol) {
                        return None;
                    }
                    let initializer = self.local_initializer(symbol, state)?;
                    expanding.push(symbol);
                    let form =
                        self.linear_form(initializer, state, substitution, expanding, depth + 1);
                    expanding.pop();
                    form
                }

                fn local_initializer(
                    &self,
                    symbol: SymbolHandle,
                    state: &State,
                ) -> Option<ExpressionHandle> {
                    let statements = state_statements(self.program, state.statement_nodes);
                    let mut initializer = None;
                    for statement in statements {
                        match statement {
                            StatementNode::LocalData(local)
                                if local.symbol == symbol
                                    && !local.is_mutable
                                    && local.ordinary_value_initializer().is_some() =>
                            {
                                if initializer.is_some() {
                                    return None;
                                }
                                initializer = local.ordinary_value_initializer();
                            }
                            StatementNode::Assignment(assignment) => {
                                let target = self
                                    .program
                                    .expression_table
                                    .peeled_expression_handle(assignment.target);
                                if self
                                    .program
                                    .expression_table
                                    .name_path_node(target)
                                    .and_then(|path| path.resolved_symbol())
                                    == Some(symbol)
                                {
                                    return None;
                                }
                            }
                            _ => {}
                        }
                    }
                    initializer
                }
            }

            fn scale_product(left: &LinearForm, right: &LinearForm) -> Option<LinearForm> {
                if let Some(factor) = right.constant_factor() {
                    return left.scale(factor);
                }
                if let Some(factor) = left.constant_factor() {
                    return right.scale(factor);
                }
                None
            }

            impl LinearForm {
                fn constant_factor(&self) -> Option<i64> {
                    self.terms.is_empty().then_some(self.constant)
                }
            }

            fn reconstruction_relation(
                operator: BinaryOperator,
                result_on_left: bool,
            ) -> Option<ReconstructionRelation> {
                match (operator, result_on_left) {
                    (BinaryOperator::GreaterOrEqual, true)
                    | (BinaryOperator::LessOrEqual, false) => Some(ReconstructionRelation::AtLeast),
                    (BinaryOperator::LessOrEqual, true)
                    | (BinaryOperator::GreaterOrEqual, false) => {
                        Some(ReconstructionRelation::AtMost)
                    }
                    (BinaryOperator::Greater, true) | (BinaryOperator::Less, false) => {
                        Some(ReconstructionRelation::Above)
                    }
                    (BinaryOperator::Less, true) | (BinaryOperator::Greater, false) => {
                        Some(ReconstructionRelation::Below)
                    }
                    (BinaryOperator::Equal, _) => Some(ReconstructionRelation::Equal),
                    _ => None,
                }
            }

            fn window_entails(
                relation: ReconstructionRelation,
                floor: &LinearForm,
                ceiling: &LinearForm,
                bound: &LinearForm,
                scalars: &ExitScalars<'_, '_>,
                state: &State,
            ) -> bool {
                let holds = |residual: Option<LinearForm>, slack: i64| -> bool {
                    residual
                        .and_then(|form| form.minimum(scalars, state))
                        .is_some_and(|minimum| minimum >= slack)
                };
                match relation {
                    ReconstructionRelation::AtLeast => holds(floor.subtract(bound), 0),
                    ReconstructionRelation::AtMost => holds(bound.subtract(ceiling), 0),
                    ReconstructionRelation::Above => holds(floor.subtract(bound), 1),
                    ReconstructionRelation::Below => holds(bound.subtract(ceiling), 1),
                    ReconstructionRelation::Equal => floor == ceiling && floor == bound,
                }
            }

            struct CopiedCall {
                target_symbol: SymbolHandle,
                receiver: ExpressionHandle,
                arguments: HandleSpan<ExpressionHandle>,
                blocked: bool,
            }

            fn exact_return(scalars: &ExitScalars<'_, '_>, reference: TypeReferenceHandle) -> bool {
                super::exact_integer_carrier(scalars.program, reference).is_some()
            }
        }

        impl ExitScalars<'_, '_> {
            pub(super) fn proves_bounds(&self, expression: ExpressionHandle) -> bool {
                has_builtin_operators(self.program, &self.facts.operators, expression)
                    && validation::has_builtin_bound_expression_meaning(
                        self.program,
                        self.machine,
                        self.program.machine_states(self.machine).first(),
                        expression,
                    )
                    && (self.result_bounds_comparison(expression) == Some(true)
                        || self.guarded_return_bounds_comparison(expression) == Some(true)
                        || self.current_bounds_comparison(expression) == Some(true))
            }

            /*
                Current storage snapshots can bound a postcondition without identifying
                one exact scalar value. Retain the selected arithmetic policy, including
                wrapping, rather than interpreting its syntax as unbounded arithmetic.
            */
            fn current_bounds_comparison(&self, expression: ExpressionHandle) -> Option<bool> {
                let binary = self.program.expression_table.binary_node(expression)?;
                if !matches!(
                    binary.operator,
                    BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                ) {
                    return None;
                }
                let entry = self.program.machine_states(self.machine).first()?;
                let parameters = self.program.state_parameters(entry);
                if parameters.iter().any(|parameter| {
                    parameter.is_const
                        || (parameter.is_mutable
                            && !crate::values::exclusive_reference(
                                self.program,
                                parameter.type_reference,
                            ))
                }) || self.contract_origins().any(|origin| {
                    !origin.state_parameter.is_valid()
                        || origin.entry_parameter != origin.state_parameter
                }) {
                    return None;
                }
                let primitive = [binary.left, binary.right]
                    .into_iter()
                    .find_map(|operand| {
                        validation::expression_result_type_reference(
                            self.program,
                            self.machine,
                            entry,
                            operand,
                        )
                        .and_then(|reference| self.program.primitive_type_reference(reference))
                        .filter(|primitive| validation::carrier_integer_range(*primitive).is_some())
                    })?;
                let lower = |operand| {
                    crate::values::lower_unit_scalar_argument(
                        self.program,
                        &self.facts.operators,
                        entry,
                        0,
                        operand,
                        primitive,
                    )
                };
                let left = lower(binary.left)?;
                let right = lower(binary.right)?;
                let symbols = parameters
                    .iter()
                    .filter(|parameter| {
                        crate::values::occupies_scalar_position(self.program, parameter)
                    })
                    .map(|parameter| parameter.symbol)
                    .collect::<Vec<_>>();
                let mut values = crate::values::LivePlaceIntervals {
                    program: self.program,
                    semantic: &self.facts.semantic,
                    contexts: self.contexts,
                    parameters,
                    symbols: &symbols,
                    state: entry.symbol,
                    contents: crate::values::CurrentIntegerBounds::new(
                        self.program,
                        &self.facts.operators,
                        &self.facts.semantic,
                        self.contexts,
                        self.exit.state_symbol,
                        facts::ProgramPoint::Exit {
                            machine_symbol: self.exit.machine_symbol,
                            state_symbol: self.exit.state_symbol,
                            statement_index: self.exit.statement_index,
                            transition_target: self.exit.transition_target,
                        },
                    ),
                };
                let left = validation::selected_scalar_interval(&left, &mut values)?;
                let right = validation::selected_scalar_interval(&right, &mut values)?;
                interval_comparison(binary.operator, &left, &right)
            }

            /*
                None means overlapping intervals or unsupported evidence, never false.
                Negation may consume only a proved or refuted comparison.
            */
            fn result_bounds_comparison(&self, expression: ExpressionHandle) -> Option<bool> {
                if let ExpressionNode::Unary(unary) =
                    self.program.expression_table.expression(expression)
                    && unary.operator == UnaryOperator::LogicalNot
                {
                    return self
                        .result_bounds_comparison(unary.operand)
                        .map(|value| !value);
                }
                let binary = self.program.expression_table.binary_node(expression)?;
                let spelling = validation::comparison_operator_spelling(binary.operator)?;
                let (result_operand, argument, result_on_left) =
                    if let Some(operand) = self.result_bound_operand(binary.left) {
                        (operand, binary.right, true)
                    } else {
                        let operand = self.result_bound_operand(binary.right)?;
                        (operand, binary.left, false)
                    };
                let (result_expression, result_type) = result_operand;
                let entry = self.program.machine_states(self.machine).first()?;
                let primitive = exact_integer_carrier(self.program, result_type)?;
                let state = self.state()?;
                let returned = exit_return_expression(self.program, self.exit);
                if self.truncating_reconstruction_proves(
                    binary.operator,
                    result_expression,
                    argument,
                    result_on_left,
                ) {
                    return Some(true);
                }
                let result_observation = if result_on_left {
                    binary.left
                } else {
                    binary.right
                };
                let completed_subject = completed_return_operand(
                    self.program,
                    self.facts,
                    self.exit,
                )
                .and_then(|operand| {
                    let projection =
                        validation::reserved_result_place(self.program, result_observation)?;
                    Some(validation::CanonicalPlace {
                        root: facts::PlaceRoot::Expression(operand.expression),
                        segments: projection.segments,
                    })
                });
                let result_bounds = completed_subject.and_then(|subject| {
                    let published = crate::values::integer_bounds_at_place(
                        self.program,
                        &self.facts.semantic,
                        self.contexts
                            .iter()
                            .map(|context| self.facts.semantic.contexts.get(*context)),
                        &subject,
                    );
                    let enclosure = match subject.root {
                        facts::PlaceRoot::Expression(operand) if subject.segments.is_empty() => {
                            self.program
                                .expression_interval(operand)
                                .and_then(|interval| interval.carrier_range(primitive))
                        }
                        _ => None,
                    };
                    match (published, enclosure) {
                        (Some(published), Some(enclosure)) => {
                            let intersection = facts::IntegerRange {
                                minimum: published.minimum.max(enclosure.minimum),
                                maximum: published.maximum.min(enclosure.maximum),
                            };
                            (intersection.minimum <= intersection.maximum).then_some(intersection)
                        }
                        (published, enclosure) => published.or(enclosure),
                    }
                });
                if let Some(result_bounds) = result_bounds
                    && let Some(value) =
                        validation::closed_integer_range_bound(self.program, argument)
                {
                    let argument_type = validation::expression_result_type_reference(
                        self.program,
                        self.machine,
                        entry,
                        argument,
                    );
                    if argument_type.is_none_or(|reference| {
                        exact_integer_carrier(self.program, reference) == Some(primitive)
                    }) && typed_trees::closed_numeric::land_integer(&value, primitive).is_some()
                    {
                        let argument_bounds = facts::IntegerRange {
                            minimum: value.clone(),
                            maximum: value,
                        };
                        let (left, right, types) = if result_on_left {
                            (
                                &result_bounds,
                                &argument_bounds,
                                [Some(result_type), argument_type],
                            )
                        } else {
                            (
                                &argument_bounds,
                                &result_bounds,
                                [argument_type, Some(result_type)],
                            )
                        };
                        if typed_trees::operator::has_builtin_spelled_expression_meaning(
                            self.program,
                            self.machine.symbol,
                            expression,
                            spelling,
                            &types,
                        ) && let Some(verdict) =
                            interval_comparison(binary.operator, left, right)
                        {
                            return Some(verdict);
                        }
                    }
                }
                if !self.return_expression_is_stable(returned) {
                    return None;
                }
                let standing = validation::immutable_integer_expression_bounds(
                    self.program,
                    self.machine,
                    state,
                    result_expression,
                );
                let observed = validation::occurrence_integer_expression_bounds(
                    self.program,
                    self.machine.symbol,
                    state.symbol,
                    self.exit.statement_index,
                    result_expression,
                );
                let result_bounds = match (standing, observed) {
                    (Some(standing), Some(observed)) => {
                        (standing.0.max(observed.0), standing.1.min(observed.1))
                    }
                    (standing, observed) => standing.or(observed)?,
                };
                let argument_bounds = validation::immutable_integer_expression_bounds(
                    self.program,
                    self.machine,
                    entry,
                    argument,
                )?;
                let argument_type = validation::expression_result_type_reference(
                    self.program,
                    self.machine,
                    entry,
                    argument,
                );
                if let Some(reference) = argument_type
                    && exact_integer_carrier(self.program, reference) != Some(primitive)
                {
                    return None;
                }
                for endpoint in [
                    result_bounds.0,
                    result_bounds.1,
                    argument_bounds.0,
                    argument_bounds.1,
                ] {
                    typed_trees::closed_numeric::land_integer(
                        &BigInt::from_i64(endpoint),
                        primitive,
                    )?;
                }
                let (left, right, types) = if result_on_left {
                    (
                        result_bounds,
                        argument_bounds,
                        [Some(result_type), argument_type],
                    )
                } else {
                    (
                        argument_bounds,
                        result_bounds,
                        [argument_type, Some(result_type)],
                    )
                };
                if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    self.program,
                    self.machine.symbol,
                    expression,
                    spelling,
                    &types,
                ) {
                    return None;
                }
                interval_comparison(
                    binary.operator,
                    &facts::IntegerRange {
                        minimum: BigInt::from_i64(left.0),
                        maximum: BigInt::from_i64(left.1),
                    },
                    &facts::IntegerRange {
                        minimum: BigInt::from_i64(right.0),
                        maximum: BigInt::from_i64(right.1),
                    },
                )
            }

            /*
                Compare a complete evaluated return enclosure with an independently
                closed endpoint. This is mathematical interval inclusion, not executable
                subtraction; no intermediate machine-width overflow is introduced.
            */
            pub(super) fn result_range_condition(
                &self,
                expression: ExpressionHandle,
            ) -> Option<checked_trees::IntegerRangeObligation> {
                let mut stage = "comparison";
                let condition = (|| {
                    let binary = self.program.expression_table.binary_node(expression)?;
                    stage = "returned operand";
                    let (returned, result_type, bound, operator) =
                        if let Some((returned, result_type)) =
                            self.result_bound_operand(binary.left)
                        {
                            (returned, result_type, binary.right, binary.operator)
                        } else {
                            let (returned, result_type) =
                                self.result_bound_operand(binary.right)?;
                            (
                                returned,
                                result_type,
                                binary.left,
                                binary.operator.mirrored(),
                            )
                        };
                    stage = "result carrier";
                    let primitive = exact_integer_carrier(self.program, result_type)?;
                    let carrier = validation::carrier_integer_range(primitive)?;
                    let entry = self.program.machine_states(self.machine).first()?;
                    stage = "endpoint type";
                    let bound_type = validation::expression_result_type_reference(
                        self.program,
                        self.machine,
                        entry,
                        bound,
                    )?;
                    stage = "endpoint carrier";
                    let bound_carrier = exact_integer_carrier(self.program, bound_type)?;
                    if bound_carrier != primitive {
                        return None;
                    }
                    stage = "completed return or replay stability";
                    let completed = completed_return_operand(self.program, self.facts, self.exit)
                        .is_some_and(|operand| operand.expression == returned);
                    if !completed
                        && !self.return_expression_is_stable(exit_return_expression(
                            self.program,
                            self.exit,
                        ))
                    {
                        return None;
                    }
                    stage = "endpoint value";
                    let endpoint = if self
                        .program
                        .constant_recipe_for_contract_expression(bound)
                        .is_valid()
                    {
                        checked_trees::IntegerRangeRecipe::observation(bound, self.machine.symbol)
                    } else {
                        let value = validation::closed_integer_range_bound(self.program, bound)?;
                        typed_trees::closed_numeric::land_integer(&value, primitive)?;
                        checked_trees::IntegerRangeRecipe::bounds(value.clone(), value)
                    };
                    let (spelling, types) = if self.result_bound_operand(binary.left).is_some() {
                        (
                            validation::comparison_operator_spelling(binary.operator)?,
                            [Some(result_type), Some(bound_type)],
                        )
                    } else {
                        (
                            validation::comparison_operator_spelling(binary.operator)?,
                            [Some(bound_type), Some(result_type)],
                        )
                    };
                    stage = "comparison meaning";
                    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        self.program,
                        self.machine.symbol,
                        expression,
                        spelling,
                        &types,
                    ) {
                        return None;
                    }
                    stage = "return occurrence";
                    let state = self.state()?;
                    let observed = validation::occurrence_integer_expression_recipe(
                        self.program,
                        self.machine.symbol,
                        state.symbol,
                        self.exit.statement_index,
                        returned,
                    )?;
                    stage = "difference recipe";
                    let difference = observed.binary(BinaryOperator::Subtract, &endpoint)?;
                    stage = "deferred observation";
                    if !difference.is_target_dependent() {
                        return None;
                    }
                    let span = carrier.maximum.sub(&carrier.minimum);
                    let (minimum, maximum) = match operator {
                        BinaryOperator::LessOrEqual => (span.negate(), BigInt::from_i64(0)),
                        BinaryOperator::Less => (span.negate(), BigInt::from_i64(-1)),
                        BinaryOperator::GreaterOrEqual => (BigInt::from_i64(0), span),
                        BinaryOperator::Greater => (BigInt::from_i64(1), span),
                        BinaryOperator::Equal => (BigInt::from_i64(0), BigInt::from_i64(0)),
                        _ => return None,
                    };
                    Some(checked_trees::IntegerRangeObligation::retain(
                        expression,
                        self.machine.symbol,
                        semantic_vocabulary::ScalarConditionRole::Result,
                        difference,
                        minimum,
                        maximum,
                        "ensures return comparison".to_owned(),
                    ))
                })();
                if condition.is_none() && ::validation::trace_flags::struct_trace() {
                    eprintln!(
                        "STRUCT return-range refused machine={} expression={expression:?} stage={stage}",
                        self.machine.name,
                    );
                }
                condition
            }

            /*
                The expression and declared type whose immutable bounds a result
                operand carries: the whole return for a bare `result` reference, or
                the constructor-selected member expression for a projected
                `result.field` operand. Longer member paths stay unproven.
            */
            fn result_bound_operand(
                &self,
                expression: ExpressionHandle,
            ) -> Option<(ExpressionHandle, TypeReferenceHandle)> {
                let entry = self.program.machine_states(self.machine).first()?;
                if is_result_reference(self.program, self.machine, expression) {
                    return Some((
                        exit_return_expression(self.program, self.exit),
                        entry.return_type,
                    ));
                }
                let (selected, remaining) = self.result_projection(expression)?;
                if !remaining.is_empty() {
                    return None;
                }
                let place = validation::reserved_result_place(self.program, expression)?;
                Some((selected, place.type_reference))
            }
        }

        fn interval_comparison(
            operator: BinaryOperator,
            left: &facts::IntegerRange,
            right: &facts::IntegerRange,
        ) -> Option<bool> {
            if left.minimum > left.maximum || right.minimum > right.maximum {
                return None;
            }
            let equal_points = left.minimum == left.maximum
                && right.minimum == right.maximum
                && left.minimum == right.minimum;
            let disjoint = left.maximum < right.minimum || right.maximum < left.minimum;
            let (proved, refuted) = match operator {
                BinaryOperator::Equal => (equal_points, disjoint),
                BinaryOperator::NotEqual => (disjoint, equal_points),
                BinaryOperator::Less => {
                    (left.maximum < right.minimum, left.minimum >= right.maximum)
                }
                BinaryOperator::LessOrEqual => {
                    (left.maximum <= right.minimum, left.minimum > right.maximum)
                }
                BinaryOperator::Greater => {
                    (left.minimum > right.maximum, left.maximum <= right.minimum)
                }
                BinaryOperator::GreaterOrEqual => {
                    (left.minimum >= right.maximum, left.maximum < right.minimum)
                }
                _ => return None,
            };
            if proved {
                Some(true)
            } else if refuted {
                Some(false)
            } else {
                None
            }
        }

        fn exact_integer_carrier(
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> Option<PrimitiveType> {
            let mut carrier = reference;
            for _ in 0..128 {
                match program.type_reference_table.type_reference(carrier) {
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        if program
                            .type_reference_table
                            .constraints(*constraints)
                            .iter()
                            .any(|constraint| {
                                matches!(constraint, TypeConstraintNode::ArithmeticDomain(domain)
                                if *domain != ArithmeticDomain::Exact)
                            })
                        {
                            return None;
                        }
                        carrier = *base_type;
                    }
                    TypeReferenceNode::Named { symbol, name } => {
                        let atom = program.symbols.builtin_type_atom(*symbol)?;
                        if name.as_str() != atom.symbol_name() {
                            return None;
                        }
                        return PrimitiveType::from_builtin_atom(atom);
                    }
                    _ => return None,
                }
            }
            None
        }
    }
    mod result_fields {
        /*
            Result projections use the same typed constructor projection as field-domain
            checking. Scalar congruence then compares selected computations in one exact
            place namespace, retaining carrier and overflow policy. It does not replay
            local initializers or calls. The whole return must preserve its operands:
            a later sibling field may otherwise invalidate an earlier field's read.

            A slice stored in a returned record publishes that field's closed extent.
            The extent is the initializer's geometry, not a second length word on the
            record. A match publishes one extent only when every arm agrees. Arms with
            different extents still satisfy a relation when each arm does, and they
            never publish one arm's extent as the other's. A value arm measures its
            window after the pattern integer replaces the matched subject, so
            `match which { 0 -> Route { events: &route[0..which + 1] } }` has length 1
            in that arm. A binding retains the selected arm and ordered pattern prefix:
            earlier failed patterns or constructor fields may change the live subject.
            The shared occurrence query admits the equality only before such effects;
            later writes cannot invalidate a window already captured.
        */
        use super::super::super::prover::captured_scalar_at_path;
        use super::super::super::return_values::completed_return_operand;
        use super::super::super::return_values::exit_return_call;
        use numerics::bignum::BigInt;
        use symbols::SymbolHandle;
        use typed_trees::data::DataField;
        use typed_trees::statement::StatementNode;
        use typed_trees::statement::TransitionTargetNode;
        use validation::expression_traversal::collect_statement_expression_trees;
        use validation::expression_traversal::expression_has_inert_prefix;
        use validation::field_domains::readable_type_reference;
        use validation::{
            CanonicalPlace, closed_collection_length, closed_collection_length_under_matches,
            collection_length_receiver, find_state_on_machine,
            has_builtin_bound_expression_meaning, project_type_reference_from_segments,
            reserved_result_owner, reserved_result_place,
        };

        mod descriptor_lengths {
            /*
                Result metadata follows the complete body-derived descriptor association.
                A stored slice descriptor's length is not a physical field named `len`,
                and its backing address does not determine that length. Only exact copies
                from immutable owned slots can share an invocation observation here. Windows,
                presentations, changed slots and divergent source identities stay unavailable.
                The finite return equations establish correspondence without importing the
                recursive guarantee being checked; ranking and loan authority remain separate.
            */
            use super::super::super::super::prover::call_guarantees::arithmetic;
            use super::super::ExitScalars;
            use crate::borrow::owner_path_from_place_segments;
            use facts::{PlaceRoot, PlaceSegment};
            use typed_trees::expression::ExpressionHandle;
            use validation::{CanonicalPlace, ScopedArithmeticValue};

            impl ExitScalars<'_, '_> {
                pub(super) fn returned_descriptor_length(
                    &self,
                    expression: ExpressionHandle,
                ) -> Option<ScopedArithmeticValue> {
                    let entry = self.program.machine_states(self.machine).first()?;
                    let member = self.program.expression_table.member_node(expression)?;
                    let receiver_type = validation::expression_result_type_reference(
                        self.program,
                        self.machine,
                        entry,
                        member.receiver,
                    )?;
                    if validation::collection_operations::collection_measure_member(
                        self.program,
                        member,
                        validation::collection_operations::MeasureReceiver::Declared(receiver_type),
                    ) != Some(
                        language_semantics::declaration_selection::CollectionMeasure::Length,
                    ) {
                        return None;
                    }
                    let result_path = self.result_receiver_segments(member.receiver)?;
                    let output = validation::reference_shell(self.program, receiver_type)?;
                    if !matches!(
                        self.program.type_reference_table.type_reference(
                            self.program
                                .type_reference_table
                                .unconstrained_type_reference(output.referee)
                        ),
                        typed_trees::types::TypeReferenceNode::Slice { .. }
                    ) {
                        return None;
                    }
                    let sources = self
                        .call_frames?
                        .reference_result_leaf_origins(entry.symbol)?;
                    let mut selected = None;
                    for source in sources
                        .into_iter()
                        .filter(|source| source.result_path == result_path)
                    {
                        if source.source.is_image_storage
                            || !source.source.recasts.is_empty()
                            || source.source.representation.is_some()
                            || source.source.segments.iter().any(|segment| {
                                !matches!(
                                    segment,
                                    PlaceSegment::Field { .. }
                                        | PlaceSegment::Case { .. }
                                        | PlaceSegment::FixedIndex { .. }
                                )
                            })
                        {
                            return None;
                        }
                        let parameter = self
                            .program
                            .state_parameter_by_symbol(entry, source.source.root)?;
                        if parameter.is_mutable
                            || parameter.is_const
                            || validation::reference_shell(self.program, parameter.type_reference)
                                .is_some()
                        {
                            return None;
                        }
                        let path =
                            owner_path_from_place_segments(self.program, &source.source.segments);
                        if !validation::loan_carrier_has_owned_storage(
                            self.program,
                            entry.symbol,
                            0,
                            parameter.symbol,
                            &path,
                        ) {
                            return None;
                        }
                        let source_type = validation::project_type_reference_from_segments(
                            self.program,
                            parameter.type_reference,
                            &source.source.segments,
                        )?;
                        let input = validation::reference_shell(self.program, source_type)?;
                        if !validation::storage_origin_types_match(
                            self.program,
                            input.referee,
                            output.referee,
                        ) {
                            return None;
                        }
                        let place = CanonicalPlace {
                            root: PlaceRoot::Symbol(parameter.symbol),
                            segments: source.source.segments,
                        };
                        if selected.as_ref().is_some_and(|prior| *prior != place) {
                            return None;
                        }
                        selected = Some(place);
                    }
                    Some(arithmetic::length_atom(selected?))
                }
            }
        }

        use super::{
            ExitScalars, ExpressionHandle, ScalarValue, exit_return_expression,
            is_scalar_roster_local, stable_segments,
        };
        use crate::flow::expressions;
        use crate::flow::state_statements;
        use crate::flow::{self};
        use checked_trees::{CheckedOperatorResolutionStatus, CheckedScalarExpression};
        use facts::{PlaceRoot, PlaceSegment};
        use typed_trees::expression::{
            BinaryOperator, ExpressionNode, MatchPattern, TableBinaryExpression,
            TableMemberExpression,
        };
        use typed_trees::state::State;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

        #[derive(Clone)]
        struct MatchedReturnInteger {
            subject: ExpressionHandle,
            value: BigInt,
            patterns: Vec<ExpressionHandle>,
            arm: ExpressionHandle,
        }

        impl ExitScalars<'_, '_> {
            /*
                Return substitution and call substitution meet in the same scoped solver.
                Only live invocation guarantees are premises; the return contract under
                proof is never imported. Constructor fields retain their exact values,
                and entry operands retain the existing origin/preservation checks.
            */
            pub(super) fn proves_return_arithmetic(&self, expression: ExpressionHandle) -> bool {
                self.return_arithmetic(expression) == Some(true)
            }

            fn return_arithmetic(&self, expression: ExpressionHandle) -> Option<bool> {
                use super::super::super::prover::call_guarantees::{
                    self, arithmetic, callable::Callable,
                };
                let entry = self.program.machine_states(self.machine).first()?;
                let state =
                    find_state_on_machine(self.program, self.machine, self.exit.state_symbol)?;
                let caller = self
                    .facts
                    .flow
                    .control
                    .state(self.machine.symbol, self.exit.state_symbol)?;
                let exit_arithmetic = arithmetic::at_exit(
                    self.program,
                    self.facts,
                    caller,
                    self.exit,
                    self.contexts,
                    self.call_frames?,
                )?;
                let goal = arithmetic::proposition(
                    self.program,
                    self.facts,
                    &Callable::Machine {
                        machine: self.machine,
                        state: entry,
                    },
                    expression,
                    |occurrence, primitive| {
                        if let Some(length) = self.returned_descriptor_length(occurrence) {
                            return Some(length);
                        }
                        if let Some(length) = arithmetic::collection_length_value(
                            self.program,
                            &Callable::Machine {
                                machine: self.machine,
                                state: entry,
                            },
                            occurrence,
                            |place, observation| {
                                arithmetic::observation_origin(
                                    self.program,
                                    self.facts.view(),
                                    caller,
                                    self.exit.statement_index,
                                    self.call_frames?,
                                    self.contract_place_at_exit(place)?,
                                    observation,
                                )
                            },
                        ) {
                            return Some(length);
                        }
                        if let Some(result) = reserved_result_place(self.program, occurrence)
                            && result.machine_symbol == self.machine.symbol
                            && let Some(call) =
                                exit_return_call(self.program, self.facts, self.exit)
                        {
                            return arithmetic::call_result_atom(
                                self.program,
                                caller,
                                call,
                                &result.segments,
                                primitive,
                            );
                        }
                        if let Some((selected, remaining)) = self.result_projection(occurrence) {
                            if matches!(
                                self.program.expression_table.expression(selected),
                                ExpressionNode::Call(_)
                            ) && let Some(capture) =
                                completed_return_operand(self.program, self.facts, self.exit)
                                && capture.expression == selected
                                && project_type_reference_from_segments(
                                    self.program,
                                    capture.type_reference,
                                    &remaining,
                                )
                                .and_then(|reference| {
                                    self.program.primitive_type_reference(reference)
                                }) == Some(primitive)
                            {
                                let mut calls =
                                    crate::flow::call_flows(&self.facts.flow.control, caller.calls)
                                        .iter()
                                        .filter(|call| {
                                            call.statement_index == self.exit.statement_index
                                                && call.authored_expression == selected
                                                && call.authored_source_custody_valid
                                        });
                                if calls.next().is_some() && calls.next().is_none() {
                                    return Some(arithmetic::atom(
                                        CanonicalPlace {
                                            root: PlaceRoot::Expression(selected),
                                            segments: remaining,
                                        },
                                        primitive,
                                    ));
                                }
                            }
                            if remaining.is_empty() {
                                let value = arithmetic::scalar_term(
                                    self.program,
                                    self.facts.view(),
                                    &Callable::Machine {
                                        machine: self.machine,
                                        state,
                                    },
                                    selected,
                                    |value, carrier| {
                                        let place =
                                            call_guarantees::direct_place(self.program, value)?;
                                        exit_arithmetic.value(place, carrier)
                                    },
                                )?;
                                return Some(value);
                            }
                            let mut place = call_guarantees::direct_place(self.program, selected)?;
                            place.extend_segments(&remaining);
                            return exit_arithmetic.value(place, primitive);
                        }
                        let mut value = crate::values::lower_unit_scalar_argument(
                            self.program,
                            &self.facts.operators,
                            entry,
                            0,
                            occurrence,
                            primitive,
                        )?;
                        let mut subjects = Vec::new();
                        self.bind_current_scalar(&mut value, entry, true, &mut subjects)?;
                        let CheckedScalarExpression::Parameter { position, .. } = value else {
                            return None;
                        };
                        exit_arithmetic.value(subjects.get(position)?.clone(), primitive)
                    },
                );
                let goal = goal?;
                Some(exit_arithmetic.proves(&goal))
            }

            pub(super) fn result_projection(
                &self,
                expression: ExpressionHandle,
            ) -> Option<(ExpressionHandle, Vec<PlaceSegment>)> {
                let place = reserved_result_place(self.program, expression)?;
                if place.machine_symbol != self.machine.symbol || !stable_segments(&place.segments)
                {
                    return None;
                }
                let returned = exit_return_expression(self.program, self.exit);
                if !self.return_expression_is_stable(returned) {
                    return None;
                }
                let entry = self.program.machine_states(self.machine).first()?;
                if place.segments.is_empty() {
                    let reference = readable_type_reference(self.program, entry.return_type)?;
                    return self
                        .program
                        .primitive_type_reference(reference)
                        .is_some()
                        .then_some((returned, Vec::new()));
                }
                let mut projections = flow::literal_expression_projections(
                    self.program,
                    returned,
                    entry.return_type,
                    &place.segments,
                    false,
                )?;
                let selected = projections.pop()?;
                projections
                    .is_empty()
                    .then_some((selected.expression, selected.remaining))
            }

            pub(super) fn result_field_value(
                &self,
                expression: ExpressionHandle,
            ) -> Option<ScalarValue> {
                let state =
                    find_state_on_machine(self.program, self.machine, self.exit.state_symbol)?;
                if let Some(receiver) =
                    collection_length_receiver(self.program, self.machine, Some(state), expression)
                {
                    if let Some(length) = self.agreed_result_collection_length(state, receiver) {
                        return Some(ScalarValue::Integer(BigInt::from_u64(length)));
                    }
                    if reserved_result_owner(self.program, receiver)
                        .is_some_and(|(owner, _)| owner == self.machine.symbol)
                    {
                        let returned = exit_return_expression(self.program, self.exit);
                        if !self.return_expression_is_stable(returned) {
                            return None;
                        }
                        return closed_collection_length(
                            self.program,
                            self.machine,
                            state,
                            returned,
                        )
                        .map(BigInt::from_u64)
                        .map(ScalarValue::Integer);
                    }
                }
                let (value, remaining) = self.result_projection(expression)?;
                if remaining.is_empty() {
                    let reference = reserved_result_place(self.program, expression)?.type_reference;
                    let reference = readable_type_reference(self.program, reference)?;
                    let primitive = self.program.primitive_type_reference(reference)?;
                    if !has_builtin_bound_expression_meaning(
                        self.program,
                        self.machine,
                        Some(state),
                        value,
                    ) {
                        return None;
                    }
                    if let Some(selected) = crate::values::lower_unit_scalar_argument(
                        self.program,
                        &self.facts.operators,
                        state,
                        self.exit.statement_index,
                        value,
                        primitive,
                    ) && let Some(value) = crate::values::evaluate_checked_scalar(
                        &selected,
                        &mut crate::values::BoundScalarValues {
                            symbols: &[],
                            value_at_symbol: |_| None,
                        },
                    ) {
                        return Some(value);
                    }
                }
                if !self
                    .facts
                    .flow
                    .control
                    .operands
                    .is_valid(self.exit.returned_value)
                {
                    return None;
                }
                let capture = self
                    .facts
                    .flow
                    .control
                    .operands
                    .get(self.exit.returned_value);
                if capture.expression != exit_return_expression(self.program, self.exit)
                    || capture.state_symbol != self.exit.state_symbol
                    || capture.statement_index != self.exit.statement_index
                {
                    return None;
                }
                let state = self
                    .facts
                    .flow
                    .control
                    .state(self.machine.symbol, self.exit.state_symbol)?;
                let place = reserved_result_place(self.program, expression)?;
                captured_scalar_at_path(
                    self.program,
                    self.facts,
                    state,
                    capture,
                    &place.segments,
                    self.exit.statement_index.checked_add(1)?,
                    self.exit.entry_constraints,
                    self.call_frames?,
                )
            }

            /*
                `result.field.len` compared with a literal. Every constructed arm must
                establish the same relation. Disagreeing extents do not become one value.
            */
            pub(super) fn proves_result_collection_length(
                &self,
                expression: ExpressionHandle,
            ) -> bool {
                self.result_collection_length_relation(expression) == Some(true)
            }

            fn result_collection_length_relation(
                &self,
                expression: ExpressionHandle,
            ) -> Option<bool> {
                let comparison = self.program.expression_table.binary_node(expression)?;
                let state =
                    find_state_on_machine(self.program, self.machine, self.exit.state_symbol)?;
                let (receiver, expected, operator) = self.length_comparison(state, comparison)?;
                let lengths = self.result_collection_lengths(state, receiver)?;
                if lengths.is_empty() {
                    return None;
                }
                Some(lengths.iter().all(|length| {
                    let length = BigInt::from_u64(*length);
                    Self::length_relation(&length, operator, &expected)
                }))
            }

            fn length_comparison(
                &self,
                state: &State,
                comparison: &TableBinaryExpression,
            ) -> Option<(ExpressionHandle, BigInt, BinaryOperator)> {
                let left_length = collection_length_receiver(
                    self.program,
                    self.machine,
                    Some(state),
                    comparison.left,
                );
                let right_length = collection_length_receiver(
                    self.program,
                    self.machine,
                    Some(state),
                    comparison.right,
                );
                let (receiver, expected, operator) = if left_length.is_some() {
                    (
                        left_length?,
                        self.integer_literal(comparison.right)?,
                        comparison.operator,
                    )
                } else if right_length.is_some() {
                    (
                        right_length?,
                        self.integer_literal(comparison.left)?,
                        Self::flipped_relation(comparison.operator)?,
                    )
                } else {
                    return None;
                };
                matches!(
                    operator,
                    BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                )
                .then_some((receiver, expected, operator))
            }

            fn integer_literal(&self, expression: ExpressionHandle) -> Option<BigInt> {
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Integer(value) => value.value_bignum(),
                    _ => None,
                }
            }

            fn flipped_relation(operator: BinaryOperator) -> Option<BinaryOperator> {
                Some(match operator {
                    BinaryOperator::Equal => BinaryOperator::Equal,
                    BinaryOperator::NotEqual => BinaryOperator::NotEqual,
                    BinaryOperator::Less => BinaryOperator::Greater,
                    BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
                    BinaryOperator::Greater => BinaryOperator::Less,
                    BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
                    _ => return None,
                })
            }

            fn length_relation(
                length: &BigInt,
                operator: BinaryOperator,
                expected: &BigInt,
            ) -> bool {
                match operator {
                    BinaryOperator::Equal => length == expected,
                    BinaryOperator::NotEqual => length != expected,
                    BinaryOperator::Less => length < expected,
                    BinaryOperator::LessOrEqual => length <= expected,
                    BinaryOperator::Greater => length > expected,
                    BinaryOperator::GreaterOrEqual => length >= expected,
                    _ => false,
                }
            }

            /* One closed extent when every arm of this result projection agrees. */
            fn agreed_result_collection_length(
                &self,
                state: &State,
                receiver: ExpressionHandle,
            ) -> Option<u64> {
                let lengths = self.result_collection_lengths(state, receiver)?;
                let first = *lengths.first()?;
                lengths
                    .iter()
                    .all(|length| *length == first)
                    .then_some(first)
            }

            /*
                Closed extents of the slice selected by `receiver` on this exit.
                A match contributes every arm. A record field contributes that field's
                initializer. The record itself is not a descriptor.
            */
            fn result_collection_lengths(
                &self,
                state: &State,
                receiver: ExpressionHandle,
            ) -> Option<Vec<u64>> {
                let segments = self.result_receiver_segments(receiver)?;
                if !stable_segments(&segments) {
                    return None;
                }
                let returned = exit_return_expression(self.program, self.exit);
                let entry = self.program.machine_states(self.machine).first()?;
                self.collection_lengths_through(state, returned, entry.return_type, &segments, &[])
            }

            /*
                Field path from this machine's reserved result to the length receiver.
                An intermediate `result.field` node is not its own contract root, so the
                result name owns the path and each field is resolved on the result type.
            */
            fn result_receiver_segments(
                &self,
                receiver: ExpressionHandle,
            ) -> Option<Vec<PlaceSegment>> {
                let mut members = Vec::new();
                let mut cursor = receiver;
                loop {
                    if !self.program.expression_table.expression_is_valid(cursor)
                        || members.len() >= 128
                    {
                        return None;
                    }
                    match self.program.expression_table.expression(cursor) {
                        ExpressionNode::Member(member) if member.case_variant.is_none() => {
                            members.push(member);
                            cursor = member.receiver;
                        }
                        ExpressionNode::Name(_) => break,
                        _ => return None,
                    }
                }
                let (owner, mut type_reference) = reserved_result_owner(self.program, cursor)?;
                if owner != self.machine.symbol {
                    return None;
                }
                let mut segments = Vec::with_capacity(members.len());
                for member in members.into_iter().rev() {
                    let field = self.field_of_type(type_reference, member)?;
                    segments.push(PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    type_reference = field.type_reference;
                }
                Some(segments)
            }

            fn field_of_type(
                &self,
                type_reference: TypeReferenceHandle,
                member: &TableMemberExpression,
            ) -> Option<&DataField> {
                let mut reference = type_reference;
                loop {
                    match self.program.type_reference_table.type_reference(reference) {
                        TypeReferenceNode::Reference { referee, .. }
                        | TypeReferenceNode::Constrained {
                            base_type: referee, ..
                        } => reference = *referee,
                        TypeReferenceNode::Generic { base_symbol, .. } => {
                            return self.field_of_data(*base_symbol, member);
                        }
                        TypeReferenceNode::Named { symbol, .. } => {
                            return self.field_of_data(*symbol, member);
                        }
                        _ => return None,
                    }
                }
            }

            fn field_of_data(
                &self,
                symbol: SymbolHandle,
                member: &TableMemberExpression,
            ) -> Option<&DataField> {
                let mut definitions = self
                    .program
                    .data_definitions()
                    .iter()
                    .filter(|data| data.symbol == symbol);
                let data = definitions.next()?;
                if definitions.next().is_some() {
                    return None;
                }
                self.program
                    .data_members(data)
                    .iter()
                    .find_map(|candidate| {
                        let field = candidate.as_field()?;
                        let selected = if member.member_symbol.is_valid() {
                            field.symbol == member.member_symbol
                        } else {
                            field.name.as_str() == member.member.as_str()
                        };
                        selected.then_some(field)
                    })
            }

            fn collection_lengths_through(
                &self,
                state: &State,
                expression: ExpressionHandle,
                reference: TypeReferenceHandle,
                segments: &[PlaceSegment],
                matched: &[MatchedReturnInteger],
            ) -> Option<Vec<u64>> {
                if let ExpressionNode::Match(dispatch) =
                    self.program.expression_table.expression(expression)
                {
                    let arms = self.program.expression_table.match_arms(dispatch.arms);
                    if arms.is_empty() {
                        return None;
                    }
                    let mut lengths = Vec::new();
                    let mut patterns = Vec::new();
                    for arm in arms {
                        patterns
                            .extend(expressions(self.program).match_pattern_operands(arm.pattern));
                        let mut arm_matched = matched.to_vec();
                        if let Some((subject, value)) =
                            self.matched_arm_integer(dispatch.subject, arm.pattern)
                        {
                            arm_matched.push(MatchedReturnInteger {
                                subject,
                                value,
                                patterns: patterns.clone(),
                                arm: arm.value,
                            });
                        }
                        lengths.extend(self.collection_lengths_through(
                            state,
                            arm.value,
                            reference,
                            segments,
                            &arm_matched,
                        )?);
                    }
                    return (!lengths.is_empty()).then_some(lengths);
                }
                if segments.is_empty() {
                    if !self.return_expression_is_stable(expression) {
                        return None;
                    }
                    let current_matches: Vec<_> = matched
                        .iter()
                        .filter(|binding| {
                            expression_has_inert_prefix(
                                self.program,
                                &binding.patterns,
                                binding.arm,
                                expression,
                            )
                        })
                        .map(|binding| (binding.subject, binding.value.clone()))
                        .collect();
                    return closed_collection_length_under_matches(
                        self.program,
                        self.machine,
                        state,
                        expression,
                        &current_matches,
                    )
                    .map(|length| vec![length]);
                }
                let projections = flow::literal_expression_projections(
                    self.program,
                    expression,
                    reference,
                    segments,
                    false,
                )?;
                if projections.is_empty() {
                    return None;
                }
                let mut lengths = Vec::new();
                for projection in projections {
                    if !projection.remaining.is_empty() {
                        return None;
                    }
                    lengths.extend(self.collection_lengths_through(
                        state,
                        projection.expression,
                        reference,
                        &[],
                        matched,
                    )?);
                }
                (!lengths.is_empty()).then_some(lengths)
            }

            /*
                The integer a value pattern contributes to its arm. A wildcard learns
                nothing. A non-integer pattern contributes nothing here; the arm still
                uses whatever extent its window has without that subject.
            */
            fn matched_arm_integer(
                &self,
                subject: ExpressionHandle,
                pattern: MatchPattern,
            ) -> Option<(ExpressionHandle, BigInt)> {
                let MatchPattern::Value(pattern) = pattern else {
                    return None;
                };
                let value = self.integer_literal(pattern)?;
                Some((subject, value))
            }

            fn bind_current_scalar(
                &self,
                expression: &mut CheckedScalarExpression,
                state: &State,
                contract: bool,
                subjects: &mut Vec<CanonicalPlace>,
            ) -> Option<()> {
                use CheckedScalarExpression as Scalar;
                let parameters = self.program.state_parameters(state);
                let scalar_parameters = || {
                    parameters.iter().filter(|parameter| {
                        !parameter.relevance.is_erased()
                            && self
                                .program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                    })
                };
                let mut pending = vec![expression];
                while let Some(expression) = pending.pop() {
                    let (mut place, primitive) = match expression {
                        Scalar::Parameter {
                            position,
                            primitive_type,
                        } => (
                            flow::canonical_place_from_symbol(
                                scalar_parameters().nth(*position)?.symbol,
                            )?,
                            *primitive_type,
                        ),
                        Scalar::StructuralParameterField {
                            parameter_position,
                            path,
                            primitive_type,
                            ..
                        } => {
                            let (symbol, segments, _, _) =
                                crate::values::resolve_structural_parameter_path(
                                    self.program,
                                    parameters,
                                    *parameter_position,
                                    path,
                                )?;
                            (
                                CanonicalPlace {
                                    root: PlaceRoot::Symbol(symbol),
                                    segments,
                                },
                                *primitive_type,
                            )
                        }
                        Scalar::StorageRead {
                            symbol,
                            primitive_type,
                        } => (flow::canonical_place_from_symbol(*symbol)?, *primitive_type),
                        Scalar::Local {
                            position,
                            primitive_type,
                        } if !contract => {
                            let ordinal = position.checked_sub(scalar_parameters().count())?;
                            let local =
                                crate::flow::state_statements(self.program, state.statement_nodes)
                                    .get(..self.exit.statement_index)?
                                    .iter()
                                    .filter_map(|node| {
                                        let local = node.as_local_data()?;
                                        is_scalar_roster_local(self.program, local).then_some(local)
                                    })
                                    .nth(ordinal)?;
                            (
                                flow::canonical_place_from_symbol(local.symbol)?,
                                *primitive_type,
                            )
                        }
                        Scalar::IntegerLiteral { .. } => continue,
                        Scalar::IntegerBinary { left, right, .. } => {
                            pending.extend([left.as_mut(), right.as_mut()]);
                            continue;
                        }
                        Scalar::IntegerBitwiseNot { operand, .. }
                        | Scalar::IntegerWiden { operand, .. }
                        | Scalar::IntegerExactCast { operand, .. } => {
                            pending.push(operand.as_mut());
                            continue;
                        }
                        _ => return None,
                    };
                    if contract {
                        let root = place.root.as_symbol()?;
                        let parameter = parameters
                            .iter()
                            .find(|parameter| parameter.symbol == root)?;
                        if parameter.is_const {
                            return None;
                        }
                        let mut origins = self
                            .contract_origins()
                            .filter(|origin| origin.entry_parameter == root);
                        let origin = origins.next()?;
                        if origins.next().is_some() || !origin.state_parameter.is_valid() {
                            return None;
                        }
                        place.root = PlaceRoot::Symbol(origin.state_parameter);
                        let mut reference = parameter.type_reference;
                        while let TypeReferenceNode::Constrained { base_type, .. } =
                            self.program.type_reference_table.type_reference(reference)
                        {
                            reference = *base_type;
                        }
                        if !matches!(
                            self.program.type_reference_table.type_reference(reference),
                            TypeReferenceNode::Reference { .. }
                        ) {
                            if parameter.is_mutable {
                                return None;
                            }
                            self.preserved_contract_subject(&place)?;
                        }
                    }
                    *expression = intern_place(place, primitive, subjects);
                }
                Some(())
            }

            fn preserved_contract_subject(&self, subject: &CanonicalPlace) -> Option<()> {
                let entry = self.program.machine_states(self.machine).first()?;
                let frames = self.call_frames?;
                if !subject.segments.is_empty()
                    && self
                        .program
                        .machine_states(self.machine)
                        .iter()
                        .any(|state| {
                            self.program
                                .statement_table
                                .statements(state.statement_nodes)
                                .iter()
                                .any(|statement| {
                                    let Some(transition) = statement.as_transition() else {
                                        return false;
                                    };
                                    [transition.target, transition.continuation].iter().any(
                                        |target| {
                                            target.is_valid()
                                                && match self
                                                    .program
                                                    .statement_table
                                                    .transition_target(*target)
                                                {
                                                    TransitionTargetNode::Named {
                                                        path, ..
                                                    } => {
                                                        path.symbol == entry.symbol
                                                            || path.symbol == self.machine.symbol
                                                    }
                                                    _ => false,
                                                }
                                        },
                                    )
                                })
                        })
                {
                    return None;
                }
                for state in self.program.machine_states(self.machine) {
                    let statements = state_statements(self.program, state.statement_nodes);
                    let prefix = if state.symbol == self.exit.state_symbol {
                        statements.get(..self.exit.statement_index)?
                    } else if entry.symbol == self.exit.state_symbol {
                        continue;
                    } else {
                        statements
                    };
                    for (ordinal, statement) in prefix.iter().enumerate() {
                        let mut nodes = Vec::new();
                        collect_statement_expression_trees(self.program, statement, &mut nodes);
                        if matches!(statement, StatementNode::AssemblyFact(_))
                            || nodes.iter().any(|node| {
                                matches!(
                                    self.program.expression_table.expression(*node),
                                    ExpressionNode::Atomic(_)
                                )
                            })
                            || self.facts.operators.uses.iter().any(|(_, selected)| {
                                nodes.contains(&selected.expression)
                                    && selected.status
                                        != CheckedOperatorResolutionStatus::BuiltinFallback
                            })
                        {
                            return None;
                        }
                        let mut writes = flow::statement_storage_writes(
                            self.program,
                            self.machine.symbol,
                            state.symbol,
                            ordinal,
                            statement,
                            self.call_frames,
                        )?;
                        let mut value_frames =
                            vec![frames.statement_value_write_frame(self.machine, statement)];
                        if let Some(call) = statement.as_call() {
                            value_frames.push(frames.may_write_frame(self.machine, call));
                        }
                        for frame in value_frames {
                            writes.extend(flow::frame_storage_writes(
                                self.program,
                                self.machine.symbol,
                                state.symbol,
                                ordinal,
                                &frame,
                                Some(frames),
                            )?);
                        }
                        for write in writes {
                            let overlaps = if entry.symbol == self.exit.state_symbol {
                                write.root == subject.root
                                    && flow::canonical_place_segments_may_overlap(
                                        self.program,
                                        &write.segments,
                                        &subject.segments,
                                    )
                            } else {
                                self.program
                                    .state_parameters(state)
                                    .iter()
                                    .any(|parameter| {
                                        write.root == PlaceRoot::Symbol(parameter.symbol)
                                    })
                            };
                            if overlaps {
                                return None;
                            }
                        }
                    }
                }
                Some(())
            }
        }

        fn intern_place(
            place: CanonicalPlace,
            primitive_type: PrimitiveType,
            subjects: &mut Vec<CanonicalPlace>,
        ) -> CheckedScalarExpression {
            let position = subjects
                .iter()
                .position(|candidate| *candidate == place)
                .unwrap_or_else(|| {
                    let position = subjects.len();
                    subjects.push(place);
                    position
                });
            CheckedScalarExpression::Parameter {
                position,
                primitive_type,
            }
        }
    }
    mod static_calls {
        /*
            Declared normal-return equalities under exact const-argument substitution.
            A callee's binder spelling is not a caller value identity. Match its retained
            binder position to the exact static selection instead. Ordinary runtime-call
            readers keep their no-machine-arguments fence; this reader separately checks
            the complete const roster before using the shared call/guarantee custody.
        */
        use super::{ExpressionHandle, ScalarValue};
        use crate::checks::contracts::exits::scalars::ExitScalars;
        use crate::checks::contracts::exits::scalars::stable_segments;
        use crate::checks::contracts::prover::evaluate_scalar;
        use crate::published::places::place_at;

        use typed_trees::data::{TypeParameter, TypeParameterKind};
        use typed_trees::expression::{BinaryOperator, ExpressionNode, StaticMachineArgument};

        #[derive(PartialEq, Eq)]
        enum StaticResult {
            CallerBinder(symbols::SymbolHandle),
            Value(ScalarValue),
        }

        impl ExitScalars<'_, '_> {
            pub(super) fn static_call_returns_symbol(
                &self,
                expression: ExpressionHandle,
                symbol: symbols::SymbolHandle,
            ) -> bool {
                self.static_call_result(expression) == Some(StaticResult::CallerBinder(symbol))
            }

            pub(super) fn closed_static_call_value(
                &self,
                expression: ExpressionHandle,
            ) -> Option<ScalarValue> {
                match self.static_call_result(expression)? {
                    StaticResult::Value(value) => Some(value),
                    StaticResult::CallerBinder(_) => None,
                }
            }

            fn static_call_result(&self, expression: ExpressionHandle) -> Option<StaticResult> {
                let expression = if matches!(
                    self.program.expression_table.expression(expression),
                    ExpressionNode::Call(_)
                ) {
                    expression
                } else {
                    let place = place_at(
                        self.program,
                        self.exit.state_symbol,
                        self.exit.statement_index,
                        expression,
                    )?;
                    if !stable_segments(&place.segments) {
                        return None;
                    }
                    self.assigned_call_at_place(&place)?
                };
                let selected = self.normal_return_call_target(expression)?;
                let authored = self.program.expression_table.call_node(expression)?;
                let parameters = self.program.machine_type_parameters(selected.callee);
                if parameters.len() != authored.machine_arguments.len() {
                    return None;
                }
                let bindings = parameters
                    .iter()
                    .zip(authored.machine_arguments.iter())
                    .map(|(parameter, argument)| self.static_argument_result(parameter, argument))
                    .collect::<Option<Vec<_>>>()?;
                let result_type = self
                    .program
                    .primitive_type_reference(selected.entry.return_type)?;
                if !result_type.accepts_integer_literal()
                    && result_type != typed_trees::types::PrimitiveType::Bool
                {
                    return None;
                }
                let mut result = None;
                for guarantee in self.normal_call_guarantees(selected) {
                    let Some(binary) = self.program.expression_table.binary_node(guarantee) else {
                        continue;
                    };
                    if binary.operator != BinaryOperator::Equal {
                        continue;
                    }
                    let is_returned_value = |expression| {
                        validation::reserved_result_owner(self.program, expression)
                            .is_some_and(|(owner, _)| owner == selected.callee.symbol)
                    };
                    let value = if is_returned_value(binary.left) {
                        binary.right
                    } else if is_returned_value(binary.right) {
                        binary.left
                    } else {
                        continue;
                    };
                    let (candidate, value_type) =
                        match self.program.expression_table.expression(value) {
                            ExpressionNode::Name(path) => {
                                if !self
                                    .program
                                    .expression_table
                                    .name_path_is_single_member(path)
                                {
                                    continue;
                                }
                                let Some(position) = parameters.iter().position(|parameter| {
                                    parameter.symbol.is_valid() && parameter.symbol == path.symbol
                                }) else {
                                    continue;
                                };
                                let (TypeParameterKind::Const { type_reference }
                                | TypeParameterKind::Value { type_reference }) =
                                    parameters[position].kind
                                else {
                                    continue;
                                };
                                (&bindings[position], type_reference)
                            }
                            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {
                                if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                                    self.program,
                                    selected.callee.symbol,
                                    guarantee,
                                    language_core::OperatorSpelling::Equal,
                                    &[
                                        Some(selected.entry.return_type),
                                        Some(selected.entry.return_type),
                                    ],
                                ) {
                                    continue;
                                }
                                let candidate = StaticResult::Value(evaluate_scalar(
                                    self.program,
                                    value,
                                    &mut |_| None,
                                    &|_, _| true,
                                )?);
                                if result.as_ref().is_some_and(|prior| prior != &candidate) {
                                    return None;
                                }
                                result = Some(candidate);
                                continue;
                            }
                            _ => continue,
                        };
                    if self.program.primitive_type_reference(value_type) != Some(result_type)
                        || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                            self.program,
                            selected.callee.symbol,
                            guarantee,
                            language_core::OperatorSpelling::Equal,
                            &[Some(selected.entry.return_type), Some(value_type)],
                        )
                    {
                        continue;
                    }
                    if result.as_ref().is_some_and(|prior| prior != candidate) {
                        return None;
                    }
                    result = Some(match candidate {
                        StaticResult::CallerBinder(symbol) => StaticResult::CallerBinder(*symbol),
                        StaticResult::Value(value) => StaticResult::Value(value.clone()),
                    });
                }
                result
            }

            fn static_argument_result(
                &self,
                parameter: &TypeParameter,
                argument: &StaticMachineArgument,
            ) -> Option<StaticResult> {
                let (TypeParameterKind::Const { type_reference }
                | TypeParameterKind::Value { type_reference }) = parameter.kind
                else {
                    return None;
                };
                if argument.application.is_some() || argument.evidence_projection.is_some() {
                    return None;
                }
                let primitive = self.program.primitive_type_reference(type_reference)?;
                if let Some(literal) = &argument.const_literal {
                    return primitive
                        .accepts_integer_literal()
                        .then(|| literal.value_bignum())
                        .flatten()
                        .map(|value| StaticResult::Value(ScalarValue::Integer(value)));
                }
                let caller = self
                    .program
                    .machine_type_parameters(self.machine)
                    .iter()
                    .find(|caller| caller.symbol.is_valid() && caller.symbol == argument.symbol)?;
                let (TypeParameterKind::Const {
                    type_reference: caller_type,
                }
                | TypeParameterKind::Value {
                    type_reference: caller_type,
                }) = caller.kind
                else {
                    return None;
                };
                (self.program.primitive_type_reference(caller_type) == Some(primitive))
                    .then_some(StaticResult::CallerBinder(caller.symbol))
            }
        }
    }

    pub(super) fn proves<'program>(
        program: &'program dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        exit: &FlowExitFact,
        contexts: &[FactContextHandle],
        requirement: &facts::Fact,
        call_frames: Option<&validation::CallFrameResolver<'program>>,
        cached_exit_values: &mut ExitValueCache<'program>,
    ) -> bool {
        let FactPayload::ContractBooleanExpression {
            fact: contract,
            expression,
            ..
        } = requirement.payload
        else {
            return false;
        };
        let Some(machine) = program.machine(exit.machine_symbol) else {
            return false;
        };
        let evaluator = ExitScalars {
            program,
            facts,
            machine,
            exit,
            contexts,
            contract,
            call_frames,
        };
        if evaluator.state().is_some_and(|state| {
            validation::float_return_postcondition(
                program,
                machine,
                state,
                exit_return_expression(program, exit),
                expression,
            ) == Some(true)
        }) {
            return true;
        }
        if !has_builtin_operators(program, &facts.operators, expression)
            || !validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                program.machine_states(machine).first(),
                expression,
            )
        {
            return false;
        }
        let cases = CaseObservation::for_requirement(
            program,
            facts,
            exit,
            contexts,
            requirement,
            call_frames,
        );
        evaluate_for_atoms(
            program,
            expression,
            &mut |leaf| evaluator.contract_value(leaf),
            &|_, _| true,
            &mut |atom| {
                program
                    .static_type_predicate_value_in(atom, exit_return_expression(program, exit))
                    .or_else(|| {
                        cases
                            .as_ref()
                            .and_then(|cases| cases.observe(atom))
                            .or_else(|| {
                                (evaluator.proves_immutable_result_comparison(atom)
                                    || evaluator.proves_return_arithmetic(atom)
                                    || evaluator.proves_result_collection_length(atom)
                                    || proves_exit_expression(
                                        program,
                                        facts,
                                        exit,
                                        contract,
                                        atom,
                                        call_frames,
                                        cached_exit_values,
                                    ))
                                .then_some(true)
                                .or_else(|| evaluator.current_predicate(atom))
                            })
                    })
            },
        ) == Some(ScalarValue::Boolean(true))
    }

    struct ExitScalars<'program, 'facts> {
        program: &'program dyn ::validation::CheckingInput,
        facts: &'facts CheckFacts,
        machine: &'program Machine,
        exit: &'facts FlowExitFact,
        contexts: &'facts [FactContextHandle],
        contract: arena::Handle<typed_trees::domain::ProofFact>,
        call_frames: Option<&'facts validation::CallFrameResolver<'program>>,
    }

    impl<'program, 'facts> ExitScalars<'program, 'facts> {
        /* The state this exit sits on, resolved through the exit's machine. */
        fn state(&self) -> Option<&'program typed_trees::state::State> {
            validation::find_state_for_exit(self.program, self.exit)
        }

        /*
            The recognized origin this exit retained for `entry_parameter` under
            this contract. A state edge rebases an entry binder only through the
            correspondence the exit recorded; later states keep the exact pair.
        */
        fn contract_origin(
            &self,
            entry_parameter: symbols::SymbolHandle,
        ) -> Option<&'facts checked_trees::FlowExitParameterOrigin> {
            self.contract_origins()
                .find(|origin| origin.entry_parameter == entry_parameter)
        }

        /* Every origin this exit retained for this contract, in roster order. */
        fn contract_origins(
            &self,
        ) -> impl Iterator<Item = &'facts checked_trees::FlowExitParameterOrigin> {
            crate::flow::exit_parameter_origins(self.facts.view(), self.exit)
                .iter()
                .filter(|origin| origin.contract == self.contract)
        }
    }

    /*
        Parameters typed primitive — the leading slots of a scalar binding
        roster at any occurrence in `state`.
    */
    pub(crate) fn primitive_parameters<'a>(
        program: &'a dyn ::validation::CheckingInput,
        state: &'a typed_trees::state::State,
    ) -> impl Iterator<Item = symbols::SymbolHandle> + 'a {
        program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
            })
            .map(|parameter| parameter.symbol)
    }

    /*
        An immutable local carrying an initializer whose declared type is
        primitive — the only locals a scalar binding roster names.
    */
    pub(crate) fn is_scalar_roster_local(
        program: &dyn ::validation::CheckingInput,
        local: &typed_trees::statement::TableLocalData,
    ) -> bool {
        !local.is_mutable
            && local.initial_value.is_valid()
            && program
                .primitive_type_reference(local.type_reference)
                .is_some()
    }

    impl ExitScalars<'_, '_> {
        /*
            Current pointees retain their entry binders; later states must retain
            the exact parameter correspondence for this contract. A write changes
            storage facts, not the declaration namespace used to name that storage.
        */
        fn contract_place_at_exit(
            &self,
            mut place: validation::CanonicalPlace,
        ) -> Option<validation::CanonicalPlace> {
            let entry = self.program.machine_states(self.machine).first()?;
            if entry.symbol != self.exit.state_symbol {
                let root = place.root.as_symbol()?;
                let origin = self
                    .contract_origin(root)
                    .filter(|origin| origin.state_parameter.is_valid())?;
                place.root = PlaceRoot::Symbol(origin.state_parameter);
            }
            Some(place)
        }

        fn proves_immutable_result_comparison(&self, expression: ExpressionHandle) -> bool {
            use typed_trees::expression::{BinaryOperator, ExpressionNode};
            if self.proves_bounds(expression) {
                return true;
            }
            let Some(binary) = self.program.expression_table.binary_node(expression) else {
                return false;
            };
            let spelling = match binary.operator {
                BinaryOperator::And => {
                    return self.proves_immutable_result_comparison(binary.left)
                        && self.proves_immutable_result_comparison(binary.right);
                }
                BinaryOperator::Or => {
                    return self.proves_immutable_result_comparison(binary.left)
                        || self.proves_immutable_result_comparison(binary.right);
                }
                BinaryOperator::Equal => language_core::OperatorSpelling::Equal,
                BinaryOperator::LessOrEqual => language_core::OperatorSpelling::LessEqual,
                BinaryOperator::GreaterOrEqual => language_core::OperatorSpelling::GreaterEqual,
                _ => return false,
            };
            let argument = if is_result_reference(self.program, self.machine, binary.left) {
                binary.right
            } else if is_result_reference(self.program, self.machine, binary.right) {
                binary.left
            } else {
                return false;
            };
            let Some(path) = self.program.expression_table.name_path_node(argument) else {
                return false;
            };
            if !path.symbol.is_valid() || path.head_symbol != path.symbol {
                return false;
            }
            let Some(entry) = self.program.machine_states(self.machine).first() else {
                return false;
            };
            if let Some(parameter) = self
                .program
                .machine_type_parameters(self.machine)
                .iter()
                .find(|parameter| parameter.symbol == path.symbol)
                && let typed_trees::data::TypeParameterKind::Const { type_reference }
                | typed_trees::data::TypeParameterKind::Value { type_reference } = parameter.kind
            {
                let primitive = self.program.primitive_type_reference(type_reference);
                if !primitive.is_some_and(|primitive| {
                    primitive.accepts_integer_literal()
                        || (primitive == typed_trees::types::PrimitiveType::Bool
                            && binary.operator == BinaryOperator::Equal)
                }) || primitive != self.program.primitive_type_reference(entry.return_type)
                {
                    return false;
                }
                let types = if argument == binary.right {
                    [Some(entry.return_type), Some(type_reference)]
                } else {
                    [Some(type_reference), Some(entry.return_type)]
                };
                return typed_trees::operator::has_builtin_spelled_expression_meaning(
                    self.program,
                    self.machine.symbol,
                    expression,
                    spelling,
                    &types,
                ) && (matches!(self.program.expression_table.expression(exit_return_expression(self.program, self.exit)),
                    ExpressionNode::Name(returned) if returned.symbol == parameter.symbol
                        && returned.head_symbol == parameter.symbol
                        && self.program.expression_table.name_path_members(returned.members).len() == 1)
                    || self.static_call_returns_symbol(
                        exit_return_expression(self.program, self.exit),
                        parameter.symbol,
                    ));
            }
            let Some(parameter) = self.program.state_parameter_by_symbol(entry, path.symbol) else {
                return false;
            };
            if (parameter.is_mutable)
                || parameter.is_const
                || !self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some_and(|primitive| {
                        primitive.accepts_integer_literal()
                            || (primitive == typed_trees::types::PrimitiveType::Bool
                                && binary.operator == BinaryOperator::Equal)
                    })
                || self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    != self.program.primitive_type_reference(entry.return_type)
            {
                return false;
            }
            let types = if argument == binary.right {
                [Some(entry.return_type), Some(parameter.type_reference)]
            } else {
                [Some(parameter.type_reference), Some(entry.return_type)]
            };
            if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                self.program,
                self.machine.symbol,
                expression,
                spelling,
                &types,
            ) {
                return false;
            }
            self.returned_immutable_parameter(parameter.symbol)
        }

        /*
            Exact return identity from the retained state-edge origin; no storage
            replay or source initializer can establish this correspondence.
        */
        fn returned_immutable_parameter(&self, symbol: symbols::SymbolHandle) -> bool {
            use typed_trees::expression::ExpressionNode;
            let Some(entry) = self.program.machine_states(self.machine).first() else {
                return false;
            };
            let Some(parameter) = self.program.state_parameter_by_symbol(entry, symbol) else {
                return false;
            };
            if parameter.is_mutable
                || parameter.is_const
                || !self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some_and(|primitive| {
                        primitive.is_integer()
                            || primitive == typed_trees::types::PrimitiveType::Bool
                    })
                || self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    != self.program.primitive_type_reference(entry.return_type)
            {
                return false;
            }
            let Some(origin) = self.contract_origin(parameter.symbol) else {
                return false;
            };
            let Some(state) = self.state() else {
                return false;
            };
            if !self
                .program
                .state_parameters(state)
                .iter()
                .any(|parameter| {
                    (parameter.symbol == origin.state_parameter && !parameter.is_mutable)
                        && !parameter.is_const
                })
            {
                return false;
            }
            matches!(self.program.expression_table.expression(exit_return_expression(self.program, self.exit)), ExpressionNode::Name(returned)
                if origin.state_parameter.is_valid() && returned.symbol == origin.state_parameter && returned.head_symbol == origin.state_parameter)
                || self.call_returns_immutable_symbol(
                    exit_return_expression(self.program, self.exit),
                    origin.state_parameter,
                )
        }

        fn contract_value(&self, expression: ExpressionHandle) -> Option<ScalarValue> {
            if let Some(value) = self.result_field_value(expression) {
                return Some(value);
            }
            if is_result_reference(self.program, self.machine, expression) {
                return self.return_value();
            }
            let entry = self.program.machine_states(self.machine).first()?;
            if matches!(
                self.program.expression_table.expression(expression),
                typed_trees::expression::ExpressionNode::Member(_)
            ) && let Some(checked_trees::CheckedScalarExpression::IntegerLiteral { literal }) =
                crate::values::lower_unit_scalar_argument(
                    self.program,
                    &self.facts.operators,
                    entry,
                    0,
                    expression,
                    typed_trees::types::PrimitiveType::U64,
                )
            {
                return literal.value_bignum().map(ScalarValue::Integer);
            }
            let mut place = place_at(self.program, entry.symbol, 0, expression)?;
            let root = place.root.as_symbol()?;
            if self
                .program
                .state_parameter_by_symbol(entry, root)
                .is_some()
            {
                let origin = self.contract_origin(root)?;
                if !origin.state_parameter.is_valid() {
                    return None;
                }
                place.root = PlaceRoot::Symbol(origin.state_parameter);
            }
            if !stable_segments(&place.segments) {
                return None;
            }
            self.value_at_place(&place)
        }

        fn return_value(&self) -> Option<ScalarValue> {
            let expression = exit_return_expression(self.program, self.exit);
            if !self.return_expression_is_stable(expression) {
                return None;
            }
            self.selected_return_value(expression)
                .or_else(|| self.closed_call_value(expression))
                .or_else(|| self.closed_static_call_value(expression))
                .or_else(|| {
                    evaluate_scalar(
                        self.program,
                        expression,
                        &mut |leaf| {
                            let place = place_at(
                                self.program,
                                self.exit.state_symbol,
                                self.exit.statement_index,
                                leaf,
                            )?;
                            if !stable_segments(&place.segments) {
                                return None;
                            }
                            self.value_at_place(&place)
                        },
                        &|_, _| true,
                    )
                })
        }

        fn return_expression_is_stable(&self, expression: ExpressionHandle) -> bool {
            let mut nodes = Vec::new();
            validation::expression_traversal::collect_expression_tree(
                self.program,
                expression,
                &mut nodes,
            );
            let state = validation::find_state_on_machine(
                self.program,
                self.machine,
                self.exit.state_symbol,
            );
            expressions(self.program).expression_is_valid(expression)
                && nodes.iter().all(|node| {
                    !matches!(
                        self.program.expression_table.expression(*node),
                        typed_trees::expression::ExpressionNode::Atomic(_)
                    ) && has_builtin_operators(self.program, &self.facts.operators, *node)
                        && validation::has_builtin_bound_expression_meaning(
                            self.program,
                            self.machine,
                            state,
                            *node,
                        )
                })
                && self.call_frames.is_some_and(|frames| {
                    frames
                        .expression_write_frame(self.machine, expression)
                        .into_complete_paths()
                        .is_some_and(|paths| paths.is_empty())
                })
        }

        fn selected_return_value(&self, expression: ExpressionHandle) -> Option<ScalarValue> {
            let (expression, symbols) = self.selected_return_expression(expression)?;
            evaluate_checked_scalar(
                expression,
                &mut crate::values::BoundScalarValues {
                    symbols,
                    value_at_symbol: |symbol| {
                        let place = canonical_place_from_symbol(symbol)?;
                        self.value_at_place(&place)
                    },
                },
            )
        }

        fn selected_return_expression(
            &self,
            expression: ExpressionHandle,
        ) -> Option<(
            &checked_trees::CheckedScalarExpression,
            &[symbols::SymbolHandle],
        )> {
            self.selected_scalar_expression(
                u32::try_from(self.exit.statement_index).ok()?,
                self.return_expression_role()?,
                expression,
            )
        }

        fn return_expression_role(&self) -> Option<CheckedScalarExpressionRole> {
            let state = self.state()?;
            Some(
                match self
                    .program
                    .statement_table
                    .statement_at(state, self.exit.statement_index)?
                {
                    typed_trees::statement::StatementNode::Transition(transition)
                        if self.exit.transition_target.is_valid()
                            && self.exit.transition_target == transition.continuation =>
                    {
                        CheckedScalarExpressionRole::ContinuationReturn
                    }
                    _ => CheckedScalarExpressionRole::Return,
                },
            )
        }

        fn selected_scalar_expression(
            &self,
            statement_ordinal: u32,
            role: CheckedScalarExpressionRole,
            expression: ExpressionHandle,
        ) -> Option<(
            &checked_trees::CheckedScalarExpression,
            &[symbols::SymbolHandle],
        )> {
            let plans = &self.facts.values.scalar_expressions;
            let mut bindings = plans.source_bindings.iter().filter(|(_, binding)| {
                binding.state == self.exit.state_symbol
                    && binding.statement_ordinal == statement_ordinal
                    && binding.role == role
            });
            let (_, binding) = bindings.next()?;
            if bindings.next().is_some() || binding.expression != expression {
                return None;
            }
            if matches!(role, CheckedScalarExpressionRole::CallArgument { .. })
                && binding.destination.is_valid()
            {
                return None;
            }
            let mut selected = plans.expressions.iter().filter(|plan| {
                plan.state == binding.state
                    && plan.statement_ordinal == binding.statement_ordinal
                    && plan.role == binding.role
            });
            let plan = selected.next()?;
            if selected.next().is_some() {
                return None;
            }
            let symbols = plans.binding_symbols.span_or_empty(binding.symbols);
            let state = self.state()?;
            let parameters = primitive_parameters(self.program, state);
            let locals = crate::flow::state_statements(self.program, state.statement_nodes)
                .get(..statement_ordinal as usize)?
                .iter()
                .filter_map(|statement| {
                    let local = statement.as_local_data()?;
                    is_scalar_roster_local(self.program, local).then_some(local.symbol)
                });
            if !parameters.chain(locals).eq(symbols.iter().copied()) {
                return None;
            }
            Some((&plan.expression, symbols))
        }
    }

    fn stable_segments(segments: &[PlaceSegment]) -> bool {
        segments.iter().all(|segment| match segment {
            PlaceSegment::Field { symbol } => symbol.is_valid(),
            PlaceSegment::Case { variant } => variant.is_valid(),
            PlaceSegment::FixedIndex { .. } => true,
            _ => false,
        })
    }
}
pub(super) use scalars::BoundaryRefutations;

pub(super) use cyclic_headers::CyclicHeaderInvariants;
pub(super) use result_domains::{
    check_exclusive_referent_field_domains, check_result_field_domains,
    check_scalar_result_domains, check_state_transfer_result_domains,
};
pub(crate) use result_domains::{
    exclusive_referent_type, is_readable_mutable_reference, value_provable_domain,
};

/*
    A helper theorem over collections: the machine's own `requires`,
    including collection-predicate premises explored through the predicate's
    checked body, prove the `ensures` expression on every normal exit. Supported
    stores retain distinct arrival/current versions; unknown effects stand down.
*/
fn collection_premises_prove_exit(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    fact: &facts::Fact,
) -> bool {
    let facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
        return false;
    };
    program
        .machine(state_flow.machine_symbol)
        .is_some_and(|machine| validation::premises_prove_expression(program, machine, expression))
}

/*
    Whether one exit's live facts prove a scalar postcondition: the entry
    contexts prove the predicate, or the exit's exact return and current scalar
    or live place values evaluate it per atom (`scalars`). Structural normal
    completion also retains nonnumeric fields through the same exact origins.
    Ordinary exit checking and package exit
    evidence share these routes, alongside separately rechecked component
    induction. Callers must establish exact invocation origins before using this
    helper; it grants no assumption, entailment or invariant authority.
*/
pub(super) fn live_contexts_prove_scalar_postcondition<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    exit_flow: &checked_trees::FlowExitFact,
    entry_contexts: &[facts::FactContextHandle],
    fact: &facts::Fact,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
    cached_exit_values: &mut Option<
        Option<(
            &'program typed_trees::machine::Machine,
            crate::checks::current_scalar_values::CurrentScalarValues<'program>,
            Option<typed_trees::expression::ExpressionHandle>,
        )>,
    >,
) -> bool {
    let facts::FactPayload::ContractBooleanExpression {
        fact: contract,
        expression,
        ..
    } = fact.payload
    else {
        return false;
    };
    if semantic_contexts_prove_contract_fact(
        program,
        facts,
        facts::ProgramPoint::Statement {
            machine_symbol: exit_flow.machine_symbol,
            state_symbol: exit_flow.state_symbol,
            statement_index: exit_flow.statement_index,
        },
        entry_contexts,
        fact,
    ) || scalars::proves(
        program,
        facts,
        exit_flow,
        entry_contexts,
        fact,
        call_frames,
        cached_exit_values,
    ) {
        return true;
    }
    let Some((machine, state)) =
        program.state_flow_site(exit_flow.machine_symbol, exit_flow.state_symbol)
    else {
        return false;
    };
    let origins = crate::flow::exit_parameter_origins(facts.view(), exit_flow)
        .iter()
        .filter(|origin| origin.contract == contract)
        .map(|origin| (origin.entry_parameter, origin.state_parameter))
        .collect::<Vec<_>>();
    bindings::proves(
        program, facts, exit_flow, machine, state, contract, expression, &origins,
    )
}

pub(super) fn check_exit_ensures(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    exit_flow: &checked_trees::FlowExitFact,
    entailment: &super::entailment::MachineEntailmentOutcome,
    content_plans: &[validation::ContentConservationSourcePlan],
    available_content: &[language_semantics::content::ContentConservationPlan],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    cyclic_headers: &mut CyclicHeaderInvariants,
    recipe_obligations: &mut Vec<checked_trees::ConstantRecipeObligation>,
    integer_range_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if program.static_type_context_is_unreachable(super::return_values::exit_return_expression(
        program, exit_flow,
    )) {
        return;
    }
    let entry_contexts =
        active_context_handles(&facts.flow.contexts, exit_flow.entry_semantic_contexts);
    let mut cached_exit_values = None;
    for ensures_context in facts
        .flow
        .semantic_constraint_contexts(exit_flow.ensures_constraints)
    {
        let context = facts.semantic.contexts.get(ensures_context);
        for fact in facts.semantic.context_view(context).facts() {
            let contract = match fact.payload {
                facts::FactPayload::ContractBooleanExpression { fact, .. }
                | facts::FactPayload::ContractDomainMembership { fact, .. }
                | facts::FactPayload::ContractCarryPermission { fact, .. }
                | facts::FactPayload::ContractPropositionApplication { fact, .. } => Some(fact),
                _ => None,
            };
            let admitted_boundary_result = facts.proof.contract_facts.iter().any(|(_, inherited)| {
                inherited.kind == checked_trees::ContractProofFactKind::Ensures
                    && Some(inherited.fact) == contract
                    && inherited.qualification_authorization.is_some()
                    && matches!(
                        inherited.owner,
                        checked_trees::ContractProofFactOwner::MachineState { machine_symbol, state_symbol }
                            if machine_symbol == state_flow.machine_symbol
                                && state_symbol == state_flow.state_symbol
                    )
            });
            if admitted_boundary_result {
                continue;
            }
            let checked_entailment = match fact.payload {
                facts::FactPayload::ContractBooleanExpression { expression, .. } => {
                    entailment.expressions.contains(&expression)
                }
                _ => false,
            };
            if checked_entailment {
                continue;
            }
            let evidence_assignment = match fact.payload {
                facts::FactPayload::ContractPropositionApplication {
                    fact: source_fact, ..
                } => facts.proof.contract_facts.iter().any(|(_, contract)| {
                    contract.kind == checked_trees::ContractProofFactKind::Ensures
                        && contract.owner
                            == (checked_trees::ContractProofFactOwner::Machine {
                                machine_symbol: state_flow.machine_symbol,
                            })
                        && contract.fact == source_fact
                        && contract.evidence_term.is_some()
                }),
                _ => false,
            };
            if evidence_assignment {
                continue;
            }
            let missing_origins = crate::flow::exit_parameter_origins(facts.view(), exit_flow)
                .iter()
                .filter(|origin| {
                    Some(origin.contract) == contract
                        && !origin.state_parameter.is_valid()
                        && !origin.retained_referent_type.is_valid()
                })
                .map(|origin| program.symbols.name(origin.entry_parameter))
                .collect::<Vec<_>>();
            let mut proved_cache = None;
            let mut is_proved = || {
                *proved_cache.get_or_insert_with(|| {
                    if matches!(
                        fact.payload,
                        facts::FactPayload::ContractBooleanExpression { .. }
                    ) {
                        live_contexts_prove_scalar_postcondition(
                            program,
                            facts,
                            exit_flow,
                            &entry_contexts,
                            fact,
                            call_frames,
                            &mut cached_exit_values,
                        )
                    } else {
                        semantic_contexts_prove_contract_fact(
                            program,
                            facts,
                            facts::ProgramPoint::Statement {
                                machine_symbol: exit_flow.machine_symbol,
                                state_symbol: exit_flow.state_symbol,
                                statement_index: exit_flow.statement_index,
                            },
                            &entry_contexts,
                            fact,
                        )
                    }
                })
            };
            let content_theorem = match fact.payload {
                facts::FactPayload::ContractBooleanExpression { expression, .. } => {
                    content_plans.iter().any(|source| {
                        source.source_expression == expression
                            && source.plan.owner == state_flow.machine_symbol
                    })
                }
                _ => false,
            };
            let mut membership_refusal = None;
            let satisfied = if content_theorem {
                content_plans.iter().any(|source| {
                    source.plan.owner == state_flow.machine_symbol
                        && source.plan.callable == state_flow.state_symbol
                        && matches!(fact.payload,
                            facts::FactPayload::ContractBooleanExpression { expression, .. }
                            if expression == source.source_expression)
                        && available_content.contains(&source.plan)
                }) || (missing_origins.is_empty()
                    && proves_composed_exit(facts, exit_flow, fact, content_plans))
            } else {
                (missing_origins.is_empty()
                    && (is_proved()
                        || result_domains::proves_result_domain(
                            program,
                            facts,
                            exit_flow,
                            &entry_contexts,
                            fact,
                            &mut membership_refusal,
                        )
                        || super::content_preservation::proves_exit(
                            program,
                            facts,
                            state_flow,
                            exit_flow,
                            fact,
                            content_plans,
                            available_content,
                        )
                        || proves_composed_exit(facts, exit_flow, fact, content_plans)
                        || super::integer_embeddings::proves_embedded_literal_exit(
                            program, state_flow, exit_flow, fact,
                        )
                        || super::entailment::transparent_proposition_proves_exit(
                            program, facts, entailment, state_flow, fact,
                        )
                        || collection_premises_prove_exit(program, state_flow, fact)
                        || {
                            let authorized_route = match fact.payload {
                            facts::FactPayload::ContractDomainMembership {
                                domain_symbol,
                                value,
                                ..
                            } => validation::reserved_result_owner(program, value).is_some_and(
                                |(owner, _)| {
                                    program.machine(state_flow.machine_symbol).is_some_and(
                                        |machine| {
                                            match crate::published::facts::qualification_evidence::machine_domain_establishment_route(
                                                program,
                                                machine,
                                                domain_symbol,
                                            ) {
                                                Some(language_semantics::DomainEstablishmentRoute::CheckedRequirement {
                                                    requirement,
                                                    ..
                                                }) => {
                                                    owner == requirement
                                                        || owner == state_flow.machine_symbol
                                                        || owner == state_flow.state_symbol
                                                }
                                                Some(_) => {
                                                    owner == state_flow.machine_symbol
                                                        || owner == state_flow.state_symbol
                                                }
                                                None => false,
                                            }
                                        },
                                    )
                                },
                            ),
                            _ => false,
                        };
                            authorized_route
                                && match fact.payload {
                                    facts::FactPayload::ContractDomainMembership {
                                        domain_symbol,
                                        ..
                                    } => program.domain_definition(domain_symbol).is_none_or(
                                        |domain| {
                                            !domain.predicate_presence.is_present() || is_proved()
                                        },
                                    ),
                                    _ => true,
                                }
                        }))
                    || cyclic_headers.proves(program, facts, exit_flow, fact)
            };

            if !satisfied
                && missing_origins.is_empty()
                && let Some(conditions) = scalars::proves_with_recipe_conditions(
                    program,
                    facts,
                    exit_flow,
                    &entry_contexts,
                    fact,
                    call_frames,
                )
            {
                recipe_obligations.extend(conditions);
                continue;
            }
            if !satisfied
                && missing_origins.is_empty()
                && let Some(conditions) = scalars::proves_with_range_conditions(
                    program,
                    facts,
                    exit_flow,
                    &entry_contexts,
                    fact,
                    call_frames,
                )
            {
                integer_range_obligations.extend(conditions);
                continue;
            }
            if !satisfied {
                let membership_diagnostic = result_domains::completed_result_domain_diagnostic(
                    program,
                    facts,
                    exit_flow,
                    fact,
                    membership_refusal,
                )
                .map(|detail| format!("; {detail}"))
                .unwrap_or_default();
                let origin_diagnostic = if missing_origins.is_empty() {
                    String::new()
                } else {
                    format!(
                        "; no exact incoming reference origin for {}",
                        missing_origins.join(", ")
                    )
                };
                let diagnostic = Diagnostic::error(format!(
                    "cannot prove ensures contract for exit from {} at statement {}: {}{}{}",
                    machine_name(program, state_flow.machine_symbol),
                    exit_flow.statement_index,
                    semantic_fact_requirement_label(program, &facts.semantic, fact),
                    origin_diagnostic,
                    membership_diagnostic,
                ));
                diagnostics.push(
                    match super::return_values::exit_source_span(program, exit_flow) {
                        Some(span) => diagnostic.with_source_span(span),
                        None => diagnostic,
                    },
                );
            }
        }
    }
}
