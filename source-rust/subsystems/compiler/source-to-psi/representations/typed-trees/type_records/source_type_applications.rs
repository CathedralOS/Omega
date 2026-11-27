//! Source applications retain the exact arguments used while reading a body.

use crate::source_state::type_identity::{NormalizedTypeIdentity, TypeIdentityRequest};
use crate::source_state::types::TypeReferenceHandle;
use crate::source_state::{SourceTypeApplication, SourceTypeScope, TypedTrees};
use symbols::SymbolHandle;

/// A slice's source occurrences retain the scopes in which their arguments
/// were supplied. Shape inspection grants neither backing nor access permission.
#[derive(Debug)]
pub struct SourceSliceApplication {
    pub reference: SourceTypeApplication,
    pub referent: SourceTypeApplication,
    pub slice: SourceTypeApplication,
    pub element: SourceTypeApplication,
    pub access: Option<language_core::ReferenceAccess>,
}

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

    /// The argument belongs to the scope before its binder's tuple. This
    /// applies equally to a named type and to the binder in an array extent.
    pub fn argument_for_parameter(&self, symbol: SymbolHandle) -> Option<SourceTypeApplication> {
        let mut frame = Some(self);
        while let Some(current) = frame {
            if let Some((_, reference)) = current
                .substitutions
                .iter()
                .find(|(binder, _)| *binder == symbol)
            {
                return Some(SourceTypeApplication {
                    reference: *reference,
                    scope: current.parent.clone().unwrap_or_default(),
                    substitutions: Vec::new(),
                });
            }
            frame = current.parent.as_deref();
        }
        None
    }

    /// Read a supplied type under the scope that preceded its argument tuple.
    /// Sibling bindings in the same tuple cannot rewrite one another's actuals.
    /// Constructor children retain this returned scope when they are inspected.
    pub fn supplied_type_reference(
        self: &std::sync::Arc<Self>,
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> Result<SourceTypeApplication, &'static str> {
        let mut application = SourceTypeApplication {
            reference,
            scope: self.clone(),
            substitutions: Vec::new(),
        };
        loop {
            if !typed.contains_type_reference(application.reference) {
                return Err("supplied type lost its retained type reference");
            }
            let crate::source_state::types::TypeReferenceNode::Named { symbol, .. } =
                typed.type_reference_node(application.reference)
            else {
                return Ok(application);
            };
            let Some(supplied) = application.scope.argument_for_parameter(*symbol) else {
                return Ok(application);
            };
            application = supplied;
        }
    }

    pub fn slice_application(
        self: &std::sync::Arc<Self>,
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> Result<Option<SourceSliceApplication>, &'static str> {
        use crate::source_state::types::TypeReferenceNode;
        let reference = self.supplied_type_reference(typed, reference)?;
        let (referent, access) = match typed.type_reference_node(reference.reference) {
            TypeReferenceNode::Slice { .. } => (reference.clone(), None),
            TypeReferenceNode::Reference {
                referee, access, ..
            } => (
                reference.scope.supplied_type_reference(typed, *referee)?,
                Some(*access),
            ),
            _ => return Ok(None),
        };
        let mut slice = referent.clone();
        let mut remaining = typed.type_reference_table.type_reference_count();
        while let TypeReferenceNode::Constrained { base_type, .. } =
            typed.type_reference_node(slice.reference)
        {
            if remaining == 0 {
                return Err("supplied slice has a cyclic qualification tree");
            }
            remaining -= 1;
            let supplied = slice.scope.supplied_type_reference(typed, *base_type)?;
            if !std::sync::Arc::ptr_eq(&slice.scope, &supplied.scope) {
                remaining = typed.type_reference_table.type_reference_count();
            }
            slice = supplied;
        }
        let TypeReferenceNode::Slice { element_type } = typed.type_reference_node(slice.reference)
        else {
            return Ok(None);
        };
        let element = SourceTypeApplication {
            reference: *element_type,
            scope: slice.scope.clone(),
            substitutions: Vec::new(),
        };
        Ok(Some(SourceSliceApplication {
            reference,
            referent,
            slice,
            element,
            access,
        }))
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

    pub fn index_identity(
        &self,
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> Option<NormalizedTypeIdentity> {
        self.with_substitutions(|substitutions, starts, _| {
            crate::source_state::type_identity::scoped_index_identity(
                typed,
                reference,
                substitutions,
                starts,
            )
        })
    }

    pub fn integer_argument_matches(
        &self,
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
        expected: crate::source_state::types::PrimitiveType,
    ) -> bool {
        self.admits_argument(typed, reference)
            && self.with_substitutions(|substitutions, starts, _| {
                crate::source_state::type_identity::scoped_scalar_argument_matches(
                    typed,
                    reference,
                    expected,
                    substitutions,
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

    /// Resolve written lifetimes while keeping the substituted type and its
    /// qualifications, including those on the elements of a sequence.
    pub fn type_identity_in_region_scope(
        &self,
        typed: &TypedTrees,
        reference: TypeReferenceHandle,
        owner: SymbolHandle,
    ) -> Option<NormalizedTypeIdentity> {
        self.with_substitutions(|substitutions, starts, _| {
            typed.scoped_type_identity_in_region_scope(
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
            crate::source_state::domain::index_argument_is_in_scope(
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
    use crate::source_state::types::TypeReferenceNode;
    use crate::source_symbols::SymbolTableBuilder;
    use std::sync::Arc;
    use symbols::{BuiltinTypeAtom, SymbolKind, SymbolNameRef};

    #[test]
    fn shared_source_frames_preserve_simultaneous_substitution() {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let mut symbols = SymbolTableBuilder::child_handles(
            builder.insert_children(
                root,
                crate::source_symbols::builtin_type_symbols()
                    .into_iter()
                    .chain([
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("U")),
                    ]),
            ),
        )
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
        let swapped = Arc::new(swapped);
        for (formal, actual) in [(t_ref, boolean), (u_ref, integer)] {
            let supplied = swapped.supplied_type_reference(&typed, formal).unwrap();
            assert_eq!(supplied.reference, actual);
            assert_eq!(
                supplied.type_identity(&typed),
                swapped.type_identity(&typed, formal)
            );
        }
        let slice = typed.type_reference_table.insert(TypeReferenceNode::Slice {
            element_type: t_ref,
        });
        let supplied = Arc::new(SourceTypeScope {
            parent: Some(outer.clone()),
            substitutions: vec![(u, slice)],
            ..Default::default()
        })
        .supplied_type_reference(&typed, u_ref)
        .unwrap();
        assert_eq!(supplied.reference, slice);
        assert!(Arc::ptr_eq(&supplied.scope, &outer));
        assert_eq!(
            supplied
                .scope
                .supplied_type_reference(&typed, t_ref)
                .unwrap()
                .reference,
            integer
        );
        let scope = Arc::new(SourceTypeScope {
            parent: Some(outer.clone()),
            substitutions: vec![(u, slice)],
            ..Default::default()
        });
        let borrowed = typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: u_ref,
                access: language_core::ReferenceAccess::Shared,
                lifetime: None,
            });
        let view = scope.slice_application(&typed, borrowed).unwrap().unwrap();
        assert_eq!(view.slice.reference, slice);
        assert!(Arc::ptr_eq(&view.slice.scope, &outer));
        assert_eq!(view.element.reference, t_ref);
        assert!(Arc::ptr_eq(&view.element.scope, &outer));
        assert_eq!(
            view.element.type_identity(&typed),
            typed.normalized_type_identity(integer)
        );
        let nested = Arc::new(SourceTypeScope {
            parent: Some(outer.clone()),
            substitutions: vec![(u, borrowed)],
            ..Default::default()
        });
        assert!(
            nested
                .slice_application(&typed, borrowed)
                .unwrap()
                .is_none(),
            "a reference to a borrowed view keeps its extra reference layer"
        );
        assert!(
            swapped
                .supplied_type_reference(&typed, TypeReferenceHandle::invalid())
                .is_err()
        );
    }
    #[test]
    fn constant_arguments_keep_their_carrier_and_simultaneous_scope() {
        use crate::source_state::data::{TypeParameter, TypeParameterKind};
        use crate::source_state::types::PrimitiveType;
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let mut children = SymbolTableBuilder::child_handles(
            builder.insert_children(
                root,
                crate::source_symbols::builtin_type_symbols()
                    .into_iter()
                    .chain([
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("N")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("M")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                        (SymbolKind::TypeParameter, SymbolNameRef::Static("Other")),
                    ]),
            ),
        )
        .collect::<Vec<_>>();
        let other = children.pop().unwrap();
        let t = children.pop().unwrap();
        let m = children.pop().unwrap();
        let n = children.pop().unwrap();
        let mut typed = TypedTrees {
            symbols: builder.finish(),
            ..Default::default()
        };
        let u64_type = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: children[BuiltinTypeAtom::U64.ordinal()],
            name: "u64".into(),
        });
        let u32_type = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: children[BuiltinTypeAtom::Integer(
                numerics::integer_carrier::FixedInteger::unsigned(32),
            )
            .ordinal()],
            name: "u32".into(),
        });
        let mut reference = |symbol, name: &str| {
            typed.type_reference_table.insert(TypeReferenceNode::Named {
                symbol,
                name: name.into(),
            })
        };
        let n_ref = reference(n, "N");
        let m_ref = reference(m, "M");
        let t_ref = reference(t, "T");
        let other_ref = reference(other, "Other");
        let four = reference(SymbolHandle::invalid(), "4");
        let five = reference(SymbolHandle::invalid(), "5");
        for (symbol, kind) in [
            (
                n,
                TypeParameterKind::Const {
                    type_reference: u64_type,
                },
            ),
            (
                m,
                TypeParameterKind::Const {
                    type_reference: u64_type,
                },
            ),
            (t, TypeParameterKind::Type),
            (
                other,
                TypeParameterKind::Const {
                    type_reference: u32_type,
                },
            ),
        ] {
            typed.data_type_parameters.insert(TypeParameter {
                symbol,
                kind,
                ..Default::default()
            });
        }
        let scope = SourceTypeScope {
            rigid_parameters: vec![n, m, t, other],
            ..Default::default()
        };
        assert!(scope.integer_argument_matches(&typed, n_ref, PrimitiveType::U64));
        assert!(!scope.integer_argument_matches(&typed, other_ref, PrimitiveType::U64));
        assert!(!scope.integer_argument_matches(&typed, t_ref, PrimitiveType::U64));
        assert!(!SourceTypeScope::default().integer_argument_matches(
            &typed,
            n_ref,
            PrimitiveType::U64
        ));
        let bound = Arc::new(SourceTypeScope {
            substitutions: vec![(n, four), (m, five)],
            ..Default::default()
        });
        let swapped = SourceTypeScope {
            parent: Some(bound),
            substitutions: vec![(n, m_ref), (m, n_ref)],
            ..Default::default()
        };
        assert_eq!(
            swapped.index_identity(&typed, n_ref),
            scope.index_identity(&typed, five)
        );
        assert_eq!(
            swapped.index_identity(&typed, m_ref),
            scope.index_identity(&typed, four)
        );
        assert!(swapped.integer_argument_matches(&typed, n_ref, PrimitiveType::U64));
        let foreign_carrier = SourceTypeScope {
            parent: Some(Arc::new(SourceTypeScope {
                parent: Some(Arc::new(scope)),
                substitutions: vec![(other, four)],
                ..Default::default()
            })),
            substitutions: vec![(n, other_ref)],
            ..Default::default()
        };
        assert!(!foreign_carrier.integer_argument_matches(&typed, n_ref, PrimitiveType::U64));
    }
}
