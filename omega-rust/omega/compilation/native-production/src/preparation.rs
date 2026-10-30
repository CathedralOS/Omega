//! Choose native work before the application completes its scalar demand.

use crate::{NativeCompileRequest, NativeScalarCompletion};
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::{PreparedNativeWork, RetainedTerminalArtifact};

pub struct PreparedNativeCompilation<'input> {
    pub(crate) retained: &'input RetainedTerminalArtifact,
    pub(crate) request: NativeCompileRequest<'input>,
    pub(crate) recovery: image_emission::ExecutableImageEmissionRequest,
    pub(crate) work: PreparedNativeWork<'input>,
    pub(crate) scalars: NativeScalarCompletion<'input>,
    pub(crate) components: Vec<crate::components::PreparedComponent<'input>>,
}

impl<'input> PreparedNativeCompilation<'input> {
    /// Each artifact/proposal group is completed once, including every
    /// authored condition even when its explicit native value demand is empty.
    pub fn scalar_completions(
        &mut self,
    ) -> impl Iterator<Item = &mut NativeScalarCompletion<'input>> {
        std::iter::once(&mut self.scalars)
            .chain(
                self.components
                    .iter_mut()
                    .map(|component| &mut component.scalars),
            )
            .filter(|completion| completion.is_required())
    }

    pub fn into_image_request(self) -> image_emission::ExecutableImageEmissionRequest {
        self.recovery
    }
}

pub fn prepare<'input>(
    retained: &'input RetainedTerminalArtifact,
    mut request: NativeCompileRequest<'input>,
) -> Result<
    PreparedNativeCompilation<'input>,
    (
        image_emission::ExecutableImageEmissionRequest,
        Vec<Diagnostic>,
    ),
> {
    let recovery = request.image_request.clone();
    let result = (|| {
        let (artifact, proposal) = request.prepare_terminal(retained)?;
        let components =
            crate::components::prepare_components(request.components, artifact.view())?;
        let selections = terminal_installation::target_context(
            artifact.view(),
            proposal.target_profile().target_name(),
        );
        let layout_target = proposal
            .target_profile()
            .layout_target_facts()
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "native entry has no admitted target layout facts",
                )]
            })?;
        let layouts = terminal_installation::admit_target_layouts(
            artifact.view(),
            &selections,
            &layout_target,
        )
        .map_err(|reason| vec![Diagnostic::error(format!("entry RAM layout: {reason:?}"))])?;
        let entry = proposal.program_entry().checked_entry().terminal_entry();
        let mut recipes = Vec::with_capacity(request.private_entry_ram.len());
        for initializer in request.private_entry_ram {
            let row = artifact
                .semantic_module()
                .placed_view_inputs
                .iter()
                .find(|row| row.machine == entry && row.position == initializer.position)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "private entry RAM initializer answers no declared entry position",
                    )]
                })?;
            let recipe = terminal_installation::admit_private_entry_ram(
                artifact.view(),
                &layouts,
                entry,
                &terminal_installation::PrivateEntryRamRecipe {
                    input: row.clone(),
                    contents: initializer.contents.clone(),
                },
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
            recipes.push(recipe);
        }
        let calls = components
            .iter()
            .flat_map(|component| &component.calls)
            .collect::<Vec<_>>();
        let work = proposal.prepare_native_execution_with_components(artifact, &recipes, &calls)?;
        let scalars = NativeScalarCompletion::new(
            artifact,
            proposal,
            work.scalar_application_demand().iter().copied(),
        );
        Ok((work, scalars, components))
    })();
    match result {
        Ok((work, scalars, components)) => Ok(PreparedNativeCompilation {
            retained,
            request,
            recovery,
            work,
            scalars,
            components,
        }),
        Err(diagnostics) => Err((recovery, diagnostics)),
    }
}
