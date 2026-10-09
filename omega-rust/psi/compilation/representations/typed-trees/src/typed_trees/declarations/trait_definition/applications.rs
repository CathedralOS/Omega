//! Read-only applied trait scopes shared by operator selection and requirement
//! calls. Each inherited edge extends the lexical substitution chain; it never
//! manufactures an instantiated type. Distinct paths remain distinct until a
//! consumer compares complete applications, rather than a rendered argument or
//! the first reachable declaring trait.

use super::TraitDefinition;
use crate::TypedTrees;
use crate::type_identity::{NormalizedTypeIdentity, TypeIdentityRequest};
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

/// Invocation-local traversal storage, not a new durable compiler stage.
#[derive(Clone)]
pub struct AppliedTraitScope<'program> {
    pub declaration: &'program TraitDefinition,
    pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    /// Each simultaneous telescope starts here in `substitutions`. Arguments
    /// belong to outer telescopes, not to the telescope replacing them.
    pub substitution_scope_starts: Vec<usize>,
}

impl<'program> AppliedTraitScope<'program> {
    pub fn new(
        program: &TypedTrees,
        declaration: &'program TraitDefinition,
        arguments: &[TypeReferenceHandle],
    ) -> Option<Self> {
        let parameters = program.trait_type_parameters(declaration);
        (parameters.len() == arguments.len()).then(|| Self {
            declaration,
            substitutions: parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.symbol, *argument))
                .collect(),
            substitution_scope_starts: vec![0],
        })
    }

    /// An authored trait contract keeps its own parameters rigid and open.
    pub fn abstract_scope(declaration: &'program TraitDefinition) -> Self {
        Self {
            declaration,
            substitutions: Vec::new(),
            substitution_scope_starts: Vec::new(),
        }
    }

    pub fn type_identity(
        &self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        program.scoped_type_identity(
            TypeIdentityRequest {
                substitutions: &self.substitutions,
                ..TypeIdentityRequest::ordinary(reference)
            },
            &self.substitution_scope_starts,
        )
    }

    /// Only a bare parameter projects to another existing node. Composite
    /// arguments retain their declaration node and this entire scope.
    pub fn argument_reference(
        &self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        self.project_reference(program, reference, self.substitutions.len())
    }

    /// Project one declaration binder through its outer scopes. A compound
    /// keeps its existing node; consumers must keep this scope alongside it.
    pub fn parameter_argument(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
    ) -> TypeReferenceHandle {
        let Some((ordinal, (_, reference))) = self
            .substitutions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (binder, _))| *binder == parameter)
        else {
            return TypeReferenceHandle::invalid();
        };
        let visible = self
            .substitution_scope_starts
            .iter()
            .rev()
            .find(|start| **start <= ordinal)
            .copied()
            .unwrap_or(0);
        self.project_reference(program, *reference, visible)
    }

    fn project_reference(
        &self,
        program: &TypedTrees,
        mut reference: TypeReferenceHandle,
        mut visible: usize,
    ) -> TypeReferenceHandle {
        if !reference.is_valid() {
            return reference;
        }
        while let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        {
            let Some((ordinal, (_, replacement))) = self.substitutions[..visible]
                .iter()
                .enumerate()
                .rev()
                .find(|(_, (parameter, _))| parameter == symbol)
            else {
                break;
            };
            if *replacement == reference {
                break;
            }
            visible = self
                .substitution_scope_starts
                .iter()
                .rev()
                .find(|start| **start <= ordinal)
                .copied()
                .unwrap_or(0);
            reference = *replacement;
        }
        reference
    }

    pub fn same_application(&self, program: &TypedTrees, other: &Self) -> bool {
        self.declaration.symbol == other.declaration.symbol
            && program
                .trait_type_parameters(self.declaration)
                .iter()
                .all(|parameter| {
                    let identity = |scope: &Self| {
                        scope
                            .substitutions
                            .iter()
                            .enumerate()
                            .rev()
                            .find(|(_, (binder, _))| *binder == parameter.symbol)
                            .map(|(ordinal, (_, reference))| {
                                let outer = scope
                                    .substitution_scope_starts
                                    .iter()
                                    .rev()
                                    .find(|start| **start <= ordinal)
                                    .copied()
                                    .unwrap_or(0);
                                program.scoped_type_identity(
                                    TypeIdentityRequest {
                                        substitutions: &scope.substitutions[..outer],
                                        ..TypeIdentityRequest::ordinary(*reference)
                                    },
                                    &scope.substitution_scope_starts[..scope
                                        .substitution_scope_starts
                                        .partition_point(|start| *start < outer)],
                                )
                            })
                    };
                    identity(self) == identity(other)
                })
    }

    /// Rejoin a declaring trait only when every reaching path agrees on its
    /// complete application. A diamond does not choose its first parent.
    pub fn declaring_scope(
        &self,
        program: &'program TypedTrees,
        declaration: SymbolHandle,
    ) -> Option<Self> {
        let mut selected: Option<Self> = None;
        let mut ambiguous = false;
        self.visit(program, &mut |scope| {
            if scope.declaration.symbol != declaration {
                return;
            }
            if let Some(known) = &selected {
                ambiguous |= !known.same_application(program, scope);
            } else {
                selected = Some(scope.clone());
            }
        });
        if ambiguous { None } else { selected }
    }

    /// Visit each reachable application, including transparent refinement
    /// heads. An active-path guard prevents cycles without discarding a sibling
    /// path's different application of the same declaring trait.
    pub fn visit(&self, program: &'program TypedTrees, visit: &mut impl FnMut(&Self)) {
        self.visit_inherited(program, &mut Vec::new(), visit);
    }

    fn visit_inherited(
        &self,
        program: &'program TypedTrees,
        active: &mut Vec<SymbolHandle>,
        visit: &mut impl FnMut(&Self),
    ) {
        if active.contains(&self.declaration.symbol) {
            return;
        }
        active.push(self.declaration.symbol);
        visit(self);
        for parent in program
            .trait_requirements(self.declaration)
            .iter()
            .chain(self.declaration.refines.iter())
        {
            let Some(declaration) = program.trait_definition_by_symbol(parent.symbol) else {
                continue;
            };
            let Some(mut scope) = Self::new(
                program,
                declaration,
                program
                    .type_reference_table
                    .type_reference_handles(parent.arguments),
            ) else {
                continue;
            };
            let mut substitutions = self.substitutions.clone();
            scope.substitution_scope_starts = self.substitution_scope_starts.clone();
            scope.substitution_scope_starts.push(substitutions.len());
            substitutions.append(&mut scope.substitutions);
            scope.substitutions = substitutions;
            scope.visit_inherited(program, active, visit);
        }
        active.pop();
    }
}
