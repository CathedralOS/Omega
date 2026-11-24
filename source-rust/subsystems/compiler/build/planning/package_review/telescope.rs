//! Generic declaration identities use the same ordered binders as checked signatures.
use super::{declarations::unavailable, input::PolicySource};
use diagnostics::Diagnostic;
use review_evidence::record::PackageReviewTypeIdentity;
use review_evidence::record::{
    PackagePolicyMachineParameterContract, PackagePolicyTypeParameter,
    PackagePolicyTypeParameterKind, PackageReviewConformanceBound, PackageReviewDataProperties,
};
use source_compiler::representations::source_state::{
    data::{MachineParameterContract, TypeParameterKind},
    machine::GenericConformanceBound,
    name::Identifier,
};
use source_compiler::representations::source_state::{
    type_identity::{
        AppliedTypeIdentityScope, ExactOwnerTypeIdentityRequest, LifetimeParameterIdentity,
        declaration_lifetime_parameters,
    },
    types::TypeReferenceHandle,
};
use symbols::SymbolHandle;

/// Declaration binders keep their two lexical namespaces when a boundary
/// requirement is reviewed as part of its service.
#[derive(Default, Clone)]
pub(super) struct SignatureScope {
    binders: Vec<(SymbolHandle, String)>,
    lifetimes: Vec<(LifetimeParameterIdentity, usize)>,
}

impl SignatureScope {
    fn extend(
        &mut self,
        input: &PolicySource<'_>,
        owner: SymbolHandle,
        name: impl Fn(usize) -> String,
    ) -> Result<(), Vec<Diagnostic>> {
        let program = input.typed();
        for (ordinal, binder) in program
            .declaration_type_parameters(owner)
            .iter()
            .enumerate()
        {
            if !binder.symbol.is_valid()
                || self
                    .binders
                    .iter()
                    .any(|(symbol, _)| *symbol == binder.symbol)
            {
                return Err(unavailable(
                    "a unique static binder in the checked signature",
                ));
            }
            self.binders.push((binder.symbol, name(ordinal)));
        }
        for ordinal in 0..declaration_lifetime_parameters(program, owner)
            .unwrap_or_default()
            .len()
        {
            self.lifetimes.push((
                LifetimeParameterIdentity {
                    declaration: owner,
                    ordinal,
                },
                self.lifetimes.len(),
            ));
        }
        Ok(())
    }

    /// Public type identities number the enclosing trait's binders before its method's.
    pub(super) fn declaration(
        input: &PolicySource<'_>,
        owners: &[SymbolHandle],
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut scope = Self::default();
        for &owner in owners {
            let start = scope.binders.len();
            scope.extend(input, owner, |index| format!("$T{}", start + index))?;
        }
        Ok(scope)
    }

    pub(super) fn service(
        input: &PolicySource<'_>,
        symbol: SymbolHandle,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut scope = Self::default();
        scope.extend(input, symbol, |index| format!("service-parameter:{index}"))?;
        Ok(scope)
    }

    pub(super) fn method(
        &self,
        input: &PolicySource<'_>,
        symbol: SymbolHandle,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut scope = self.clone();
        scope.extend(input, symbol, |index| format!("type-parameter:{index}"))?;
        Ok(scope)
    }
}

impl PolicySource<'_> {
    pub(super) fn type_parameters(
        &self,
        owner: SymbolHandle,
    ) -> Result<Vec<PackagePolicyTypeParameter>, Vec<Diagnostic>> {
        let scope = SignatureScope::declaration(self, &[owner])?;
        self.scoped_parameters(owner, &scope)
    }

    pub(super) fn scoped_parameters(
        &self,
        owner: SymbolHandle,
        scope: &SignatureScope,
    ) -> Result<Vec<PackagePolicyTypeParameter>, Vec<Diagnostic>> {
        self.typed()
            .declaration_type_parameters(owner)
            .iter()
            .map(|parameter| {
                let kind = match &parameter.kind {
                    TypeParameterKind::Type => PackagePolicyTypeParameterKind::Type,
                    TypeParameterKind::Implementation => {
                        PackagePolicyTypeParameterKind::Implementation
                    }
                    TypeParameterKind::Const { type_reference } => {
                        PackagePolicyTypeParameterKind::Const(self.scoped_type(
                            owner,
                            *type_reference,
                            scope,
                        )?)
                    }
                    TypeParameterKind::Value { type_reference } => {
                        PackagePolicyTypeParameterKind::Value(self.scoped_type(
                            owner,
                            *type_reference,
                            scope,
                        )?)
                    }
                    TypeParameterKind::Machine {
                        contract: MachineParameterContract::RequirementIdentity,
                    } => PackagePolicyTypeParameterKind::Machine(
                        PackagePolicyMachineParameterContract::RequirementIdentity,
                    ),
                    TypeParameterKind::Machine { .. }
                    | TypeParameterKind::Proof { .. }
                    | TypeParameterKind::Proposition { .. } => {
                        return Err(unavailable(
                            "a higher-order generic parameter's complete contract",
                        ));
                    }
                };
                Ok(PackagePolicyTypeParameter {
                    kind,
                    bounds: PackageReviewDataProperties {
                        multiplicity: parameter.bounds.multiplicity,
                    },
                })
            })
            .collect()
    }

    pub(super) fn conformance_bounds(
        &self,
        owner: SymbolHandle,
        lifetimes: &[Identifier],
        bounds: &[GenericConformanceBound],
    ) -> Result<Vec<PackageReviewConformanceBound>, Vec<Diagnostic>> {
        let parameters = self.typed().declaration_type_parameters(owner);
        let mut rows = Vec::new();
        for bound in bounds {
            let ordinal = parameters
                .iter()
                .position(|p| p.symbol == bound.binder)
                .and_then(|ordinal| u32::try_from(ordinal).ok())
                .ok_or_else(|| unavailable("a conformance bound's binder ordinal"))?;
            let lifetime_arguments = bound
                .lifetime_arguments
                .iter()
                .map(|name| {
                    lifetimes
                        .iter()
                        .position(|p| p.as_str() == name.as_str())
                        .and_then(|i| u32::try_from(i).ok())
                        .ok_or_else(|| unavailable("a conformance bound's declared lifetime"))
                })
                .collect::<Result<_, _>>()?;
            rows.push(PackageReviewConformanceBound {
                binder_ordinal: ordinal,
                trait_identity: self.nominal(bound.trait_symbol)?,
                trait_lifetime_arguments: lifetime_arguments,
                arguments: bound
                    .arguments
                    .iter()
                    .map(|reference| self.declared_type(owner, *reference))
                    .collect::<Result<_, _>>()?,
            });
        }
        rows.sort();
        Ok(rows)
    }

    pub(super) fn declared_type(
        &self,
        owner: SymbolHandle,
        reference: TypeReferenceHandle,
    ) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
        let scope = SignatureScope::declaration(self, &[owner])?;
        self.scoped_type(owner, reference, &scope)
    }

    pub(super) fn scoped_type(
        &self,
        owner: SymbolHandle,
        reference: TypeReferenceHandle,
        scope: &SignatureScope,
    ) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
        self.applied_scoped_type(owner, reference, scope, &[])
    }

    pub(super) fn applied_scoped_type(
        &self,
        owner: SymbolHandle,
        reference: TypeReferenceHandle,
        scope: &SignatureScope,
        applications: &[AppliedTypeIdentityScope<'_>],
    ) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
        let program = self.typed();
        self.require_type_dependencies(
            owner,
            reference,
            review_evidence::record::PackageReviewSemanticDependencyExposure::PublicInterface,
        )?;
        let request = ExactOwnerTypeIdentityRequest {
            type_reference: reference,
            lifetime_scope: Some((owner, &scope.lifetimes)),
            binders: &scope.binders,
            substitutions: &[],
            exact_toolchain_sources: self.sources().exact_toolchain_sources(),
        };
        let canonical = program
            .exact_applied_owner_type_identity(request, applications)
            .and_then(|identity| program.type_identity_text(identity))
            .ok_or_else(|| unavailable("a signature type in its generic declaration"))?;
        Ok(PackageReviewTypeIdentity { canonical })
    }
}
