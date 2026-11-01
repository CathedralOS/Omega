//! Publish declaration ABI storage in the one structural type catalog.

use super::{LoweringError, UnitPlans, lookup_type_id};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::TerminalIdentityTableBuilder;
use terminal_psi::{CallingAbiDeclaration, CallingAbiStorage, CallingSignatureTelescope};
use typed_trees::type_identity::TypeIdentityId;

pub(super) fn lower(
    identities: &mut TerminalIdentityTableBuilder,
    plans: UnitPlans<'_>,
    types: &[(TypeIdentityId, StructuralTypeId)],
) -> Result<Vec<CallingAbiDeclaration>, LoweringError> {
    let mut rows = Vec::with_capacity(plans.calling_signatures().len());
    for source in plans.calling_signatures() {
        let storage = match &source.storage {
            Ok(storage) => Ok(CallingAbiStorage {
                parameters: storage.parameters.iter().map(|parameter| Ok(terminal_psi::CallingAbiParameter {
                    structural_type: lookup_type_id(types, &parameter.type_identity)?,
                    access: crate::expression_preparation::bindings::structural_paths::terminal_access(parameter.access),
                })).collect::<Result<_, LoweringError>>()?,
                result: storage.result.map(|identity| lookup_type_id(types, &identity)).transpose()?,
                telescope: CallingSignatureTelescope {
                    parameters: storage.telescope.parameters.iter().map(|formal| terminal_psi::CallingRuntimeFormal {
                        identity: formal.identity, native_ordinal: formal.native_ordinal,
                    }).collect(),
                    binders: storage.telescope.binders.iter().map(|binder| terminal_psi::CallingCallbackBinder {
                        binder: binder.binder, requirement: binder.requirement,
                        requirement_identity: identities.intern(&binder.requirement_identity),
                        static_machine_ordinal: binder.static_machine_ordinal,
                        invokes_synchronously: binder.invokes_synchronously,
                    }).collect(),
                    direct_callbacks: storage.telescope.direct_callbacks.iter().map(|callback| terminal_psi::CallingDirectCallback {
                        identity: callback.identity, native_ordinal: callback.native_ordinal,
                        binder: callback.binder, requirement: callback.requirement,
                    }).collect(),
                },
            }),
            Err(reason) => Err(reason.clone()),
        };
        rows.push(CallingAbiDeclaration {
            id: source.id,
            requirement_identity: identities.intern(&source.requirement_identity),
            declaring_application: identities.intern(&source.declaring_application),
            operands: source
                .operands
                .iter()
                .map(|operand| terminal_psi::CallingStaticOperand {
                    ordinal: operand.ordinal,
                    kind: operand.kind,
                    identity: identities.intern(&operand.identity),
                })
                .collect(),
            default_eligible: source.default_eligible,
            formals: source.formals.clone(),
            storage,
        });
    }
    rows.sort_by_key(|row| row.id);
    if rows.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err(LoweringError::Unsupported(
            "declaration ABI application identities collide",
        ));
    }
    Ok(rows)
}
