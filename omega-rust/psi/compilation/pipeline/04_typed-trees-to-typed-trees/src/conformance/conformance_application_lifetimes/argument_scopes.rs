//! Ordinary argument regions belong to the exact projected source application.
//! Field storage elaboration runs later, after constant/generic preparation;
//! borrowing its declaration now would turn Session<'storage> into the caller's
//! undeclared 'storage rather than resolving its actual activation operand.

use symbols::SymbolHandle;
use typed_trees::type_identity::applied_lifetime_parameter_identity;
use typed_trees::type_identity::lifetime_parameter_identity;
use typed_trees::type_identity::{AppliedTypeIdentityScope, IndexIdentityArgument};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use validation::canonical_place_from_expression_in_state;

/// Read the complete source occurrence before field storage elaboration.
/// Missing prefixes withhold inference; an ordinary value has an empty roster.
/// The shared projection narrows Type-binder actuals to their supplying prefix.
pub(super) fn source_application(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    argument: typed_trees::expression::ExpressionHandle,
) -> Option<(TypeReferenceHandle, Vec<TypeReferenceHandle>)> {
    let place =
        canonical_place_from_expression_in_state(program, state.symbol, usize::MAX, argument)?;
    validation::canonical_place_source_type_application(program, state.symbol, usize::MAX, &place)
}

#[derive(Clone, Copy)]
pub(super) struct Scope<'scopes> {
    pub(super) owner: SymbolHandle,
    pub(super) frames: &'scopes [AppliedTypeIdentityScope<'scopes>],
}

impl<'scopes> Scope<'scopes> {
    pub(super) fn resolve(
        mut self,
        program: &dyn validation::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Self)> {
        let mut visited = Vec::new();
        loop {
            let coordinate = (reference, self.owner, self.frames.len());
            if !reference.is_valid() || visited.contains(&coordinate) {
                return None;
            }
            // The same binder can occur in two simultaneous applications of
            // one declaration. Shrinking the supplying prefix is progress;
            // only a repeated complete scoped coordinate forms a cycle.
            visited.push(coordinate);
            let owner = program
                .type_reference_table
                .operand_lifetime_owner(reference);
            if owner.is_valid() {
                let count = self
                    .frames
                    .iter()
                    .rposition(|frame| {
                        program.lifetime_declaration_owner(frame.declaration)
                            == program.lifetime_declaration_owner(owner)
                    })
                    .map_or(0, |ordinal| ordinal + 1);
                self.frames = &self.frames[..count];
                self.owner = owner;
            }
            let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                return Some((reference, self));
            };
            let supplied = self
                .frames
                .iter()
                .enumerate()
                .rev()
                .find_map(|(ordinal, frame)| {
                    frame.arguments.iter().find_map(|(parameter, actual)| {
                        if parameter != symbol {
                            return None;
                        }
                        match actual {
                            IndexIdentityArgument::TypeArgument(actual)
                            | IndexIdentityArgument::TypeReference(actual) => {
                                Some((*actual, ordinal))
                            }
                            _ => None,
                        }
                    })
                });
            if let Some((actual, ordinal)) = supplied {
                reference = actual;
                self.frames = &self.frames[..ordinal];
                continue;
            }
            if let Some(origin) = program
                .data_definition_by_symbol(*symbol)
                .and_then(|definition| definition.generic_instance)
            {
                reference = origin;
                continue;
            }
            return Some((reference, self));
        }
    }

    /// Payload spelling is recovered only after declaration/ordinal resolution.
    /// A foreign same-spelled region cannot become a caller-owned operand.
    pub(super) fn argument(
        self,
        program: &dyn validation::CheckingInput,
        caller: SymbolHandle,
        name: &typed_trees::name::Identifier,
    ) -> Option<typed_trees::name::Identifier> {
        if !caller.is_valid() {
            return Some(name.clone());
        }
        let region =
            applied_lifetime_parameter_identity(program, self.owner, name.as_str(), self.frames)?;
        let spelling = typed_trees::type_identity::lifetime_parameter_name(program, region)?;
        (lifetime_parameter_identity(program, caller, spelling.as_str()) == Some(region))
            .then(|| spelling.clone())
    }
}
