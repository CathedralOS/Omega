/*
Compile-time evaluation admission (`compiler_evaluation`) and synchronous
call-graph closure (`synchronous_calls`).
*/

pub(crate) mod compiler_evaluation {
    use semantic_vocabulary::compiler_evaluation::{
        CompilerEvaluationDeclaration, CompilerEvaluationEffects, DeclarationCustody,
        DeclarationSelectionAuthority, require_compiler_evaluation_floor,
    };
    use terminal_psi::{ProofApplicationScope, ProofDeclarationId, TargetMachineCallable};

    pub(crate) mod failures {
        use proof_admission::{ClosedIntegerEvaluator, check_predicate_evaluation_size};
        use semantic_vocabulary::compiler_evaluation::require_compiler_evaluation_failures;
        use semantic_vocabulary::{
            IntegerMathTerm, IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType,
        };
        use terminal_psi::{
            CrashRouteBucket, CrashRouteGuard, ProofCallableAssociation, ProofDeclarationId,
            TargetMachineCallable, TerminalModule,
        };

        pub(crate) fn require_root_failure_admission(
            module: &TerminalModule,
            root: ProofDeclarationId,
        ) -> Result<(), String> {
            let scope = &module.scalar_applications.proof_scope;
            let header = super::header(scope, root)?;
            let facts = header.compiler_evaluation.as_ref().map_err(Clone::clone)?;
            let mut bodies =
                scope
                    .callables
                    .iter()
                    .filter_map(|(_, association)| match association {
                        ProofCallableAssociation::Callable {
                            declaration,
                            callable,
                        } if *declaration == root => Some(*callable),
                        _ => None,
                    });
            let Some(TargetMachineCallable::Machine(machine)) = bodies.next() else {
                return Err(
                    "compiler crash admission requires an exact emitted machine body".into(),
                );
            };
            if bodies.next().is_some() {
                return Err(
                    "compiler crash admission has competing emitted body associations".into(),
                );
            }
            let machine = module
                .machine(machine)
                .ok_or("compiler crash admission has no emitted root body")?;
            let context = machine.entry_proposition_context().map_err(|reason| {
                format!("compiler crash admission has an invalid entry context: {reason:?}")
            })?;
            let failures = exclude_failure_routes(&machine.contract.crash_routes, &context);
            require_compiler_evaluation_failures(
                &facts.name,
                failures.as_ref().map(|_| ()).map_err(String::as_str),
            )
        }

        fn exclude_failure_routes(
            routes: &[CrashRouteBucket],
            context: &PropositionContext,
        ) -> Result<(), String> {
            if routes.windows(2).any(|pair| pair[0].cause >= pair[1].cause)
                || routes.iter().any(|bucket| {
                    bucket.alternatives.is_empty()
                        || bucket
                            .alternatives
                            .windows(2)
                            .any(|pair| pair[0] >= pair[1])
                        || (bucket.alternatives.contains(&CrashRouteGuard::Truth)
                            && bucket.alternatives != [CrashRouteGuard::Truth])
                })
            {
                return Err("the checked crash summary is malformed".into());
            }
            check_predicate_evaluation_size(routes.iter().flat_map(|bucket| {
                bucket.alternatives.iter().filter_map(|guard| match guard {
                    CrashRouteGuard::Truth => None,
                    CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
                })
            }))
            .map_err(|reason| {
                format!("the checked crash summary exceeds evaluation limits: {reason:?}")
            })?;
            let mut integers = ClosedIntegerEvaluator::default();
            for guard in routes.iter().flat_map(|bucket| &bucket.alternatives) {
                if let CrashRouteGuard::Predicate(predicate) = guard {
                    context
                        .validate(predicate.proposition())
                        .map_err(|reason| {
                            format!("the checked crash guard is malformed: {reason:?}")
                        })?;
                    require_formed_operands(predicate.proposition(), &mut integers)?;
                }
            }
            for bucket in routes {
                for guard in &bucket.alternatives {
                    let possible = match guard {
                        CrashRouteGuard::Truth => Some(true),
                        CrashRouteGuard::Predicate(predicate) => {
                            closed_guard(predicate.proposition(), &mut integers)?
                        }
                    };
                    match possible {
                        Some(false) => (),
                        Some(true) => {
                            return Err(format!(
                                "the checked {:?} crash route can occur",
                                bucket.cause
                            ));
                        }
                        None => {
                            return Err(format!(
                                "the checked {:?} crash route cannot be proved impossible from the available invocation facts",
                                bucket.cause
                            ));
                        }
                    }
                }
            }
            Ok(())
        }

        fn require_formed_operands(
            proposition: &Proposition,
            integers: &mut ClosedIntegerEvaluator,
        ) -> Result<(), String> {
            match proposition {
                semantic_vocabulary::scalar_comparison_operands!(left, right) => {
                    require_formed_scalar(left)?;
                    require_formed_scalar(right)?;
                }
                semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                    require_formed_integer_math(left, integers)?;
                    require_formed_integer_math(right, integers)?;
                }
                Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                    for child in children {
                        require_formed_operands(child, integers)?;
                    }
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    require_formed_operands(premise, integers)?;
                    require_formed_operands(conclusion, integers)?;
                }
                _ => (),
            }
            Ok(())
        }

        fn require_formed_scalar(term: &ScalarTerm) -> Result<bool, String> {
            let mut closed = true;
            for operand in term.operands() {
                closed &= require_formed_scalar(operand)?;
            }
            match term {
                ScalarTerm::ExactIntegerDivide { right, .. }
                | ScalarTerm::ExactIntegerRemainder { right, .. }
                | ScalarTerm::WrappingIntegerDivide { right, .. }
                | ScalarTerm::WrappingIntegerRemainder { right, .. }
                | ScalarTerm::SaturatingIntegerDivide { right, .. }
                | ScalarTerm::SaturatingIntegerRemainder { right, .. }
                    if matches!(
                        right.integer_value(),
                        Some((_, IntegerValue::Signed(0) | IntegerValue::Unsigned(0)))
                    ) =>
                {
                    return Err(
                        "the checked crash guard contains an undefined zero-divisor operand".into(),
                    );
                }
                ScalarTerm::ExactIntegerShiftLeft {
                    value_type,
                    count_type,
                    count,
                    ..
                }
                | ScalarTerm::ExactIntegerShiftRight {
                    value_type,
                    count_type,
                    count,
                    ..
                } => {
                    if let Some((_, count)) = count.integer_value()
                        && value_type
                            .exact_shift_right(value_type.minimum_value(), *count_type, count)
                            .is_none()
                    {
                        return Err(
                            "the checked crash guard contains an undefined exact-shift count"
                                .into(),
                        );
                    }
                }
                _ => (),
            }
            match term {
                ScalarTerm::Integer { .. }
                | ScalarTerm::Boolean(_)
                | ScalarTerm::IeeeFloatLiteral(_) => Ok(true),
                ScalarTerm::Application { .. }
                | ScalarTerm::Value { .. }
                | ScalarTerm::StructuralRead { .. }
                | ScalarTerm::BooleanField { .. }
                | ScalarTerm::IntegerField { .. }
                | ScalarTerm::IeeeFloatField { .. }
                | ScalarTerm::ViewExtent { .. }
                | ScalarTerm::ByteSequencePredicate { .. } => Ok(false),
                _ if !closed => Ok(false),
                _ => {
                    let defined = match term.scalar_type() {
                        ScalarType::Boolean => term.boolean_value().is_some(),
                        ScalarType::Integer(_) => term.integer_value().is_some(),
                        _ => false,
                    };
                    if !defined {
                        return Err(
                            "the checked crash guard contains an undefined closed scalar operand"
                                .into(),
                        );
                    }
                    Ok(true)
                }
            }
        }

        fn require_formed_integer_math(
            term: &IntegerMathTerm,
            integers: &mut ClosedIntegerEvaluator,
        ) -> Result<bool, String> {
            if let IntegerMathTerm::Embed(scalar) = term {
                return require_formed_scalar(scalar);
            }
            let mut closed = !matches!(term, IntegerMathTerm::MathValue { .. });
            for operand in term.operands() {
                closed &= require_formed_integer_math(operand, integers)?;
            }
            if let IntegerMathTerm::ShiftLeft { count, .. } = term
                && let Some(count) = integers.evaluate_closed(count).map_err(|reason| {
                    format!("the checked crash guard could not be formed: {reason}")
                })?
                && count.is_negative()
            {
                return Err(
                    "the checked crash guard contains an undefined negative mathematical shift"
                        .into(),
                );
            }
            if closed
                && integers
                    .evaluate_closed(term)
                    .map_err(|reason| {
                        format!("the checked crash guard could not be formed: {reason}")
                    })?
                    .is_none()
            {
                return Err(
                    "the checked crash guard contains an undefined closed mathematical operand"
                        .into(),
                );
            }
            Ok(closed)
        }

        fn closed_guard(
            proposition: &Proposition,
            integers: &mut ClosedIntegerEvaluator,
        ) -> Result<Option<bool>, String> {
            // The shared walk decides the guard. This only keeps the admission
            // message for a closed-integer resource limit.
            proof_admission::closed_guard(proposition, integers).map_err(|reason| {
                format!("the checked crash guard could not be evaluated: {reason}")
            })
        }

        #[cfg(test)]
        pub(crate) mod tests {
            use super::*;
            use semantic_vocabulary::compiler_evaluation::{
                CompilerEvaluationDeclaration, CompilerEvaluationEffects, DeclarationCustody,
            };
            use semantic_vocabulary::{
                IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ScalarApplicationId,
                ScalarTerm, ValueId,
            };
            use terminal_psi::{CrashCause, CrashPredicateTerm, ProofCallableHeader};

            fn exclude_failure_routes(
                routes: &Result<Vec<CrashRouteBucket>, String>,
            ) -> Result<(), String> {
                let context = PropositionContext::from_value_types([(
                    ValueId::FIRST,
                    ScalarType::Integer(integer_type()),
                )])
                .unwrap();
                super::exclude_failure_routes(routes.as_ref().map_err(Clone::clone)?, &context)
            }

            fn integer_type() -> IntegerType {
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
            }

            fn integer(value: u128) -> ScalarTerm {
                ScalarTerm::integer(integer_type(), IntegerValue::Unsigned(value)).unwrap()
            }

            fn route(cause: CrashCause, guard: Proposition) -> CrashRouteBucket {
                CrashRouteBucket {
                    cause,
                    alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(guard))],
                }
            }

            #[test]
            fn complete_summary_distinguishes_no_routes_and_false_routes() {
                assert!(exclude_failure_routes(&Ok(Vec::new())).is_ok());
                for value in [1, 7] {
                    assert!(
                        exclude_failure_routes(&Ok(vec![route(
                            CrashCause::Trap,
                            Proposition::Equal(integer(value), integer(0)),
                        )]))
                        .is_ok()
                    );
                }
            }

            #[test]
            fn any_possible_or_unresolved_failure_prevents_execution() {
                let unknowns = [
                    ScalarTerm::value(ValueId::FIRST, ScalarType::Integer(integer_type())),
                    ScalarTerm::Application {
                        application: ScalarApplicationId::new(1).unwrap(),
                        scalar_type: ScalarType::Integer(integer_type()),
                    },
                ];
                for unknown in unknowns {
                    let reason = exclude_failure_routes(&Ok(vec![route(
                        CrashCause::Trap,
                        Proposition::Equal(unknown, integer(0)),
                    )]))
                    .unwrap_err();
                    assert!(reason.contains("cannot be proved impossible"));
                }
                let reason = exclude_failure_routes(&Ok(vec![
                    route(CrashCause::Trap, Proposition::Falsehood),
                    CrashRouteBucket {
                        cause: CrashCause::Abort,
                        alternatives: vec![CrashRouteGuard::Truth],
                    },
                ]))
                .unwrap_err();
                assert!(reason.contains("Abort crash route can occur"));
                assert!(
                    exclude_failure_routes(&Ok(vec![route(
                        CrashCause::Trap,
                        Proposition::Equal(integer(0), integer(0)),
                    )]))
                    .unwrap_err()
                    .contains("Trap crash route can occur")
                );
            }

            #[test]
            fn exact_scalar_terms_and_boolean_guards_keep_their_checked_meaning() {
                let sum =
                    ScalarTerm::exact_integer_add(integer_type(), integer(1), integer(2)).unwrap();
                assert!(
                    exclude_failure_routes(&Ok(vec![route(
                        CrashCause::Trap,
                        Proposition::Equal(sum, integer(0)),
                    )]))
                    .is_ok()
                );
                assert!(
                    exclude_failure_routes(&Ok(vec![route(
                        CrashCause::Trap,
                        Proposition::Equal(ScalarTerm::Boolean(false), ScalarTerm::Boolean(true)),
                    )]))
                    .is_ok()
                );
                assert!(
                    exclude_failure_routes(&Ok(vec![route(
                        CrashCause::Trap,
                        Proposition::IntegerMathLessThan(
                            IntegerMathTerm::literal(IntegerValue::Unsigned(2)),
                            IntegerMathTerm::literal(IntegerValue::Unsigned(1)),
                        ),
                    )]))
                    .is_ok()
                );
                let open = Proposition::Equal(
                    ScalarTerm::value(ValueId::FIRST, ScalarType::Integer(integer_type())),
                    integer(0),
                );
                assert!(
                    exclude_failure_routes(&Ok(vec![route(
                        CrashCause::Trap,
                        Proposition::Conjunction(vec![open.clone(), Proposition::Falsehood]),
                    )]))
                    .is_ok()
                );
                assert!(
                    exclude_failure_routes(&Ok(vec![route(
                        CrashCause::Trap,
                        Proposition::Implication {
                            premise: Box::new(Proposition::Truth),
                            conclusion: Box::new(Proposition::Falsehood),
                        },
                    )]))
                    .is_ok()
                );
                assert!(
                    exclude_failure_routes(&Ok(vec![route(
                        CrashCause::Trap,
                        Proposition::Disjunction(vec![open, Proposition::Falsehood]),
                    )]))
                    .is_err()
                );
            }

            #[test]
            fn float_guard_decides_literal_ieee_comparison() {
                // The same Binary32 guards the interpreter decides: 1.0 < 2.0,
                // and a NaN is not equal to itself.
                use proof_admission::ClosedIntegerEvaluator;
                use semantic_vocabulary::{
                    IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue,
                };
                let guard = |kind, left, right| Proposition::ScalarIeeeFloatComparison {
                    kind,
                    format: IeeeFloatFormat::Binary32,
                    left: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(left)),
                    right: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(right)),
                };
                let mut integers = ClosedIntegerEvaluator::default();
                assert_eq!(
                    super::closed_guard(
                        &guard(IeeeFloatComparisonKind::LessThan, 0x3f80_0000, 0x4000_0000),
                        &mut integers,
                    )
                    .unwrap(),
                    Some(true)
                );
                assert_eq!(
                    super::closed_guard(
                        &guard(IeeeFloatComparisonKind::Equal, 0x7fc0_0000, 0x7fc0_0000),
                        &mut integers,
                    )
                    .unwrap(),
                    Some(false)
                );
                let mismatched = Proposition::ScalarIeeeFloatComparison {
                    kind: IeeeFloatComparisonKind::LessThan,
                    format: IeeeFloatFormat::Binary32,
                    left: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0x3f80_0000)),
                    right: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(
                        0x4000_0000_0000_0000,
                    )),
                };
                assert_eq!(
                    super::closed_guard(&mismatched, &mut integers).unwrap(),
                    None
                );
            }

            #[test]
            fn malformed_or_oversized_guard_never_proves_absence() {
                assert!(
                    exclude_failure_routes(&Ok(vec![CrashRouteBucket {
                        cause: CrashCause::Trap,
                        alternatives: Vec::new(),
                    }]))
                    .is_err()
                );
                assert!(
                    exclude_failure_routes(&Ok(vec![route(
                        CrashCause::Trap,
                        Proposition::Equal(ScalarTerm::Boolean(false), integer(0)),
                    )]))
                    .unwrap_err()
                    .contains("malformed")
                );
                let mut too_deep = Proposition::Falsehood;
                for _ in 0..70 {
                    too_deep = Proposition::Conjunction(vec![too_deep]);
                }
                assert!(
                    exclude_failure_routes(&Ok(vec![route(CrashCause::Trap, too_deep)]))
                        .unwrap_err()
                        .contains("evaluation limits")
                );
            }

            fn header(declaration: ProofDeclarationId) -> ProofCallableHeader {
                ProofCallableHeader {
                    declaration,
                    compiler_evaluation: Ok(CompilerEvaluationDeclaration {
                        name: format!("root{}", declaration.0),
                        custody: DeclarationCustody::Toolchain,
                        effects: CompilerEvaluationEffects::default(),
                        local_terminates: true,
                        entry_requires: false,
                        linear_carrier: None,
                        static_application: None,
                        selections: Ok(Vec::new()),
                    }),
                    ..ProofCallableHeader::default()
                }
            }

            #[test]
            fn admission_uses_only_the_complete_root_summary() {
                let root = ProofDeclarationId(1);
                let unrelated = ProofDeclarationId(2);
                let mut module = terminal_psi::TerminalModule::declarations();
                module.machines = vec![machine(1), machine(2)];
                module.machines[1].contract.crash_routes = vec![CrashRouteBucket {
                    cause: CrashCause::Trap,
                    alternatives: vec![CrashRouteGuard::Truth],
                }];
                let scope = &mut module.scalar_applications.proof_scope;
                for declaration in [root, unrelated] {
                    scope.callables.insert(ProofCallableAssociation::Callable {
                        declaration,
                        callable: TargetMachineCallable::Machine(
                            semantic_vocabulary::MachineId::new(u64::from(declaration.0)).unwrap(),
                        ),
                    });
                }
                scope.callable_headers.insert(header(root));
                scope.callable_headers.insert(header(unrelated));
                assert!(require_root_failure_admission(&module, root).is_ok());
                assert!(
                    require_root_failure_admission(&module, unrelated)
                        .unwrap_err()
                        .contains("constant evaluation requires proof")
                );
                assert!(require_root_failure_admission(&module, ProofDeclarationId(3)).is_err());
                module
                    .scalar_applications
                    .proof_scope
                    .callable_headers
                    .insert(header(root));
                assert!(
                    require_root_failure_admission(&module, root)
                        .unwrap_err()
                        .contains("competing")
                );
            }

            fn machine(id: u64) -> terminal_psi::TerminalMachine {
                use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId};
                use terminal_psi::{Block, TerminalMachine, TerminalMachineResult, Terminator};
                let block = BlockId::new(1).unwrap();
                TerminalMachine {
                    assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
                    id: MachineId::new(id).unwrap(),
                    attachment: None,
                    nominal_cleanup_subject: None,
                    ranked_scc: None,
                    structural_places: Vec::new(),
                    block_entry_claims: Vec::new(),
                    loan_completions: Vec::new(),
                    content_identity_reshuffles: Vec::new(),
                    content_partition_compositions: Vec::new(),
                    entry: block,
                    blocks: vec![Block::plain(
                        block,
                        Vec::new(),
                        Terminator::ReturnUnit {
                            edge: EdgeId::new(1).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    )],
                    contract: terminal_psi::MachineContract {
                        selected_application: None,
                        declaration: None,
                        parameters: vec![terminal_psi::ValueDeclaration {
                            id: ValueId::FIRST,
                            scalar_type: ScalarType::Integer(integer_type()),
                            qualifications: Default::default(),
                        }],
                        parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar],
                        structural_parameters: Vec::new(),
                        result: TerminalMachineResult::Unit,
                        entry_claims: Vec::new(),
                        declared_service_reach: None,
                        closed_reach_application: None,
                        published_synchronous_entries: Vec::new(),
                        published_service_ceiling: Vec::new(),
                        content_entry_claims: Vec::new(),
                        ..terminal_psi::MachineContract::empty(ContractId::new(1).unwrap())
                    },
                }
            }

            fn suppressed(predicate: Proposition) -> Proposition {
                Proposition::Conjunction(vec![Proposition::Falsehood, predicate])
            }

            #[test]
            fn false_guards_cannot_hide_undefined_scalar_operands() {
                let open = ScalarTerm::value(ValueId::FIRST, ScalarType::Integer(integer_type()));
                let undefined = [
                    ScalarTerm::ExactIntegerDivide {
                        scalar_type: integer_type(),
                        left: Box::new(integer(1)),
                        right: Box::new(integer(0)),
                    },
                    ScalarTerm::ExactIntegerRemainder {
                        scalar_type: integer_type(),
                        left: Box::new(integer(1)),
                        right: Box::new(integer(0)),
                    },
                    ScalarTerm::ExactIntegerMultiply {
                        scalar_type: integer_type(),
                        left: Box::new(integer(u64::MAX.into())),
                        right: Box::new(integer(2)),
                    },
                    ScalarTerm::ExactIntegerDivide {
                        scalar_type: integer_type(),
                        left: Box::new(open.clone()),
                        right: Box::new(integer(0)),
                    },
                    ScalarTerm::ExactIntegerShiftLeft {
                        value_type: integer_type(),
                        count_type: integer_type(),
                        value: Box::new(open.clone()),
                        count: Box::new(integer(64)),
                    },
                    ScalarTerm::ExactIntegerShiftRight {
                        value_type: integer_type(),
                        count_type: integer_type(),
                        value: Box::new(open),
                        count: Box::new(integer(64)),
                    },
                ];
                for term in undefined {
                    let equality = Proposition::Equal(term, integer(0));
                    for guard in [
                        suppressed(equality.clone()),
                        Proposition::Implication {
                            premise: Box::new(Proposition::Falsehood),
                            conclusion: Box::new(equality.clone()),
                        },
                    ] {
                        assert!(
                            exclude_failure_routes(&Ok(vec![route(CrashCause::Trap, guard)]))
                                .unwrap_err()
                                .contains("undefined")
                        );
                    }
                }
                let open = ScalarTerm::value(ValueId::FIRST, ScalarType::Integer(integer_type()));
                for term in [
                    ScalarTerm::ExactIntegerDivide {
                        scalar_type: integer_type(),
                        left: Box::new(open.clone()),
                        right: Box::new(integer(1)),
                    },
                    ScalarTerm::ExactIntegerShiftRight {
                        value_type: integer_type(),
                        count_type: integer_type(),
                        value: Box::new(open),
                        count: Box::new(integer(63)),
                    },
                ] {
                    assert!(
                        exclude_failure_routes(&Ok(vec![route(
                            CrashCause::Trap,
                            suppressed(Proposition::Equal(term, integer(0)))
                        )]))
                        .is_ok()
                    );
                }
            }

            #[test]
            fn mathematical_formation_checks_open_siblings_and_embedded_partial_operations() {
                let open = IntegerMathTerm::MathValue {
                    source_type: integer_type(),
                    value: ValueId::FIRST,
                };
                for value in [
                    IntegerMathTerm::literal(IntegerValue::Unsigned(1)),
                    open.clone(),
                ] {
                    let shifted = IntegerMathTerm::ShiftLeft {
                        value: Box::new(value),
                        count: Box::new(IntegerMathTerm::literal(IntegerValue::Signed(-1))),
                    };
                    let term = IntegerMathTerm::Add(Box::new(open.clone()), Box::new(shifted));
                    let guard = suppressed(Proposition::IntegerMathLessThan(
                        term,
                        IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
                    ));
                    assert!(
                        exclude_failure_routes(&Ok(vec![route(CrashCause::Trap, guard)]))
                            .unwrap_err()
                            .contains("undefined negative mathematical shift")
                    );
                }
                let embedded = IntegerMathTerm::Embed(Box::new(ScalarTerm::ExactIntegerDivide {
                    scalar_type: integer_type(),
                    left: Box::new(integer(1)),
                    right: Box::new(integer(0)),
                }));
                let guard = suppressed(Proposition::IntegerMathLessThan(
                    embedded,
                    IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
                ));
                assert!(
                    exclude_failure_routes(&Ok(vec![route(CrashCause::Trap, guard)]))
                        .unwrap_err()
                        .contains("undefined zero-divisor")
                );
                let safe = IntegerMathTerm::ShiftLeft {
                    value: Box::new(open),
                    count: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
                };
                let guard = suppressed(Proposition::IntegerMathLessThan(
                    safe,
                    IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
                ));
                assert!(exclude_failure_routes(&Ok(vec![route(CrashCause::Trap, guard)])).is_ok());
            }

            #[test]
            fn admission_validates_the_whole_guard_in_the_emitted_entry_context() {
                let root = ProofDeclarationId(1);
                let mut module = terminal_psi::TerminalModule::declarations();
                module.machines.push(machine(1));
                module.scalar_applications.proof_scope.callables.insert(
                    ProofCallableAssociation::Callable {
                        declaration: root,
                        callable: TargetMachineCallable::Machine(
                            semantic_vocabulary::MachineId::new(1).unwrap(),
                        ),
                    },
                );
                let valid = suppressed(Proposition::Equal(
                    ScalarTerm::value(ValueId::FIRST, ScalarType::Integer(integer_type())),
                    integer(0),
                ));
                let undeclared = suppressed(Proposition::Equal(
                    ScalarTerm::value(
                        ValueId::new(99).unwrap(),
                        ScalarType::Integer(integer_type()),
                    ),
                    integer(0),
                ));
                let unbound_place = suppressed(Proposition::Equal(
                    ScalarTerm::ViewExtent {
                        root: semantic_vocabulary::PlaceId::new(1).unwrap(),
                        path: Vec::new(),
                        scalar_type: integer_type(),
                    },
                    integer(0),
                ));
                module
                    .scalar_applications
                    .proof_scope
                    .callable_headers
                    .insert(header(root));
                for (guard, passes) in [(valid, true), (undeclared, false), (unbound_place, false)]
                {
                    module.machines[0].contract.crash_routes = vec![route(CrashCause::Trap, guard)];
                    let result = require_root_failure_admission(&module, root);
                    if passes {
                        assert!(result.is_ok(), "{result:?}");
                    } else {
                        assert!(result.unwrap_err().contains("malformed"));
                    }
                }
                module.machines.clear();
                assert!(
                    require_root_failure_admission(&module, root)
                        .unwrap_err()
                        .contains("no emitted root body")
                );
            }
        }
    }
    pub(crate) use failures::require_root_failure_admission;

    #[derive(Debug, Clone)]
    pub struct AdmittedCompilerEvaluation {
        providers: crate::AdmittedProviderSelections,
        artifact: terminal_codec::TerminalExecutionIdentity,
        target: String,
        declaration: ProofDeclarationId,
        requester: DeclarationCustody,
    }

    impl AdmittedCompilerEvaluation {
        pub const fn requester(&self) -> DeclarationCustody {
            self.requester
        }
        pub fn target(&self) -> &str {
            &self.target
        }
        pub fn providers(&self) -> &crate::AdmittedProviderSelections {
            &self.providers
        }
        pub fn require_invocation<'code>(
            &self,
            artifact: impl Into<terminal_codec::TerminalExecutionView<'code>>,
            target: &str,
            entry: TargetMachineCallable,
        ) -> Result<(), String> {
            let artifact = artifact.into();
            let declaration = artifact
                .module()
                .scalar_applications
                .proof_scope
                .callable_declaration(entry);
            if self.artifact != artifact.custody_identity()
                || self.target != target
                || declaration != Some(self.declaration)
            {
                return Err(
                "compiler-evaluation admission names a different artifact, target or invocation"
                    .into(),
            );
            }
            Ok(())
        }
    }

    pub fn admit_compiler_evaluation<'code>(
        artifact: impl Into<terminal_codec::TerminalExecutionView<'code>>,
        target: &str,
        root: ProofDeclarationId,
        providers: crate::AdmittedProviderSelections,
        requester: DeclarationCustody,
        authority: Option<&dyn DeclarationSelectionAuthority>,
    ) -> Result<AdmittedCompilerEvaluation, String> {
        let artifact = artifact.into();
        if providers.execution_identity() != &artifact.custody_identity() {
            return Err(
                "compiler-evaluation provider installation names a different artifact".into(),
            );
        }
        let scope = &artifact.module().scalar_applications.proof_scope;
        let root = scope
            .plan
            .callable_for_target(root, target)
            .ok_or("compiler-evaluation declaration has no selected target body")?;
        let root_facts = declaration(scope, root)?;
        if root_facts.entry_requires {
            return Err(format!(
                "machine `{}` has an authored `requires` premise; no checked invocation proof for that premise",
                root_facts.name
            ));
        }
        require_root_failure_admission(artifact.module(), root)?;
        let invocation = compiler_invocation_scope(scope, target, &providers, root)?;
        let CompilerInvocationScope {
            selected,
            closure,
            dependencies,
        } = invocation;
        for binding in providers.assumed_bindings() {
            for requirement in
                std::iter::once(binding.required).chain(binding.supply_path.iter().copied())
            {
                let declaration = scope
                    .callable_declaration(TargetMachineCallable::Boundary(requirement))
                    .ok_or("compiler-evaluation assumed requirement lost its declaration")?;
                if closure.calls.contains(&declaration) || closure.machines.contains(&declaration) {
                    return Err("assumed binding contracts are runtime-only and cannot supply compiler evaluation".into());
                }
            }
        }
        for (_, owner) in scope.pending_evaluation_conditions.iter() {
            if owner.0 == 0 || dependencies.contains(owner) {
                return Err("constant invocation retains a pending target proof condition in its demanded closure; it cannot establish its own pending target condition".into());
            }
        }
        let mut effects = CompilerEvaluationEffects::default();
        let mut closure_violation = None;
        let mut selection_violation =
            require_selection(requester, root_facts.custody, authority).err();
        for &machine in &closure.machines {
            let facts = declaration(scope, machine)?;
            for service in &facts.effects.services {
                if !effects.services.contains(service) {
                    effects.services.push(service.clone());
                }
            }
            effects.may_suspend |= facts.effects.may_suspend;
            effects.may_block |= facts.effects.may_block;
            if closure_violation.is_none() {
                closure_violation = facts
                    .static_application
                    .clone()
                    .or_else(|| facts.linear_carrier.clone());
            }
            if !facts.local_terminates
                && !selected
                    .iter()
                    .any(|(requirement, _)| *requirement == machine)
                && closure_violation.is_none()
            {
                closure_violation = Some(format!(
                    "machine call path `{}` has no ordinary checked `Terminates` guarantee",
                    facts.name
                ));
            }
            let requests = facts.selections.as_ref().map_err(Clone::clone)?;
            for request in requests {
                if let Err(reason) = require_selection(
                    DeclarationCustody::Package(request.requester),
                    DeclarationCustody::Package(request.owner),
                    authority,
                ) && selection_violation.is_none()
                {
                    selection_violation = Some(reason);
                }
            }
        }
        for edge in &closure.invocations {
            if !edge.provider
                && !edge.static_delegation
                && let (Ok(source), Ok(target)) = (
                    declaration(scope, edge.source.machine()),
                    declaration(scope, edge.target.machine()),
                )
                && let Err(reason) = require_selection(source.custody, target.custody, authority)
                && selection_violation.is_none()
            {
                selection_violation = Some(reason);
            }
            let mut pending = vec![edge.target.clone()];
            let mut visited = Vec::new();
            let mut recurrent = false;
            while let Some(application) = pending.pop() {
                if application == edge.source {
                    recurrent = true;
                    break;
                }
                if visited.contains(&application) {
                    continue;
                }
                visited.push(application.clone());
                pending.extend(
                    closure
                        .invocations
                        .iter()
                        .filter(|next| next.source == application)
                        .map(|next| next.target.clone()),
                );
            }
            if recurrent && !covered_recursive_edge(scope, edge) {
                closure_violation = Some("recursive machine-call cycle has no ordinary termination proof for its exact selected application edges".into());
                break;
            }
        }
        require_compiler_evaluation_floor(
            &root_facts.name,
            &effects,
            closure_violation.as_deref(),
            selection_violation.as_deref(),
        )?;
        Ok(AdmittedCompilerEvaluation {
            providers,
            artifact: artifact.custody_identity(),
            target: target.to_owned(),
            declaration: root,
            requester,
        })
    }

    pub(crate) struct CompilerInvocationScope {
        pub selected: Vec<(ProofDeclarationId, ProofDeclarationId)>,
        pub closure: semantic_vocabulary::proof_applications::ProofApplicationReachability<
            ProofDeclarationId,
        >,
        pub dependencies: Vec<ProofDeclarationId>,
    }

    pub(crate) fn compiler_invocation_scope<'scope>(
        scope: &ProofApplicationScope,
        target: impl Into<Option<&'scope str>>,
        providers: &crate::AdmittedProviderSelections,
        root: ProofDeclarationId,
    ) -> Result<CompilerInvocationScope, String> {
        let target = target.into();
        let root = scope
            .plan
            .callable_for_target(root, target)
            .ok_or("compiler-evaluation declaration has no selected target body")?;
        let mut selected = Vec::new();
        for (requirement, body) in providers.providers().chain(
            providers
                .assumed_bindings()
                .iter()
                .map(|binding| (binding.required, binding.implementation)),
        ) {
            selected.push((
                scope
                    .callable_declaration(TargetMachineCallable::Boundary(requirement))
                    .ok_or("compiler-evaluation provider requirement lost its declaration")?,
                scope
                    .callable_declaration(TargetMachineCallable::Machine(body))
                    .ok_or("compiler-evaluation selected body lost its declaration")?,
            ));
        }
        let closure = scope
            .applications()
            .invocation_reachability(target, &selected, &[root]);
        let mut dependencies = closure.machines.clone();
        dependencies.extend(closure.calls.iter().copied());
        for (_, machine) in scope.plan.machines.iter() {
            if closure.machines.contains(&machine.machine) {
                dependencies.extend(
                    scope
                        .plan
                        .symbols
                        .span_or_empty(machine.states)
                        .iter()
                        .copied(),
                );
            }
        }
        dependencies.extend(scope.plan.declaration_closure(&dependencies.clone()));
        Ok(CompilerInvocationScope {
            selected,
            closure,
            dependencies,
        })
    }

    fn header(
        scope: &ProofApplicationScope,
        declaration: ProofDeclarationId,
    ) -> Result<&terminal_psi::ProofCallableHeader, String> {
        let mut headers = scope
            .callable_headers
            .iter()
            .filter_map(|(_, header)| (header.declaration == declaration).then_some(header));
        let header = headers
            .next()
            .ok_or("compiler-evaluation declaration has no retained checked facts")?;
        if headers.next().is_some() {
            return Err("compiler-evaluation declaration has competing checked facts".into());
        }
        Ok(header)
    }

    fn declaration(
        scope: &ProofApplicationScope,
        declaration: ProofDeclarationId,
    ) -> Result<&CompilerEvaluationDeclaration, String> {
        header(scope, declaration)?
            .compiler_evaluation
            .as_ref()
            .map_err(Clone::clone)
    }

    fn require_selection(
        requester: DeclarationCustody,
        owner: DeclarationCustody,
        authority: Option<&dyn DeclarationSelectionAuthority>,
    ) -> Result<(), String> {
        match (requester, owner) {
            (DeclarationCustody::Missing, _) | (_, DeclarationCustody::Missing) => {
                Err("build-time invocation lacks compiler-owned source/package provenance".into())
            }
            // Exact checked selections bind the contract closure before lowering.
            // Its retained role never grants a package caller a default std edge.
            (DeclarationCustody::ToolchainContract, DeclarationCustody::Package(_))
            | (DeclarationCustody::Package(_), DeclarationCustody::ToolchainContract) => {
                Err("build-time invocation crosses closed-contract/package custody".into())
            }
            (DeclarationCustody::ToolchainContract, DeclarationCustody::ToolchainContract) => {
                Ok(())
            }
            (DeclarationCustody::Toolchain, _) | (_, DeclarationCustody::Toolchain) => Ok(()),
            (DeclarationCustody::Package(requester), DeclarationCustody::Package(owner))
                if requester == owner =>
            {
                Ok(())
            }
            (DeclarationCustody::Package(requester), DeclarationCustody::Package(owner))
                if authority.is_some_and(|authority| {
                    authority.allows_declaration_selection(requester, owner)
                }) =>
            {
                Ok(())
            }
            (DeclarationCustody::Package(requester), DeclarationCustody::Package(owner)) => {
                let authority = authority
                    .ok_or("build-time invocation has no direct dependency selection authority")?;
                Err(format!(
                    "build-time invocation selects package {} from package {} without direct dependency authority",
                    authority.package_label(owner),
                    authority.package_label(requester)
                ))
            }
            (DeclarationCustody::UnownedUser, _) | (_, DeclarationCustody::UnownedUser) => Err(
                "build-time invocation has user source without reconciled package custody".into(),
            ),
        }
    }

    #[test]
    fn closed_contract_custody_never_substitutes_for_a_package_edge() {
        struct AllowPackages;
        impl DeclarationSelectionAuthority for AllowPackages {
            fn allows_declaration_selection(
                &self,
                _: semantic_vocabulary::PackageKeyIdentity,
                _: semantic_vocabulary::PackageKeyIdentity,
            ) -> bool {
                true
            }
            fn package_label(&self, _: semantic_vocabulary::PackageKeyIdentity) -> String {
                "package".into()
            }
        }
        let package = DeclarationCustody::Package(
            semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap(),
        );
        let contract = DeclarationCustody::ToolchainContract;
        let core = DeclarationCustody::Toolchain;
        for (requester, owner) in [
            (package, contract),
            (contract, package),
            (DeclarationCustody::Missing, contract),
        ] {
            assert!(require_selection(requester, owner, Some(&AllowPackages)).is_err());
        }
        for (requester, owner) in [
            (contract, contract),
            (contract, core),
            (core, contract),
            (package, package),
        ] {
            assert!(require_selection(requester, owner, Some(&AllowPackages)).is_ok());
        }
        let other = DeclarationCustody::Package(
            semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32]).unwrap(),
        );
        assert!(require_selection(package, package, None).is_ok());
        assert!(require_selection(package, other, None).is_err());
        assert!(require_selection(package, other, Some(&AllowPackages)).is_ok());
        assert!(require_selection(DeclarationCustody::Missing, core, None).is_err());
    }

    fn covered_recursive_edge(
        scope: &ProofApplicationScope,
        edge: &semantic_vocabulary::proof_applications::ProofInvocationEdge<ProofDeclarationId>,
    ) -> bool {
        let Some(ordinal) = edge
            .call_ordinal
            .and_then(|ordinal| u32::try_from(ordinal).ok())
        else {
            return false;
        };
        !edge.provider
            && scope.recursive_components.iter().any(|(_, component)| {
                let members = scope.recursive_members.span_or_empty(component.members);
                members.contains(&edge.source.machine())
                    && members.contains(&edge.target.machine())
                    && scope
                        .recursive_calls
                        .span_or_empty(component.calls)
                        .iter()
                        .any(|call| {
                            call.caller == edge.source.machine()
                                && call.callee == edge.target.machine()
                                && call.call_ordinal == ordinal
                        })
            })
    }
}

pub(crate) mod synchronous_calls {
    use std::collections::{BTreeMap, BTreeSet};

    use semantic_vocabulary::{BoundaryMachineId, MachineId};
    use terminal_psi::{
        OperationKind, TargetMachineCallable, TerminalAffineCleanupAction, TerminalModule,
    };

    use crate::AdmittedTargetMachineSelections;

    pub struct SynchronousCallClosure<'module> {
        pub machines: Vec<&'module terminal_psi::TerminalMachine>,
        pub boundaries: BTreeSet<BoundaryMachineId>,
    }

    pub struct SynchronousCallGraph<'module> {
        module: &'module TerminalModule,
        machines: BTreeMap<MachineId, &'module terminal_psi::TerminalMachine>,
        target_members: Option<&'module AdmittedTargetMachineSelections>,
        entry_bridge: Option<(BoundaryMachineId, MachineId)>,
        boundary_calls: Vec<(BoundaryMachineId, MachineId)>,
    }

    impl<'module> SynchronousCallGraph<'module> {
        pub fn machine(
            &self,
            identity: MachineId,
        ) -> Option<&'module terminal_psi::TerminalMachine> {
            self.machines.get(&identity).copied()
        }

        pub fn new(
            module: &'module TerminalModule,
            target_members: Option<&'module AdmittedTargetMachineSelections>,
            entry_bridge: Option<(BoundaryMachineId, MachineId)>,
        ) -> Result<Self, String> {
            let mut machines = BTreeMap::new();
            for machine in &module.machines {
                if machines.insert(machine.id, machine).is_some() {
                    return Err(format!(
                        "synchronous call catalog repeats machine {:?}",
                        machine.id
                    ));
                }
            }
            Ok(Self {
                module,
                machines,
                target_members,
                entry_bridge,
                boundary_calls: Vec::new(),
            })
        }

        pub fn with_boundary_calls(
            mut self,
            calls: impl IntoIterator<Item = (BoundaryMachineId, MachineId)>,
        ) -> Result<Self, String> {
            self.boundary_calls.extend(calls);
            self.boundary_calls
                .sort_unstable_by_key(|(boundary, _)| *boundary);
            for &(boundary, machine) in &self.boundary_calls {
                if self.module.boundary_machine(boundary).is_none()
                    || !self.machines.contains_key(&machine)
                {
                    return Err("synchronous boundary call names an absent boundary or body".into());
                }
            }
            if self
                .boundary_calls
                .windows(2)
                .any(|rows| rows[0].0 == rows[1].0)
            {
                return Err("synchronous boundary call repeats a resolved boundary".into());
            }
            Ok(self)
        }

        pub fn boundary_callee(
            &self,
            boundary: BoundaryMachineId,
        ) -> Result<Option<MachineId>, String> {
            self.boundary_call(boundary).map(|(machine, _)| machine)
        }

        fn boundary_call(
            &self,
            boundary: BoundaryMachineId,
        ) -> Result<(Option<MachineId>, bool), String> {
            if self.module.boundary_machine(boundary).is_none() {
                return Err(format!(
                    "synchronous call names absent boundary {boundary:?}"
                ));
            }
            if let Some((_, entry)) = self.entry_bridge.filter(|(head, _)| *head == boundary) {
                return Ok((Some(entry), true));
            }
            match crate::admitted_boundary_member(self.module, self.target_members, boundary)? {
                Some(TargetMachineCallable::Machine(member)) => Ok((Some(member), false)),
                Some(TargetMachineCallable::Boundary(_)) | None => {
                    let callee = self
                        .boundary_calls
                        .binary_search_by_key(&boundary, |(boundary, _)| *boundary)
                        .ok()
                        .map(|index| self.boundary_calls[index].1);
                    Ok((callee, true))
                }
            }
        }

        pub fn from_root(
            &self,
            root: MachineId,
        ) -> Result<SynchronousCallClosure<'module>, String> {
            let mut pending = vec![TargetMachineCallable::Machine(root)];
            let mut visited = BTreeSet::new();
            let mut machines = Vec::new();
            let mut boundaries = BTreeSet::new();
            while let Some(callable) = pending.pop() {
                if !visited.insert(callable) {
                    continue;
                }
                if let TargetMachineCallable::Boundary(boundary) = callable {
                    let (callee, retain_boundary) = self.boundary_call(boundary)?;
                    if retain_boundary {
                        boundaries.insert(boundary);
                    }
                    if let Some(callee) = callee {
                        pending.push(TargetMachineCallable::Machine(callee));
                    }
                    continue;
                }
                let TargetMachineCallable::Machine(machine) = callable else {
                    continue;
                };
                let body =
                    self.machines.get(&machine).copied().ok_or_else(|| {
                        format!("synchronous call names absent machine {machine:?}")
                    })?;
                machines.push(body);
                pending.extend(
                    body.module_invocations()
                        .map(|call| TargetMachineCallable::Machine(call.inputs.callee)),
                );
                pending.extend(
                    body.boundary_invocations()
                        .map(|call| TargetMachineCallable::Boundary(call.boundary)),
                );
                for block in &body.blocks {
                    for (_, actions) in block.terminator.cleanup_rosters() {
                        for action in actions {
                            if let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action {
                                pending
                                    .push(TargetMachineCallable::Machine(cleanup.cleanup_machine));
                            }
                        }
                    }
                    for operation in &block.operations {
                        if let OperationKind::CallDynamic {
                            interface,
                            slot,
                            family_actuals,
                            ..
                        } = &operation.kind
                        {
                            pending.extend(
                                self.dynamic_callees(
                                    *interface,
                                    *slot,
                                    !family_actuals.is_empty(),
                                )?
                                .into_iter()
                                .map(TargetMachineCallable::Machine),
                            );
                        }
                    }
                }
            }
            Ok(SynchronousCallClosure {
                machines,
                boundaries,
            })
        }

        pub fn dynamic_callees(
            &self,
            interface_id: u32,
            slot: u32,
            finite_family: bool,
        ) -> Result<Vec<MachineId>, String> {
            let mut callees = Vec::new();
            let catalog = &self.module.dynamic_dispatch;
            let interface = catalog
                .interface(interface_id)
                .ok_or("synchronous dynamic call has no interface")?;
            let first = interface
                .slots
                .get(slot as usize)
                .filter(|row| row.slot == slot)
                .ok_or("synchronous dynamic call has no exact interface slot")?;
            let slots = interface
                .slots
                .iter()
                .skip(slot as usize)
                .take_while(|row| {
                    if !finite_family {
                        row.slot == slot
                    } else {
                        !row.family_tuple.is_empty()
                            && row.requirement_identity == first.requirement_identity
                    }
                });
            for row in slots {
                for table in &catalog.tables {
                    if table.interface != interface.id {
                        continue;
                    }
                    if let Some(callee) = table.rows.get(row.slot as usize).copied().flatten() {
                        callees.push(callee);
                    }
                }
            }
            Ok(callees)
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use terminal_psi::{
            BoundaryMachineResult, TerminalDynamicInterface, TerminalDynamicSlot,
            TerminalDynamicTable, TerminalIdentityTableBuilder,
        };

        #[test]
        fn dynamic_call_follows_the_invoked_slot_or_complete_finite_roster_only() {
            let mut identities = TerminalIdentityTableBuilder::default();
            let interface_identity = identities.intern("Shape");
            let first_requirement = identities.intern("Shape::first");
            let unrelated = identities.intern("Shape::other");
            let mut module = TerminalModule::declarations();
            let mut slots = Vec::new();
            for slot in 0..3 {
                slots.push(TerminalDynamicSlot {
                    slot,
                    declaring_trait_identity: interface_identity,
                    requirement_identity: if slot < 2 {
                        first_requirement
                    } else {
                        unrelated
                    },
                    family_tuple: vec![slot.to_string()],
                    scalar_parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    entry_claim_paths: Vec::new(),
                    result: BoundaryMachineResult::Unit,
                    result_claim_paths: Vec::new(),
                    result_reference_regions: Vec::new(),
                });
            }
            module
                .dynamic_dispatch
                .interfaces
                .push(TerminalDynamicInterface {
                    id: 0,
                    identity: interface_identity,
                    slots,
                });
            module.dynamic_dispatch.tables.push(TerminalDynamicTable {
                id: 0,
                interface: 0,
                implementation_identity: identities.intern("Selected"),
                rows: (1..=3).map(MachineId::new).collect(),
            });
            module.identities = identities.build();
            let calls = SynchronousCallGraph::new(&module, None, None).unwrap();
            let pending = calls.dynamic_callees(0, 0, false).unwrap();
            assert_eq!(pending, vec![MachineId::new(1).unwrap()]);
            let pending = calls.dynamic_callees(0, 0, true).unwrap();
            assert_eq!(
                pending,
                vec![MachineId::new(1).unwrap(), MachineId::new(2).unwrap(),]
            );
            assert!(calls.dynamic_callees(0, 3, false).is_err());
            assert!(calls.dynamic_callees(1, 0, false).is_err());
        }
    }
}
