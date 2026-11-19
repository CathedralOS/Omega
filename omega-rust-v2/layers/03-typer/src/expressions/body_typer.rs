//! One body owns its local types, write authority and current storage version. Checking an expression records its type and obligations in the stage product.

use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::expressions::place_facts;
use crate::{ObligationKind, Site, TypedStage, TypingObligation, ValueType};
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use std::collections::{HashMap, HashSet};

pub(super) struct BodyTyper<'a, 'stage> {
    pub(super) static_argument: bool,
    pub(super) logical_argument: bool,
    pub(super) unit: &'a ResolvedSourceFile<'stage>,
    pub(super) catalog: &'a DeclarationCatalog<'stage>,
    pub(super) locals: HashMap<SourceSpan, ValueType>,
    pub(super) writable: HashSet<SourceSpan>,
    pub(super) output: &'a mut TypedStage,
    pub(super) subject: Option<ValueType>,
    pub(super) caller: Option<omega_resolver::DeclarationId>,
    pub(super) local_views: HashSet<SourceSpan>,
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
            static_argument: false,
            logical_argument: false,
            unit,
            catalog,
            output,
            locals: HashMap::new(),
            writable: HashSet::new(),
            local_views: HashSet::new(),
            cases: HashMap::new(),
            version: None,
            caller: None,
            subject: None,
        }
    }
}
impl BodyTyper<'_, '_> {
    pub(super) fn call_use(
        &self,
        node_index: usize,
    ) -> crate::CallUse {
        if self.logical_argument {
            crate::CallUse::Fact
        } else if self.static_argument {
            crate::CallUse::StaticArgument
        } else if self.unit.file.enclosing_contexts(node_index).any(|p| {
            matches!(
                self.unit.file.nodes[p].kind,
                NodeKind::Crashes | NodeKind::Requires | NodeKind::Ensures | NodeKind::Hypothesis | NodeKind::MathLet | NodeKind::Terminates
            )
        }) {
            crate::CallUse::Fact
        } else {
            crate::CallUse::Execution
        }
    }
    pub(super) fn record_obligation(
        &mut self,
        node_index: usize,
        kind: ObligationKind,
    ) {
        self.output.obligations.push(TypingObligation {
            site: Site::at(&self.unit.file, node_index),
            kind,
        });
    }
}
