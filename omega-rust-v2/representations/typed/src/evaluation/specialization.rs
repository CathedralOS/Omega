use crate::ValueType;
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SyntaxKey {
    pub(super) owner: DeclarationId,
    nodes: Vec<(NodeKind, usize, Vec<String>, Option<Symbol>)>,
}
impl SyntaxKey {
    pub fn new(
        owner: DeclarationId,
        nodes: Vec<(NodeKind, usize, Vec<String>, Option<Symbol>)>,
    ) -> Self {
        Self { owner, nodes }
    }
}
/// Only fully substituted arguments may index an admitted evaluation result.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ClosedArguments(Vec<ValueType>);

impl ClosedArguments {
    pub fn into_values(self) -> Vec<ValueType> {
        self.0
    }
}
impl TryFrom<Vec<ValueType>> for ClosedArguments {
    type Error = Vec<ValueType>;

    fn try_from(arguments: Vec<ValueType>) -> Result<Self, Self::Error> {
        if arguments.iter().any(ValueType::contains_parameter) {
            Err(arguments)
        } else {
            Ok(Self(arguments))
        }
    }
}
