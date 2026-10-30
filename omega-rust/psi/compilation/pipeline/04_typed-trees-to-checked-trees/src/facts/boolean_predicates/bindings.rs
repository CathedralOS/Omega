//! Bind each genuine observation in its declared value telescope.
//! Scalar ordinals and structural subject ordinals are different namespaces;
//! Result is an explicit value binding, never a guessed parameter position.
use super::{BoundObservation, CanonicalPlace, Operand, Scalar, SymbolHandle, TypeReferenceHandle};
use checked_trees::{CheckedStructuralCaseRoot, CheckedStructuralCaseSubject};

pub(super) fn parameter(
    program: &dyn validation::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
    result: Option<(&CanonicalPlace, TypeReferenceHandle)>,
    operand: &Operand,
) -> Option<(CanonicalPlace, TypeReferenceHandle)> {
    match operand {
        Operand::Scalar(operand) => scalar_parameter(program, parameters, result, operand),
        Operand::CaseSubject(subject) => case_parameter(program, parameters, result, subject),
    }
}

fn scalar_parameter(
    program: &dyn validation::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
    result: Option<(&CanonicalPlace, TypeReferenceHandle)>,
    operand: &Scalar,
) -> Option<(CanonicalPlace, TypeReferenceHandle)> {
    let resolved = match operand {
        Scalar::Parameter { position, .. } => {
            let mut scalars = parameters
                .iter()
                .filter(|parameter| crate::values::occupies_scalar_position(program, parameter));
            if let Some(parameter) = scalars.nth(*position) {
                (
                    crate::flow::canonical_place_from_symbol(parameter.symbol)?,
                    parameter.type_reference,
                )
            } else if *position
                == parameters
                    .iter()
                    .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
                    .count()
            {
                let (place, reference) = result?;
                (place.clone(), reference)
            } else {
                return None;
            }
        }
        Scalar::StorageRead { symbol, .. } => {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == *symbol)?;
            (
                crate::flow::canonical_place_from_symbol(*symbol)?,
                parameter.type_reference,
            )
        }
        Scalar::StructuralParameterField { .. } | Scalar::StructuralParameterIndexedRead { .. } => {
            let selected = operand.parameter_observation_projection()?;
            let parameter_position = selected.parameter_position;
            let path = selected.path;
            if usize::try_from(parameter_position).ok()? == parameters.len() {
                let (place, reference) = result?;
                let (segments, reference, _) =
                    crate::values::scalar::structural_fields::resolve_structural_path(
                        program,
                        reference,
                        false,
                        &path,
                        &mut |_, _| Some(()),
                    )?;
                let reference =
                    validation::field_domains::readable_type_reference(program, reference)?;
                let mut place = place.clone();
                place.segments.extend(segments);
                (place, reference)
            } else {
                let (root, segments, reference, _) =
                    crate::values::resolve_structural_parameter_path(
                        program,
                        parameters,
                        parameter_position,
                        &path,
                    )?;
                // An empty projection can observe a whole primitive loan.
                // Bind its readable referent carrier, as scalar contract
                // lowering does, rather than comparing the reference type
                // itself with the scalar observation. Write-only stays absent.
                let reference =
                    validation::field_domains::readable_type_reference(program, reference)?;
                (
                    CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(root),
                        segments,
                    },
                    reference,
                )
            }
        }
        _ => return None,
    };
    Some(resolved)
}

fn case_parameter(
    program: &dyn validation::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
    result: Option<(&CanonicalPlace, TypeReferenceHandle)>,
    subject: &CheckedStructuralCaseSubject,
) -> Option<(CanonicalPlace, TypeReferenceHandle)> {
    let (place, reference) = match subject.root {
        CheckedStructuralCaseRoot::Parameter { position } => {
            let (root, segments, reference, _) = crate::values::resolve_structural_parameter_path(
                program,
                parameters,
                position,
                &subject.path,
            )?;
            (
                CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    segments,
                },
                reference,
            )
        }
        CheckedStructuralCaseRoot::Result => {
            let (place, reference) = result?;
            let (segments, selected, _) =
                crate::values::scalar::structural_fields::resolve_structural_path(
                    program,
                    reference,
                    false,
                    &subject.path,
                    &mut |_, _| Some(()),
                )?;
            let mut place = place.clone();
            place.segments.extend(segments);
            (place, selected)
        }
    };
    let reference = validation::field_domains::readable_type_reference(program, reference)?;
    compatible_type(program, &Operand::CaseSubject(subject.clone()), reference)
        .then_some((place, reference))
}

/// This is type authentication, not formation, tag selection or access proof.
/// Those obligations remain at the actual checked observation and load.
pub(super) fn compatible_type(
    program: &dyn validation::CheckingInput,
    operand: &Operand,
    reference: TypeReferenceHandle,
) -> bool {
    match operand {
        Operand::Scalar(scalar) => scalar.primitive_type().is_some_and(|primitive| {
            program.primitive_type_reference(reference) == Some(primitive)
        }),
        Operand::CaseSubject(_) => program
            .closed_data_definition(reference)
            .is_some_and(|data| {
                program
                    .data_members(data)
                    .iter()
                    .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
            }),
    }
}

pub(super) fn entry(
    program: &dyn validation::CheckingInput,
    machine: SymbolHandle,
    position: u32,
    path: Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
    operand: &Operand,
) -> Option<Operand> {
    match operand {
        Operand::Scalar(scalar) => {
            super::entry_scalar(program, machine, position, path, scalar.primitive_type()?)
                .map(Operand::Scalar)
        }
        Operand::CaseSubject(_) => {
            let owner = program.machine_by_symbol(machine)?;
            let entry = validation::activation_target_state_index(program, owner, machine)?;
            let parameters = program.state_parameters(program.machine_states(owner).get(entry)?);
            let subject = CheckedStructuralCaseSubject {
                root: CheckedStructuralCaseRoot::Parameter { position },
                path,
            };
            case_parameter(program, parameters, None, &subject)?;
            Some(Operand::CaseSubject(subject))
        }
    }
}

impl BoundObservation {
    /// A matching absent primitive is not type correspondence. Structural
    /// subjects retain the complete nominal application identity, independent
    /// of the selected case and without borrowing a scalar representation.
    pub(crate) fn compatible_type(
        &self,
        program: &dyn validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        let other = Self {
            operand: self.operand.clone(),
            place: self.place.clone(),
            type_reference: reference,
        };
        self.compatible_with(program, &other)
    }

    pub(crate) fn compatible_with(
        &self,
        program: &dyn validation::CheckingInput,
        other: &Self,
    ) -> bool {
        match (&self.operand, &other.operand) {
            (Operand::Scalar(left), Operand::Scalar(right)) => left
                .primitive_type()
                .is_some_and(|primitive| right.primitive_type() == Some(primitive)),
            (Operand::CaseSubject(_), Operand::CaseSubject(_)) => {
                if !compatible_type(program, &self.operand, self.type_reference)
                    || !compatible_type(program, &other.operand, other.type_reference)
                {
                    return false;
                }
                let left = program.type_identity(
                    typed_trees::type_identity::TypeIdentityRequest::ordinary(self.type_reference),
                );
                let right = program.type_identity(
                    typed_trees::type_identity::TypeIdentityRequest::ordinary(other.type_reference),
                );
                left != Default::default() && left == right
            }
            _ => false,
        }
    }
}
