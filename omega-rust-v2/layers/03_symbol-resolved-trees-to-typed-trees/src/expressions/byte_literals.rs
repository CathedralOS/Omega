use crate::expressions::body_typer::{BodyTyper, ExpressionContext};
use crate::{TypeError, ValueType};
use omega_lexer::{TokenKind, decode_quoted_bytes};

impl BodyTyper<'_, '_> {
    /// Static expressions and machine results may land a quote into an
    /// owned byte array. Other quoted expressions keep their shared view type.
    pub(super) fn owned_byte_literal(
        &mut self,
        node: usize,
        expected: Option<&ValueType>,
        machine_result: bool,
    ) -> Result<Option<ValueType>, TypeError> {
        let source = self.unit.file;
        let token = source.nodes[node].leaves[0];
        if token.kind != TokenKind::Quoted
            || self.context != ExpressionContext::StaticArgument && !(matches!(expected, Some(ValueType::Array(..))) && machine_result)
        {
            return Ok(None);
        }
        let (element, extent) = match expected {
            Some(ValueType::Array(element, extent)) => (element, Some(*extent)),
            Some(ValueType::IndexedArray(element, _)) => (element, None),
            _ => return Ok(None),
        };
        if !matches!(element.as_ref(), ValueType::Integer { signed: false, width: Some(8) }) {
            return Ok(None);
        }
        let bytes = decode_quoted_bytes(token.text.as_bytes()).ok_or_else(|| TypeError::at(source, node, "invalid quoted byte literal"))?;
        let actual = bytes.len() as u64;
        if let Some(extent) = extent
            && extent != actual
        {
            return Err(TypeError::at(
                source,
                node,
                format!("byte literal has {actual} bytes, but its array requires {extent}"),
            ));
        }
        self.output
            .owned_byte_literals
            .insert(source.node_id(node), bytes.into());
        Ok(Some(ValueType::Array(element.clone(), actual)))
    }
}
