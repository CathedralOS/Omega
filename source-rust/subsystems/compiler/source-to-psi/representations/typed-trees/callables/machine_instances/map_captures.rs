//! Index the prepared declarations of each named map by its caller and complete argument tuple.

use super::{CapturedMachineApplication, SymbolHandle, TypedTrees};
use crate::source_state::type_identity::NormalizedTypeIdentity;

type CaptureHandle = arena::Handle<CapturedMachineApplication>;

type CaptureKey = (SymbolHandle, NormalizedTypeIdentity);

/// A transient sparse index of existing capture records. Entries retain
/// handles only; all source and body correspondence stays in the record.
#[derive(Default)]
pub struct NamedConformanceCaptureIndex {
    rows: arena::HandleKeyMap<CaptureKey, Option<CaptureHandle>>,
}

impl NamedConformanceCaptureIndex {
    pub fn new(program: &TypedTrees) -> Self {
        let mut index = Self::default();
        for (handle, _) in program.captured_machine_applications.iter() {
            // Unrelated unsupported records grant no authority. A demanded
            // named application still needs one valid, unambiguous entry.
            let _ = index.insert(program, handle);
        }
        index
    }

    pub fn insert(
        &mut self,
        program: &TypedTrees,
        handle: CaptureHandle,
    ) -> Result<(), &'static str> {
        if !program.captured_machine_applications.is_valid(handle) {
            return Err("named capture has a foreign record");
        }
        let key = Self::capture_key(program, program.captured_machine_applications.get(handle))?;
        self.rows
            .entry(key)
            .and_modify(|known| {
                if *known != Some(handle) {
                    *known = None;
                }
            })
            .or_insert(Some(handle));
        Ok(())
    }

    pub fn get(
        &self,
        program: &TypedTrees,
        owner: SymbolHandle,
        call: crate::source_state::references::StaticArgumentCall<'_>,
    ) -> Result<Option<CaptureHandle>, &'static str> {
        use crate::source_state::references::StaticArgumentCall;
        let (key, enclosing) = match call {
            StaticArgumentCall::Expression(_, written)
                if written.requirement_application.is_valid() =>
            {
                let Some(key) = program.named_conformance_capture_key(written)? else {
                    return Ok(None);
                };
                (key, None)
            }
            _ => {
                let Some(group) = program.enclosing_map_capture(owner)? else {
                    return Ok(None);
                };
                (Self::capture_key(program, group)?, Some(group))
            }
        };
        let handle = self
            .rows
            .get(&key)
            .copied()
            .map(|entry| entry.ok_or("named application has ambiguous captured bodies"))
            .transpose()?;
        if let Some(group) = enclosing
            && !handle.is_some_and(|handle| {
                std::ptr::eq(program.captured_machine_applications.get(handle), group)
            })
        {
            return Err("lexical caller has no unique selected map application");
        }
        Ok(handle)
    }

    fn capture_key(
        program: &TypedTrees,
        capture: &CapturedMachineApplication,
    ) -> Result<CaptureKey, &'static str> {
        use crate::source_state::expression::{StaticMachineArgument, StaticSymbolApplication};
        if !program
            .expression_table
            .static_machine_application_is_valid(capture.binding_application())
            || !program
                .expression_table
                .static_machine_application_is_valid(capture.supplying_application)
        {
            return Err("named capture has no actual instantiation applications");
        }
        let source = program.static_machine_application(capture.binding_application());
        let supplying = program.static_machine_application(capture.supplying_application);
        let declaration = capture.conformance_application.declaration;
        if source.registration_operation != declaration
            || source.binding_declaration != declaration
            || source.lexical_owner != supplying.lexical_owner
            || !program
                .symbols
                .symbols()
                .nodes()
                .is_valid(source.lexical_owner)
        {
            return Err("named capture changed its constructor or source owner");
        }
        let implementation = program
            .conformance_by_symbol(declaration)
            .ok_or("named capture lost its implementation")?;
        let mut rows = implementation
            .rows
            .iter()
            .filter(|row| row.requirement == supplying.registration_operation);
        let row = rows
            .next()
            .ok_or("named capture lost its supplying requirement")?;
        if rows.next().is_some()
            || !row.source.owns_realization()
            || capture.declaration(row.realization_machine)?.is_none()
        {
            return Err("named capture changed its actual requirement row");
        }
        let identity = |arguments: &[StaticMachineArgument]| {
            let selected = StaticMachineArgument {
                symbol: declaration,
                application: Some(Box::new(StaticSymbolApplication {
                    member: arena::Handle::invalid(),
                    lifetime_arguments: Box::new([]),
                    arguments: arguments.into(),
                })),
                ..Default::default()
            };
            crate::source_state::type_identity::applied_index_identity(
                program,
                crate::source_state::type_identity::IndexIdentityArgument::StaticArgument(
                    &selected,
                ),
                source.lexical_owner,
                &[],
            )
        };
        let selected = identity(&capture.conformance_application.arguments)
            .ok_or("named capture lost its complete map identity")?;
        let parameters = program.conformance_type_parameters(implementation);
        let bindings = program.static_machine_application_bindings(capture.binding_application());
        if bindings.len() != parameters.len()
            || capture.conformance_application.arguments.len() != parameters.len()
            || parameters
                .iter()
                .zip(bindings)
                .zip(capture.conformance_application.arguments.iter())
                .any(|((parameter, binding), argument)| {
                    binding.parameter != parameter.symbol
                        || !program.static_binding_matches_argument(
                            &parameter.kind,
                            &binding.value,
                            argument,
                            source.lexical_owner,
                            source.lexical_owner,
                        )
                })
        {
            return Err("named capture changed its actual constructor bindings");
        }
        if Some(selected)
            != identity(program.static_machine_application_arguments(capture.binding_application()))
            || program
                .type_identity_text(selected)
                .as_deref()
                .map(str::as_bytes)
                != Some(capture.conformance_application.identity_encoding.as_ref())
        {
            return Err("named capture constructor disagrees with its complete map tuple");
        }
        Ok((source.lexical_owner, selected))
    }
}
