//! Read-only applied trait scopes shared by operator selection and requirement
//! calls. Each inherited edge extends the lexical substitution chain; it never
//! manufactures an instantiated type. Distinct paths remain distinct until a
//! consumer compares complete applications, rather than a rendered argument or
//! the first reachable declaring trait.

use super::TraitDefinition;
use crate::TypedTrees;
use crate::type_identity::{
    NormalizedTypeIdentity, TypeIdentityQualification, TypeIdentityRequest,
};
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

/// A borrowed reference and its remaining lexical scopes. Resolving a binder
/// narrows the visible scopes before its compound argument is traversed: the
/// argument's children must not see the telescope that argument replaces.
/// This is invocation-local inspection, not an instantiated arena node.
#[derive(Clone, Copy)]
pub struct AppliedTypeReference<'scope> {
    pub reference: TypeReferenceHandle,
    substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &'scope [usize],
}

impl<'scope> AppliedTypeReference<'scope> {
    pub fn child(self, program: &TypedTrees, reference: TypeReferenceHandle) -> Self {
        project_type_reference(program, reference, self.substitutions, self.scope_starts)
    }

    pub fn type_identity(self, program: &TypedTrees) -> NormalizedTypeIdentity {
        program.scoped_type_identity(
            TypeIdentityRequest {
                substitutions: self.substitutions,
                ..TypeIdentityRequest::ordinary(self.reference)
            },
            self.scope_starts,
        )
    }

    /// Applied array/constrained carriers take the resolved child's
    /// multiplicity; nominal carriers keep the declaration's own judgment.
    pub fn multiplicity(self, program: &TypedTrees) -> language_semantics::Multiplicity {
        if !self.reference.is_valid() {
            return language_semantics::Multiplicity::Affine;
        }
        match program.type_reference_table.type_reference(self.reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                self.child(program, *base_type).multiplicity(program)
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                self.child(program, *element_type).multiplicity(program)
            }
            _ => program.type_multiplicity(self.reference),
        }
    }
}

fn project_type_reference<'scope>(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
    scope_starts: &'scope [usize],
) -> AppliedTypeReference<'scope> {
    let mut visible = substitutions.len();
    if reference.is_valid() {
        while let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        {
            let Some((ordinal, (_, replacement))) = substitutions[..visible]
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
            visible = scope_starts
                .iter()
                .rev()
                .find(|start| **start <= ordinal)
                .copied()
                .unwrap_or(0);
            reference = *replacement;
        }
    }
    AppliedTypeReference {
        reference,
        substitutions: &substitutions[..visible],
        scope_starts: &scope_starts[..scope_starts.partition_point(|start| *start < visible)],
    }
}

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
        self.type_reference(program, reference).reference
    }

    pub fn type_reference(
        &self,
        program: &TypedTrees,
        reference: TypeReferenceHandle,
    ) -> AppliedTypeReference<'_> {
        project_type_reference(
            program,
            reference,
            &self.substitutions,
            &self.substitution_scope_starts,
        )
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
        project_type_reference(
            program,
            *reference,
            &self.substitutions[..visible],
            &self.substitution_scope_starts[..self
                .substitution_scope_starts
                .partition_point(|start| *start < visible)],
        )
        .reference
    }

    pub fn same_application(&self, program: &TypedTrees, other: &Self) -> bool {
        self.declaration.symbol == other.declaration.symbol
            && program
                .trait_type_parameters(self.declaration)
                .iter()
                .all(|parameter| {
                    self.parameter_identity(
                        program,
                        parameter.symbol,
                        TypeIdentityQualification::Ordinary,
                    ) == other.parameter_identity(
                        program,
                        parameter.symbol,
                        TypeIdentityQualification::Ordinary,
                    )
                })
    }

    /// Normalize one actual argument in its outer lexical scopes, not in the
    /// simultaneous telescope whose binder it replaces. A compound argument
    /// needs this complete chain; projecting only its existing node loses the
    /// inherited application. An open declaration keeps its own binder rigid.
    pub fn parameter_identity(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
        qualification: TypeIdentityQualification,
    ) -> NormalizedTypeIdentity {
        let Some((request, scopes)) = self.parameter_identity_request(parameter, qualification)
        else {
            return program
                .normalized_nominal_type_identity(parameter)
                .unwrap_or_default();
        };
        program.scoped_type_identity(request, scopes)
    }

    /// Portable exact argument matching is unavailable when ordinary identity
    /// erases regions or the argument remains an uninstantiated binder.
    pub fn region_free_parameter_identity(
        &self,
        program: &TypedTrees,
        parameter: SymbolHandle,
        qualification: TypeIdentityQualification,
    ) -> Option<NormalizedTypeIdentity> {
        let (request, scopes) = self.parameter_identity_request(parameter, qualification)?;
        program.region_free_scoped_type_identity(request, scopes)
    }

    fn parameter_identity_request(
        &self,
        parameter: SymbolHandle,
        qualification: TypeIdentityQualification,
    ) -> Option<(TypeIdentityRequest<'_>, &[usize])> {
        let Some((ordinal, (_, reference))) = self
            .substitutions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, (binder, _))| *binder == parameter)
        else {
            return None;
        };
        let outer = self
            .substitution_scope_starts
            .iter()
            .rev()
            .find(|start| **start <= ordinal)
            .copied()
            .unwrap_or(0);
        Some((
            TypeIdentityRequest {
                substitutions: &self.substitutions[..outer],
                qualification,
                ..TypeIdentityRequest::ordinary(*reference)
            },
            &self.substitution_scope_starts[..self
                .substitution_scope_starts
                .partition_point(|start| *start < outer)],
        ))
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
