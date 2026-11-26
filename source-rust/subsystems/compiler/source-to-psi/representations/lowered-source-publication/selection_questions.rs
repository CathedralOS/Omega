//! The source questions stage 05 captured for one lowered form. They retain
//! exact operands and grant no permission.

use crate::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
use crate::declaration_selection::AuthoredDeclarationSelectionLateBinding;
use crate::source_selections::AuthoredDeclarationSelectionOccurrenceId;
use crate::source_state::TypedTrees;
use crate::source_state::callable_values::RuntimeCallBinding;
use crate::source_state::expression::ExpressionHandle;
use semantic_vocabulary::compiler_evaluation::DeclarationCustody;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RawSelectionRole {
    Execution,
    ProductDesignation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawSelectionTarget {
    Declaration(SymbolHandle),
    CoreIntegerOperator(numerics::intrinsics::CoreIntegerOperatorSignature),
    LocalBinding {
        symbol: SymbolHandle,
        owner: SymbolHandle,
    },
    Intrinsic(AuthoredDeclarationSelectionIntrinsic),
    RuntimeCall {
        node: crate::source_state::SourceNode,
        binding: RuntimeCallBinding,
    },
    ProductInput {
        binding: AuthoredDeclarationSelectionLateBinding,
        input: RawDesignationInput,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSelectionQuestion {
    pub lexical_owner: SymbolHandle,
    pub occurrence: AuthoredDeclarationSelectionOccurrenceId,
    pub role: RawSelectionRole,
    pub target: RawSelectionTarget,
    pub designation: Option<RawDesignationInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RawDesignationInput {
    pub expression: ExpressionHandle,
    pub request: arena::Handle<crate::source_state::expression::BuildSelectionCall>,
    pub argument_ordinal: u32,
    pub resolved_target: Option<SymbolHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RawDependencyKind {
    Declaration,
    Invocation,
    CodeSelection,
    Description,
    Layout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSelectionDependency {
    pub lexical_owner: SymbolHandle,
    pub target: SymbolHandle,
    pub role: RawSelectionRole,
    pub kind: RawDependencyKind,
}

#[derive(Debug, Clone, Default)]
pub struct RawSelectionQuestions {
    /// Anonymous source declarations in this form can own an application
    /// without supplying emitted code. Their identity belongs to the same
    /// invocation as the machine and record identities in the lowered form.
    invocation_declarations: Vec<(SymbolHandle, String)>,
    pub declarations: Vec<SymbolHandle>,
    pub questions: Vec<RawSelectionQuestion>,
    pub dependencies: Vec<RawSelectionDependency>,
}

impl RawSelectionQuestions {
    /// Start the questions of one form from the declarations it selects.
    pub fn for_declarations(declarations: Vec<SymbolHandle>) -> Self {
        Self {
            declarations,
            ..Self::default()
        }
    }

    pub fn invocation_declarations(&self) -> &[(SymbolHandle, String)] {
        &self.invocation_declarations
    }

    /// Record the invocation identity stage 05 gave an anonymous declaration
    /// that emits no row of its own.
    pub fn bind_invocation_declaration(&mut self, declaration: SymbolHandle, identity: String) {
        self.invocation_declarations.push((declaration, identity));
    }
}

/// Source custody is a question input, never a package permission or proof.
pub fn declaration_custody(
    typed: &TypedTrees,
    symbol: SymbolHandle,
) -> Result<DeclarationCustody, &'static str> {
    if !symbol.is_valid() || !typed.symbols.symbols().nodes().is_valid(symbol) {
        return Err("declaration custody has a missing or stale symbol");
    }
    Ok(typed.symbols.declaration_custody(symbol))
}
