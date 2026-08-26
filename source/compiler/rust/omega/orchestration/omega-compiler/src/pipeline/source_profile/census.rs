use super::catalog::{SOURCE_FEATURE_CATALOG, SOURCE_FEATURE_IDS, SOURCE_RESOURCE_IDS};
use crate::pipeline::source_inspection::SourceClosureSnapshot;
use psi_syntax_trees::snapshot::{
    CapabilityContractKindSnapshot, CapabilityContractSnapshot, CapabilityMemberSnapshot,
    ConformanceBodySnapshot, ConformanceMemberSnapshot, DataMemberSnapshot,
    DataPropertiesSnapshot, ExpressionSnapshot, ExternalBindingSnapshot, FixedArrayLengthSnapshot,
    GenericConformanceBoundSnapshot, IdentifierSnapshot, ItemSnapshot, OperatorSnapshot,
    ProofFactSnapshot, PropositionBodySnapshot, SatisfiesClauseSnapshot, StateParameterSnapshot,
    StateSignatureSnapshot, StateSnapshot, StatementSnapshot, StaticArgumentSnapshot,
    StructLiteralFieldSnapshot, TargetHostSettingValueSnapshot, TransitionGuardSnapshot,
    TransitionTargetSnapshot, TypeConstraintSnapshot, TypeParameterSnapshot,
    TypeReferenceSnapshot, WireDataMemberSnapshot,
};
use serde::Serialize;
use std::collections::BTreeMap;

pub const SOURCE_FEATURE_CENSUS_SCHEMA: &str = "omega.omega-source-feature-census.v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceFeatureCount {
    pub id: &'static str,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceResourceObservation {
    pub id: &'static str,
    pub unit: &'static str,
    pub scope: &'static str,
    pub observed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceFeatureCensus {
    pub schema: &'static str,
    pub feature_catalog: &'static str,
    pub entry_source: String,
    pub selected_target: Option<String>,
    pub native_provider_substitution: bool,
    pub features: Vec<SourceFeatureCount>,
    pub resources: Vec<SourceResourceObservation>,
}

impl SourceFeatureCensus {
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

pub fn census_source_closure(snapshot: &SourceClosureSnapshot) -> SourceFeatureCensus {
    let mut census = Census::new();
    census.maximum("source.units", snapshot.sources.len());
    census.maximum(
        "source.bytes_total",
        snapshot
            .sources
            .iter()
            .map(|source| source.byte_length)
            .sum(),
    );
    census.maximum(
        "source.bytes_per_unit",
        snapshot
            .sources
            .iter()
            .map(|source| source.byte_length)
            .max()
            .unwrap_or(0),
    );
    census.maximum("syntax.root_items", snapshot.syntax.root_items.len());
    for item in &snapshot.syntax.root_items {
        census.item(item);
    }
    census.finish(snapshot)
}

struct Census {
    features: BTreeMap<&'static str, usize>,
    resources: BTreeMap<&'static str, usize>,
    expression_depth: usize,
}

impl Census {
    fn new() -> Self {
        Self {
            features: SOURCE_FEATURE_IDS.iter().map(|id| (*id, 0)).collect(),
            resources: SOURCE_RESOURCE_IDS
                .iter()
                .map(|(id, _, _)| (*id, 0))
                .collect(),
            expression_depth: 0,
        }
    }

    fn finish(self, snapshot: &SourceClosureSnapshot) -> SourceFeatureCensus {
        SourceFeatureCensus {
            schema: SOURCE_FEATURE_CENSUS_SCHEMA,
            feature_catalog: SOURCE_FEATURE_CATALOG,
            entry_source: snapshot.entry_source.clone(),
            selected_target: snapshot.selected_target.clone(),
            native_provider_substitution: snapshot.native_provider_substitution,
            features: SOURCE_FEATURE_IDS
                .iter()
                .map(|id| SourceFeatureCount {
                    id,
                    count: self.features[id],
                })
                .collect(),
            resources: SOURCE_RESOURCE_IDS
                .iter()
                .map(|(id, unit, scope)| SourceResourceObservation {
                    id,
                    unit,
                    scope,
                    observed: self.resources[id],
                })
                .collect(),
        }
    }

    fn bump(&mut self, id: &'static str) {
        *self
            .features
            .get_mut(id)
            .unwrap_or_else(|| panic!("uncatalogued source feature {id}")) += 1;
    }

    fn maximum(&mut self, id: &'static str, value: usize) {
        let current = self
            .resources
            .get_mut(id)
            .unwrap_or_else(|| panic!("uncatalogued source resource {id}"));
        *current = (*current).max(value);
    }

    fn identifier(&mut self, identifier: &IdentifierSnapshot) {
        let IdentifierSnapshot {
            text,
            source_id: _,
            start: _,
            end: _,
            source_backed: _,
        } = identifier;
        self.maximum("identifier.bytes", text.len());
    }

    fn path(&mut self, path: &[IdentifierSnapshot]) {
        self.maximum("path.components", path.len());
        for identifier in path {
            self.identifier(identifier);
        }
    }

    fn item(&mut self, item: &ItemSnapshot) {
        match item {
            ItemSnapshot::Capability { name, members } => {
                self.bump("item.capability");
                self.identifier(name);
                for member in members {
                    match member {
                        CapabilityMemberSnapshot::Field {
                            name,
                            type_reference,
                        } => {
                            self.identifier(name);
                            self.type_reference(type_reference);
                        }
                        CapabilityMemberSnapshot::State {
                            signature,
                            contracts,
                        } => {
                            self.state_signature(signature);
                            self.contracts(contracts);
                        }
                    }
                }
            }
            ItemSnapshot::Conformance {
                is_public,
                lifetime_parameters,
                type_parameters,
                type_name,
                subjectless: _,
                trait_name,
                trait_arguments,
                alias,
                body,
            } => {
                self.bump("item.conformance");
                if *is_public {
                    self.bump("item.public");
                }
                self.lifetime_parameters(lifetime_parameters);
                self.type_parameters(type_parameters);
                if let Some(type_name) = type_name {
                    self.identifier(type_name);
                }
                self.identifier(trait_name);
                for argument in trait_arguments {
                    self.type_reference(argument);
                }
                if let Some(alias) = alias {
                    self.identifier(alias);
                }
                match body {
                    ConformanceBodySnapshot::AttachedRequirementMachines => {}
                    ConformanceBodySnapshot::Closed { members } => {
                        for member in members {
                            match member {
                                ConformanceMemberSnapshot::Machine { declaration } => {
                                    self.item(declaration)
                                }
                                ConformanceMemberSnapshot::TraitDefault {
                                    declaring_trait,
                                    requirement_ordinal: _,
                                    declaration,
                                } => {
                                    self.identifier(declaring_trait);
                                    self.item(declaration);
                                }
                                ConformanceMemberSnapshot::Reference {
                                    declaring_trait,
                                    requirement,
                                    target,
                                } => {
                                    self.identifier(declaring_trait);
                                    self.identifier(requirement);
                                    self.path(target);
                                }
                            }
                        }
                    }
                }
            }
            ItemSnapshot::Const {
                scope,
                name,
                is_public,
                type_reference,
                value,
            } => {
                self.bump("item.const");
                if *is_public {
                    self.bump("item.public");
                }
                self.identifier(scope);
                self.identifier(name);
                self.type_reference(type_reference);
                self.expression(value);
            }
            ItemSnapshot::Data {
                name,
                is_public,
                supply,
                lifetime_parameters,
                type_parameters,
                generic_instance: _,
                properties,
                quotient,
                where_facts,
                members,
            } => {
                self.bump("item.data");
                if *is_public {
                    self.bump("item.public");
                }
                self.identifier(name);
                self.lifetime_parameters(lifetime_parameters);
                self.type_parameters(type_parameters);
                self.data_properties(properties);
                if *supply == "checked_shape" {
                    self.bump("data.supply.checked_shape");
                } else {
                    self.bump("data.supply.other");
                }
                if let Some(quotient) = quotient {
                    self.bump("data.quotient");
                    self.type_reference(&quotient.carrier);
                    self.path(&quotient.relation);
                    if let Some(equivalence) = &quotient.equivalence {
                        self.path(&equivalence.relation);
                        self.identifier(&equivalence.trait_name);
                        for argument in &equivalence.trait_arguments {
                            self.type_reference(argument);
                        }
                        self.identifier(&equivalence.conformance_name);
                    }
                }
                if !where_facts.is_empty() {
                    self.bump("data.where_fact");
                }
                self.proof_facts(where_facts);
                self.maximum("data.members", members.len());
                let mut fields = false;
                let mut variants = false;
                for member in members {
                    match member {
                        DataMemberSnapshot::Field {
                            identity,
                            name,
                            relevance,
                            type_reference,
                        } => {
                            fields = true;
                            self.bump("data.field");
                            if identity.is_some() {
                                self.bump("data.stable_identity");
                            }
                            self.relevance(relevance);
                            self.identifier(name);
                            self.type_reference(type_reference);
                        }
                        DataMemberSnapshot::Variant {
                            identity,
                            name,
                            payload,
                            retired_payload_identities,
                        } => {
                            variants = true;
                            self.bump("data.variant");
                            if identity.is_some() {
                                self.bump("data.stable_identity");
                            }
                            self.identifier(name);
                            self.maximum("data.variant_payload_fields", payload.len());
                            if !payload.is_empty() {
                                self.bump("data.variant_payload");
                            }
                            for field in payload {
                                if field.identity.is_some() {
                                    self.bump("data.stable_identity");
                                }
                                self.identifier(&field.name);
                                self.relevance(field.relevance);
                                self.type_reference(&field.type_reference);
                            }
                            for _ in retired_payload_identities {
                                self.bump("data.retired_identity");
                            }
                        }
                        DataMemberSnapshot::Retired { identity: _ } => {
                            self.bump("data.retired_identity");
                        }
                    }
                }
                if fields && variants {
                    self.bump("data.mixed_record_sum");
                }
            }
            ItemSnapshot::Domain {
                name,
                type_parameters,
                target_type,
                index_arguments,
                is_public,
                alias,
                authored_routes,
                classification: _,
                predicate_body: _,
                facts,
                operators,
                semantic_clause_token_count: _,
            } => {
                self.bump("item.domain");
                if *is_public {
                    self.bump("item.public");
                }
                self.identifier(name);
                self.type_parameters(type_parameters);
                self.type_reference(target_type);
                for argument in index_arguments {
                    self.type_reference(argument);
                }
                for path in alias {
                    self.path(path);
                }
                for path in authored_routes {
                    self.path(path);
                }
                self.proof_facts(facts);
                for operator in operators {
                    self.operator(operator);
                }
            }
            ItemSnapshot::Measure {
                name,
                parameter,
                return_type,
                lexicographic: _,
                body,
                token_count: _,
            } => {
                self.bump("item.measure");
                self.path(name);
                if let Some(parameter) = parameter {
                    self.state_parameter(parameter);
                }
                self.type_reference(return_type);
                for expression in body {
                    self.expression(expression);
                }
            }
            ItemSnapshot::Operator { operator } => {
                self.bump("item.operator");
                self.operator(operator);
            }
            ItemSnapshot::Module { path } => {
                self.bump("item.module");
                self.path(path);
            }
            ItemSnapshot::Package { path } => {
                self.bump("item.package");
                self.path(path);
            }
            ItemSnapshot::Proposition {
                name,
                is_public,
                type_parameters,
                parameters,
                body,
            } => {
                self.bump("item.proposition");
                if *is_public {
                    self.bump("item.proposition.public");
                }
                self.identifier(name);
                self.type_parameters(type_parameters);
                for parameter in parameters {
                    self.state_parameter(parameter);
                }
                match body {
                    PropositionBodySnapshot::Primitive => {}
                    PropositionBodySnapshot::Witness { evidence } => self.type_reference(evidence),
                    PropositionBodySnapshot::Transparent { proposition } => {
                        self.expression(proposition)
                    }
                }
            }
            ItemSnapshot::Use { path } => {
                self.bump("item.use");
                self.path(path);
            }
            ItemSnapshot::Machine {
                name,
                attached_data,
                is_public,
                bodyless,
                target,
                boundary,
                lifetime_parameters,
                type_parameters,
                satisfies,
                conformance_bounds,
                terminates_guarantee,
                ranking_subjects,
                ranking_view,
                ranking_view_arguments,
                ranking_range,
                service_reach_is_installation_bound,
                service_reaches,
                invokes,
                suspends,
                blocks,
                contracts,
                states,
            } => {
                self.bump("item.machine");
                if *is_public {
                    self.bump("item.public");
                }
                self.identifier(name);
                if *bodyless {
                    self.bump("machine.bodyless");
                }
                if let Some(target) = target {
                    self.bump("machine.target_qualified");
                    self.identifier(target);
                }
                if *boundary {
                    self.bump("machine.boundary");
                }
                if let Some(attached_data) = attached_data {
                    self.bump("machine.attached");
                    self.identifier(attached_data);
                } else {
                    self.bump("machine.free");
                }
                self.lifetime_parameters(lifetime_parameters);
                self.type_parameters(type_parameters);
                for clause in satisfies {
                    self.satisfies_clause(clause);
                }
                for bound in conformance_bounds {
                    self.conformance_bound("machine.conformance_bound", bound);
                }
                if *terminates_guarantee {
                    self.bump("machine.terminates");
                }
                if !ranking_subjects.is_empty() || !ranking_view.is_empty() {
                    self.bump("machine.ranking");
                }
                if !ranking_subjects.is_empty() {
                    self.bump("machine.ranking.subject");
                }
                for expression in ranking_subjects {
                    self.expression(expression);
                }
                self.path(ranking_view);
                if !ranking_view.is_empty() {
                    self.bump("machine.ranking.view");
                }
                if !ranking_view_arguments.is_empty() {
                    self.bump("machine.ranking.arguments");
                }
                for argument in ranking_view_arguments {
                    self.expression(argument);
                }
                if let Some(range) = ranking_range {
                    self.bump("machine.ranking.range");
                    self.expression(range);
                }
                if *service_reach_is_installation_bound {
                    self.bump("machine.service_reach.installation_bound");
                }
                for reach in service_reaches {
                    self.bump("machine.service_reach");
                    self.identifier(reach);
                }
                for invoke in invokes {
                    self.bump("machine.invokes");
                    self.identifier(invoke);
                }
                if *suspends {
                    self.bump("machine.suspends");
                }
                if *blocks {
                    self.bump("machine.blocks");
                }
                self.contracts(contracts);
                self.maximum("machine.states", states.len());
                for state in states {
                    self.state(state);
                }
            }
            ItemSnapshot::Platform { name, states } => {
                self.bump("item.platform");
                self.identifier(name);
                for state in states {
                    self.state_signature(state);
                }
            }
            ItemSnapshot::Trait {
                name,
                is_boundary,
                is_public,
                lifetime_parameters,
                type_parameters,
                conformance_bounds,
                parents,
                requires,
                machines,
            } => {
                self.bump("item.trait");
                if *is_public {
                    self.bump("item.public");
                }
                self.identifier(name);
                if *is_boundary {
                    self.bump("trait.boundary");
                }
                self.lifetime_parameters(lifetime_parameters);
                self.type_parameters(type_parameters);
                for bound in conformance_bounds {
                    self.conformance_bound("trait.conformance_bound", bound);
                }
                for parent in parents {
                    self.bump("trait.parent");
                    self.type_reference(parent);
                }
                for requirement in requires {
                    self.bump("trait.requires");
                    self.identifier(requirement);
                }
                for machine in machines {
                    if !machine.default_body.is_empty() {
                        self.bump("trait.default_body");
                    }
                    self.state_signature(machine);
                }
            }
            ItemSnapshot::Target {
                name,
                host,
                boundary_policies,
            } => {
                self.bump("item.target");
                self.identifier(name);
                if let Some(host) = host {
                    self.bump("target.host");
                    self.path(&host.provider);
                    for setting in &host.settings {
                        self.identifier(&setting.name);
                        match &setting.value {
                            TargetHostSettingValueSnapshot::Call {
                                name,
                                argument_tokens: _,
                            } => {
                                self.bump("target.host.setting.call");
                                self.identifier(name);
                            }
                            TargetHostSettingValueSnapshot::Named { name } => {
                                self.bump("target.host.setting.named");
                                self.identifier(name);
                            }
                        }
                    }
                }
                for policy in boundary_policies {
                    self.bump("target.boundary_policy");
                    self.bump(match policy.mode {
                        "checked" => "target.boundary_policy.checked",
                        "unchecked" => "target.boundary_policy.unchecked",
                        _ => "target.boundary_policy.other",
                    });
                    self.path(&policy.path);
                }
            }
            ItemSnapshot::WireData {
                name,
                is_public,
                encoding,
                members,
            } => {
                self.bump("item.wire_data");
                if *is_public {
                    self.bump("item.public");
                }
                self.identifier(name);
                if let Some(encoding) = encoding {
                    self.identifier(encoding);
                }
                self.wire_members(members);
            }
        }
    }

    fn wire_members(&mut self, members: &[WireDataMemberSnapshot]) {
        for member in members {
            match member {
                WireDataMemberSnapshot::Field {
                    number: _,
                    name,
                    relevance,
                    type_reference,
                } => {
                    self.bump("wire.field");
                    self.identifier(name);
                    self.relevance(relevance);
                    self.type_reference(type_reference);
                }
                WireDataMemberSnapshot::Reserved { number: _ } => self.bump("wire.reserved"),
                WireDataMemberSnapshot::Version { name, members } => {
                    self.bump("wire.version");
                    self.identifier(name);
                    self.wire_members(members);
                }
            }
        }
    }

    fn operator(&mut self, operator: &OperatorSnapshot) {
        let OperatorSnapshot {
            is_public,
            is_boundary,
            name,
            lifetime_parameters,
            type_parameters,
            parameters,
            return_type,
            contracts,
            spelling,
            token_count: _,
        } = operator;
        if *is_public {
            self.bump("item.public");
        }
        if *is_boundary {
            self.bump("operator.boundary");
        }
        if spelling.is_some() {
            self.bump("operator.spelling");
        }
        self.path(name);
        self.lifetime_parameters(lifetime_parameters);
        self.type_parameters(type_parameters);
        for parameter in parameters {
            self.state_parameter(parameter);
        }
        self.type_reference(return_type);
        self.contracts(contracts);
    }

    fn lifetime_parameters(&mut self, parameters: &[IdentifierSnapshot]) {
        for parameter in parameters {
            self.bump("generic.lifetime_parameter");
            self.identifier(parameter);
        }
    }

    fn type_parameters(&mut self, parameters: &[TypeParameterSnapshot]) {
        for parameter in parameters {
            let TypeParameterSnapshot {
                name,
                kind,
                const_type,
                machine_contract,
                machine_requirement,
                proposition_contract,
                bounds,
            } = parameter;
            self.identifier(name);
            match *kind {
                "type" => self.bump("generic.type_parameter"),
                "const" => self.bump("generic.const_parameter"),
                "machine" => self.bump("generic.machine_parameter"),
                "proposition" => self.bump("generic.proposition_parameter"),
                _ => self.bump("generic.parameter_kind.other"),
            }
            if let Some(const_type) = const_type {
                self.type_reference(const_type);
            }
            if let Some(machine_contract) = machine_contract {
                self.state_signature(machine_contract);
            }
            if let Some(machine_requirement) = machine_requirement {
                self.path(machine_requirement);
            }
            if let Some(contract) = proposition_contract {
                self.identifier(&contract.name);
                for parameter in &contract.parameters {
                    self.state_parameter(parameter);
                }
            }
            if bounds.multiplicity != "affine" || bounds.carry.is_some() {
                self.bump("generic.bounds");
            }
            self.data_properties(bounds);
        }
    }

    fn data_properties(&mut self, properties: &DataPropertiesSnapshot) {
        match properties.multiplicity {
            "affine" => self.bump("data.multiplicity.affine"),
            "unrestricted" => self.bump("data.multiplicity.unrestricted"),
            "linear" => self.bump("data.multiplicity.linear"),
            _ => self.bump("data.multiplicity.other"),
        }
        if properties.carry.is_some() {
            self.bump("data.carry_policy");
        }
    }

    fn state_signature(&mut self, signature: &StateSignatureSnapshot) {
        let StateSignatureSnapshot {
            name,
            spelling,
            lifetime_parameters,
            type_parameters,
            is_default,
            parameters,
            return_type,
            service_reach_is_installation_bound,
            service_reaches,
            invokes,
            suspends,
            blocks,
            contracts,
            default_body,
            terminates_guarantee,
        } = signature;
        if spelling.is_some() {
            self.bump("state.signature.spelling");
        }
        if *is_default {
            self.bump("state.signature.default");
        }
        self.identifier(name);
        self.lifetime_parameters(lifetime_parameters);
        self.type_parameters(type_parameters);
        self.maximum("state.parameters", parameters.len());
        for parameter in parameters {
            self.state_parameter(parameter);
        }
        self.type_reference(return_type);
        if *service_reach_is_installation_bound {
            self.bump("state.signature.installation_bound");
        }
        if *terminates_guarantee {
            self.bump("state.signature.terminates");
        }
        for reach in service_reaches {
            self.bump("machine.service_reach");
            self.identifier(reach);
        }
        for invoke in invokes {
            self.bump("machine.invokes");
            self.identifier(invoke);
        }
        if *suspends {
            self.bump("machine.suspends");
        }
        if *blocks {
            self.bump("machine.blocks");
        }
        self.contracts(contracts);
        if !default_body.is_empty() {
            self.bump("trait.default_body");
        }
        self.maximum("state.statements", default_body.len());
        for statement in default_body {
            self.statement(statement);
        }
    }

    fn state(&mut self, state: &StateSnapshot) {
        let StateSnapshot {
            name,
            parameters,
            return_type,
            contracts,
            statements,
        } = state;
        self.identifier(name);
        self.maximum("state.parameters", parameters.len());
        for parameter in parameters {
            self.state_parameter(parameter);
        }
        self.type_reference(return_type);
        self.contracts(contracts);
        self.maximum("state.statements", statements.len());
        for statement in statements {
            self.statement(statement);
        }
    }

    fn state_parameter(&mut self, parameter: &StateParameterSnapshot) {
        self.identifier(&parameter.name);
        self.type_reference(&parameter.type_reference);
        if parameter.is_const {
            self.bump("state.parameter.const");
        }
        if parameter.is_mutable {
            self.bump("state.parameter.mutable");
        }
        if parameter.is_self {
            self.bump("state.parameter.self");
        }
    }

    fn contracts(&mut self, contracts: &[CapabilityContractSnapshot]) {
        for contract in contracts {
            match &contract.kind {
                CapabilityContractKindSnapshot::Ensures => self.bump("contract.ensures"),
                CapabilityContractKindSnapshot::Requires => self.bump("contract.requires"),
                CapabilityContractKindSnapshot::Crashes { cause: _ } => {
                    self.bump("contract.crashes")
                }
            }
            if let Some(binding) = &contract.binding {
                self.bump("contract.evidence_binding");
                self.identifier(binding);
            }
            self.proof_facts(&contract.facts);
        }
    }

    fn proof_facts(&mut self, facts: &[ProofFactSnapshot]) {
        for fact in facts {
            match fact {
                ProofFactSnapshot::Expression { expression } => {
                    self.bump("proof.fact.expression");
                    self.expression(expression);
                }
                ProofFactSnapshot::Membership { value, domain } => {
                    self.bump("proof.fact.membership");
                    self.expression(value);
                    self.path(domain);
                }
            }
        }
    }

    fn relevance(&mut self, relevance: &str) {
        match relevance {
            "relevant" => {}
            "erased" => self.bump("binding.erased"),
            _ => self.bump("binding.relevance.other"),
        }
    }

    fn satisfies_clause(&mut self, clause: &SatisfiesClauseSnapshot) {
        self.bump("machine.satisfies");
        self.identifier(&clause.trait_name);
        if !clause.arguments.is_empty() {
            self.bump("machine.satisfies.arguments");
        }
        for argument in &clause.arguments {
            self.type_reference(argument);
        }
        if let Some(requirement) = &clause.requirement {
            self.bump("machine.satisfies.requirement");
            self.identifier(requirement);
        }
        if let Some(alias) = &clause.alias {
            self.bump("machine.satisfies.alias");
            self.identifier(alias);
        }
        if let Some(via) = &clause.via {
            match via {
                ExternalBindingSnapshot::Syscall { number: _ } => self.bump("machine.via.syscall"),
                ExternalBindingSnapshot::DllImport { module, symbol } => {
                    self.bump("machine.via.dll_import");
                    self.maximum("binding.string_bytes", module.len());
                    self.maximum("binding.string_bytes", symbol.len());
                }
                ExternalBindingSnapshot::CompilerIntrinsic => {
                    self.bump("machine.via.compiler_intrinsic")
                }
                ExternalBindingSnapshot::VtableSlot { index: _ } => {
                    self.bump("machine.via.vtable_slot")
                }
                ExternalBindingSnapshot::VtableField { field } => {
                    self.bump("machine.via.vtable_field");
                    self.identifier(field);
                }
                ExternalBindingSnapshot::TableFunction { field } => {
                    self.bump("machine.via.table_function");
                    self.identifier(field);
                }
            }
        }
    }

    fn conformance_bound(
        &mut self,
        feature: &'static str,
        bound: &GenericConformanceBoundSnapshot,
    ) {
        self.bump(feature);
        if let Some(binder) = &bound.binder {
            self.identifier(binder);
        }
        self.identifier(&bound.subject);
        self.identifier(&bound.carrier);
        for argument in &bound.arguments {
            self.type_reference(argument);
        }
        if let Some(conformance) = &bound.conformance {
            self.identifier(conformance);
        }
    }

    fn statement(&mut self, statement: &StatementSnapshot) {
        match statement {
            StatementSnapshot::AssemblyFact {
                contract_kind: _,
                expression,
            } => {
                self.bump("statement.assembly_fact");
                self.expression(expression);
            }
            StatementSnapshot::Assignment { target, value } => {
                self.bump("statement.assignment");
                self.expression(target);
                self.expression(value);
            }
            StatementSnapshot::Call {
                receiver,
                receiver_starts_at_self,
                target,
                machine_arguments,
                arguments,
                evidence_arguments,
                acknowledgement_synthesized,
                acknowledges_suspend,
                acknowledges_block,
                discards_result,
            } => {
                self.bump("statement.call");
                if receiver.is_empty() {
                    self.bump("call.receiver.free");
                } else {
                    self.bump("call.receiver.path");
                }
                if *receiver_starts_at_self {
                    self.bump("statement.call.receiver_starts_at_self");
                }
                if *discards_result {
                    self.bump("statement.call.discards_result");
                }
                self.path(receiver);
                self.identifier(target);
                self.static_arguments(machine_arguments);
                self.maximum("call.static_arguments", machine_arguments.len());
                self.maximum("call.arguments", arguments.len());
                for argument in arguments {
                    self.expression(argument);
                }
                for argument in evidence_arguments {
                    self.bump("call.evidence_argument");
                    self.identifier(argument);
                }
                if *acknowledges_suspend {
                    self.bump("call.acknowledgement.suspend");
                }
                if *acknowledges_block {
                    self.bump("call.acknowledgement.block");
                }
                if *acknowledgement_synthesized {
                    self.bump("call.acknowledgement.synthesized");
                }
            }
            StatementSnapshot::ProofOutputBindingStatement { bindings, call } => {
                self.bump("statement.proof_output_binding");
                for (public, local) in bindings {
                    self.identifier(public);
                    self.identifier(local);
                }
                self.expression(call);
            }
            StatementSnapshot::Expression { value } => {
                self.bump("statement.expression");
                self.expression(value);
            }
            StatementSnapshot::LocalData {
                name,
                type_reference,
                initial_value,
                is_mutable,
            } => {
                self.bump("statement.local_data");
                if *is_mutable {
                    self.bump("statement.local_data.mutable");
                }
                self.identifier(name);
                self.type_reference(type_reference);
                self.expression(initial_value);
            }
            StatementSnapshot::Transition {
                target,
                continuation,
                guard,
                crash_cause,
            } => {
                self.bump("statement.transition");
                self.transition_target(target);
                if let Some(continuation) = continuation {
                    self.bump("transition.continuation");
                    self.transition_target(continuation);
                }
                if crash_cause.is_some() {
                    self.bump("transition.crash");
                }
                match guard {
                    TransitionGuardSnapshot::Always => self.bump("transition.guard.always"),
                    TransitionGuardSnapshot::When { expression } => {
                        self.bump("transition.guard.when");
                        self.expression(expression);
                    }
                }
            }
        }
    }

    fn transition_target(&mut self, target: &TransitionTargetSnapshot) {
        match target {
            TransitionTargetSnapshot::Named {
                path,
                path_starts_at_self,
                arguments,
                evidence_arguments,
            } => {
                self.bump("transition.target.named");
                if *path_starts_at_self {
                    self.bump("transition.target.named.starts_at_self");
                }
                self.path(path);
                self.maximum("transition.arguments", arguments.len());
                for argument in arguments {
                    if matches!(
                        argument,
                        ExpressionSnapshot::ArrayLiteral { .. }
                            | ExpressionSnapshot::String { .. }
                            | ExpressionSnapshot::StructLiteral { .. }
                    ) {
                        self.bump("transition.aggregate_literal_argument");
                    }
                    self.expression(argument);
                }
                for argument in evidence_arguments {
                    self.bump("transition.evidence_argument");
                    self.identifier(argument);
                }
            }
            TransitionTargetSnapshot::Value { expression } => {
                self.bump("transition.target.value");
                self.expression(expression);
            }
            TransitionTargetSnapshot::SelfTarget => self.bump("transition.target.self"),
            TransitionTargetSnapshot::Terminal => self.bump("transition.target.terminal"),
        }
    }

    fn static_arguments(&mut self, arguments: &[StaticArgumentSnapshot]) {
        for argument in arguments {
            match argument {
                StaticArgumentSnapshot::Path(path) => {
                    self.bump("call.static_argument.path");
                    self.path(path);
                }
                StaticArgumentSnapshot::Application {
                    path,
                    lifetime_arguments,
                    arguments,
                } => {
                    self.bump("call.static_argument.application");
                    self.path(path);
                    self.lifetime_parameters(lifetime_arguments);
                    self.static_arguments(arguments);
                }
                StaticArgumentSnapshot::Const(_) => self.bump("call.static_argument.const"),
                StaticArgumentSnapshot::EvidenceProjection { term, member } => {
                    self.bump("call.static_argument.evidence_projection");
                    self.identifier(term);
                    self.identifier(member);
                }
            }
        }
    }

    fn type_reference(&mut self, type_reference: &TypeReferenceSnapshot) {
        match type_reference {
            TypeReferenceSnapshot::Reference {
                referee,
                access,
                lifetime,
            } => {
                match *access {
                    "shared" => self.bump("type.reference.shared"),
                    "mutable" => self.bump("type.reference.mutable"),
                    _ => self.bump("type.reference.other"),
                }
                if let Some(lifetime) = lifetime {
                    self.bump("type.reference.lifetime");
                    self.identifier(lifetime);
                }
                self.type_reference(referee);
            }
            TypeReferenceSnapshot::Constrained {
                base_type,
                constraints,
            } => {
                self.bump("type.constrained");
                self.type_reference(base_type);
                for constraint in constraints {
                    self.type_constraint(constraint);
                }
            }
            TypeReferenceSnapshot::FixedArray {
                element_type,
                length,
            } => {
                self.bump("type.fixed_array");
                self.type_reference(element_type);
                match length {
                    FixedArrayLengthSnapshot::Literal { value: _ } => {
                        self.bump("type.fixed_array_length.literal")
                    }
                    FixedArrayLengthSnapshot::ConstParameter { name } => {
                        self.bump("type.fixed_array_length.const_parameter");
                        self.identifier(name);
                    }
                    FixedArrayLengthSnapshot::ConstCall { name } => {
                        self.bump("type.fixed_array_length.const_call");
                        self.identifier(name);
                    }
                }
                if let FixedArrayLengthSnapshot::Literal { value } = length {
                    self.maximum("type.fixed_array_length", *value);
                }
            }
            TypeReferenceSnapshot::Slice { element_type } => {
                self.bump("type.slice");
                self.type_reference(element_type);
            }
            TypeReferenceSnapshot::Generic {
                base_name,
                lifetime_arguments,
                arguments,
            } => {
                self.bump("type.generic");
                self.identifier(base_name);
                self.lifetime_parameters(lifetime_arguments);
                for argument in arguments {
                    self.type_reference(argument);
                }
            }
            TypeReferenceSnapshot::ConstExpression { expression } => {
                self.bump("type.const_expression");
                self.expression(expression);
            }
            TypeReferenceSnapshot::DynamicTrait { name, conformance } => {
                self.bump("type.dynamic_trait");
                self.identifier(name);
                if let Some(conformance) = conformance {
                    self.identifier(conformance);
                }
            }
            TypeReferenceSnapshot::Named { name } => {
                self.bump("type.named");
                self.identifier(name);
            }
            TypeReferenceSnapshot::SelfType => self.bump("type.self"),
            TypeReferenceSnapshot::Unit => self.bump("type.unit"),
            TypeReferenceSnapshot::Missing => self.bump("type.missing"),
        }
    }

    fn type_constraint(&mut self, constraint: &TypeConstraintSnapshot) {
        match constraint {
            TypeConstraintSnapshot::Named { name } => {
                self.bump("type.constraint.named");
                self.identifier(name);
            }
            TypeConstraintSnapshot::Range { minimum, maximum } => {
                self.bump("type.constraint.range");
                self.expression(minimum);
                self.expression(maximum);
            }
            TypeConstraintSnapshot::ArithmeticDomain { domain } => {
                if domain == "Trapping" {
                    self.bump("type.constraint.arithmetic_domain.trapping");
                } else {
                    self.bump("type.constraint.arithmetic_domain.other");
                }
            }
            TypeConstraintSnapshot::Domain { name, arguments } => {
                self.bump("type.constraint.domain");
                self.identifier(name);
                for argument in arguments {
                    self.type_reference(argument);
                }
            }
        }
    }

    fn expression(&mut self, expression: &ExpressionSnapshot) {
        self.expression_at(expression, 1);
    }

    fn expression_child(&mut self, expression: &ExpressionSnapshot) {
        self.expression_at(expression, self.expression_depth + 1);
    }

    fn expression_at(&mut self, expression: &ExpressionSnapshot, depth: usize) {
        let previous_depth = self.expression_depth;
        self.expression_depth = depth;
        self.maximum("expression.nesting_depth", depth);
        match expression {
            ExpressionSnapshot::ArrayLiteral { values } => {
                self.bump("expression.array_literal");
                self.maximum("expression.array_elements", values.len());
                for value in values {
                    self.expression_child(value);
                }
            }
            ExpressionSnapshot::Atomic {
                value,
                result,
                ordering: _,
            } => {
                self.bump("expression.atomic");
                self.expression_child(value);
                if let Some(result) = result {
                    self.expression_child(result);
                }
            }
            ExpressionSnapshot::Binary {
                left,
                operator,
                right,
            } => {
                self.bump("expression.binary");
                self.bump(match *operator {
                    "add" => "expression.binary.add",
                    "and" => "expression.binary.and",
                    "bitwise_and" => "expression.binary.bitwise_and",
                    "bitwise_or" => "expression.binary.bitwise_or",
                    "bitwise_xor" => "expression.binary.bitwise_xor",
                    "divide" => "expression.binary.divide",
                    "equal" => "expression.binary.equal",
                    "greater" => "expression.binary.greater",
                    "greater_or_equal" => "expression.binary.greater_or_equal",
                    "less" => "expression.binary.less",
                    "less_or_equal" => "expression.binary.less_or_equal",
                    "modulo" => "expression.binary.modulo",
                    "multiply" => "expression.binary.multiply",
                    "not_equal" => "expression.binary.not_equal",
                    "or" => "expression.binary.or",
                    "shift_left" => "expression.binary.shift_left",
                    "shift_right" => "expression.binary.shift_right",
                    "subtract" => "expression.binary.subtract",
                    _ => "expression.binary.other",
                });
                self.expression_child(left);
                self.expression_child(right);
            }
            ExpressionSnapshot::Boolean { value: _ } => self.bump("expression.boolean"),
            ExpressionSnapshot::Cast {
                value,
                target_type,
                arithmetic_domain,
                form,
                semantic_domain,
                semantic_domain_arguments,
            } => {
                self.bump("expression.cast");
                self.bump(match *arithmetic_domain {
                    "Exact" => "expression.cast.domain.exact",
                    "Wrapping" => "expression.cast.domain.wrapping",
                    "Saturating" => "expression.cast.domain.saturating",
                    "Trapping" => "expression.cast.domain.trapping",
                    _ => "expression.cast.domain.other",
                });
                self.bump(match *form {
                    "value" => "expression.cast.form.value",
                    "recast_shared" => "expression.cast.form.recast_shared",
                    "recast_mutable" => "expression.cast.form.recast_mutable",
                    _ => "expression.cast.form.other",
                });
                self.expression_child(value);
                self.type_reference(target_type);
                if !semantic_domain.is_empty() {
                    self.bump("expression.cast.semantic_domain");
                }
                self.path(semantic_domain);
                if !semantic_domain_arguments.is_empty() {
                    self.bump("expression.cast.semantic_domain_argument");
                }
                for argument in semantic_domain_arguments {
                    self.type_reference(argument);
                }
            }
            ExpressionSnapshot::Call {
                receiver,
                target,
                machine_arguments,
                arguments,
                evidence_arguments,
                acknowledgement_synthesized,
                acknowledges_suspend,
                acknowledges_block,
            } => {
                self.bump("expression.call");
                if let Some(receiver) = receiver {
                    self.bump("call.receiver.expression");
                    self.expression_child(receiver);
                } else {
                    self.bump("call.receiver.free");
                }
                self.identifier(target);
                self.static_arguments(machine_arguments);
                self.maximum("call.static_arguments", machine_arguments.len());
                self.maximum("call.arguments", arguments.len());
                for argument in arguments {
                    self.expression_child(argument);
                }
                for argument in evidence_arguments {
                    self.bump("call.evidence_argument");
                    self.identifier(argument);
                }
                if *acknowledges_suspend {
                    self.bump("call.acknowledgement.suspend");
                }
                if *acknowledges_block {
                    self.bump("call.acknowledgement.block");
                }
                if *acknowledgement_synthesized {
                    self.bump("call.acknowledgement.synthesized");
                }
            }
            ExpressionSnapshot::Float { text: _ } => self.bump("expression.float"),
            ExpressionSnapshot::Indexed { collection, index } => {
                self.bump("expression.indexed");
                self.expression_child(collection);
                self.expression_child(index);
            }
            ExpressionSnapshot::Integer { text: _ } => self.bump("expression.integer"),
            ExpressionSnapshot::Membership { value, domain } => {
                self.bump("expression.membership");
                self.expression_child(value);
                self.path(domain);
            }
            ExpressionSnapshot::Member {
                receiver,
                member,
                case_variant,
            } => {
                self.bump("expression.member");
                if let Some(case_variant) = case_variant {
                    self.bump("expression.member.case_payload");
                    self.identifier(case_variant);
                }
                self.expression_child(receiver);
                self.identifier(member);
            }
            ExpressionSnapshot::Borrow { access: _, value } => {
                self.bump("expression.borrow");
                self.expression_child(value);
            }
            ExpressionSnapshot::Name { path } => {
                self.bump("expression.name");
                self.path(path);
            }
            ExpressionSnapshot::Range {
                start,
                end,
                end_inclusive,
            } => {
                self.bump("expression.range");
                if let Some(start) = start {
                    self.bump("expression.range.start");
                    self.expression_child(start);
                }
                if let Some(end) = end {
                    self.bump("expression.range.end");
                    self.expression_child(end);
                }
                if *end_inclusive {
                    self.bump("expression.range.inclusive");
                }
            }
            ExpressionSnapshot::SelfValue => self.bump("expression.self_value"),
            ExpressionSnapshot::StructLiteral {
                type_name,
                case_name,
                fields,
            } => {
                self.bump("expression.struct_literal");
                self.identifier(type_name);
                if let Some(case_name) = case_name {
                    self.bump("expression.struct_literal.case");
                    self.identifier(case_name);
                }
                self.maximum("expression.struct_fields", fields.len());
                for field in fields {
                    self.struct_literal_field(field);
                }
            }
            ExpressionSnapshot::String { bytes } => {
                self.bump("expression.string");
                self.maximum("expression.string_bytes", bytes.len());
            }
            ExpressionSnapshot::Unary { operator, operand } => {
                self.bump("expression.unary");
                self.bump(match *operator {
                    "~" => "expression.unary.bitwise_not",
                    "!" => "expression.unary.logical_not",
                    _ => "expression.unary.other",
                });
                self.expression_child(operand);
            }
            ExpressionSnapshot::ZeroValue { type_reference } => {
                self.bump("expression.zero_value");
                self.type_reference(type_reference);
            }
        }
        self.expression_depth = previous_depth;
    }

    fn struct_literal_field(&mut self, field: &StructLiteralFieldSnapshot) {
        self.identifier(&field.name);
        self.expression_child(&field.value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_and_resources_are_sorted_and_unique() {
        assert!(SOURCE_FEATURE_IDS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(!SOURCE_FEATURE_IDS.contains(&"contract.boundary"));
        assert!(!SOURCE_FEATURE_IDS.contains(&"item.library"));
        assert!(
            SOURCE_RESOURCE_IDS
                .windows(2)
                .all(|pair| pair[0].0 < pair[1].0)
        );
    }
}
