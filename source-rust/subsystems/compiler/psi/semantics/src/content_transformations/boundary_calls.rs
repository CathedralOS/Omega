//! Instantiate a callable boundary's authored law at its normal operation result.
//! A composition describes this call. It never selects a provider or supplies a
//! native binding, introduces a qualification, or proves the provider's body.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    BoundaryCompletion, ContentConservation, ContentPlaceVersion, ContentStructuralPlace,
    ContentTerm,
};
use terminal_psi::{
    BoundaryContentGuarantee, BoundaryDeclarationRole, BoundaryMachineDeclaration,
    CompletionReceipt, ContentConservationGuarantee, ContentPartitionComposition,
    ContentPlaceSubstitution, Operation, OperationKind, StructuralAccess, StructuralArgument,
    StructuralOperationResult, TerminalMachine, TerminalModule,
};

use super::{ancestry, content_path, structural_path};
use crate::exactly_one::exactly_one;

pub fn has_declared_conservation(module: &TerminalModule) -> bool {
    module.interfaces.boundary_machines.iter().any(|boundary| {
        boundary
            .content_guarantees
            .iter()
            .any(|guarantee| matches!(guarantee, BoundaryContentGuarantee::Conservation(_)))
    })
}

/// Reconstruct the rows only from the actual operation and its declared callee.
pub fn compositions(
    module: &TerminalModule,
    caller: &TerminalMachine,
    operation: &Operation,
) -> Result<Vec<ContentPartitionComposition>, &'static str> {
    let OperationKind::BoundaryCall {
        boundary,
        structural_arguments,
        completion_receipts,
        ..
    } = &operation.kind
    else {
        return Ok(Vec::new());
    };
    let declaration = module
        .boundary_machine(*boundary)
        .ok_or("content call has no declared requirement")?;
    let mut rows = Vec::new();
    for guarantee in &declaration.content_guarantees {
        let BoundaryContentGuarantee::Conservation(guarantee) = guarantee else {
            continue;
        };
        if declaration.role != BoundaryDeclarationRole::Callable
            || declaration.completion != BoundaryCompletion::NormalReturn
        {
            return Err("content composition needs a callable normal completion");
        }
        let result = operation
            .result
            .as_structural()
            .ok_or("content composition needs an owned structural result")?;
        let call = Call {
            module,
            caller,
            declaration,
            result,
            arguments: structural_arguments,
            receipts: completion_receipts,
        };
        rows.push(call.instantiate(operation, guarantee)?);
    }
    rows.sort_unstable();
    if rows.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("boundary call repeats a conservation guarantee");
    }
    Ok(rows)
}

/// The caller's rows name normal producer operations. Rebuilding them checks
/// substitutions, consumed claims and the actual returned claim binding together.
pub fn for_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<Vec<ContentPartitionComposition>, &'static str> {
    let mut rows = Vec::new();
    for operation in machine.operations() {
        rows.extend(compositions(module, machine, operation)?);
    }
    rows.sort_unstable();
    Ok(rows)
}

pub fn verify_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<(), &'static str> {
    let expected = for_machine(module, machine)?;
    let mut actual = Vec::new();
    for row in &machine.content_partition_compositions {
        let producer = machine
            .operation(row.producer_operation)
            .ok_or("content composition has no producer operation")?;
        if let OperationKind::BoundaryCall { boundary, .. } = &producer.kind {
            // Describing a normal call in an open unit does not establish that a
            // candidate can discharge the public law. Candidate/body witness
            // replay is required before publishing that combination.
            if module
                .interfaces
                .provider_candidates
                .iter()
                .any(|candidate| candidate.boundary == *boundary)
            {
                return Err(
                    "boundary content call needs the checked provider body's conservation correspondence",
                );
            }
            actual.push(row);
        } else if producer.kind.direct_callee().is_none() {
            return Err("content composition producer is not a normal declared call");
        }
    }
    if actual.into_iter().ne(&expected) {
        return Err("boundary content compositions differ from the exact normal call occurrences");
    }
    if !expected.is_empty() {
        // The equation cannot authenticate the completion receipt it consumes.
        crate::plain_affine::verify_claims_in(module, machine.id).map_err(
            |_| "boundary content completion receipt disagrees with the live claim frontier",
        )?;
    }
    Ok(())
}

struct Call<'a> {
    module: &'a TerminalModule,
    caller: &'a TerminalMachine,
    declaration: &'a BoundaryMachineDeclaration,
    result: &'a StructuralOperationResult,
    arguments: &'a [StructuralArgument],
    receipts: &'a [CompletionReceipt],
}

impl Call<'_> {
    fn input(
        &self,
        source: &ContentStructuralPlace,
        projection: semantic_vocabulary::ContentProjectionIdentity,
        origins: &mut ancestry::OriginTraversal,
    ) -> Result<(semantic_vocabulary::ClaimId, ContentStructuralPlace), &'static str> {
        let (ordinal, formal) = self
            .declaration
            .structural_parameters
            .iter()
            .enumerate()
            .find(|(_, formal)| formal.place == source.root)
            .ok_or("content input is not a declared structural formal")?;
        let argument = self
            .arguments
            .get(ordinal)
            .ok_or("content input has no structural argument")?;
        if formal.access != StructuralAccess::Owned || argument.access != StructuralAccess::Owned {
            return Err("content conservation input has no owned claim transfer");
        }
        let path = structural_path(
            self.module.types.structural_types.iter(),
            &self.module.identities,
            formal.structural_type,
            &source.segments,
        )
        .ok_or("content input path is not a static declared projection")?;
        let receipt = exactly_one(self.receipts.iter(), |receipt| {
            receipt.argument_index as usize == ordinal && receipt.path == path
        })
        .ok_or("content call has no unique completion receipt for its exact input path")?;
        ancestry::local_roots(self.module, self.caller, receipt.claim, projection, origins)?;
        let actual_type = self
            .caller
            .structural_place_type(argument.place)
            .ok_or("content input actual has no structural carrier")?;
        let actual = crate::runtime_structural_path_tip(
            self.module.types.structural_types.iter(),
            actual_type,
            &argument.path,
        )
        .ok_or("content call actual does not select a structural carrier")?;
        if actual.id != formal.structural_type {
            return Err("content call actual does not match its formal carrier");
        }
        if self
            .caller
            .structural_place(argument.place)
            .is_some_and(|place| {
                matches!(
                    place.kind,
                    semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal { .. }
                        | semantic_vocabulary::StructuralPlaceKind::BlockParameter { .. }
                )
            })
        {
            return Err("content call needs a stable argument revision");
        }
        let mut actual_path = argument.path.clone();
        actual_path.extend(path);
        let segments = content_path(
            self.module.types.structural_types.iter(),
            &self.module.identities,
            actual_type,
            &actual_path,
        )
        .ok_or("content input actual path is not reachable on its carrier")?;
        Ok((
            receipt.claim,
            ContentStructuralPlace {
                root: argument.place,
                version: ContentPlaceVersion::Current,
                segments,
            },
        ))
    }

    fn output(
        &self,
        source: &ContentStructuralPlace,
        carrier: semantic_vocabulary::StructuralTypeId,
    ) -> Result<ContentStructuralPlace, &'static str> {
        let path = structural_path(
            self.module.types.structural_types.iter(),
            &self.module.identities,
            carrier,
            &source.segments,
        )
        .ok_or("content output path is not a static declared projection")?;
        exactly_one(self.result.claims.iter(), |binding| binding.path == path)
            .ok_or("content call has no unique returned claim at its exact result path")?;
        Ok(ContentStructuralPlace {
            root: self.result.place,
            version: ContentPlaceVersion::Current,
            segments: source.segments.clone(),
        })
    }

    fn instantiate(
        &self,
        operation: &Operation,
        guarantee: &ContentConservationGuarantee,
    ) -> Result<ContentPartitionComposition, &'static str> {
        let (declared_result, carrier) = self
            .declaration
            .structural_contract_result()
            .ok_or("content call has no declared result coordinate")?;
        if self.result.structural_type != carrier
            || *guarantee
                != ContentConservationGuarantee::from_declaration(
                    &guarantee.conservation,
                    &self.declaration.structural_parameters,
                    Some(declared_result),
                )?
        {
            return Err("content call changed the declared conservation report or result carrier");
        }
        let law = &guarantee.conservation;
        let mut inputs = BTreeSet::new();
        let mut has_output = false;
        let mut substitutions = BTreeMap::new();
        let mut origins = ancestry::OriginTraversal::new();
        law.try_for_each_projection(|projection, source| {
            let owner = self
                .module
                .types
                .structural_domains
                .iter()
                .filter_map(|domain| domain.content_projection.as_ref())
                .find(|owner| owner.identity == projection)
                .ok_or("content call projection has no domain owner")?;
            if &owner.algebra != law.algebra() {
                return Err("content call changed its projection algebra");
            }
            let target = match source.version {
                ContentPlaceVersion::Entry => {
                    let (claim, actual) = self.input(source, projection, &mut origins)?;
                    inputs.insert(claim);
                    actual
                }
                ContentPlaceVersion::Current => {
                    has_output = true;
                    self.output(source, carrier)?
                }
            };
            substitutions.insert(source.clone(), target);
            Ok(())
        })?;
        if inputs.is_empty() || !has_output {
            return Err("content transfer needs both an entry claim and a normal result claim");
        }
        let derived = ContentConservation::new(
            law.algebra().clone(),
            substitute_term(law.left(), &substitutions)?,
            substitute_term(law.right(), &substitutions)?,
        );
        Ok(ContentPartitionComposition {
            producer_operation: operation.id,
            source_report_fingerprint: guarantee.report_fingerprint,
            source_structural_places: guarantee.structural_places.clone(),
            source: law.clone(),
            input_claims: inputs.into_iter().collect(),
            substitutions: substitutions
                .into_iter()
                .map(|(source, target)| ContentPlaceSubstitution { source, target })
                .collect(),
            derived,
        })
    }
}

fn substitute_term(
    term: &ContentTerm,
    substitutions: &BTreeMap<ContentStructuralPlace, ContentStructuralPlace>,
) -> Result<ContentTerm, &'static str> {
    let project = |term: &ContentTerm| {
        let ContentTerm::Projection {
            projection,
            subject,
        } = term
        else {
            return Err("content separation is not canonical");
        };
        Ok(ContentTerm::Projection {
            projection: *projection,
            subject: substitutions
                .get(subject)
                .ok_or("content term lost its substitution")?
                .clone(),
        })
    };
    match term {
        ContentTerm::Projection { .. } => project(term),
        ContentTerm::Separate(parts) => {
            ContentTerm::separate(parts.iter().map(project).collect::<Result<Vec<_>, _>>()?)
                .map_err(|_| "content actuals collapse separated projections")
        }
    }
}
