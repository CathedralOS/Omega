//! PRV1 (design-ruled 2026-07-17): the typed **ProviderPlan** policy
//! carrier -- one value per (provider type, service schema, target), unifying the two
//! retirees: authored `provides` rows (the closed Binding sum) and the
//! hardcoded platform-lowering tables (Console/time's `PlatformCallData`
//! call shaping). CONSTRUCTION IS FREE: any code can build a plan; PRV2
//! validates coverage/signatures/identity, PRV3 admits semantic claims
//! through the chapter-10 grant/receipt carrier and selects by a
//! slot-owner capability, PRV4 relocates the built-in populate tables into
//! ordinary std target packages. Trust classification is ADMISSION OUTPUT,
//! never author-selected plan data -- which is why no trust field exists
//! on these types.

use crate::EffectSet;

/// The service schema a plan serves: a boundary trait's callable surface,
/// reified from the typed `TraitDefinition` (today that read is scattered
/// -- parameter-count walks in the compiler pipeline, Console detection in
/// the interpreter; the schema type is the one honest carrier).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceSchema {
    /// The boundary trait's name (`Console`, `FilesystemHost`).
    pub trait_name: String,
    /// One entry per trait machine, in declaration order.
    pub methods: Vec<ServiceMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceMethod {
    pub name: String,
    /// Declared parameter count (excluding any receiver) -- the same count
    /// the vtable-field encoder compares against call operands.
    pub parameter_count: usize,
    /// Whether the method declares a return type.
    pub has_result: bool,
    /// The method's declared effect names (`stdout_io`, `filesystem_io`).
    pub effects: Vec<String>,
    /// Canonical validated `BoundaryEntryPlan` identity selected by a concrete
    /// `Calling<C>` relationship. Policy type/source identity is excluded.
    pub calling_plan_fingerprint: Option<u64>,
}

/// How one method binds on one target -- the Binding sum's union with the
/// platform tables' mechanisms. Aligned with the host-ABI plan's
/// `HostBindingMechanism` so PRV4's relocation is a rename. Instructions are
/// checked `asm` bodies whose catalog contracts contribute their obligations;
/// they are deliberately not a second, bodiless provider-binding mechanism.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderBinding {
    /// Dynamic-library import (`DllImport { module, symbol }`).
    Import { library: String, symbol: String },
    /// Direct system call by number.
    Syscall { number: u32 },
    /// COM/UEFI slot dispatch: callee address read from the receiver.
    VtableSlot { index: i64 },
    /// Field-model vtable dispatch: the fn-ptr field of a named table
    /// struct; the byte offset resolves from the layout plan downstream.
    VtableField { table: String, field: String },
    /// UEFI service-table function (the boot-services shape).
    TableFunction { table: String, field: String },
    /// A portable compile-time constant (`provides` Value rows): no call,
    /// the name substitutes to the integer before resolution.
    Value { value: i64 },
    /// An ORDINARY CHECKED MACHINE realizing the requirement (the ruling's
    /// composite form: lowering sequences and argument adaptation are
    /// checked Omega code with an explicit satisfies edge, never authored
    /// rows). Admission checks the adapter as a REFINEMENT: its transitive
    /// effects must fit inside the satisfied requirement's declared
    /// ceiling.
    CheckedAdapter { machine: String },
}

/// One method's normalized provider binding. Composite argument adaptation is
/// checked Omega code, so plan rows carry only irreducible leaf mechanisms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanRow {
    pub method: String,
    pub binding: ProviderBinding,
}

/// The PRV1 carrier: one provider type's plan for one service schema on one
/// target. `origin_package` is provenance INPUT to admission (a package
/// can never self-grant); the admission verdict itself lives in the
/// chapter-10 receipts, never here.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderPlan {
    /// The plan's own name (`omega::host::standard::console`, the future
    /// slot-selection key).
    pub name: String,
    /// The nominal provider type whose explicit conformance closure produced
    /// this plan. Empty only for the legacy free-machine / `provides` bridge;
    /// slot overrides select this identity, never individual rows.
    pub provider_type: String,
    /// The target this plan serves (`windows_x64`; empty = every target,
    /// the portable-Value shape).
    pub target: String,
    /// The schema served.
    pub schema: ServiceSchema,
    /// One row per bound method.
    pub rows: Vec<ProviderPlanRow>,
    /// The plan's declared effect surface (union of its methods').
    pub effect_set: EffectSet,
    /// Where the plan came from -- admission provenance input.
    pub origin_package: String,
}

impl ServiceSchema {
    /// PRV2: reify a typed boundary trait's callable surface. `None` for a
    /// non-boundary trait (only boundary traits have service schemas).
    pub fn from_typed(
        program: &omega_typed_trees::TypedTrees,
        trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
    ) -> Option<Self> {
        Self::from_typed_instance(program, trait_definition, &[])
    }

    /// Reify one concrete generic boundary instance. The argument tuple is
    /// semantic input only for resolving evaluated calling-plan identity;
    /// policy type/source names remain absent from the published schema.
    pub fn from_typed_instance(
        program: &omega_typed_trees::TypedTrees,
        trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
        boundary_arguments: &[omega_typed_trees::types::TypeReferenceHandle],
    ) -> Option<Self> {
        if !trait_definition.is_boundary {
            return None;
        }
        let mut methods = Vec::new();
        let mut visited = Vec::new();
        collect_service_methods(
            program,
            trait_definition,
            trait_definition.symbol,
            boundary_arguments,
            &mut visited,
            &mut methods,
        );
        Some(Self {
            trait_name: trait_definition.name.as_str().to_owned(),
            methods,
        })
    }
}

fn collect_service_methods(
    program: &omega_typed_trees::TypedTrees,
    trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
    policy_owner: omega_core::symbols::SymbolHandle,
    boundary_arguments: &[omega_typed_trees::types::TypeReferenceHandle],
    visited: &mut Vec<omega_core::symbols::SymbolHandle>,
    methods: &mut Vec<ServiceMethod>,
) {
    if visited.contains(&trait_definition.symbol) {
        return;
    }
    visited.push(trait_definition.symbol);

    for requirement in program.trait_requirements(trait_definition) {
        let Some(parent) = program
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == requirement.symbol)
        else {
            continue;
        };
        collect_service_methods(
            program,
            parent,
            policy_owner,
            boundary_arguments,
            visited,
            methods,
        );
    }

    for signature in program.trait_machine_signatures(trait_definition) {
        if methods
            .iter()
            .any(|method| method.name == signature.name.as_str())
        {
            continue;
        }
        methods.push(ServiceMethod {
            name: signature.name.as_str().to_owned(),
            parameter_count: program
                .state_signature_parameters(signature)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count(),
            has_result: signature.return_type.is_valid(),
            effects: program
                .state_signature_effects(signature)
                .iter()
                .map(|effect| effect.as_str().to_owned())
                .collect(),
            calling_plan_fingerprint: program.boundary_calling_plan_fingerprint_for_arguments(
                policy_owner,
                boundary_arguments,
                signature.symbol,
            ),
        });
    }
}

impl ProviderPlan {
    /// PRV2: the plan's NORMALIZED IDENTITY -- an FNV-1a fingerprint over
    /// the canonical rendering (name, target, schema surface, rows in
    /// method order). Two plans with the same fingerprint are the same
    /// policy; presentation (row order, whitespace) is excluded.
    pub fn identity_fingerprint(&self) -> u64 {
        let mut rendered = format!(
            "{}\n{}\n{}\n{}",
            self.name, self.provider_type, self.target, self.schema.trait_name
        );
        let mut methods: Vec<&ServiceMethod> = self.schema.methods.iter().collect();
        methods.sort_by(|left, right| left.name.cmp(&right.name));
        for method in methods {
            rendered.push_str(&format!(
                "\nm:{}/{}/{}/{}",
                method.name,
                method.parameter_count,
                method.has_result,
                method.effects.join(",")
            ));
            if let Some(fingerprint) = method.calling_plan_fingerprint {
                rendered.push_str(&format!("/calling:{fingerprint:016x}"));
            }
        }
        let mut rows: Vec<&ProviderPlanRow> = self.rows.iter().collect();
        rows.sort_by(|left, right| left.method.cmp(&right.method));
        for row in rows {
            rendered.push_str(&format!("\nr:{}/{:?}", row.method, row.binding));
        }
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in rendered.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// PRV2: full structural validation against the schema -- every method
    /// bound exactly once, no stray rows, and per-method shape checks
    /// (a Value binding cannot serve a result-less method's call; a
    /// byte-read shape needs a result). Returns NAMED errors; empty =
    /// structurally valid.
    pub fn validate_against_schema(&self) -> Vec<String> {
        let mut errors = self.validate_candidate_against_schema();
        for method in &self.schema.methods {
            let count = self
                .rows
                .iter()
                .filter(|row| row.method == method.name)
                .count();
            if count == 0 {
                errors.push(format!(
                    "plan `{}` does not bind `{}::{}`",
                    self.name, self.schema.trait_name, method.name
                ));
            }
        }
        for row in &self.rows {
            if matches!(row.binding, ProviderBinding::Value { .. })
                && !self
                    .schema
                    .methods
                    .iter()
                    .any(|method| method.name == row.method)
            {
                errors.push(format!(
                    "plan `{}` binds `{}`, which is not a `{}` method",
                    self.name, row.method, self.schema.trait_name
                ));
            }
        }
        errors
    }

    /// Validate one candidate before coverage/selection. Partial candidates
    /// are legitimate, but a candidate cannot duplicate a requirement, name a
    /// callable row outside its schema, or use a Value row where call arguments
    /// exist. Unmatched compatibility Value rows remain inert until PRV4f
    /// removes that surface. This is the additive-only conformance check;
    /// selection decides whether the surviving candidate covers the complete
    /// slot.
    pub fn validate_candidate_against_schema(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for method in &self.schema.methods {
            let count = self
                .rows
                .iter()
                .filter(|row| row.method == method.name)
                .count();
            if count > 1 {
                errors.push(format!(
                    "plan `{}` binds `{}::{}` {count} times; one row per method",
                    self.name, self.schema.trait_name, method.name
                ));
            }
        }
        for row in &self.rows {
            let Some(method) = self
                .schema
                .methods
                .iter()
                .find(|method| method.name == row.method)
            else {
                // Compatibility `provides` plans still carry per-target Value
                // constants beside callable rows until PRV4f migrates the
                // remaining foreign-record facts. They do not participate in
                // candidate coverage and must not mask call-row defects.
                if matches!(row.binding, ProviderBinding::Value { .. }) {
                    continue;
                }
                errors.push(format!(
                    "plan `{}` binds `{}`, which is not a `{}` method",
                    self.name, row.method, self.schema.trait_name
                ));
                continue;
            };
            if matches!(row.binding, ProviderBinding::Value { .. }) && method.parameter_count > 0 {
                errors.push(format!(
                    "plan `{}` binds `{}::{}` to a portable Value, but the method \
                     takes {} argument(s) -- Value rows serve zero-argument \
                     constants",
                    self.name, self.schema.trait_name, method.name, method.parameter_count
                ));
            }
        }
        errors
    }

    /// PRV2 preview (the cheapest structural fact, used by tests today):
    /// every schema method has exactly one row and every row names a
    /// schema method.
    pub fn covers_schema(&self) -> bool {
        self.schema.methods.iter().all(|method| {
            self.rows
                .iter()
                .filter(|row| row.method == method.name)
                .count()
                == 1
        }) && self.rows.iter().all(|row| {
            self.schema
                .methods
                .iter()
                .any(|method| method.name == row.method)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The built-in Console lowering, spelled as a ProviderPlan VALUE --
    /// the PRV4 relocation target (windows.rs insert_platform_lowering's
    /// rows as data). Construction is free; nothing consumes this yet.
    fn windows_console_plan() -> ProviderPlan {
        let schema = ServiceSchema {
            trait_name: "Console".to_owned(),
            methods: vec![
                ServiceMethod {
                    name: "write_line".to_owned(),
                    parameter_count: 1,
                    has_result: false,
                    effects: vec!["stdout_io".to_owned()],
                    calling_plan_fingerprint: None,
                },
                ServiceMethod {
                    name: "read_byte".to_owned(),
                    parameter_count: 0,
                    has_result: true,
                    effects: vec!["stdin_io".to_owned()],
                    calling_plan_fingerprint: None,
                },
                ServiceMethod {
                    name: "exit_process".to_owned(),
                    parameter_count: 1,
                    has_result: false,
                    effects: Vec::new(),
                    calling_plan_fingerprint: None,
                },
            ],
        };
        ProviderPlan {
            name: "omega::host::standard::console".to_owned(),
            provider_type: "StandardConsole".to_owned(),
            target: "windows_x64".to_owned(),
            schema,
            rows: vec![
                ProviderPlanRow {
                    method: "write_line".to_owned(),
                    binding: ProviderBinding::Import {
                        library: "kernel32.dll".to_owned(),
                        symbol: "WriteFile".to_owned(),
                    },
                },
                ProviderPlanRow {
                    method: "read_byte".to_owned(),
                    binding: ProviderBinding::Import {
                        library: "kernel32.dll".to_owned(),
                        symbol: "ReadFile".to_owned(),
                    },
                },
                ProviderPlanRow {
                    method: "exit_process".to_owned(),
                    binding: ProviderBinding::Import {
                        library: "kernel32.dll".to_owned(),
                        symbol: "ExitProcess".to_owned(),
                    },
                },
            ],
            effect_set: EffectSet::empty(),
            origin_package: "omega::language::std".to_owned(),
        }
    }

    #[test]
    fn evaluated_calling_plan_is_published_provider_identity() {
        let mut first = windows_console_plan();
        let baseline = first.identity_fingerprint();
        first.schema.methods[0].calling_plan_fingerprint = Some(0x1234);
        assert_ne!(baseline, first.identity_fingerprint());

        let mut refactored = first.clone();
        refactored.schema.methods[0].calling_plan_fingerprint = Some(0x1234);
        assert_eq!(
            first.identity_fingerprint(),
            refactored.identity_fingerprint()
        );
    }

    #[test]
    fn console_plan_constructs_and_covers_its_schema() {
        let plan = windows_console_plan();
        assert!(plan.covers_schema());
    }

    #[test]
    fn validation_names_every_structural_defect() {
        // PRV2: missing binding, stray row, and the Value-with-arguments
        // shape check each produce a NAMED error.
        let mut plan = windows_console_plan();
        plan.rows.remove(0);
        plan.rows.push(ProviderPlanRow {
            method: "not_a_method".to_owned(),
            binding: ProviderBinding::Value { value: 1 },
        });
        plan.rows.push(ProviderPlanRow {
            method: "exit_process".to_owned(),
            binding: ProviderBinding::Value { value: 0 },
        });
        let errors = plan.validate_against_schema();
        assert!(
            errors
                .iter()
                .any(|error| error.contains("does not bind `Console::write_line`")),
            "missing binding named: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("not a `Console` method")),
            "stray row named: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("binds `Console::exit_process` 2 times")),
            "duplicate named: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("portable Value") && error.contains("exit_process")),
            "Value-with-arguments named: {errors:?}"
        );

        assert!(windows_console_plan().validate_against_schema().is_empty());
    }

    #[test]
    fn coverage_detects_missing_and_stray_rows() {
        let mut plan = windows_console_plan();
        plan.rows.pop();
        assert!(
            plan.validate_candidate_against_schema().is_empty(),
            "a partial candidate is structurally valid before slot selection"
        );
        assert!(
            !plan.covers_schema(),
            "a missing method row must fail coverage"
        );

        let mut plan = windows_console_plan();
        plan.rows.push(ProviderPlanRow {
            method: "not_in_schema".to_owned(),
            binding: ProviderBinding::VtableSlot { index: 0 },
        });
        assert!(
            !plan.validate_candidate_against_schema().is_empty(),
            "a stray row is invalid even before coverage selection"
        );
        assert!(!plan.covers_schema(), "a stray row must fail coverage");
    }
}
