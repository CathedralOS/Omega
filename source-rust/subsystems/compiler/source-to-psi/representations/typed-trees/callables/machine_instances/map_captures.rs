//! Index the prepared declarations of each named map by its caller and complete argument tuple.

use super::{CapturedMachineApplication, SymbolHandle, TypedTrees};
use crate::source_state::type_identity::NormalizedTypeIdentity;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};

type CaptureHandle = arena::Handle<CapturedMachineApplication>;

type CaptureKey = (SymbolHandle, NormalizedTypeIdentity);

pub enum NamedCaptureError {
    Invalid(&'static str),
    Expanding,
}

impl NamedCaptureError {
    pub fn detail(&self) -> &'static str {
        match self {
            Self::Invalid(message) => message,
            Self::Expanding => "recursive map call has no established finite argument closure",
        }
    }
}

impl From<&'static str> for NamedCaptureError {
    fn from(message: &'static str) -> Self {
        Self::Invalid(message)
    }
}

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
    ) -> Result<Option<CaptureHandle>, NamedCaptureError> {
        use crate::source_state::references::StaticArgumentCall;
        let written = match call {
            StaticArgumentCall::Expression(_, call) if call.requirement_application.is_valid() => {
                let Some(key) = program.named_conformance_capture_key(call)? else {
                    return Ok(None);
                };
                Some(key)
            }
            _ => None,
        };
        let requested = written.map(|_| {
            program
                .static_machine_application(call.application())
                .registration_operation
        });
        let mut holder = owner;
        let mut visited = Vec::new();
        let mut recursive_arguments = Vec::new();
        while let Some(group) = program.enclosing_map_capture(holder)? {
            let key = Self::capture_key(program, group)?;
            if written.is_none_or(|written| written.1 == key.1) {
                let handle = self
                    .lookup(key)?
                    .ok_or("enclosing map has no indexed capture")?;
                if !std::ptr::eq(program.captured_machine_applications.get(handle), group) {
                    return Err("enclosing map call changed its capture".into());
                }
                return Ok(Some(handle));
            }
            let incoming = program.static_machine_application(group.supplying_application);
            if requested == Some(incoming.registration_operation)
                && let StaticArgumentCall::Expression(_, site) = call
                && program
                    .expression_table
                    .name_path_node(site.receiver)
                    .is_some_and(|receiver| {
                        receiver.symbol == group.conformance_application.declaration
                    })
            {
                recursive_arguments.push(
                    group
                        .conformance_application
                        .arguments
                        .iter()
                        .map(|argument| argument.type_reference)
                        .collect::<Vec<_>>(),
                );
            }
            if visited.contains(&key.0) {
                return Err("named capture has cyclic supplying owners".into());
            }
            visited.push(key.0);
            holder = program
                .machine_by_own_or_state_symbol(key.0)
                .ok_or("named capture has no supplying machine")?
                .symbol;
        }
        let Some(key) = written else {
            return Ok(None);
        };
        let selected = self.lookup(key)?;
        if selected.is_none()
            && let StaticArgumentCall::Expression(_, site) = call
            && let TypeReferenceNode::Generic { arguments, .. } =
                program.type_reference_node(site.requirement_application)
        {
            // A substitution may move a growing term through the whole telescope
            // before replacing it with a constant. Allow that finite chain.
            let mut current = program.type_reference_handles(*arguments);
            let mut growth_steps = 0;
            for older in &recursive_arguments {
                if introduces_nested_type(program, current, older) {
                    growth_steps += 1;
                    if growth_steps > current.len() {
                        return Err(NamedCaptureError::Expanding);
                    }
                    current = older;
                }
            }
        }
        Ok(selected)
    }

    fn lookup(&self, key: CaptureKey) -> Result<Option<CaptureHandle>, &'static str> {
        self.rows
            .get(&key)
            .copied()
            .map(|entry| entry.ok_or("named application has ambiguous captured bodies"))
            .transpose()
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
        if supplying.registration_operation == declaration {
            // A Build selects the whole record. Tie that choice to the actual
            // Product operand and require a body for each member the record owns.
            let product = program
                .live_product_operand_applications()
                .into_iter()
                .any(|handle| {
                    let operand = program.product_operand_applications.get(handle);
                    operand.supplying_application == capture.supplying_application
                        && operand.declaration == declaration
                        && operand.implementation.as_ref() == Some(&capture.conformance_application)
                });
            if !product {
                return Err("provider capture has no matching selected Product operand");
            }
            for row in implementation
                .rows
                .iter()
                .filter(|row| row.source.owns_realization())
            {
                if capture.declaration(row.realization_machine)?.is_none() {
                    return Err("provider capture omits a selected member");
                }
            }
        } else {
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

fn introduces_nested_type(
    program: &TypedTrees,
    current: &[TypeReferenceHandle],
    older: &[TypeReferenceHandle],
) -> bool {
    let previous = older
        .iter()
        .map(|&reference| program.normalized_type_identity(reference))
        .collect::<Vec<_>>();
    current.iter().any(|&reference| {
        !previous.contains(&program.normalized_type_identity(reference))
            && crate::source_state::references::type_reference_inputs(program, reference)
                .types
                .into_iter()
                .any(|child| {
                    child != reference
                        && previous.contains(&program.normalized_type_identity(child))
                })
    })
}
