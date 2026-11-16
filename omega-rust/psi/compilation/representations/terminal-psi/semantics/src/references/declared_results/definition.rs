//! The original header keeps its complete declared types. Derive the public
//! input union from their lifetime binders, independently of the editable
//! application groups and of the implementation's actual return.

use language_semantics::type_identity::{
    TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, visit_type_identity_package_owners,
    vocabulary::{TypeForm, Unadmitted},
};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    BoundaryLifetimeDependency, ClosedConformanceSchema, DeclaredResultLifetime, MachineContract,
    StructuralAccess, StructuralPathSegment, StructuralTypeShape, TerminalModule,
};

type Occurrence = (u32, Vec<StructuralPathSegment>, StructuralAccess, bool);

pub(super) fn result_lifetimes(
    module: &TerminalModule,
    contract: &MachineContract,
    schema: &ClosedConformanceSchema,
) -> Result<Vec<DeclaredResultLifetime>, &'static str> {
    let (lifetimes, parameters, result) = crate::closed_conformances::definition_types(schema)?
        .ok_or("selected result has no complete original declared types")?;
    let result_type = contract
        .result
        .structural()
        .ok_or("selected reference has no original structural result")?
        .structural_type;
    let results = occurrences(module, result_type, result, lifetimes, false)?;
    let mut inputs = Vec::new();
    for (lane, parameter) in contract.structural_parameters.iter().enumerate() {
        let declared = parameters
            .get(parameter.position as usize)
            .ok_or("public structural formal has no original declared type")?;
        let lane = u32::try_from(lane).map_err(|_| "public structural input roster exceeds u32")?;
        for (lifetime, path, access, referent) in occurrences(
            module,
            parameter.structural_type,
            declared,
            lifetimes,
            parameter.access != StructuralAccess::Owned,
        )? {
            let input = if referent {
                if access != parameter.access {
                    return Err("public borrowed formal differs from its declared access");
                }
                BoundaryLifetimeDependency::Referent {
                    argument_index: lane,
                    access,
                }
            } else {
                BoundaryLifetimeDependency::Stored {
                    argument_index: lane,
                    path,
                    access,
                }
            };
            inputs.push((lifetime, input));
        }
    }
    inputs.sort_unstable();
    let mut results = results;
    results.sort_unstable();
    let mut groups: Vec<(u32, DeclaredResultLifetime)> = Vec::new();
    for (lifetime, path, _, _) in results {
        if groups.last().is_none_or(|(known, _)| *known != lifetime) {
            let start = inputs.partition_point(|(known, _)| *known < lifetime);
            let end = inputs.partition_point(|(known, _)| *known <= lifetime);
            let mut sources = inputs[start..end]
                .iter()
                .map(|(_, input)| input.clone())
                .collect::<Vec<_>>();
            sources.sort_unstable();
            sources.dedup();
            groups.push((
                lifetime,
                DeclaredResultLifetime {
                    inputs: sources,
                    results: Vec::new(),
                },
            ));
        }
        groups
            .last_mut()
            .expect("the declared lifetime group was just installed")
            .1
            .results
            .push(path);
    }
    let mut groups = groups
        .into_iter()
        .map(|(_, mut group)| {
            group.results.sort_unstable();
            group.results.dedup();
            group
        })
        .collect::<Vec<_>>();
    groups.sort_unstable();
    Ok(groups)
}

fn occurrences(
    module: &TerminalModule,
    root: StructuralTypeId,
    identity: &str,
    lifetimes: u32,
    borrowed_formal: bool,
) -> Result<Vec<Occurrence>, &'static str> {
    let mut visitor = DeclaredTypeWalk {
        module,
        root,
        lifetimes,
        borrowed_formal,
        depth: 0,
        metadata_depth: 0,
        remaining: identity.len().saturating_mul(2),
        stack: Vec::new(),
        path: Vec::new(),
        occurrences: Vec::new(),
    };
    visit_type_identity_package_owners(identity, &mut visitor).map_err(|reason| match reason {
        TypeIdentityVisitError::MalformedIdentity => "original public type identity is malformed",
        TypeIdentityVisitError::ResourceLimitExceeded => {
            "original public type exceeds the declared frontier resource budget"
        }
        TypeIdentityVisitError::AllocationFailed => "original public type allocation failed",
        TypeIdentityVisitError::UnknownPackage => "original public type has an unknown package",
        TypeIdentityVisitError::UnsupportedEmbeddedName => {
            "original public type has an unserved embedded declaration"
        }
        TypeIdentityVisitError::Unadmitted(Unadmitted::InvalidIdentity) => {
            "original public type has no admitted nominal identity"
        }
        TypeIdentityVisitError::Unadmitted(Unadmitted::ApplicationScope) => {
            "original public type has an unserved declared lifetime frontier"
        }
    })?;
    // Lifetime groups omit access, so compare each original reference with
    // its current storage before grouping. Otherwise a rewritten result
    // could retain every lifetime while gaining write permission.
    for (_, path, access, referent) in &visitor.occurrences {
        if *referent {
            continue; // The caller checks the formal's separate access column.
        }
        let actual = crate::runtime_structural_path_tip(module.structural_types.iter(), root, path)
            .and_then(|tip| crate::stored_reference_access(&tip.shape));
        if actual != Some(*access) {
            return Err("public reference path or access differs from its original declared type");
        }
    }
    // Distinct nested references cannot collapse to the same current leaf.
    // A borrowed formal's outer referent and its stored child are separate
    // observations even when both have an empty projected path.
    visitor
        .occurrences
        .sort_unstable_by(|left, right| (left.3, &left.1).cmp(&(right.3, &right.1)));
    if visitor
        .occurrences
        .windows(2)
        .any(|pair| (pair[0].3, &pair[0].1) == (pair[1].3, &pair[1].1))
    {
        return Err("original public references collapse to the same current storage path");
    }
    Ok(visitor.occurrences)
}

struct DeclaredTypeWalk<'module> {
    module: &'module TerminalModule,
    root: StructuralTypeId,
    lifetimes: u32,
    borrowed_formal: bool,
    depth: usize,
    metadata_depth: usize,
    remaining: usize,
    stack: Vec<(TypeForm, usize)>,
    path: Vec<StructuralPathSegment>,
    occurrences: Vec<Occurrence>,
}

impl TypeIdentityPackageOwnerVisitor for DeclaredTypeWalk<'_> {
    fn enter(&mut self) -> Result<(), TypeIdentityVisitError> {
        if self.depth == 128 {
            return Err(TypeIdentityVisitError::ResourceLimitExceeded);
        }
        self.depth += 1;
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), TypeIdentityVisitError> {
        self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or(TypeIdentityVisitError::ResourceLimitExceeded)?;
        Ok(())
    }

    fn package_owner(&mut self, _: [u8; 32]) -> Result<(), TypeIdentityVisitError> {
        Ok(())
    }

    fn embedded_name(&mut self, _: &str) -> Result<(), TypeIdentityVisitError> {
        Ok(())
    }

    fn enter_type_metadata(&mut self) {
        self.metadata_depth += 1;
    }

    fn leave_type_metadata(&mut self) {
        self.metadata_depth -= 1;
    }

    fn enter_type(&mut self, form: TypeForm) -> Result<(), TypeIdentityVisitError> {
        let previous = self.path.len();
        if self.metadata_depth == 0
            && let Some((parent, _)) = self.stack.last()
        {
            match parent {
                TypeForm::Reference | TypeForm::MutableReference | TypeForm::WriteOnlyReference
                    if !(self.borrowed_formal && self.outer_reference()) =>
                {
                    let tip = crate::runtime_structural_path_tip(
                        self.module.structural_types.iter(),
                        self.root,
                        &self.path,
                    )
                    .ok_or(TypeIdentityVisitError::MalformedIdentity)?;
                    if matches!(tip.shape, StructuralTypeShape::Reference { .. }) {
                        self.path.push(StructuralPathSegment::Referent);
                    }
                }
                TypeForm::Slice | TypeForm::Array => {
                    self.path.push(StructuralPathSegment::AllElements);
                }
                _ => {}
            }
        }
        if self.metadata_depth == 0
            && matches!(
                form,
                TypeForm::Nominal | TypeForm::Named | TypeForm::Generic
            )
        {
            let tip = crate::runtime_structural_path_tip(
                self.module.structural_types.iter(),
                self.root,
                &self.path,
            )
            .ok_or(TypeIdentityVisitError::MalformedIdentity)?;
            // A nominal application's lifetime arguments do not identify
            // its fields. Missing declaration-owned field correspondence
            // cannot be interpreted as an empty borrowed frontier, including
            // behind an outer reference that stops an ordinary leaf walk.
            if !crate::descriptors::descriptor_results::owned_shape::plain_owned_shape(
                self.module.structural_types.iter(),
                tip.id,
            ) {
                return Err(TypeIdentityVisitError::UnsupportedEmbeddedName);
            }
        }
        self.stack.push((form, previous));
        Ok(())
    }

    fn leave_type(&mut self, _: TypeForm) {
        let (_, previous) = self.stack.pop().expect("the canonical type walk is nested");
        self.path.truncate(previous);
    }

    fn schema_region(&mut self, ordinal: u32) -> Result<(), TypeIdentityVisitError> {
        if ordinal >= self.lifetimes {
            return Err(TypeIdentityVisitError::MalformedIdentity);
        }
        if self.metadata_depth != 0 {
            return Ok(());
        }
        // Type arguments describe the nominal application, not the fields
        // it stores. Its declaration-owned frontier must be available before
        // those occurrences can supply any result lifetime.
        if self
            .stack
            .iter()
            .any(|(form, _)| matches!(form, TypeForm::Generic))
        {
            return Err(TypeIdentityVisitError::Unadmitted(
                Unadmitted::ApplicationScope,
            ));
        }
        let access = match self.stack.last().map(|(form, _)| *form) {
            Some(TypeForm::Reference) => StructuralAccess::SharedBorrow,
            Some(TypeForm::MutableReference) => StructuralAccess::MutableBorrow,
            Some(TypeForm::WriteOnlyReference) => StructuralAccess::WriteOnlyBorrow,
            _ => return Err(TypeIdentityVisitError::MalformedIdentity),
        };
        self.occurrences.push((
            ordinal,
            self.path.clone(),
            access,
            self.borrowed_formal && self.outer_reference(),
        ));
        Ok(())
    }
}

impl DeclaredTypeWalk<'_> {
    fn outer_reference(&self) -> bool {
        self.stack
            .iter()
            .filter(|(form, _)| *form != TypeForm::Constrained)
            .count()
            == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{ScalarType, StructuralFieldId};
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage,
        StructuralTypeDeclaration, TerminalIdentityIndex,
    };

    fn id(value: u64) -> StructuralTypeId {
        StructuralTypeId::new(value).unwrap()
    }

    fn module() -> TerminalModule {
        let declaration = |value, shape| StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            id: id(value),
            identity: TerminalIdentityIndex(value as u32),
            shape,
            quotient_carrier: None,
            owns_linear_claim: false,
            copyable: false,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            scalar_restriction: None,
            requires: Vec::new(),
        };
        let mut module = TerminalModule::declarations();
        module.structural_types = vec![
            declaration(1, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
            declaration(
                2,
                StructuralTypeShape::Reference {
                    referent: id(1),
                    access: StructuralAccess::MutableBorrow,
                },
            ),
            declaration(
                3,
                StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Packed,
                    fields: (1..=2)
                        .map(|value| StructuralFieldDeclaration {
                            declaration_identity: Default::default(),
                            id: StructuralFieldId::new(value).unwrap(),
                            identity: TerminalIdentityIndex(value as u32),
                            field_type: StructuralFieldType::Structural(id(2)),
                            relevance: BindingRelevance::Relevant,
                            inline_carrier: None,
                            storage: None,
                        })
                        .collect(),
                },
            ),
            declaration(
                4,
                StructuralTypeShape::Reference {
                    referent: id(3),
                    access: StructuralAccess::MutableBorrow,
                },
            ),
        ];
        module.structural_types.push(declaration(
            5,
            StructuralTypeShape::Reference {
                referent: id(2),
                access: StructuralAccess::MutableBorrow,
            },
        ));
        module
    }

    #[test]
    fn reference_access_and_nested_paths_match_the_original_declaration() {
        let module = module();
        let scalar = "compiler-type(atom(bool))";
        let reference = format!("ref-mut({scalar},region(owner(schema),ordinal(0)))");
        assert_eq!(
            occurrences(&module, id(2), &reference, 1, false).unwrap(),
            vec![(0, Vec::new(), StructuralAccess::MutableBorrow, false)],
        );
        let nested = format!("ref-mut({reference},region(owner(schema),ordinal(0)))");
        assert_eq!(
            occurrences(&module, id(5), &nested, 1, false).unwrap(),
            vec![
                (0, Vec::new(), StructuralAccess::MutableBorrow, false),
                (
                    0,
                    vec![StructuralPathSegment::Referent],
                    StructuralAccess::MutableBorrow,
                    false
                ),
            ],
        );
        assert_eq!(
            occurrences(
                &module,
                id(2),
                &reference.replace("ref-mut(", "ref("),
                1,
                false
            ),
            Err("public reference path or access differs from its original declared type"),
        );
        assert_eq!(
            occurrences(&module, id(2), &nested, 1, false),
            Err("public reference path or access differs from its original declared type"),
            "flattening a nested result cannot retain its original paths",
        );
    }

    #[test]
    fn a_nominal_reference_frontier_needs_its_field_definition() {
        let module = module();
        let nominal = format!("nominal(package-owner(32:{}),path(Pair))", "11".repeat(32));
        let outer = format!("ref-mut({nominal},region(owner(schema),ordinal(0)))");
        for (root, identity) in [(id(3), nominal.as_str()), (id(4), outer.as_str())] {
            assert_eq!(
                occurrences(&module, root, identity, 2, false),
                Err("original public type has an unserved embedded declaration"),
                "neither missing fields nor an outer reference may hide the inner lifetimes",
            );
        }
    }
}
