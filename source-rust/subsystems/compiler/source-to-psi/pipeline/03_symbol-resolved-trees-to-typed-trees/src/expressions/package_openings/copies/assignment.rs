//! All place operands of one stored value are read before its destination changes.
use super::*;

pub(crate) struct Assignment {
    destination: PackageSubject,
    pub(super) sources: Vec<FieldSource>,
}

pub(super) struct FieldSource {
    member: Vec<PackageSubjectStep>,
    input: PackageSubject,
}

impl Assignment {
    pub(crate) fn replacement(destination: PackageSubject) -> Self {
        Self {
            destination,
            sources: Vec::new(),
        }
    }

    pub(super) fn from_value(
        program: &TypedTrees,
        state: &State,
        value: ExpressionHandle,
        destination: PackageSubject,
    ) -> Option<Self> {
        let mut assignment = Self::replacement(destination);
        assignment.collect(program, state, value, &mut Vec::new());
        (!assignment.sources.is_empty()).then_some(assignment)
    }

    fn collect(
        &mut self,
        program: &TypedTrees,
        state: &State,
        value: ExpressionHandle,
        member: &mut Vec<PackageSubjectStep>,
    ) {
        let table = &program.expression_table;
        match table.expression(value) {
            ExpressionNode::StructLiteral(record) => {
                for field in table.struct_fields(record.fields) {
                    member.push(PackageSubjectStep::Field(field.field_symbol));
                    self.collect(program, state, field.value, member);
                    member.pop();
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                for (i, &item) in table.expression_handles(*elements).iter().enumerate() {
                    member.push(PackageSubjectStep::ConstantIndex(i as u64));
                    self.collect(program, state, item, member);
                    member.pop();
                }
            }
            _ => {
                if let Some(place) = package_place(program, value)
                    && let Some(input) = stable_subject(program, state, &place)
                {
                    self.sources.push(FieldSource {
                        member: member.clone(),
                        input,
                    });
                }
            }
        }
    }

    /// A record literal identifies each supplied member, while the enclosing
    /// package's own hidden type is fresh. Whole copies use an empty member path.
    pub(super) fn before(
        &self,
        program: &TypedTrees,
        state: &State,
        place: &mut PackageSubject,
    ) -> bool {
        if place.root == self.destination.root
            && let Some(path) = place.steps.strip_prefix(self.destination.steps.as_slice())
        {
            let mut matches = self.sources.iter().filter_map(|field| {
                let suffix = path.strip_prefix(field.member.as_slice())?;
                Some((&field.input, suffix))
            });
            let Some((input, suffix)) = matches.next() else {
                return false;
            };
            if matches.next().is_some() {
                return false;
            }
            let mut origin = input.clone();
            origin.steps.extend_from_slice(suffix);
            *place = origin;
            return true;
        }
        !write_replaces(program, state, &self.destination, place)
    }
}
