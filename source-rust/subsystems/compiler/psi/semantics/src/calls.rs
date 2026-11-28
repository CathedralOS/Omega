/*
Semantic judgments for direct, indirect, and boundary call sites, argument/result binding, crash
continuation routing, and call-graph reachability.
*/

pub(crate) mod call_composition;
pub mod closed_conformances;
pub mod closed_reach;
pub mod operational_dependencies;
pub mod evidence_calls;
pub(crate) mod invocation_inputs;
pub mod layout_target;

/// Whether one call's scalar actuals establish the exact extent indices of
/// its structural actual. Parameter identities stay local to each machine;
/// the invocation supplies the only substitution between them.
pub fn const_array_argument_matches(
    module: &terminal_psi::TerminalModule,
    caller: &terminal_psi::TerminalMachine,
    callee: &terminal_psi::TerminalMachine,
    arguments: &[semantic_vocabulary::ValueId],
    actual: semantic_vocabulary::StructuralTypeId,
    expected: semantic_vocabulary::StructuralTypeId,
) -> bool {
    dependent_argument_matches(module, caller, callee, arguments, actual, expected, None)
}

/// A checked source correspondence supplies nominal equality; the lowered
/// representation still supplies and checks access, fields and array geometry.
pub fn nominal_call_type_matches(
    module: &terminal_psi::TerminalModule,
    caller: &terminal_psi::TerminalMachine,
    callee: &terminal_psi::TerminalMachine,
    arguments: &[semantic_vocabulary::ValueId],
    row: &terminal_psi::generic_calls::NominalCallType,
) -> bool {
    if row.caller != caller.id
        || row.callee != callee.id
        || row.actual_application.is_absent()
        || row.actual_application != row.expected_application
        || row.declaration.is_absent()
        || row.const_arguments.is_empty()
    {
        return false;
    }
    let constructor = |mut ty| {
        let mut remaining = module.types.structural_types.len();
        while remaining > 0 {
            remaining -= 1;
            let declaration = module.structural_type(ty)?;
            if !declaration.declaration_identity.is_absent()
                && !matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::TypeParameter
                )
            {
                return Some(ty);
            }
            ty = match declaration.shape {
                terminal_psi::StructuralTypeShape::Reference {
                    referent: child, ..
                }
                | terminal_psi::StructuralTypeShape::ElementView { element: child, .. }
                | terminal_psi::StructuralTypeShape::FixedArray { element: child, .. } => child,
                _ => return None,
            };
        }
        None
    };
    if constructor(row.actual) != Some(row.actual_constructor)
        || constructor(row.expected) != Some(row.expected_constructor)
        || module
            .identities
            .text(row.actual_application)
            .is_none_or(str::is_empty)
    {
        return false;
    }
    for constructor in [row.actual_constructor, row.expected_constructor] {
        if !module
            .structural_type(constructor)
            .is_some_and(|ty| ty.declaration_identity == row.declaration)
        {
            return false;
        }
    }
    let expressions = crate::descriptor_results::TerminalScalarExpressions::new(caller);
    let mut previous = None;
    for binding in &row.const_arguments {
        if previous.is_some_and(|previous| previous >= binding.parameter) {
            return false;
        }
        previous = Some(binding.parameter);
        let Some(actual) = arguments.get(binding.parameter as usize) else {
            return false;
        };
        let Some(formal) = callee.parameters.get(binding.parameter as usize) else {
            return false;
        };
        if formal.scalar_type != binding.actual.scalar_type()
            || expressions.expression(*actual).as_ref() != Some(&binding.actual)
        {
            return false;
        }
    }
    dependent_argument_matches(
        module,
        caller,
        callee,
        arguments,
        row.actual,
        row.expected,
        Some(&row.const_arguments),
    )
}

fn dependent_argument_matches(
    module: &terminal_psi::TerminalModule,
    caller: &terminal_psi::TerminalMachine,
    callee: &terminal_psi::TerminalMachine,
    arguments: &[semantic_vocabulary::ValueId],
    actual: semantic_vocabulary::StructuralTypeId,
    expected: semantic_vocabulary::StructuralTypeId,
    const_arguments: Option<&[terminal_psi::generic_calls::NominalConstArgument]>,
) -> bool {
    use terminal_psi::{ArrayExtent, StructuralTypeShape};
    if arguments.len() != callee.parameters.len() {
        return false;
    }
    let mut pending = vec![(actual, expected)];
    let mut visited = std::collections::BTreeSet::new();
    while let Some((actual, expected)) = pending.pop() {
        if !visited.insert((actual, expected)) {
            continue;
        }
        let identical = actual == expected;
        let (Some(actual), Some(expected)) = (
            module.structural_type(actual),
            module.structural_type(expected),
        ) else {
            if identical
                && module
                    .types
                    .structural_referents
                    .iter()
                    .any(|referent| referent.id == expected && referent.abstract_opaque)
            {
                continue;
            }
            return false;
        };
        // Equal local types still contain invocation-dependent equations in
        // a recursive call. Inspect their selected children before accepting
        // identity; a caller may forward a different scalar to the same N.
        if identical {
            pending.extend(expected.quotient_carrier.map(|child| (child, child)));
            if let Some(restriction) = &expected.scalar_restriction {
                pending.extend(restriction.as_structural().map(|child| (child, child)));
            }
        }
        if identical && !matches!(expected.shape, StructuralTypeShape::FixedArray { .. }) {
            match &expected.shape {
                StructuralTypeShape::Reference { referent, .. }
                | StructuralTypeShape::ElementView {
                    element: referent, ..
                } => {
                    pending.push((*referent, *referent));
                }
                shape => {
                    for (_, field) in shape.fields_with_case() {
                        pending.extend(field.inline_carrier.map(|child| (child, child)));
                        pending
                            .extend(field.field_type.as_structural().map(|child| (child, child)));
                    }
                }
            }
            continue;
        }
        if !identical
            && (actual.declaration_identity != expected.declaration_identity
                || actual.owns_linear_claim != expected.owns_linear_claim
                || actual.multiplicity != expected.multiplicity
                || actual.no_code_affine_cleanup != expected.no_code_affine_cleanup
                || actual.descriptor_cleanup != expected.descriptor_cleanup
                || actual.nominal_cleanup.is_some()
                || expected.nominal_cleanup.is_some()
                || actual.quotient_carrier.is_some()
                || expected.quotient_carrier.is_some()
                || actual.scalar_restriction.is_some()
                || expected.scalar_restriction.is_some()
                || !dependent_requirements_match(module, actual, expected, callee, const_arguments))
        {
            return false;
        }
        if const_arguments.is_some() {
            let children = match (&actual.shape, &expected.shape) {
                (
                    StructuralTypeShape::Reference {
                        referent: left,
                        access: la,
                    },
                    StructuralTypeShape::Reference {
                        referent: right,
                        access: ra,
                    },
                ) if la == ra => Some((*left, *right)),
                (
                    StructuralTypeShape::ElementView {
                        element: left,
                        access: la,
                    },
                    StructuralTypeShape::ElementView {
                        element: right,
                        access: ra,
                    },
                ) if la == ra => Some((*left, *right)),
                _ => None,
            };
            if let Some(children) = children {
                pending.push(children);
                continue;
            }
            let same_shape = match (&actual.shape, &expected.shape) {
                (
                    StructuralTypeShape::Record { storage: left, .. },
                    StructuralTypeShape::Record { storage: right, .. },
                ) => left == right,
                (StructuralTypeShape::Sum { .. }, StructuralTypeShape::Sum { .. })
                | (StructuralTypeShape::Mixed { .. }, StructuralTypeShape::Mixed { .. }) => true,
                _ => false,
            };
            if same_shape {
                if actual.declaration_identity.is_absent()
                    || actual.shape.cases().len() != expected.shape.cases().len()
                    || actual.shape.cases().iter().zip(expected.shape.cases()).any(
                        |(left, right)| {
                            left.identity != right.identity
                                || left.declaration_identity != right.declaration_identity
                                || left.fields.len() != right.fields.len()
                        },
                    )
                {
                    return false;
                }
                let mut left = actual.shape.fields();
                let mut right = expected.shape.fields();
                loop {
                    match (left.next(), right.next()) {
                        (None, None) => break,
                        (Some(left), Some(right)) => {
                            if left.identity != right.identity
                                || left.declaration_identity != right.declaration_identity
                                || left.relevance != right.relevance
                                || left.storage != right.storage
                            {
                                return false;
                            }
                            match (&left.field_type, &right.field_type) {
                                (
                                    terminal_psi::StructuralFieldType::Structural(left),
                                    terminal_psi::StructuralFieldType::Structural(right),
                                ) => pending.push((*left, *right)),
                                (left, right) if left == right => {}
                                _ => return false,
                            }
                            match (left.inline_carrier, right.inline_carrier) {
                                (Some(left), Some(right)) => pending.push((left, right)),
                                (None, None) => {}
                                _ => return false,
                            }
                        }
                        _ => return false,
                    }
                }
                continue;
            }
        }
        let (
            StructuralTypeShape::FixedArray {
                element: ae,
                length: al,
            },
            StructuralTypeShape::FixedArray {
                element: ee,
                length: el,
            },
        ) = (&actual.shape, &expected.shape)
        else {
            return false;
        };
        let length = if let ArrayExtent::Parameter { owner, value } = el {
            if *owner != callee.id {
                return false;
            }
            let Some(position) = callee
                .parameters
                .iter()
                .position(|formal| formal.id == *value)
            else {
                return false;
            };
            let Some(actual_value) = arguments.get(position) else {
                return false;
            };
            match al {
                ArrayExtent::Parameter { owner, value } => {
                    *owner == caller.id && value == actual_value
                }
                ArrayExtent::Literal(length) => {
                    crate::constant_runtime_index(caller, *actual_value) == Some(*length)
                }
                ArrayExtent::Application(_) => false,
            }
        } else {
            al == el
        };
        if !length {
            return false;
        }
        pending.push((*ae, *ee));
    }
    true
}

fn dependent_requirements_match(
    module: &terminal_psi::TerminalModule,
    actual: &terminal_psi::StructuralTypeDeclaration,
    expected: &terminal_psi::StructuralTypeDeclaration,
    callee: &terminal_psi::TerminalMachine,
    const_arguments: Option<&[terminal_psi::generic_calls::NominalConstArgument]>,
) -> bool {
    let Some(const_arguments) = const_arguments else {
        return actual.requires == expected.requires;
    };
    if actual.requires.len() != expected.requires.len() {
        return false;
    }
    // Record clauses retain their owning machine's const value IDs. Compare
    // the callee's clauses after binding those IDs to the checked actuals.
    let substitute = |value| {
        const_arguments.iter().find_map(|argument| {
            let formal = callee.parameters.get(argument.parameter as usize)?;
            (formal.id == value).then_some(&argument.actual)
        })
    };
    let actual_type = actual.id;
    let expected_type = expected.id;
    actual
        .requires
        .iter()
        .zip(&expected.requires)
        .all(|(actual, expected)| {
            let mut bound = true;
            let complete = expected.visit_value_ids(|value| bound &= substitute(value).is_some());
            if !bound {
                return false;
            }
            let mut expected = expected.clone();
            if !complete
                && !rebind_requirements_paths(&mut expected, &|path| {
                    corresponding_record_path(module, actual_type, expected_type, path)
                })
            {
                return false;
            }
            *actual
                == crate::substitution::substitute_proposition_values_with(&expected, &substitute)
        })
}

fn corresponding_record_path(
    module: &terminal_psi::TerminalModule,
    actual: semantic_vocabulary::StructuralTypeId,
    expected: semantic_vocabulary::StructuralTypeId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
) -> Option<Vec<semantic_vocabulary::CanonicalStructuralPathSegment>> {
    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::CanonicalStructuralPathSegment;

    // A case selects a field namespace inside its sum. It does not
    // select another structural type until its payload field is chosen.
    fn field_owner(
        prefix: &[CanonicalStructuralPathSegment],
    ) -> (
        &[CanonicalStructuralPathSegment],
        Option<semantic_vocabulary::StructuralCaseId>,
    ) {
        match prefix.split_last() {
            Some((CanonicalStructuralPathSegment::Case(case), parent)) => (parent, Some(*case)),
            _ => (prefix, None),
        }
    }

    let types = module.types.structural_types.iter();
    let mut translated = Vec::with_capacity(path.len());
    for (position, step) in path.iter().enumerate() {
        let (from, source_case) = field_owner(&path[..position]);
        let (to, destination_case) = field_owner(&translated);
        if source_case.is_some() && !matches!(step, CanonicalStructuralPathSegment::Field(_)) {
            return None;
        }
        let source = crate::canonical_structural_path_tip(types.clone(), expected, from)?;
        let destination = crate::canonical_structural_path_tip(types.clone(), actual, to)?;
        let step = match *step {
            CanonicalStructuralPathSegment::Field(id) => {
                let (_, field) = exactly_one(source.shape.fields_with_case(), |(case, field)| {
                    case.map(|case| case.id) == source_case && field.id == id
                })?;
                let (_, corresponding) =
                    exactly_one(destination.shape.fields_with_case(), |(case, candidate)| {
                        case.map(|case| case.id) == destination_case
                            && candidate.identity == field.identity
                            && candidate.declaration_identity == field.declaration_identity
                    })?;
                CanonicalStructuralPathSegment::Field(corresponding.id)
            }
            CanonicalStructuralPathSegment::Case(id) => {
                let case = exactly_one(source.shape.cases().iter(), |case| case.id == id)?;
                let corresponding = exactly_one(destination.shape.cases().iter(), |candidate| {
                    candidate.identity == case.identity
                        && candidate.declaration_identity == case.declaration_identity
                })?;
                CanonicalStructuralPathSegment::Case(corresponding.id)
            }
            step => step,
        };
        translated.push(step);
    }
    Some(translated)
}

fn rebind_requirements_paths(
    clause: &mut semantic_vocabulary::Proposition,
    path: &impl Fn(
        &[semantic_vocabulary::CanonicalStructuralPathSegment],
    ) -> Option<Vec<semantic_vocabulary::CanonicalStructuralPathSegment>>,
) -> bool {
    use semantic_vocabulary::{IntegerMathTerm, Proposition, ScalarTerm};

    let mut clauses = vec![clause];
    let mut terms = Vec::new();
    let mut math = Vec::new();
    while let Some(clause) = clauses.pop() {
        match clause {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right)
            | Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                terms.extend([left, right]);
            }
            Proposition::IntegerMathEqual(left, right)
            | Proposition::IntegerMathLessThan(left, right)
            | Proposition::IntegerMathLessOrEqual(left, right) => {
                math.extend([left, right]);
            }
            Proposition::Conjunction(clauses_to_visit)
            | Proposition::Disjunction(clauses_to_visit) => clauses.extend(clauses_to_visit),
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                clauses.extend([premise.as_mut(), conclusion.as_mut()]);
            }
            Proposition::StructuralCaseMembership { subject, case } => {
                if subject.root().get() != 1 {
                    return false;
                }
                let mut selection = subject.path().to_vec();
                selection.push(semantic_vocabulary::CanonicalStructuralPathSegment::Case(
                    *case,
                ));
                let Some(mut rebound) = path(&selection) else {
                    return false;
                };
                let Some(semantic_vocabulary::CanonicalStructuralPathSegment::Case(selected)) =
                    rebound.pop()
                else {
                    return false;
                };
                *subject = semantic_vocabulary::StructuralCaseSubject::new(subject.root(), rebound);
                *case = selected;
            }
            Proposition::Truth | Proposition::Falsehood => {}
            _ => return false,
        }
    }
    while let Some(term) = math.pop() {
        if let IntegerMathTerm::Embed(scalar) = term {
            terms.push(scalar.as_mut());
        } else {
            math.extend(term.operands_mut());
        }
    }
    while let Some(term) = terms.pop() {
        match term {
            semantic_vocabulary::scalar_term_predicate_quint_pattern!(path: current, root, ..) => {
                if root.get() != 1 {
                    return false;
                }
                let Some(rebound) = path(current) else {
                    return false;
                };
                *current = rebound;
            }
            ScalarTerm::StructuralRead { .. } => return false,
            _ => terms.extend(term.operands_mut()),
        }
    }
    true
}

pub mod callable_refinement;
