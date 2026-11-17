//! Source applications retain the exact arguments used while reading a body.

use crate::type_identity::{NormalizedTypeIdentity, TypeIdentityRequest};
use crate::types::TypeReferenceHandle;
use crate::{SourceTypeApplication, SourceTypeScope, TypedTrees};
use symbols::SymbolHandle;

impl SourceTypeScope {
    fn with_substitutions<R>(
        &self,
        read: impl FnOnce(&[(SymbolHandle, TypeReferenceHandle)], &[usize], &[SymbolHandle]) -> R,
    ) -> R {
        let mut frames = Vec::new();
        let mut current = Some(self);
        while let Some(frame) = current {
            frames.push(frame);
            current = frame.parent.as_deref();
        }
        let mut substitutions =
            Vec::with_capacity(frames.iter().map(|frame| frame.substitutions.len()).sum());
        let mut starts = Vec::with_capacity(frames.len());
        let mut rigid = Vec::with_capacity(
            frames
                .iter()
                .map(|frame| frame.rigid_parameters.len())
                .sum(),
        );
        for frame in frames.into_iter().rev() {
            if !frame.substitutions.is_empty() {
                starts.push(substitutions.len());
                substitutions.extend_from_slice(&frame.substitutions);
            }
            rigid.extend_from_slice(&frame.rigid_parameters);
        }
        read(&substitutions, &starts, &rigid)
    }

    pub fn type_identity(
        &self,
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        self.with_substitutions(|substitutions, starts, _| {
            typed.scoped_type_identity(
                TypeIdentityRequest {
                    substitutions,
                    ..TypeIdentityRequest::ordinary(reference)
                },
                starts,
            )
        })
    }

    pub fn declared_carrier_identity(
        &self,
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> Option<NormalizedTypeIdentity> {
        self.with_substitutions(|substitutions, starts, _| {
            typed.scoped_declared_carrier_identity(
                TypeIdentityRequest {
                    substitutions,
                    ..TypeIdentityRequest::ordinary(reference)
                },
                starts,
            )
        })
    }

    pub fn carrier_identity_in_region_scope(
        &self,
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
        owner: SymbolHandle,
    ) -> Option<NormalizedTypeIdentity> {
        self.with_substitutions(|substitutions, starts, _| {
            typed.scoped_carrier_identity_in_region_scope(
                TypeIdentityRequest {
                    substitutions,
                    ..TypeIdentityRequest::ordinary(reference)
                },
                starts,
                owner,
            )
        })
    }

    pub fn region_free_type_identity(
        &self,
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> Option<NormalizedTypeIdentity> {
        self.with_substitutions(|substitutions, starts, _| {
            typed.region_free_scoped_type_identity(
                TypeIdentityRequest {
                    substitutions,
                    ..TypeIdentityRequest::ordinary(reference)
                },
                starts,
            )
        })
    }

    /// An open argument is usable only when its free parameters belong to
    /// this generic check. An actual argument is interpreted before the frame
    /// that binds it, so a parameter cannot justify its own substitution.
    pub fn admits_argument(&self, typed: &TypedTrees, reference: TypeReferenceHandle) -> bool {
        self.with_substitutions(|substitutions, starts, rigid| {
            crate::domain::index_argument_is_in_scope(
                typed,
                reference,
                substitutions,
                starts,
                rigid,
            )
        })
    }
}

impl SourceTypeApplication {
    pub fn type_identity(&self, typed: &TypedTrees) -> NormalizedTypeIdentity {
        self.scope.type_identity(typed, self.reference)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TypeReferenceNode;
    use std::sync::Arc;
    use symbols::{BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder};

    #[test]
    fn shared_source_frames_preserve_simultaneous_substitution() {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let mut symbols = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            symbols::builtin_type_symbols().into_iter().chain([
                (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                (SymbolKind::TypeParameter, SymbolNameRef::Static("U")),
            ]),
        ))
        .collect::<Vec<_>>();
        let u = symbols.pop().unwrap();
        let t = symbols.pop().unwrap();
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let mut reference = |symbol, name: &str| {
            typed.type_reference_table.insert(TypeReferenceNode::Named {
                symbol,
                name: name.into(),
            })
        };
        let t_ref = reference(t, "T");
        let u_ref = reference(u, "U");
        let boolean = reference(symbols[BuiltinTypeAtom::Bool.ordinal()], "bool");
        let integer = reference(symbols[BuiltinTypeAtom::U64.ordinal()], "u64");
        let outer = Arc::new(SourceTypeScope {
            substitutions: vec![(t, integer), (u, boolean)],
            ..Default::default()
        });
        let swapped = SourceTypeScope {
            parent: Some(outer.clone()),
            substitutions: vec![(t, u_ref), (u, t_ref)],
            ..Default::default()
        };
        assert!(Arc::ptr_eq(swapped.parent.as_ref().unwrap(), &outer));
        assert_eq!(
            swapped.type_identity(&typed, t_ref),
            typed.normalized_type_identity(boolean)
        );
        assert_eq!(
            swapped.type_identity(&typed, u_ref),
            typed.normalized_type_identity(integer)
        );
    }
}
