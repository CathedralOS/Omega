//! Resolve lifecycle result roles inside their authenticated core application.
//!
//! Core declaration order is semantic: it selects retained case and field IDs,
//! not their numeric encodings. Source projection and ingress admission own
//! authentication; matching this shape cannot authorize a lookalike boundary.
//! The same result contract serves ingress consistency and result construction.
//! Selecting a case supplies no execution evidence or custody: the operation
//! must still complete its byte transaction and transfer the actual inputs.

use semantic_vocabulary::{IntegerSign, StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration, StructuralCaseDeclaration,
    StructuralFieldDeclaration, StructuralMultiplicity, StructuralPathQualification,
    StructuralPathSegment, StructuralTypeShape, TerminalAccessLifecycleOperation,
    TerminalAccessLifecyclePlan, TerminalModule,
};

/// Borrowed declarations of the exact core result application, not a second
/// portable catalog and not an authorization token.
pub enum AccessResultContract<'module> {
    View {
        result: &'module BoundaryStructuralResultDeclaration,
        ready: &'module StructuralCaseDeclaration,
        rejected: &'module StructuralCaseDeclaration,
        failures: &'module [StructuralCaseDeclaration; 3],
        carrier: StructuralTypeId,
        access: terminal_psi::StructuralAccess,
    },
    Initialize {
        result: &'module BoundaryStructuralResultDeclaration,
        ready: &'module StructuralCaseDeclaration,
        rejected: &'module StructuralCaseDeclaration,
        returned: &'module [StructuralFieldDeclaration; 2],
        failures: &'module [StructuralCaseDeclaration; 3],
    },
    MoveOut {
        result: &'module BoundaryStructuralResultDeclaration,
        moved: &'module StructuralCaseDeclaration,
    },
    Relocate {
        result: &'module BoundaryStructuralResultDeclaration,
        ready: &'module StructuralCaseDeclaration,
        rejected: &'module StructuralCaseDeclaration,
        moved: &'module [StructuralFieldDeclaration; 2],
        returned: &'module [StructuralFieldDeclaration; 2],
        failures: &'module [StructuralCaseDeclaration; 3],
    },
}

/// Locate the Ready descriptor in the canonical borrowed outcome. This is a
/// structural role query, not authority: callers must already select the
/// authenticated View declaration or its retained lifecycle plan.
pub fn access_view_reference_slot(
    types: &[terminal_psi::StructuralTypeDeclaration],
    result: StructuralTypeId,
) -> Option<(Vec<StructuralPathSegment>, StructuralTypeId)> {
    let shape = &terminal_psi::find_structural_type(types, result)?.shape;
    let StructuralTypeShape::Sum { cases } = shape else {
        return None;
    };
    let [ready, _] = cases.as_slice() else {
        return None;
    };
    let [field] = ready.fields.as_slice() else {
        return None;
    };
    let StructuralTypeShape::Reference {
        referent,
        access: terminal_psi::reading_borrow_pair_pattern!(),
    } = terminal_psi::find_structural_type(types, field.field_type.as_structural()?)?.shape
    else {
        return None;
    };
    Some((
        terminal_psi::structural_case_initializer_path(shape, ready.id, 0, field.id)?,
        referent,
    ))
}

pub fn access_result_contract<'module>(
    module: &'module TerminalModule,
    plan: &TerminalAccessLifecyclePlan,
) -> Result<AccessResultContract<'module>, &'static str> {
    let boundary = module
        .boundary_machine(plan.boundary)
        .ok_or("placement result lost its boundary declaration")?;
    let BoundaryMachineResult::Structural(result) = &boundary.result else {
        return Err("access lifecycle must return its owned structural outcome");
    };
    if result.multiplicity != StructuralMultiplicity::Affine || !result.qualifications.is_empty() {
        return Err("placement outcome changed its core ownership or root qualifications");
    }
    let cases = sum_cases(module, result.structural_type)?;
    match plan.operation {
        TerminalAccessLifecycleOperation::View => {
            let [ready, rejected] = cases else {
                return Err("borrowed placement requires exactly Ready and Rejected outcomes");
            };
            let [view] = ready.fields.as_slice() else {
                return Err("borrowed placement Ready must carry one reference");
            };
            let Some(StructuralTypeShape::Reference { referent, access }) = module
                .structural_type(structural_type(view)?)
                .map(|declaration| &declaration.shape)
            else {
                return Err("borrowed placement Ready does not retain its reference carrier");
            };
            let storage = boundary
                .structural_parameters
                .first()
                .ok_or("borrowed placement lost its exact storage parameter")?;
            if !matches!(access, terminal_psi::reading_borrow_pair_pattern!())
                || *access != storage.access
                || !module.access_view_plans.iter().any(|view| {
                    view.carrier == *referent
                        && view.schema == plan.schema
                        && view.policy_identity == plan.policy_identity
                        && view.placement == plan.placement
                })
            {
                return Err("borrowed placement Ready changed its exact placed interpretation");
            }
            let [returned, reason] = rejected.fields.as_slice() else {
                return Err("borrowed placement Rejected must retain its empty return and reason");
            };
            let returned = module
                .structural_type(structural_type(returned)?)
                .ok_or("borrowed placement return record is absent")?;
            if returned.owns_linear_claim
                || !matches!(&returned.shape, StructuralTypeShape::Record { fields, .. } if fields.is_empty())
                || !result.projected_qualifications.is_empty()
            {
                return Err(
                    "borrowed placement must not manufacture returned storage or qualifications",
                );
            }
            Ok(AccessResultContract::View {
                result,
                ready,
                rejected,
                failures: failure_cases(module, reason)?,
                carrier: *referent,
                access: *access,
            })
        }
        TerminalAccessLifecycleOperation::Initialize => {
            let (vacant, resident_domain) = plan.storage.owned_content()?;
            let [ready, rejected] = cases else {
                return Err(
                    "placement initialization requires exactly Ready and Rejected outcomes",
                );
            };
            let [resident] = ready.fields.as_slice() else {
                return Err("placement Ready must carry its single resident extent");
            };
            let [returned_field, reason] = rejected.fields.as_slice() else {
                return Err("placement Rejected must carry its returned inputs and reason");
            };
            require_type(resident, plan.storage.owner_carrier())?;
            let returned_type = structural_type(returned_field)?;
            let Some(StructuralTypeShape::Record { fields, .. }) = module
                .structural_type(returned_type)
                .map(|declaration| &declaration.shape)
            else {
                return Err("placement rejection lost its returned-input record");
            };
            let returned: &[StructuralFieldDeclaration; 2] = fields
                .as_slice()
                .try_into()
                .map_err(|_| "placement rejection must return exactly storage and value")?;
            require_type(&returned[0], plan.storage.carrier)?;
            require_type(&returned[1], plan.schema)?;
            let failures = failure_cases(module, reason)?;
            let owner_path = [
                StructuralPathSegment::Case(ready.id),
                StructuralPathSegment::Field(resident.identity),
            ];
            match plan.storage.owner {
                terminal_psi::TerminalAccessOwner::Extent => storage_qualifications(
                    result,
                    &owner_path,
                    plan.storage.granted,
                    resident_domain,
                )?,
                terminal_psi::TerminalAccessOwner::Run {
                    carrier,
                    storage,
                    singleton,
                    ..
                } => {
                    let singleton = singleton
                        .ok_or("typed run initialization lost its singleton qualification")?;
                    let (whole, descendants) = StructuralPathQualification::select(
                        &result.qualifications,
                        &result.projected_qualifications,
                        &owner_path,
                    );
                    let Some(StructuralTypeShape::Record { fields, .. }) = module
                        .structural_type(carrier)
                        .map(|declaration| &declaration.shape)
                    else {
                        return Err("typed run initialization lost its owner record");
                    };
                    let backing = fields
                        .iter()
                        .find(|field| field.id == storage)
                        .ok_or("typed run initialization lost its backing field")?;
                    let (backing_domains, nested) = StructuralPathQualification::select(
                        &whole,
                        &descendants,
                        &[StructuralPathSegment::Field(backing.identity)],
                    );
                    if whole.len() != 2
                        || !whole.contains(&resident_domain)
                        || !whole.contains(&singleton)
                        || backing_domains.as_slice() != [plan.storage.granted]
                        || !nested.is_empty()
                        || descendants.len() != 1
                    {
                        return Err(
                            "typed run initialization changed its live owner or nested backing qualifications",
                        );
                    }
                }
            }
            storage_qualifications(
                result,
                &[
                    StructuralPathSegment::Case(rejected.id),
                    StructuralPathSegment::Field(returned_field.identity),
                    StructuralPathSegment::Field(returned[0].identity),
                ],
                plan.storage.granted,
                vacant,
            )?;
            // A field's structural carrier does not include its occurrence's
            // domain contract. Rejection returns the exact value input rather
            // than rebuilding equal-shaped data with fewer obligations.
            let value = boundary
                .structural_parameters
                .get(1)
                .ok_or("placement rejection lost its original value contract")?;
            let (whole, descendants) = StructuralPathQualification::select(
                &result.qualifications,
                &result.projected_qualifications,
                &[
                    StructuralPathSegment::Case(rejected.id),
                    StructuralPathSegment::Field(returned_field.identity),
                    StructuralPathSegment::Field(returned[1].identity),
                ],
            );
            if whole != value.qualifications || descendants != value.projected_qualifications {
                return Err("placement rejection changed the original value qualifications");
            }
            Ok(AccessResultContract::Initialize {
                result,
                ready,
                rejected,
                returned,
                failures,
            })
        }
        TerminalAccessLifecycleOperation::Relocate => {
            let (vacant, live) = plan.storage.owned_content()?;
            let [ready, rejected] = cases else {
                return Err("run relocation requires Ready and Rejected outcomes");
            };
            let [moved_field] = ready.fields.as_slice() else {
                return Err("run relocation Ready must carry its moved run and vacated backing");
            };
            let [returned_field, reason] = rejected.fields.as_slice() else {
                return Err("run relocation Rejected must retain both inputs and a reason");
            };
            let moved = record_pair(module, moved_field)?;
            let returned = record_pair(module, returned_field)?;
            require_type(&moved[0], plan.storage.owner_carrier())?;
            require_type(&moved[1], plan.storage.carrier)?;
            require_type(&returned[0], plan.storage.owner_carrier())?;
            require_type(&returned[1], plan.storage.carrier)?;
            let owner_path = [
                StructuralPathSegment::Case(ready.id),
                StructuralPathSegment::Field(moved_field.identity),
                StructuralPathSegment::Field(moved[0].identity),
            ];
            let (whole, descendants) = StructuralPathQualification::select(
                &result.qualifications,
                &result.projected_qualifications,
                &owner_path,
            );
            let backing_path = plan
                .storage
                .owner_storage_path(&module.structural_types)
                .ok_or("run relocation lost its backing projection")?;
            let (backing, nested) =
                StructuralPathQualification::select(&whole, &descendants, &backing_path);
            if !matches!(
                plan.storage.owner,
                terminal_psi::TerminalAccessOwner::Run { .. }
            ) || whole.as_slice() != [live]
                || backing.as_slice() != [plan.storage.granted]
                || !nested.is_empty()
                || descendants.len() != 1
            {
                return Err("run relocation changed its live owner or backing qualifications");
            }
            storage_qualifications(
                result,
                &[
                    StructuralPathSegment::Case(ready.id),
                    StructuralPathSegment::Field(moved_field.identity),
                    StructuralPathSegment::Field(moved[1].identity),
                ],
                plan.storage.granted,
                vacant,
            )?;
            for (position, field) in returned.iter().enumerate() {
                let parameter = boundary
                    .structural_parameters
                    .get(position)
                    .ok_or("run relocation lost an original owner")?;
                let (whole, descendants) = StructuralPathQualification::select(
                    &result.qualifications,
                    &result.projected_qualifications,
                    &[
                        StructuralPathSegment::Case(rejected.id),
                        StructuralPathSegment::Field(returned_field.identity),
                        StructuralPathSegment::Field(field.identity),
                    ],
                );
                if whole != parameter.qualifications
                    || descendants != parameter.projected_qualifications
                {
                    return Err(
                        "run relocation rejection changed an original owner's qualifications",
                    );
                }
            }
            Ok(AccessResultContract::Relocate {
                result,
                ready,
                rejected,
                moved,
                returned,
                failures: failure_cases(module, reason)?,
            })
        }
        TerminalAccessLifecycleOperation::MoveOut => {
            let (vacant, _) = plan.storage.owned_content()?;
            // This declaration constrains the extracted value; it does not
            // establish it. Execution must recover actual memberships from
            // the original resident's retained custody and join them here.
            let [moved] = cases else {
                return Err("placement extraction requires exactly its Moved outcome");
            };
            let [storage, value] = moved.fields.as_slice() else {
                return Err("placement extraction must return exactly storage and value");
            };
            require_type(storage, plan.storage.carrier)?;
            require_type(value, plan.schema)?;
            storage_qualifications(
                result,
                &[
                    StructuralPathSegment::Case(moved.id),
                    StructuralPathSegment::Field(storage.identity),
                ],
                plan.storage.granted,
                vacant,
            )?;
            Ok(AccessResultContract::MoveOut { result, moved })
        }
    }
}

fn record_pair<'module>(
    module: &'module TerminalModule,
    field: &StructuralFieldDeclaration,
) -> Result<&'module [StructuralFieldDeclaration; 2], &'static str> {
    let Some(StructuralTypeShape::Record { fields, .. }) = module
        .structural_type(structural_type(field)?)
        .map(|declaration| &declaration.shape)
    else {
        return Err("relocation result must retain its two-owner record");
    };
    fields
        .as_slice()
        .try_into()
        .map_err(|_| "relocation result must retain exactly two owners")
}

fn failure_cases<'module>(
    module: &'module TerminalModule,
    reason: &StructuralFieldDeclaration,
) -> Result<&'module [StructuralCaseDeclaration; 3], &'static str> {
    let failures: &[StructuralCaseDeclaration; 3] = sum_cases(module, structural_type(reason)?)?
        .try_into()
        .map_err(|_| "placement rejection lost its three core failure cases")?;
    let [range, alignment, incompatible] = failures;
    for case in [range, alignment] {
        let [first, second] = case.fields.as_slice() else {
            return Err("placement failure must carry its two unsigned coordinates");
        };
        for field in [first, second] {
            if field.relevance.is_erased()
                || field
                    .field_type
                    .scalar_type()
                    .and_then(|scalar| scalar.as_integer())
                    .is_none_or(|integer| {
                        integer.is_address()
                            || integer.sign() != IntegerSign::Unsigned
                            || integer.bits() != 64
                    })
            {
                return Err("placement failure coordinate is not a represented u64");
            }
        }
    }
    if !incompatible.fields.is_empty() {
        return Err("incompatible placement supply must have an empty payload");
    }
    Ok(failures)
}

fn sum_cases(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
) -> Result<&[StructuralCaseDeclaration], &'static str> {
    match module.structural_type(structural_type) {
        Some(declaration) if !declaration.owns_linear_claim => match &declaration.shape {
            StructuralTypeShape::Sum { cases } => Ok(cases),
            _ => Err("placement result requires its exact core sum declaration"),
        },
        _ => Err("placement result sum declaration is absent or has changed ownership"),
    }
}

fn structural_type(field: &StructuralFieldDeclaration) -> Result<StructuralTypeId, &'static str> {
    if field.relevance.is_erased() {
        return Err("placement result cannot erase an owned payload");
    }
    field
        .field_type
        .as_structural()
        .ok_or("placement result lost its structural payload")
}

fn require_type(
    field: &StructuralFieldDeclaration,
    expected: StructuralTypeId,
) -> Result<(), &'static str> {
    if structural_type(field)? != expected {
        return Err("placement result payload differs from its exact storage or schema");
    }
    Ok(())
}

fn storage_qualifications(
    result: &BoundaryStructuralResultDeclaration,
    path: &[StructuralPathSegment],
    granted: StructuralDomainId,
    content: StructuralDomainId,
) -> Result<(), &'static str> {
    let (whole, descendants) = StructuralPathQualification::select(
        &result.qualifications,
        &result.projected_qualifications,
        path,
    );
    if whole.len() != 2
        || !whole.contains(&granted)
        || !whole.contains(&content)
        || !descendants.is_empty()
    {
        return Err("placement result storage lost its exact grant and content qualifications");
    }
    Ok(())
}
