//! A call can write through a saved pointer or a pointer stored in a record.
//! Match its complete captured storage interpretation before using a checked
//! write to keep an old fact. A conservative write footprint, declaration,
//! or lifetime debt cannot establish that exact presentation and extent.

use super::ArgumentSources;
use crate::flow::mutation::{CallMutationWrite, StorageRegion, WriteDestination};

impl ArgumentSources<'_, '_> {
    pub(in crate::flow) fn captured_reference_confines_write(
        &self,
        control: &checked_trees::FlowControlFacts,
        write: &CallMutationWrite,
    ) -> Option<Vec<checked_trees::FlowReferenceForwarding>> {
        let result = self.captured_reference_write_verdict(control, write);
        if std::env::var_os("OMEGA_CALL_WRITE_TRACE").is_some() {
            eprintln!(
                "CHECKSITE write-confinement state={:?} statement={} source={:?} region={:?} destination={:?} checked_type={:?} verdict={result:?}",
                self.state,
                self.statement,
                write.source,
                write.region,
                write.destination,
                write.checked_write_type,
            );
        }
        result.ok().flatten()
    }

    fn captured_reference_write_verdict(
        &self,
        control: &checked_trees::FlowControlFacts,
        write: &CallMutationWrite,
    ) -> Result<Option<Vec<checked_trees::FlowReferenceForwarding>>, &'static str> {
        if write.destination != Some(WriteDestination::Referent) {
            return Err("the write does not select referent contents");
        }
        let Some(StorageRegion::Declared(relative)) = &write.source else {
            return Err("the write has no declared relative source");
        };
        let StorageRegion::Declared(written) = &write.region else {
            return Err("the write has no concrete declared region");
        };
        let position = self
            .parameters
            .iter()
            .position(|parameter| relative.root.as_symbol() == Some(parameter.symbol))
            .ok_or("the relative write root is not a captured formal")?;
        let parameter = &self.parameters[position];
        let mut selected = parameter.type_reference;
        let mut boundary = None;
        for index in 0..=relative.segments.len() {
            if let Some(shell) = validation::reference_shell(self.program, selected) {
                boundary = Some((index, shell.referee));
                break;
            }
            let Some(segment) = relative.segments.get(index) else {
                break;
            };
            selected = validation::project_type_reference_from_segments(
                self.program,
                selected,
                std::slice::from_ref(segment),
            )
            .ok_or("the relative write has an invalid typed projection")?;
        }
        let (boundary, referee) = boundary.ok_or("the relative write crosses no reference")?;
        let suffix = &relative.segments[boundary..];
        // A second pointer load needs its own captured source. Projecting a
        // type through it is not evidence about the pointer value it held.
        for length in 0..=suffix.len() {
            let projected = validation::project_type_reference_from_segments(
                self.program,
                referee,
                &suffix[..length],
            )
            .ok_or("the reference-relative suffix has no typed projection")?;
            if validation::reference_shell(self.program, projected).is_some() {
                return Err("the write needs a further pointer's captured value");
            }
        }
        let capture = self
            .captures
            .iter()
            .find(|capture| capture.position == position && capture.destination.is_empty())
            .ok_or("the writer formal has no whole captured actual")?;
        let path = validation::corresponding_place_projection(
            self.program,
            capture.type_reference,
            &relative.segments[..boundary],
        )
        .ok_or("the reference leaf has no captured member correspondence")?;
        let actual = validation::project_type_reference_from_segments(
            self.program,
            capture.type_reference,
            &path,
        )
        .and_then(|reference| validation::reference_shell(self.program, reference))
        .ok_or("the selected captured path is not a reference")?;
        let carrier = self.program.normalized_type_identity(referee);
        if !carrier.is_valid() || self.program.normalized_type_identity(actual.referee) != carrier {
            return Err("the formal and captured reference have different referee carriers");
        }
        let sources = capture.reference_sources.get_or_init(|| {
            let machine = self
                .program
                .machine_by_symbol(self.call_scope.machine)
                .ok_or("captured argument has no machine owner")?;
            let resolver = validation::CallFrameResolver::new(self.program)
                .ok_or("captured argument has no call-frame context")?;
            resolver.completed_operand_reference_sources(
                machine,
                self.state,
                self.statement,
                capture.expression,
                capture.type_reference,
            )
        });
        let leaves = sources.as_ref().map_err(|reason| *reason)?;
        let leaf = leaves.iter().find(|leaf| leaf.path == path);
        if std::env::var_os("OMEGA_CALL_WRITE_TRACE").is_some() {
            eprintln!(
                "CHECKSITE write-confinement-capture state={:?} statement={} actual={:?} captured_type={:?} expected_path={path:?} leaves={:?}",
                self.state,
                self.statement,
                capture.expression,
                capture.type_reference,
                leaves.iter().map(|leaf| &leaf.path).collect::<Vec<_>>(),
            );
        }
        let sources = leaf
            .ok_or("the completed actual has no matching reference leaf")?
            .sources
            .as_ref()
            .map_err(|reason| *reason)?;
        if sources.is_empty() {
            return Err("the captured leaf has no supplying storage");
        }
        let state = self
            .program
            .state_by_symbol(self.state)
            .ok_or("the captured call has no state owner")?;
        let mut matched = false;
        for source in sources {
            if std::env::var_os("OMEGA_CALL_WRITE_TRACE").is_some() {
                eprintln!(
                    "CHECKSITE write-confinement-source state={:?} statement={} source={source:?} suffix={suffix:?}",
                    self.state, self.statement,
                );
            }
            // These interpretations need a separate whole-region bridge.
            // Their equal carrier or backing root cannot provide one.
            if source.invocation_output.is_some()
                || source.invocation_result.is_some()
                || source.is_image_storage
                || source.collection_coarse
                || source.representation.is_some()
                || !source.recasts.is_empty()
            {
                return Err("the captured source has opaque or nonidentical storage presentation");
            }
            let region = crate::flow::mutation::exact_source_place(
                self.program,
                state,
                self.statement,
                source,
            )
            .ok_or("the captured source has no exact structured storage coordinate")?;
            let projected = validation::completed_storage::project_captured_storage_place(
                self.program,
                self.state,
                self.statement,
                region,
                suffix,
            )
            .ok_or("the captured storage suffix has no exact projection")?;
            matched |= projected == *written;
        }
        if !matched {
            return Ok(None);
        }
        Ok(Some(self.reference_forwarding_dependencies(
            control, capture, &path,
        )?))
    }
}
