use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::types::{PrimitiveType, TypeReferenceNode};

/// Transient declaration-local projection. A measure's current typed carrier
/// has no populated symbol, so the unique table occurrence is its identity;
/// matching field spellings in another measure do not make it the same order.
pub(super) struct RankProjection {
    measure_index: usize,
    pub(super) data: SymbolHandle,
    pub(super) parameter: SymbolHandle,
    pub(super) argument_position: usize,
    pub(super) fields: Vec<SymbolHandle>,
}

impl RankProjection {
    pub(super) fn resolve(program: &TypedTrees, machine: &Machine) -> Option<Self> {
        let witness = machine.termination_plan.implementation_witness.as_ref()?;
        if !witness.view_arguments.is_empty() || witness.rank_range.is_some() {
            return None;
        }
        // Never rediscover the subject by scanning rendered expressions.
        let custody = program.ranking_expression_custody_for(machine.symbol)?;
        let [subject] = custody.subjects.as_slice() else {
            return None;
        };
        if witness.subjects.len() != 1
            || !custody.view_arguments.is_empty()
            || custody.rank_range.is_some()
        {
            return None;
        }
        let ExpressionNode::Name(path) = program
            .expression_table
            .expression(unwrapped(program, *subject))
        else {
            return None;
        };
        if !path.symbol.is_valid() {
            return None;
        }
        let entry = program.machine_states(machine).first()?;
        let mut parameters = program
            .state_parameters(entry)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .enumerate()
            .filter(|(_, parameter)| parameter.symbol == path.symbol);
        let (argument_position, parameter) = parameters.next()?;
        if parameters.next().is_some() {
            return None;
        }
        let TypeReferenceNode::Named { symbol: data, .. } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return None;
        };
        if !data.is_valid() {
            return None;
        }
        let definition = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *data)?;

        // The normalized view path is the existing measure-selection carrier.
        // Resolve it once, reject ambiguity, then use only this occurrence and
        // exact field declarations for rank equality/decrease comparisons.
        let mut measures = program
            .measures()
            .iter()
            .enumerate()
            .filter(|(_, measure)| {
                program
                    .measure_path_members(measure.name)
                    .iter()
                    .map(|member| member.as_str())
                    .eq(witness.view_path.split("::"))
            });
        let (measure_index, measure) = measures.next()?;
        if measures.next().is_some() || !measure.lexicographic || measure.parameter.is_some() {
            return None;
        }
        let (owner, _) = witness.view_path.rsplit_once("::")?;
        if owner != program.symbols.display_path(*data, "::") {
            return None;
        }

        let components = program.expression_table.expression_handles(measure.body);
        if components.is_empty() {
            return None;
        }
        let mut fields = Vec::with_capacity(components.len());
        for component in components {
            // Lexicographic syntax declares unqualified projections in its
            // exact owner. No arbitrary member expression or global name
            // search can contribute a ranking component.
            let ExpressionNode::Name(component) = program.expression_table.expression(*component)
            else {
                return None;
            };
            let [name] = program
                .expression_table
                .name_path_members(component.members)
            else {
                return None;
            };
            let mut declarations = program
                .data_members(definition)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) if field.name.as_str() == name.as_str() => Some(field),
                    _ => None,
                });
            let field = declarations.next()?;
            if declarations.next().is_some()
                || !field.symbol.is_valid()
                || fields.contains(&field.symbol)
                || (component.symbol.is_valid() && component.symbol != field.symbol)
                || !matches!(
                    program
                        .type_reference_table
                        .primitive_type(field.type_reference),
                    Some(
                        PrimitiveType::U8
                            | PrimitiveType::U16
                            | PrimitiveType::U32
                            | PrimitiveType::U64
                    )
                )
            {
                return None;
            }
            fields.push(field.symbol);
        }
        Some(Self {
            measure_index,
            data: *data,
            parameter: parameter.symbol,
            argument_position,
            fields,
        })
    }

    pub(super) fn same_order(&self, other: &Self) -> bool {
        self.measure_index == other.measure_index
            && self.data == other.data
            && self.fields == other.fields
    }

    pub(super) fn is_subject(&self, program: &TypedTrees, expression: ExpressionHandle) -> bool {
        matches!(program.expression_table.expression(unwrapped(program, expression)),
            ExpressionNode::Name(path) if path.symbol == self.parameter)
    }

    pub(super) fn is_field(
        &self,
        program: &TypedTrees,
        expression: ExpressionHandle,
        field: SymbolHandle,
    ) -> bool {
        let ExpressionNode::Member(member) = program
            .expression_table
            .expression(unwrapped(program, expression))
        else {
            return false;
        };
        if !self.is_subject(program, member.receiver) || member.case_variant.is_some() {
            return false;
        }
        let Some(data) = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == self.data)
        else {
            return false;
        };
        // Ordinary typed member expressions can still lack their field
        // symbol. Resolve that one hop under the exact retained parameter's
        // nominal owner, using the same unique-member judgment as proof SCCs.
        super::super::exact_data_member_field(
            program,
            data,
            member.member_symbol,
            member.member.as_str(),
            None,
        )
        .is_some_and(|selected| selected.symbol == field)
    }
}

pub(super) fn unwrapped(
    program: &TypedTrees,
    mut expression: ExpressionHandle,
) -> ExpressionHandle {
    while let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) {
        expression = atomic.value;
    }
    expression
}
