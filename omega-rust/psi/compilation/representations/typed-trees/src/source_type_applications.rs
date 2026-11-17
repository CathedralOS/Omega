//! Source applications retain the exact arguments used while reading a body.

use crate::type_identity::{NormalizedTypeIdentity, TypeIdentityRequest};
use crate::types::TypeReferenceHandle;
use crate::{SourceTypeApplication, SourceTypeScope, TypedTrees};
use symbols::{SymbolHandle, SymbolKind};

impl SourceTypeScope {
    pub fn type_identity(
        &self,
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        typed.scoped_type_identity(
            TypeIdentityRequest {
                substitutions: &self.substitutions,
                ..TypeIdentityRequest::ordinary(reference)
            },
            &self.scope_starts,
        )
    }

    pub fn declared_carrier_identity(
        &self,
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> Option<NormalizedTypeIdentity> {
        typed.scoped_declared_carrier_identity(
            TypeIdentityRequest {
                substitutions: &self.substitutions,
                ..TypeIdentityRequest::ordinary(reference)
            },
            &self.scope_starts,
        )
    }

    pub fn carrier_identity_in_region_scope(
        &self,
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
        owner: SymbolHandle,
    ) -> Option<NormalizedTypeIdentity> {
        typed.scoped_carrier_identity_in_region_scope(
            TypeIdentityRequest {
                substitutions: &self.substitutions,
                ..TypeIdentityRequest::ordinary(reference)
            },
            &self.scope_starts,
            owner,
        )
    }

    /// An open argument is usable only when its free parameters belong to
    /// this generic check. An actual argument is interpreted before the frame
    /// that binds it, so a parameter cannot justify its own substitution.
    pub fn admits_argument(&self, typed: &TypedTrees, reference: TypeReferenceHandle) -> bool {
        represented(
            typed,
            reference,
            &self.substitutions,
            &self.scope_starts,
            &self.rigid_parameters,
        )
    }
}

impl SourceTypeApplication {
    pub fn type_identity(&self, typed: &TypedTrees) -> NormalizedTypeIdentity {
        self.scope.type_identity(typed, self.reference)
    }
}

fn represented(
    typed: &TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    starts: &[usize],
    rigid: &[SymbolHandle],
) -> bool {
    struct Inputs<'a> {
        typed: &'a TypedTrees,
        substitutions: &'a [(SymbolHandle, TypeReferenceHandle)],
        starts: &'a [usize],
        rigid: &'a [SymbolHandle],
        types: std::collections::HashSet<TypeReferenceHandle>,
        expressions: std::collections::HashSet<crate::expression::ExpressionHandle>,
        valid: bool,
    }
    impl Extend<SymbolHandle> for Inputs<'_> {
        fn extend<I: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: I) {
            for symbol in symbols {
                if !symbol.is_valid() {
                    continue;
                }
                if let Some((position, (_, actual))) = self
                    .substitutions
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, (binder, _))| *binder == symbol)
                {
                    let Some(frame) = self.starts.iter().rposition(|start| *start <= position)
                    else {
                        self.valid = false;
                        continue;
                    };
                    self.valid &= represented(
                        self.typed,
                        *actual,
                        &self.substitutions[..self.starts[frame]],
                        &self.starts[..frame],
                        self.rigid,
                    );
                } else if matches!(
                    self.typed.symbols.get(symbol).kind,
                    SymbolKind::TypeParameter
                        | SymbolKind::MachineParameter
                        | SymbolKind::ConformanceParameter
                        | SymbolKind::PropositionParameter
                        | SymbolKind::Parameter
                        | SymbolKind::Local
                ) {
                    self.valid &= self.rigid.contains(&symbol)
                        && self.typed.symbols.get(symbol).kind == SymbolKind::TypeParameter;
                }
            }
        }
    }
    impl crate::references::TypedReferences for Inputs<'_> {
        fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
            self.types.insert(reference)
        }
        fn expression(&mut self, expression: crate::expression::ExpressionHandle) -> bool {
            self.expressions.insert(expression)
        }
    }
    if !typed.contains_type_reference(reference) {
        return false;
    }
    let mut inputs = Inputs {
        typed,
        substitutions,
        starts,
        rigid,
        types: Default::default(),
        expressions: Default::default(),
        valid: true,
    };
    crate::references::visit_type_reference(typed, &mut inputs, reference);
    inputs.valid
}
