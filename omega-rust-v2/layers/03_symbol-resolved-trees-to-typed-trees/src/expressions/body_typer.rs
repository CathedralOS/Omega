//! One body owns its local types, write authority and current storage version. Checking an expression records its type and obligations in the stage product.

use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::expressions::place_facts;
use crate::expressions::storage_origin::{Origin, StoragePlace};
use crate::{ObligationKind, Site, TypedStage, TypingObligation, ValueType};
use omega_lexer::SourceSpan;
use std::collections::{HashMap, HashSet};

/// The enclosing construct decides how its expressions may be used. Crash guards and
/// ranking expressions check values without granting access to erased evidence.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionContext {
    Execution,
    Proof,
    ContractValue,
    StaticArgument,
}

pub(super) struct BodyTyper<'a, 'stage> {
    pub(super) context: ExpressionContext,
    pub(super) unit: &'a ResolvedSourceFile<'stage>,
    pub(super) catalog: &'a DeclarationCatalog<'stage>,
    pub(super) locals: HashMap<SourceSpan, ValueType>,
    pub(super) writable: HashSet<SourceSpan>,
    pub(super) output: &'a mut TypedStage,
    pub(super) subject: Option<ValueType>,
    pub(super) caller: Option<omega_resolver::DeclarationId>,
    /// Origins of the values bindings hold; absent means `Origin::Caller`.
    pub(super) binding_origins: HashMap<SourceSpan, Origin>,
    /// Bindings that name a field of borrowed storage in place.
    pub(super) borrowed_places: HashMap<SourceSpan, (usize, StoragePlace)>,
    /// Origins of typed expressions, by node index; absent means `Origin::Caller`.
    pub(super) origins: HashMap<usize, Origin>,
    /// Backing storage and access established when selecting each place.
    pub(super) places: HashMap<usize, StoragePlace>,
    pub(super) version: Option<Site>,
    pub(super) cases: HashMap<place_facts::Subject, omega_resolver::DeclarationId>,
}

impl<'a, 'stage> BodyTyper<'a, 'stage> {
    pub(super) fn new(
        unit: &'a ResolvedSourceFile<'stage>,
        catalog: &'a DeclarationCatalog<'stage>,
        output: &'a mut TypedStage,
    ) -> Self {
        Self {
            context: ExpressionContext::Execution,
            unit,
            catalog,
            output,
            locals: HashMap::new(),
            writable: HashSet::new(),
            binding_origins: HashMap::new(),
            borrowed_places: HashMap::new(),
            origins: HashMap::new(),
            places: HashMap::new(),
            cases: HashMap::new(),
            version: None,
            caller: None,
            subject: None,
        }
    }
}
impl BodyTyper<'_, '_> {
    pub(super) fn record_call_use(
        &mut self,
        node_index: usize,
    ) -> crate::CallUse {
        match self.context {
            ExpressionContext::Proof | ExpressionContext::ContractValue => crate::CallUse::Fact,
            ExpressionContext::StaticArgument => crate::CallUse::StaticArgument,
            ExpressionContext::Execution => {
                self.version = Some(Site::at(self.unit.file, node_index));
                self.cases.clear();
                crate::CallUse::Execution
            }
        }
    }
    pub(super) fn record_expression(
        &mut self,
        node: usize,
        ty: ValueType,
    ) {
        self.output
            .expressions
            .push(crate::TypedExpression::at(self.unit.file, node, ty));
    }

    pub(super) fn record_obligation(
        &mut self,
        node_index: usize,
        kind: ObligationKind,
    ) {
        self.output.obligations.push(TypingObligation {
            node: self.unit.file.node_id(node_index),
            site: Site::at(self.unit.file, node_index),
            kind,
        });
    }
}
