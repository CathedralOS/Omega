//! Canonical declaration storage to a target's policy input.
//!
//! This route never creates source-shaped placeholders or requires a checked
//! program. Native-only callback arguments and private storage demands join
//! the same exact published declaration as ordinary semantic formals.

use super::CallingPolicySignature;
use calling_conventions::validate_fresh_native_parameter_identity;
use calling_conventions::{
    BoundaryNativeParameterGeometry, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
};
use layout::BoundaryValueLayout;

pub struct CanonicalCallingSignature {
    pub abi: semantic_vocabulary::CallingAbiIdentity,
    pub declaration: semantic_vocabulary::CallingSignatureId,
    pub requirement_identity: String,
    pub policy: CallingPolicySignature,
    pub opaque_uses: Vec<CanonicalOpaqueUse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalOpaqueUse {
    pub shape_root: u16,
    pub application: language_semantics::opaque_representations::SelectedOpaqueApplication,
}

impl CanonicalCallingSignature {
    /// Project an exact declaration application, including dormant declarations
    /// requested by a policy that have no emitted boundary invocation.
    pub fn materialize(
        module: &terminal_psi::TerminalModule,
        declaration: semantic_vocabulary::CallingSignatureId,
        target: target::TargetProfile,
        extents: &dyn Fn(
            semantic_vocabulary::ScalarApplicationId,
        ) -> Result<u64, terminal_semantics::StorageGeometryError>,
    ) -> Result<Self, String> {
        let signature = module
            .calling_abi_signature(declaration)
            .ok_or("calling signature lost its exact declaration application")?;
        let storage = signature.storage.as_ref().map_err(Clone::clone)?;
        let telescope = &storage.telescope;
        if storage.parameters.len() != telescope.parameters.len() {
            return Err(
                "calling telescope differs from its exact boundary parameter roster".into(),
            );
        }
        let mut recipes =
            layout::TerminalBoundaryRecipes::new(module, target.target_name(), extents);
        let layout_target = layout::LayoutTarget::for_profile(target)?;
        let mut values = BoundaryValueLayout::new(layout_target);
        let mut parameters = Vec::with_capacity(telescope.parameters.len());
        let mut native_formals =
            Vec::<BoundaryNativeParameterGeometry>::with_capacity(telescope.parameters.len());
        let mut callback_storage = layout::TerminalCallbackLayouts::new(module, target, extents);
        let mut private_layouts = Vec::new();
        for (formal_ordinal, (parameter, formal)) in storage
            .parameters
            .iter()
            .zip(&telescope.parameters)
            .enumerate()
        {
            // Parameter storage already names the referent for an
            // ordinary borrowed formal. An owned reference carrier
            // names it explicitly; references inside records remain
            // leaves rather than recursively expanding pointees.
            let mut storage_root = parameter.structural_type;
            let mut reference_shells = Vec::new();
            while let Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) =
                module.structural_type(storage_root).map(|row| &row.shape)
            {
                if reference_shells.contains(&storage_root) || reference_shells.len() >= 128 {
                    return Err(
                        "callback formal has a recursive or excessive reference shell".into(),
                    );
                }
                reference_shells.push(storage_root);
                storage_root = *referent;
            }
            private_layouts.extend(
                callback_storage.for_parameter(layout::CallbackStorageRoot {
                    record: storage_root,
                    parameter: formal.identity,
                    formal_ordinal: u32::try_from(formal_ordinal)
                        .map_err(|_| "calling signature has too many formals")?,
                    native_ordinal: formal.native_ordinal,
                })?,
            );
            let recipe = match parameter.access {
                terminal_psi::StructuralAccess::Owned => {
                    recipes.value(parameter.structural_type)?
                }
                _ => recipes.reference(parameter.structural_type)?,
            };
            let (_, root) = values.value_type(recipes.recipes(), recipe)?;
            validate_fresh_native_parameter_identity(
                native_formals.iter().map(|parameter| parameter.identity),
                formal.identity,
                formal.native_ordinal,
            )?;
            parameters.push(root);
            native_formals.push(BoundaryNativeParameterGeometry {
                identity: formal.identity,
                native_ordinal: formal.native_ordinal,
                shape: BoundaryNativeParameterShape::Semantic(root),
                origin: BoundaryNativeParameterOrigin::SemanticFormal {
                    formal_ordinal: u32::try_from(formal_ordinal)
                        .map_err(|_| "calling signature has too many formals")?,
                },
            });
        }
        let result = storage
            .result
            .map(|result| recipes.value(result))
            .transpose()?
            .map(|recipe| {
                values
                    .value_type(recipes.recipes(), recipe)
                    .map(|(_, root)| root)
            })
            .transpose()?;
        module
            .identities
            .text(signature.requirement_identity)
            .filter(|identity| !identity.is_empty())
            .ok_or("calling signature has no exact requirement identity")?;
        let (shapes, fields, occurrences) = values.into_parts();
        let opaque_uses = occurrences
            .into_iter()
            .map(|occurrence| {
                let selection = module
                    .opaque_representations
                    .get(occurrence.selection_ordinal)
                    .ok_or("boundary opaque occurrence lost its exact canonical selection")?;
                Ok(CanonicalOpaqueUse {
                    shape_root: occurrence.shape_root,
                    application: selection.application.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let closed = calling_conventions::NativeParameterTelescope::close(
            native_formals,
            telescope.direct_callbacks.iter().cloned(),
            target.native_target(),
        )?;
        let binders =
            calling_conventions::TerminalCallingTelescope::callback_bindings(module, telescope)?;
        // Compact requirement coordinates are report keys, not equality of
        // nominal requirements. Close the complete exact-name census before
        // publishing the smaller materialization context.
        let mut requirements = Vec::new();
        for (report, identity) in binders
            .iter()
            .map(|binder| (binder.requirement, binder.requirement_identity.as_str()))
            .chain(private_layouts.iter().map(|layout| {
                (
                    layout.terminal.requirement,
                    layout.terminal.requirement_identity.as_str(),
                )
            }))
        {
            super::callback_bindings::validate_callback_requirement_report_collision(
                &requirements,
                report,
                identity,
            )?;
            if !requirements.iter().any(|(prior, _)| *prior == report) {
                requirements.push((report, identity.to_owned()));
            }
        }
        let mut callbacks = calling_conventions::CallbackMaterializationContext {
            binders: binders
                .iter()
                .map(|binder| calling_conventions::CallbackBinderRequirement {
                    binder: binder.binder,
                    requirement: binder.requirement,
                })
                .collect(),
            demands: closed.direct_demands().collect(),
        };
        callbacks
            .demands
            .extend(private_layouts.iter().map(|layout| {
                calling_conventions::NativeCallbackDemand {
                    destination: layout.destination.clone(),
                    requirement: layout.terminal.requirement,
                }
            }));
        callbacks
            .demands
            .sort_by(|left, right| left.destination.cmp(&right.destination));
        private_layouts.sort_by(|left, right| left.destination.cmp(&right.destination));
        let mut callback_bindings = arena::Arena::with_capacity(binders.len());
        let callback_binding_rows = callback_bindings.insert_many(binders);
        let mut callback_layouts = arena::Arena::with_capacity(private_layouts.len());
        let callback_layout_rows = callback_layouts.insert_many(private_layouts);
        let mut native_parameters = arena::Arena::with_capacity(closed.parameters.len());
        let native_parameter_rows = native_parameters.insert_many(closed.parameters);
        Ok(Self {
            abi: signature.identity(&module.identities)?,
            declaration,
            requirement_identity: module
                .identities
                .text(signature.requirement_identity)
                .ok_or("canonical signature lost its exact requirement identity")?
                .to_owned(),
            opaque_uses,
            policy: CallingPolicySignature {
                target: target.native_target(),
                geometry: calling_conventions::BoundarySignatureGeometry {
                    shapes,
                    fields,
                    parameters,
                    result,
                    native_parameters,
                    native_parameter_rows,
                    callback_bindings,
                    callback_binding_rows,
                    callback_layouts,
                    callback_layout_rows,
                },
                callbacks,
            },
        })
    }
}
