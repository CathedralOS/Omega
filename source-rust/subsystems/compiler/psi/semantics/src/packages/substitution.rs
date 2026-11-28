//! Instantiate a package signature without weakening its callable contract.
use super::*;

mod storage;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Comparison {
    Storage(StructuralTypeId, StructuralTypeId),
    Callable(CallableContractId, CallableContractId),
}

pub(super) struct Substitution<'a> {
    pub(super) public: &'a [StructuralTypeId],
    pub(super) actual: &'a [StructuralTypeId],
}

impl Substitution<'_> {
    fn identities<'a>(&self, module: &'a TerminalModule) -> Option<Vec<(&'a str, &'a str)>> {
        let name = |id| module.identities.text(module.structural_type(id)?.identity);
        self.public
            .iter()
            .zip(self.actual)
            .map(|(&from, &to)| Some((name(from)?, name(to)?)))
            .collect()
    }

    fn witness(&self, id: StructuralTypeId) -> Option<StructuralTypeId> {
        self.public
            .iter()
            .position(|formal| *formal == id)
            .map(|index| self.actual[index])
    }

    fn parameters(
        &self,
        module: &TerminalModule,
        parameters: &mut [StructuralParameterDeclaration],
        supplied: &[StructuralParameterDeclaration],
        nested: &mut Vec<Comparison>,
    ) -> bool {
        if parameters.len() != supplied.len() {
            return false;
        }
        for (parameter, actual) in parameters.iter_mut().zip(supplied) {
            if let Some(to) = self.witness(parameter.structural_type) {
                if parameter.access == StructuralAccess::Owned {
                    return false;
                }
                parameter.structural_type = to;
            } else if self
                .public
                .iter()
                .any(|witness| fields::nominal_uses(module, parameter.structural_type, *witness))
            {
                nested.push(Comparison::Storage(
                    parameter.structural_type,
                    actual.structural_type,
                ));
                if !self.projected_qualifications(
                    module,
                    &parameter.projected_qualifications,
                    &actual.projected_qualifications,
                ) || !self.projected_qualifications(
                    module,
                    &parameter.additional_input_qualifications,
                    &actual.additional_input_qualifications,
                ) {
                    return false;
                }
                parameter
                    .projected_qualifications
                    .clone_from(&actual.projected_qualifications);
                parameter
                    .additional_input_qualifications
                    .clone_from(&actual.additional_input_qualifications);
                parameter.structural_type = actual.structural_type;
            }
        }
        true
    }

    fn projected_qualifications(
        &self,
        module: &TerminalModule,
        public: &[terminal_psi::StructuralPathQualification],
        actual: &[terminal_psi::StructuralPathQualification],
    ) -> bool {
        if public == actual {
            return true;
        }
        if public.len() != actual.len() {
            return false;
        }
        // Domain IDs are local to an application. Resident<P,S> and
        // Resident<P,File> can sort on different sides of Granted. Every
        // obligation must still have one match on exactly its field path.
        let mut remaining: Vec<_> = actual.iter().collect();
        for required in public {
            let Some(index) = remaining.iter().position(|supplied| {
                required.path == supplied.path
                    && self.domain(module, required.domain, supplied.domain)
            }) else {
                return false;
            };
            remaining.swap_remove(index);
        }
        true
    }

    fn domain(
        &self,
        module: &TerminalModule,
        from: semantic_vocabulary::StructuralDomainId,
        to: semantic_vocabulary::StructuralDomainId,
    ) -> bool {
        if from == to {
            return true;
        }
        let (Some(public), Some(actual)) =
            (module.structural_domain(from), module.structural_domain(to))
        else {
            return false;
        };
        let (Some(public_name), Some(actual_name), Some(witnesses)) = (
            module.identities.text(public.identity),
            module.identities.text(actual.identity),
            self.identities(module),
        ) else {
            return false;
        };
        let substitutions: Vec<_> = witnesses.iter().map(|&(name, _)| name).collect();
        let Some(arguments) = application_identity::type_identity::package_applications::supplied(
            public_name,
            actual_name,
            &substitutions,
        ) else {
            return false;
        };
        if arguments.is_empty()
            || !arguments.iter().all(|&(formal, argument)| {
                witnesses
                    .iter()
                    .enumerate()
                    .any(|(index, &(public, concrete))| {
                        formal == public
                            && (argument == concrete
                                || module
                                    .structural_type(self.actual[index])
                                    .is_some_and(|row| {
                                        primitive_domain_argument(&row.shape, concrete, argument)
                                    }))
                    })
            })
        {
            return false;
        }
        // Preserve subject, carrier, establishment routes, predicate,
        // prerequisite memberships, access policy and content contract.
        let mut expected = public.clone();
        expected.id = actual.id;
        expected.semantic_domain = actual.semantic_domain;
        expected.identity = actual.identity;
        expected == *actual
    }

    fn signature(
        &self,
        module: &TerminalModule,
        from: CallableContractId,
        to: CallableContractId,
        nested: &mut Vec<Comparison>,
    ) -> bool {
        let catalog = &module.execution.callables;
        let (Some(public), Some(actual)) = (catalog.contract(from), catalog.contract(to)) else {
            return false;
        };
        let mut expected = public.clone();
        expected.id = actual.id;
        expected.identity = actual.identity;
        expected.proof.id = actual.proof.id;
        // Each contract owns its published guarantee IDs. Pair the clauses in
        // declaration order; equality below still compares their count and
        // every proposition, including the formal and result coordinates.
        for (public, opened) in expected.proof.ensures.iter_mut().zip(&actual.proof.ensures) {
            public.obligation = opened.obligation;
        }
        if !self.parameters(
            module,
            &mut expected.structural_parameters,
            &actual.structural_parameters,
            nested,
        ) || !self.parameters(
            module,
            &mut expected.proof.structural_parameters,
            &actual.proof.structural_parameters,
            nested,
        ) {
            return false;
        }
        // This equality includes the proof header, checked effects, evidence
        // lanes, reference regions, scalar qualifications and result.
        expected == *actual
    }

    fn field(
        public: &StructuralFieldType,
        actual: &StructuralFieldType,
        nested: &mut Vec<Comparison>,
    ) -> bool {
        let comparison = match (public, actual) {
            (StructuralFieldType::Structural(from), StructuralFieldType::Structural(to)) => {
                Comparison::Storage(*from, *to)
            }
            (
                StructuralFieldType::Scalar(ScalarType::Callable(from)),
                StructuralFieldType::Scalar(ScalarType::Callable(to)),
            ) => Comparison::Callable(*from, *to),
            _ => return public == actual,
        };
        nested.push(comparison);
        true
    }

    fn check(&self, module: &TerminalModule, mut pending: Vec<Comparison>) -> bool {
        // Shared subtypes occur in multiple fields and in both contract headers.
        // Each pair denotes the same comparison throughout this substitution.
        let mut seen = std::collections::BTreeSet::new();
        while let Some(pair) = pending.pop() {
            if !seen.insert(pair) {
                continue;
            }
            let matches = match pair {
                Comparison::Storage(from, to) => self.storage(module, from, to, &mut pending),
                Comparison::Callable(from, to) => self.signature(module, from, to, &mut pending),
            };
            if !matches {
                return false;
            }
        }
        true
    }

    pub(super) fn callable(
        &self,
        module: &TerminalModule,
        from: CallableContractId,
        to: CallableContractId,
    ) -> bool {
        self.check(module, vec![Comparison::Callable(from, to)])
    }

    pub(super) fn admits(
        &self,
        module: &TerminalModule,
        public: &StructuralFieldType,
        actual: &StructuralFieldType,
    ) -> bool {
        let mut pending = Vec::new();
        Self::field(public, actual, &mut pending) && self.check(module, pending)
    }
}

// Domain indices retain the compiler-owned identity of a builtin. A scalar
// storage row uses its source spelling. Authenticate both spellings against
// the actual primitive representation before identifying them.
fn primitive_domain_argument(shape: &StructuralTypeShape, storage: &str, argument: &str) -> bool {
    use application_identity::type_identity::encoding::{atom, compound};
    use application_identity::type_identity::vocabulary::{Label, NameForm, TypeForm};
    use numerics::integer_carrier::FixedInteger;
    use semantic_vocabulary::{IeeeFloatFormat, IntegerCarrier};

    let StructuralTypeShape::PrimitiveScalar(scalar) = shape else {
        return false;
    };
    let name = match scalar {
        ScalarType::Boolean => "bool".to_owned(),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => "f32".to_owned(),
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => "f64".to_owned(),
        ScalarType::Integer(integer) if integer.carrier() == IntegerCarrier::Fixed => {
            let Some(fixed) = integer
                .bits()
                .and_then(|bits| FixedInteger::new(integer.sign(), u64::from(bits)))
            else {
                return false;
            };
            fixed.to_string()
        }
        _ => return false,
    };
    let named = |value: &str| compound(TypeForm::Named, [atom(Label::Name, value)]);
    storage == named(&name)
        && argument
            == named(&compound(
                NameForm::CompilerType,
                [atom(Label::Atom, &name)],
            ))
}
