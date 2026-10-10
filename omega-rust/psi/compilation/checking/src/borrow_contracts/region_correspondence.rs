//! Region identity refines the complete type, not just its physical loans.
//! Reuse conformance's structural query for phantom nominal arguments and
//! reference descendants. Simultaneous operands re-enter their supplying
//! scope; source names never equate a captured region with a fresh binder.

use super::callable_refinement::{CallableBorrowContract, captured_lifetimes};
use super::carried_lifetimes::RegionIdentity;
use crate::type_regions::{TypeRegionScope, type_reference_regions_match};
use symbols::SymbolHandle;
use typed_trees::name::Identifier;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Copy)]
enum RegionEnvironment<'scope> {
    Resolved(&'scope [(String, RegionIdentity)]),
    Captured(SymbolHandle),
}

impl RegionEnvironment<'_> {
    fn identity(
        self,
        program: &dyn crate::CheckingInput,
        name: &Identifier,
    ) -> Option<RegionIdentity> {
        match self {
            Self::Resolved(regions) => regions
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == name.as_str())
                .map(|(_, identity)| identity.clone()),
            Self::Captured(owner) => captured_lifetimes(program, owner)
                .into_iter()
                .find(|(parameter, _)| parameter == name.as_str())
                .map(|(_, identity)| identity),
        }
    }
}

#[derive(Clone, Copy)]
struct RegionScope<'scope> {
    declaration: SymbolHandle,
    substitutions: &'scope [(SymbolHandle, TypeReferenceHandle)],
    binders: &'scope [(SymbolHandle, String)],
    regions: RegionEnvironment<'scope>,
    arguments: RegionEnvironment<'scope>,
    output_elisions: &'scope [(TypeReferenceHandle, RegionIdentity)],
}

impl TypeRegionScope for RegionScope<'_> {
    fn erased_identity(
        self,
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> typed_trees::type_identity::NormalizedTypeIdentity {
        program.type_identity(typed_trees::type_identity::TypeIdentityRequest {
            binders: self.binders,
            substitutions: self.substitutions,
            ..typed_trees::type_identity::TypeIdentityRequest::ordinary(reference)
        })
    }

    fn resolve(
        mut self,
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Self)> {
        let mut visited = Vec::new();
        loop {
            if !reference.is_valid() {
                return Some((reference, self));
            }
            if visited.contains(&reference)
                || !program
                    .type_reference_table
                    .contains_type_reference(reference)
            {
                return None;
            }
            visited.push(reference);
            let owner = program
                .type_reference_table
                .operand_lifetime_owner(reference);
            if owner.is_valid() {
                self.regions = RegionEnvironment::Captured(owner);
                self.output_elisions = &[];
                if program.lifetime_declaration_owner(owner)
                    != program.lifetime_declaration_owner(self.declaration)
                {
                    self.substitutions = &[];
                }
            }
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return Some((reference, self));
            };
            if let Some((_, operand)) = self
                .substitutions
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == symbol)
            {
                if *operand == reference {
                    return Some((reference, self));
                }
                reference = *operand;
                self.regions = self.arguments;
                self.substitutions = &[];
                self.output_elisions = &[];
                continue;
            }
            if let Some(application) = program
                .data_definition_by_symbol(*symbol)
                .and_then(|definition| definition.generic_instance)
            {
                reference = application;
                continue;
            }
            return Some((reference, self));
        }
    }
}

pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    required: &CallableBorrowContract<'_>,
    actual: &CallableBorrowContract<'_>,
    required_regions: &[(String, RegionIdentity)],
    actual_regions: &[(String, RegionIdentity)],
    output_elisions: &[(TypeReferenceHandle, RegionIdentity)],
) -> Result<(), &'static str> {
    let required_scope = RegionScope {
        declaration: required.lexical_owner,
        substitutions: required.substitutions,
        binders: required.binders,
        regions: RegionEnvironment::Resolved(required_regions),
        arguments: RegionEnvironment::Captured(required.argument_owner),
        output_elisions: &[],
    };
    let actual_scope = RegionScope {
        declaration: actual.lexical_owner,
        substitutions: actual.substitutions,
        binders: actual.binders,
        regions: RegionEnvironment::Resolved(actual_regions),
        arguments: RegionEnvironment::Captured(actual.argument_owner),
        output_elisions: &[],
    };
    let matches = |actual_scope: RegionScope<'_>,
                   required_scope: RegionScope<'_>,
                   reference: TypeReferenceHandle,
                   actual: Option<&Identifier>,
                   required: Option<&Identifier>| {
        let Some(required) = required else {
            return true;
        };
        required_scope
            .regions
            .identity(program, required)
            .zip(match actual {
                Some(actual) => actual_scope.regions.identity(program, actual),
                None => actual_scope
                    .output_elisions
                    .iter()
                    .find(|(output, _)| *output == reference)
                    .map(|(_, region)| region.clone()),
            })
            .is_some_and(|(required, actual)| required == actual)
    };
    for (required, actual) in required
        .parameters
        .iter()
        .map(|parameter| parameter.type_reference)
        .zip(
            actual
                .parameters
                .iter()
                .map(|parameter| parameter.type_reference),
        )
    {
        if !type_reference_regions_match(
            program,
            actual,
            required,
            actual_scope,
            required_scope,
            &matches,
        ) {
            return Err("callable type does not preserve its region identity");
        }
    }
    if !type_reference_regions_match(
        program,
        actual.return_type,
        required.return_type,
        RegionScope {
            output_elisions,
            ..actual_scope
        },
        required_scope,
        &matches,
    ) {
        return Err("callable type does not preserve its region identity");
    }
    Ok(())
}
