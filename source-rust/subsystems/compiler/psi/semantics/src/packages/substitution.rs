//! Instantiate a package signature without weakening its callable contract.
use super::*;

pub(super) struct Substitution<'a> {
    pub(super) public: &'a [StructuralTypeId],
    pub(super) actual: &'a [StructuralTypeId],
}

impl Substitution<'_> {
    fn witness(&self, id: StructuralTypeId) -> Option<StructuralTypeId> {
        self.public
            .iter()
            .position(|formal| *formal == id)
            .map(|index| self.actual[index])
    }

    // A stored reference has fixed representation. Its access and every
    // declaration property other than the referent and nominal identity stay
    // identical. Owning an unknown value inline has no such rule.
    fn reference(
        &self,
        module: &TerminalModule,
        from: StructuralTypeId,
        to: StructuralTypeId,
    ) -> bool {
        let (Some(public), Some(actual)) =
            (module.structural_type(from), module.structural_type(to))
        else {
            return false;
        };
        let (
            StructuralTypeShape::Reference { referent, access },
            StructuralTypeShape::Reference {
                referent: target,
                access: mode,
            },
        ) = (&public.shape, &actual.shape)
        else {
            return false;
        };
        if self.witness(*referent) != Some(*target) || access != mode {
            return false;
        }
        let mut expected = public.clone();
        expected.id = actual.id;
        expected.identity = actual.identity;
        expected.declaration_identity = actual.declaration_identity;
        expected.shape = actual.shape.clone();
        expected == *actual
    }

    fn parameters(&self, parameters: &mut [StructuralParameterDeclaration]) -> bool {
        for parameter in parameters {
            if let Some(to) = self.witness(parameter.structural_type) {
                if parameter.access == StructuralAccess::Owned {
                    return false;
                }
                parameter.structural_type = to;
            }
        }
        true
    }

    pub(super) fn callable(
        &self,
        catalog: &terminal_psi::TerminalCallableCatalog,
        from: CallableContractId,
        to: CallableContractId,
    ) -> bool {
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
        if !self.parameters(&mut expected.structural_parameters)
            || !self.parameters(&mut expected.proof.structural_parameters)
        {
            return false;
        }
        // This equality includes the proof header, checked effects, evidence
        // lanes, reference regions, scalar qualifications and result.
        expected == *actual
    }

    pub(super) fn admits(
        &self,
        module: &TerminalModule,
        public: &StructuralFieldType,
        actual: &StructuralFieldType,
    ) -> bool {
        match (public, actual) {
            (StructuralFieldType::Structural(from), StructuralFieldType::Structural(to)) => {
                if let Some(StructuralTypeShape::Reference { referent, .. }) =
                    module.structural_type(*from).map(|ty| &ty.shape)
                    && self.witness(*referent).is_some()
                {
                    self.reference(module, *from, *to)
                } else {
                    from == to && self.witness(*from).is_none()
                }
            }
            (
                StructuralFieldType::Scalar(ScalarType::Callable(from)),
                StructuralFieldType::Scalar(ScalarType::Callable(to)),
            ) => self.callable(&module.execution.callables, *from, *to),
            _ => public == actual,
        }
    }
}
