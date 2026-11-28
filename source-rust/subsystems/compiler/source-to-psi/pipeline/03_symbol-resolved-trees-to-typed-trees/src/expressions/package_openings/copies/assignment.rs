//! Trace a queried package to the place supplying that member of a stored value.
use super::*;

pub(crate) struct Assignment {
    destination: PackageSubject,
    pub(super) value: Option<ExpressionHandle>,
}

impl Assignment {
    pub(crate) fn replacement(destination: PackageSubject) -> Self {
        Self {
            destination,
            value: None,
        }
    }

    pub(super) fn from_value(
        program: &TypedTrees,
        value: ExpressionHandle,
        destination: PackageSubject,
    ) -> Option<Self> {
        let aggregate = matches!(
            program.expression_table.expression(value),
            ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
        );
        (aggregate || package_place(program, value).is_some()).then_some(Self {
            destination,
            value: Some(value),
        })
    }

    /// A literal supplies its individual members. Only a whole-place copy
    /// supplies the enclosing package's own hidden type.
    pub(super) fn before(
        &self,
        program: &TypedTrees,
        state: &State,
        place: &mut PackageSubject,
    ) -> bool {
        if place.root == self.destination.root
            && let Some(path) = place.steps.strip_prefix(self.destination.steps.as_slice())
        {
            let Some(input) = self
                .value
                .and_then(|v| member_input(program, state, v, path))
            else {
                return false;
            };
            *place = input;
            return true;
        }
        !write_replaces(program, state, &self.destination, place)
    }
}

/// Select the input at the assignment's own point in history. All literal
/// members share that point, including assignments that swap two elements.
fn member_input(
    program: &TypedTrees,
    state: &State,
    mut value: ExpressionHandle,
    mut member: &[PackageSubjectStep],
) -> Option<PackageSubject> {
    let table = &program.expression_table;
    loop {
        match table.expression(value) {
            ExpressionNode::StructLiteral(record) => {
                let (PackageSubjectStep::Field(symbol), rest) = member.split_first()? else {
                    return None;
                };
                let mut fields = table
                    .struct_fields(record.fields)
                    .iter()
                    .filter(|field| field.field_symbol == *symbol);
                value = fields.next()?.value;
                if fields.next().is_some() {
                    return None;
                }
                member = rest;
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let (PackageSubjectStep::ConstantIndex(i), rest) = member.split_first()? else {
                    return None;
                };
                value = *table
                    .expression_handles(*elements)
                    .get(usize::try_from(*i).ok()?)?;
                member = rest;
            }
            _ => {
                let mut input = stable_subject(program, state, &package_place(program, value)?)?;
                input.steps.extend_from_slice(member);
                return Some(input);
            }
        }
    }
}
