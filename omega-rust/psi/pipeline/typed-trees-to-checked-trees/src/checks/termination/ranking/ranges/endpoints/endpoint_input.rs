//! Exact scalar or member-chain inputs of an independently formed endpoint.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataField;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceNode;

pub(super) struct EndpointInput<'program> {
    pub argument_position: usize,
    parameter: &'program StateParameter,
    /// Member chain from the parameter to the endpoint leaf, paired with each
    /// field's declaring data symbol: `indirect.target.remaining` is
    /// `[(target, Indirect), (remaining, Wrap)]`.
    chain: Vec<(&'program DataField, SymbolHandle)>,
}

impl<'program> EndpointInput<'program> {
    /// The caller has already proved this whole expression's invariant bounds.
    /// Resolve its leaves again for exact arrival identity, not value inference.
    pub fn resolve(
        program: &'program TypedTrees,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<Self> {
        // `a.b.c` parses leaf-out: the expression is Member(c) whose receiver
        // is Member(b) over Name(a). Collect each Member, then walk the chain
        // root-to-leaf through declared field types.
        let mut members = Vec::new();
        let mut cursor = expression;
        loop {
            match program.expression_table.expression(cursor) {
                ExpressionNode::Member(member) => {
                    members.push(member);
                    cursor = member.receiver;
                }
                ExpressionNode::Name(_) => break,
                _ => return None,
            }
        }
        let ExpressionNode::Name(name) = program.expression_table.expression(cursor) else {
            return None;
        };
        let [spelling] = program.expression_table.name_path_members(name.members) else {
            return None;
        };
        let (argument_position, parameter) = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .enumerate()
            .find(|(_, parameter)| {
                name.symbol.is_valid()
                    && name.head_symbol == name.symbol
                    && parameter.symbol == name.symbol
                    && parameter.name == *spelling
                    && !parameter.is_mutable
                    && !parameter.is_const
            })?;
        let mut input = Self {
            argument_position,
            parameter,
            chain: Vec::new(),
        };
        let mut owner_type = parameter.type_reference;
        for member in members.iter().rev() {
            while let TypeReferenceNode::Constrained { base_type, .. } =
                program.type_reference_table.type_reference(owner_type)
            {
                owner_type = *base_type;
            }
            // Readable borrows expose store-enforced member bounds. Mutable
            // access is not itself a write: the endpoint owner separately
            // checks complete prefix frames against this exact path, including
            // its reference-binding prefixes, on every backedge. Write-only
            // access cannot contribute an observation of existing contents.
            while let TypeReferenceNode::Reference {
                referee, access, ..
            } = program.type_reference_table.type_reference(owner_type)
            {
                if !access.is_readable() {
                    return None;
                }
                owner_type = *referee;
                while let TypeReferenceNode::Constrained { base_type, .. } =
                    program.type_reference_table.type_reference(owner_type)
                {
                    owner_type = *base_type;
                }
            }
            let TypeReferenceNode::Named { symbol: owner, .. } =
                program.type_reference_table.type_reference(owner_type)
            else {
                return None;
            };
            if !member.member_symbol.is_valid() || member.case_variant.is_some() {
                return None;
            }
            let declaration = program
                .data_definitions()
                .iter()
                .find(|declaration| owner.is_valid() && declaration.symbol == *owner)?;
            let field = validation::exact_data_member_field(
                program,
                declaration,
                member.member_symbol,
                member.member.as_str(),
                None,
            )?;
            input.chain.push((field, *owner));
            owner_type = field.type_reference;
        }
        Some(input)
    }

    /// The declared bounds of a member endpoint whose chain traverses a readable
    /// borrow. The general invariant query stays out of references entirely;
    /// here the endpoint's own preservation judgment supplies the storage
    /// evidence, so the leaf's store-enforced field range bounds the read.
    pub(super) fn declared_member_bounds(
        program: &'program TypedTrees,
        state: &'program State,
        expression: ExpressionHandle,
    ) -> Option<(i64, i64)> {
        let input = Self::resolve(program, state, expression)?;
        let (leaf, _) = input.chain.last()?;
        validation::enforced_integer_type_bounds(program, leaf.type_reference)
    }

    pub fn path(&self) -> String {
        let mut path = self.parameter.name.as_str().to_owned();
        for (field, _) in &self.chain {
            path.push('.');
            path.push_str(field.name.as_str());
        }
        path
    }

    /// An immutable local names the storage its initializer spelled, so a
    /// forwarded local is judged against that initializer instead of the
    /// binding itself. `locals` pairs each immutable binding's symbol with its
    /// initializer in statement order; later shadows simply resolve further.
    pub fn preserved_by(
        &self,
        program: &TypedTrees,
        actual: ExpressionHandle,
        locals: &[(SymbolHandle, ExpressionHandle)],
    ) -> bool {
        let actual = resolve_local(program, actual, locals);
        if self.is_parameter(program, actual) {
            return true;
        }
        if self.chain.is_empty() {
            return false;
        }
        // A local can also spell the endpoint's own member path or a
        // containing prefix of it: forwarding `param.a.b` (or the enclosing
        // `param.a`) carries the endpoint storage into the next state.
        if self.is_prefix_path(program, actual, self.chain.len() - 1)
            || self.is_prefixed_by(program, actual, locals)
        {
            return true;
        }
        let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(actual)
        else {
            return false;
        };
        let (_, root_owner) = self.chain[0];
        if literal.type_symbol != root_owner
            || literal.case_symbol.is_some()
            || literal.case_name.is_some()
        {
            return false;
        }
        self.literal_preserves_at(program, literal, 0, locals)
    }

    /// The actual spells `param.chain[0..=prefix]` for some prefix of the
    /// endpoint's chain — the forwarded subtree still contains the leaf.
    fn is_prefixed_by(
        &self,
        program: &TypedTrees,
        actual: ExpressionHandle,
        locals: &[(SymbolHandle, ExpressionHandle)],
    ) -> bool {
        (1..self.chain.len()).any(|prefix| {
            self.is_prefix_path(program, resolve_local(program, actual, locals), prefix - 1)
        })
    }

    /// The actual's literal field at `depth` either forwards the same storage
    /// (a member path spelling exactly `param.chain[0..=depth]`) or rebuilds it
    /// as a nested literal that preserves the remaining chain the same way.
    fn literal_preserves_at(
        &self,
        program: &TypedTrees,
        literal: &TableStructLiteral,
        depth: usize,
        locals: &[(SymbolHandle, ExpressionHandle)],
    ) -> bool {
        let (field, _) = self.chain[depth];
        let mut matching = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .filter(|actual| actual.field_symbol == field.symbol && actual.name == field.name);
        let Some(actual) = matching.next() else {
            return false;
        };
        if matching.next().is_some() {
            return false;
        }
        let value = resolve_local(program, actual.value, locals);
        if self.is_prefix_path(program, value, depth) {
            return true;
        }
        if let Some((_, next_owner)) = self.chain.get(depth + 1) {
            if let ExpressionNode::StructLiteral(inner) = program.expression_table.expression(value)
            {
                if inner.type_symbol == *next_owner
                    && inner.case_symbol.is_none()
                    && inner.case_name.is_none()
                {
                    return self.literal_preserves_at(program, inner, depth + 1, locals);
                }
            }
        }
        false
    }

    /// `expression` spells the member path `param.chain[0..=depth]` exactly.
    fn is_prefix_path(
        &self,
        program: &TypedTrees,
        mut cursor: ExpressionHandle,
        depth: usize,
    ) -> bool {
        for index in (0..=depth).rev() {
            let ExpressionNode::Member(member) = program.expression_table.expression(cursor) else {
                return false;
            };
            let (field, _) = self.chain[index];
            if member.member_symbol != field.symbol
                || member.member != field.name
                || member.case_variant.is_some()
            {
                return false;
            }
            cursor = member.receiver;
        }
        self.is_parameter(program, cursor)
    }

    fn is_parameter(&self, program: &TypedTrees, expression: ExpressionHandle) -> bool {
        matches!(program.expression_table.expression(expression), ExpressionNode::Name(name)
            if name.symbol == self.parameter.symbol && name.head_symbol == name.symbol
                && matches!(program.expression_table.name_path_members(name.members),
                    [spelling] if *spelling == self.parameter.name))
    }
}

/// Immutable locals name exactly the storage their initializer spelled. Each
/// hop lands on a strictly earlier binding's initializer, so the walk cannot
/// cycle and needs no extra depth budget beyond the local count.
fn resolve_local(
    program: &TypedTrees,
    mut expression: ExpressionHandle,
    locals: &[(SymbolHandle, ExpressionHandle)],
) -> ExpressionHandle {
    for _ in 0..=locals.len() {
        let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
            break;
        };
        if !(name.symbol.is_valid() && name.head_symbol == name.symbol)
            || !matches!(
                program.expression_table.name_path_members(name.members),
                [_]
            )
        {
            break;
        }
        let Some((_, initializer)) = locals.iter().rfind(|(symbol, _)| *symbol == name.symbol)
        else {
            break;
        };
        expression = *initializer;
    }
    expression
}
