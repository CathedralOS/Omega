//! Canonical declaration storage to a target's policy input.
//!
//! This route never creates source-shaped placeholders or requires a checked
//! program. Callback layout closure remains a separate obligation; unsupported
//! callback declarations refuse rather than dropping their hidden arguments.

use super::CallingPolicySignature;
use calling_conventions::validate_fresh_native_parameter_identity;
use calling_conventions::{
    BoundaryNativeParameterGeometry, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
};
use layout::BoundaryValueLayout;

impl CallingPolicySignature {
    /// Materialize one exact published signature directly from canonical storage.
    /// This projection constructs no source correspondence and grants no provider or
    /// representation authority. Private callback geometry must be closed by
    /// its dedicated catalog owner before that surface can enter this route.
    pub fn from_terminal(
        module: &terminal_psi::TerminalModule,
        boundary: semantic_vocabulary::BoundaryMachineId,
        target: target::TargetProfile,
    ) -> Result<Self, String> {
        let signature = module
            .calling_signatures
            .iter()
            .find(|signature| signature.boundary == boundary)
            .ok_or("boundary has no retained calling signature declaration")?;
        let declaration = module
            .boundary_machine(boundary)
            .ok_or("calling signature lost its exact boundary declaration")?;
        let telescope = signature.telescope.as_ref().map_err(Clone::clone)?;
        if !telescope.binders.is_empty() || !telescope.direct_callbacks.is_empty() {
            return Err(
                "Terminal calling signature still requires private callback geometry closure"
                    .into(),
            );
        }
        if declaration.parameter_order.len() != telescope.parameters.len() {
            return Err(
                "calling telescope differs from its exact boundary parameter roster".into(),
            );
        }
        let mut recipes = layout::TerminalBoundaryRecipes::new(module, target.target_name());
        let layout_target = layout::LayoutTarget::for_profile(target)?;
        let mut values = BoundaryValueLayout::new(layout_target);
        let mut parameters = Vec::with_capacity(telescope.parameters.len());
        let mut native_formals =
            Vec::<BoundaryNativeParameterGeometry>::with_capacity(telescope.parameters.len());
        let mut scalars = declaration.scalar_parameters.iter();
        let mut structural = declaration.structural_parameters.iter();
        for (formal_ordinal, (kind, formal)) in declaration
            .parameter_order
            .iter()
            .zip(&telescope.parameters)
            .enumerate()
        {
            let recipe = match kind {
                terminal_psi::BoundaryParameterKind::Scalar => recipes.scalar(
                    *scalars
                        .next()
                        .ok_or("calling scalar parameter is outside its exact lane")?,
                )?,
                terminal_psi::BoundaryParameterKind::Structural => {
                    let parameter = structural
                        .next()
                        .ok_or("calling structural parameter is outside its exact lane")?;
                    match parameter.access {
                        terminal_psi::StructuralAccess::Owned => {
                            recipes.value(parameter.structural_type)?
                        }
                        _ => recipes.reference(parameter.structural_type)?,
                    }
                }
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
        if scalars.next().is_some() || structural.next().is_some() {
            return Err("calling telescope omitted a declared parameter lane".into());
        }
        let result = match &declaration.result {
            terminal_psi::BoundaryMachineResult::Unit => None,
            terminal_psi::BoundaryMachineResult::Scalar(scalar) => {
                Some(recipes.scalar(scalar.scalar_type)?)
            }
            terminal_psi::BoundaryMachineResult::Structural(result) => {
                Some(recipes.value(result.structural_type)?)
            }
        }
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
        let (shapes, fields, _) = values.into_parts();
        let closed = calling_conventions::NativeParameterTelescope::close(
            native_formals,
            telescope.direct_callbacks.iter().cloned(),
            target.native_target(),
        )?;
        let mut native_parameters = arena::Arena::with_capacity(closed.parameters.len());
        let native_parameter_rows = native_parameters.insert_many(closed.parameters);
        Ok(Self {
            target: target.native_target(),
            geometry: calling_conventions::BoundarySignatureGeometry {
                shapes,
                fields,
                parameters,
                result,
                native_parameters,
                native_parameter_rows,
                callback_bindings: arena::Arena::new(),
                callback_binding_rows: arena::HandleSpan::empty(),
                callback_layouts: arena::Arena::new(),
                callback_layout_rows: arena::HandleSpan::empty(),
            },
            callbacks: calling_conventions::CallbackMaterializationContext::default(),
        })
    }
}
