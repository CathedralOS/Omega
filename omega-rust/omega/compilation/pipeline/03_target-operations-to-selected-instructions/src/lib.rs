#![forbid(unsafe_code)]

/*
We legalize target operations and select machine instructions (`Omega 03`). Our
entry point, `stage_optimized_instruction_selection` in `optimized.rs`, validates
the target register environment, runs `legalize_target_operations` (`legalization.rs`)
followed by `select_instructions` (`selection.rs`), seals both plans with their
validation receipts, and records their identities in one
`StagedOptimizedSelectionCustodyReceipt`.
*/

/*
Target legalization and instruction selection bridge portable/target operation
plans and physical register assignment without re-running source borrow checking
or losing structural storage geometry. We separate the raw-target to
legal-operation join (`legalization*.rs`), the legal-operation to selected-CFG
join (`selection*.rs`), and the shared structural reference/unit geometry
(`structural_inputs.rs`) so ordinary compile runs construct instructions directly
from retained target rows while the independent `verify_target_operation_plan`
audit remains available for corpus and foreign-artifact review.
*/

/*
- Input: `TargetSelectionInput` (`optimized.rs`) carrying the `TargetOperationPlan`, `AbstractOperationPlan`, `PsiOptimizationUnit`, and admitted boundary/interrupt facts from `Omega 02`.
- Output: `StagedOptimizedSelectedInstructions` and `ValidatedSelectedInstructions` consumed by `Omega 04` (`selected-instructions-to-selected-instructions`).
- Flat module layout:
  - `optimized.rs`: stage entry, target-register-environment custody, and `StagedOptimizedSelectedInstructions`.
  - `structural_inputs.rs`: structural reference, collection-view, and unit input geometry.
  - `legalization.rs`, `legalization_scalar_graph.rs`, `legalization_reference_custody.rs`, `legalization_target.rs`, `legalization_source.rs`: target-operation legalization and independent target-plan auditing.
  - `selection.rs`, `selection_inputs.rs`, `selection_edge_transfers.rs`, `selection_call_abi.rs`: instruction-selection driver, input projections, block-edge transfers, and scalar/foreign call ABI placement.
  - `selection_scalar_graph.rs`, `selection_structural.rs`, `selection_views_and_bytes.rs`, `selection_access_lifecycles.rs`, `selection_interrupts.rs`: selected-CFG instruction emission across scalar, structural, byte/element-view, access-lifecycle, and interrupt operations.
*/

macro_rules! failed_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }
    };
}

pub(crate) mod installed_interrupt_geometry {
    use crate::structural_inputs::structural_reference_input as geometry;
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralTypeId};
    use terminal_psi::{
        StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
    };
    fn failure(reason: &'static str) -> &'static str {
        reason
    }
    pub struct InstalledInterruptOperandLayout {
        pub table_bytes: u32,
        pub gdt_bytes: u32,
        pub stack_stride: u32,
        pub stack_words: u32,
        pub stack_ist: u32,
        pub arrival_stride: u32,
        pub arrival_vector: u32,
        pub arrival_frame: u32,
        pub frame_bytes: u32,
        pub no_error_tag: u64,
        pub error_tag: u64,
        pub opaque_fatal_tag: u64,
    }
    fn scalar(bits: u8) -> Result<ScalarType, &'static str> {
        Ok(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, bits.into())
                .map_err(|_| failure("invalid compiler scalar width"))?,
        ))
    }

    fn fields(
        declarations: &[StructuralTypeDeclaration],
        ty: StructuralTypeId,
    ) -> Result<&[terminal_psi::StructuralFieldDeclaration], &'static str> {
        let Some(StructuralTypeShape::Record { fields, .. }) =
            terminal_psi::find_structural_type(declarations, ty).map(|row| &row.shape)
        else {
            return Err(failure("sealed installation row is not a record"));
        };
        if fields.len() != 2 || fields.iter().any(|field| field.relevance.is_erased()) {
            return Err(failure(
                "sealed installation row lost its two runtime fields",
            ));
        }
        Ok(fields)
    }

    fn element(
        declarations: &[StructuralTypeDeclaration],
        ty: StructuralTypeId,
    ) -> Result<StructuralTypeId, &'static str> {
        match terminal_psi::find_structural_type(declarations, ty).map(|row| &row.shape) {
            Some(StructuralTypeShape::ElementView { element, .. }) => Ok(*element),
            _ => Err(failure(
                "installation collection lost its element descriptor",
            )),
        }
    }

    impl InstalledInterruptOperandLayout {
        pub fn derive(
            declarations: &[StructuralTypeDeclaration],
            types: [StructuralTypeId; 6],
        ) -> Result<Self, &'static str> {
            let table = geometry::shape(types[0], declarations)
                .ok_or_else(|| failure("table has no selected storage layout"))?;
            let gdt = geometry::shape(types[1], declarations)
                .ok_or_else(|| failure("GDT has no selected storage layout"))?;
            let task = geometry::shape(types[2], declarations)
                .ok_or_else(|| failure("TSS has no selected storage layout"))?;
            if table.byte_size == 0
                || table.byte_size > 4096
                || table.byte_size % 16 != 0
                || gdt.byte_size < 16
                || gdt.byte_size > 65536
                || gdt.byte_size % 8 != 0
                || task.byte_size != 104
            {
                return Err(failure(
                    "installation table, GDT or TSS extent is outside x86 geometry",
                ));
            }
            let stack = element(declarations, types[3])?;
            let stack_fields = fields(declarations, stack)?;
            let StructuralFieldType::Structural(words) = stack_fields[0].field_type else {
                return Err(failure("stack lease has no typed word view"));
            };
            let StructuralTypeShape::ElementView {
                element: word,
                access: Some(terminal_psi::StructuralAccess::MutableBorrow),
            } = terminal_psi::find_structural_type(declarations, words)
                .ok_or_else(|| failure("stack word view declaration is missing"))?
                .shape
            else {
                return Err(failure("stack lease must retain exclusive word backing"));
            };
            if terminal_psi::find_structural_type(declarations, word).map(|row| &row.shape)
                != Some(&StructuralTypeShape::PrimitiveScalar(scalar(64)?))
            {
                return Err(failure("stack lease element is not a 64-bit word"));
            }
            let (_, stack_words) = geometry::project(
                stack,
                &[StructuralPathSegment::Field(stack_fields[0].identity)],
                declarations,
            )
            .ok_or_else(|| failure("stack word descriptor has no target offset"))?;
            let (stack_ist, _, _) = geometry::primitive_geometry(
                stack,
                &[StructuralPathSegment::Field(stack_fields[1].identity)],
                scalar(8)?,
                declarations,
            )
            .ok_or_else(|| failure("stack IST field has no target byte layout"))?;
            let arrival = element(declarations, types[4])?;
            let arrival_fields = fields(declarations, arrival)?;
            let (arrival_vector, _, _) = geometry::primitive_geometry(
                arrival,
                &[StructuralPathSegment::Field(arrival_fields[0].identity)],
                scalar(16)?,
                declarations,
            )
            .ok_or_else(|| failure("arrival vector has no target word layout"))?;
            let (frame, arrival_frame) = geometry::project(
                arrival,
                &[StructuralPathSegment::Field(arrival_fields[1].identity)],
                declarations,
            )
            .ok_or_else(|| failure("arrival frame has no target offset"))?;
            let Some(StructuralTypeShape::Sum { cases }) =
                terminal_psi::find_structural_type(declarations, frame).map(|row| &row.shape)
            else {
                return Err(failure("arrival frame is not the sealed sum"));
            };
            if cases.len() != 3 || cases.iter().any(|case| !case.fields.is_empty()) {
                return Err(failure("arrival frame lost its three empty cases"));
            }
            let frame_layout = geometry::case_layout(frame, declarations)
                .ok_or_else(|| failure("arrival frame has no selected tag layout"))?;
            if frame_layout.tag_byte_offset != 0
                || u64::from(frame_layout.tag_shape.byte_size) != runtime_abi::CASE_TAG_BYTE_SIZE
                || frame_layout.cases.len() != 3
                || frame_layout
                    .cases
                    .iter()
                    .any(|case| !case.fields.is_empty())
            {
                return Err(failure(
                    "installed arrival frame lost its conventional case tag",
                ));
            }
            Ok(Self {
                table_bytes: table.byte_size,
                gdt_bytes: gdt.byte_size,
                stack_stride: geometry::shape(stack, declarations)
                    .and_then(|shape| geometry::align(shape.byte_size, shape.alignment))
                    .ok_or_else(|| failure("stack row stride is unavailable"))?,
                stack_words,
                stack_ist,
                arrival_stride: geometry::shape(arrival, declarations)
                    .and_then(|shape| geometry::align(shape.byte_size, shape.alignment))
                    .ok_or_else(|| failure("arrival row stride is unavailable"))?,
                arrival_vector,
                arrival_frame: arrival_frame
                    .checked_add(frame_layout.tag_byte_offset)
                    .ok_or_else(|| failure("arrival frame offset overflows"))?,
                frame_bytes: frame_layout.tag_shape.byte_size,
                no_error_tag: 0,
                error_tag: 1,
                opaque_fatal_tag: 2,
            })
        }
    }
}
pub(crate) mod installed_interrupts {
    use language_core::static_interrupts::StaticInterruptOperation;
    use provider_admission::{
        AdmittedInstalledInterruptCohort, AdmittedInstalledInterruptEntry,
        AdmittedInstalledInterruptHelper, AdmittedInstalledInterruptScope,
    };
    use semantic_vocabulary::OperationId;
    use terminal_psi::{OperationKind, TerminalModule, TerminalPsiIdentity};

    struct Rows<T>(Vec<(OperationId, Vec<T>)>);

    type HelperAnswer = (StaticInterruptOperation, Result<(), &'static str>);
    impl<T> Rows<T> {
        fn new(rows: impl Iterator<Item = (OperationId, T)>) -> Self {
            let mut rows: Vec<_> = rows.collect();
            rows.sort_unstable_by_key(|(operation, _)| *operation);
            let mut groups: Vec<(OperationId, Vec<T>)> = Vec::new();
            for (operation, row) in rows {
                if let Some((_, values)) = groups.last_mut().filter(|(last, _)| *last == operation)
                {
                    values.push(row);
                } else {
                    groups.push((operation, vec![row]));
                }
            }
            Self(groups)
        }
        fn get(&self, operation: OperationId) -> &[T] {
            self.0
                .binary_search_by_key(&operation, |(id, _)| *id)
                .map_or(&[], |index| self.0[index].1.as_slice())
        }
    }

    struct Associations<T>(Vec<((OperationId, u32), T)>);
    impl<T> Associations<T> {
        fn new(rows: impl Iterator<Item = ((OperationId, u32), T)>) -> Self {
            let mut rows: Vec<_> = rows.collect();
            rows.sort_unstable_by_key(|(key, _)| *key);
            Self(rows)
        }
        fn unique(&self, installation: OperationId, application: u32) -> Option<&T> {
            let key = (installation, application);
            let index = self.0.partition_point(|(candidate, _)| *candidate < key);
            let (actual, value) = self.0.get(index)?;
            if *actual != key || self.0.get(index + 1).is_some_and(|(next, _)| *next == key) {
                return None;
            }
            Some(value)
        }
    }

    pub(crate) struct PreparedInstallationSources<'a> {
        psi: TerminalPsiIdentity,
        scopes: Rows<&'a AdmittedInstalledInterruptScope>,
        operations: Rows<&'a abstract_operations::AbstractOperation>,
    }
    impl<'a> PreparedInstallationSources<'a> {
        fn new(
            cohort: &'a AdmittedInstalledInterruptCohort,
            source: &'a abstract_operations::AbstractOperationPlan,
        ) -> Self {
            Self {
                psi: cohort.psi(),
                scopes: Rows::new(
                    cohort
                        .scopes()
                        .iter()
                        .map(|scope| (scope.operation(), scope)),
                ),
                operations: Rows::new(source.operations().filter_map(|row| {
                    if let abstract_operations::AbstractOperation::BoundaryCall {
                        psi_operation,
                        ..
                    } = row
                    {
                        Some((*psi_operation, row))
                    } else {
                        None
                    }
                })),
            }
        }
        pub(crate) fn psi(&self) -> TerminalPsiIdentity {
            self.psi
        }
        pub(crate) fn scope(
            &self,
            operation: OperationId,
        ) -> Option<&'a AdmittedInstalledInterruptScope> {
            let [scope] = self.scopes.get(operation) else {
                return None;
            };
            Some(scope)
        }
        pub(crate) fn boundary_calls(
            &self,
            operation: OperationId,
        ) -> &[&'a abstract_operations::AbstractOperation] {
            self.operations.get(operation)
        }
    }

    pub(crate) struct PreparedHelperCohort<'a> {
        helpers: Rows<&'a AdmittedInstalledInterruptHelper>,
        scopes: Rows<&'a AdmittedInstalledInterruptScope>,
        entries: Associations<&'a AdmittedInstalledInterruptEntry>,
        requirements: Vec<(OperationId, HelperAnswer)>,
    }
    impl<'a> PreparedHelperCohort<'a> {
        pub(crate) fn new(cohort: &'a AdmittedInstalledInterruptCohort) -> Self {
            let mut result = Self {
                helpers: Rows::new(cohort.helpers().iter().map(|row| (row.operation(), row))),
                scopes: Rows::new(cohort.scopes().iter().map(|row| (row.operation(), row))),
                entries: Associations::new(
                    cohort
                        .entries()
                        .iter()
                        .map(|row| ((row.operation, row.placement.application_index), row)),
                ),
                requirements: Vec::new(),
            };
            result.requirements = result
                .helpers
                .0
                .iter()
                .filter_map(|(operation, helpers)| {
                    let first = helpers.first()?;
                    Some((
                        *operation,
                        (first.kind(), result.validate(*operation, first.kind())),
                    ))
                })
                .collect();
            result
        }
        pub(crate) fn installation_scope(
            &self,
            operation: OperationId,
        ) -> Result<&'a AdmittedInstalledInterruptScope, &'static str> {
            let [scope] = self.scopes.get(operation) else {
                return Err("installation does not have exactly one admitted scope");
            };
            Ok(scope)
        }
        pub(crate) fn installation_entries(
            &self,
            operation: OperationId,
        ) -> impl Iterator<Item = &'a AdmittedInstalledInterruptEntry> + '_ {
            let first = self
                .entries
                .0
                .partition_point(|((owner, _), _)| *owner < operation);
            let last = self
                .entries
                .0
                .partition_point(|((owner, _), _)| *owner <= operation);
            self.entries.0[first..last].iter().map(|(_, entry)| *entry)
        }
        fn scope(
            &self,
            helper: &AdmittedInstalledInterruptHelper,
        ) -> Result<&'a AdmittedInstalledInterruptScope, &'static str> {
            let [scope] = self.scopes.get(helper.installation()) else {
                return Err("interrupt helper does not have exactly one admitted Scope");
            };
            if scope.acknowledgement_application() != helper.callback_application() {
                return Err("interrupt helper token belongs to another acknowledgement entry");
            }
            Ok(scope)
        }
        fn entry(
            &self,
            helper: &AdmittedInstalledInterruptHelper,
        ) -> Result<&'a AdmittedInstalledInterruptEntry, &'static str> {
            self.entries
                .unique(helper.installation(), helper.callback_application())
                .copied()
                .ok_or("interrupt helper does not have exactly one admitted native entry")
        }
        pub(crate) fn require(
            &self,
            operation: OperationId,
            kind: StaticInterruptOperation,
        ) -> Result<(), &'static str> {
            require_answer(&self.requirements, operation, kind)
        }
        fn validate(
            &self,
            operation: OperationId,
            kind: StaticInterruptOperation,
        ) -> Result<(), &'static str> {
            if kind == StaticInterruptOperation::Install {
                return Err("an installation operation is not an arrival token helper");
            }
            let helpers = self.helpers.get(operation);
            if helpers.is_empty() {
                return Err("interrupt helper has no admitted installation association");
            }
            let mut roots: Vec<_> = helpers
                .iter()
                .map(|helper| (helper.installation(), helper.callback_application()))
                .collect();
            roots.sort_unstable();
            if roots.windows(2).any(|pair| pair[0] == pair[1])
                || helpers.iter().any(|helper| helper.kind() != kind)
            {
                return Err("interrupt helper association has a changed kind or duplicate root");
            }
            for helper in helpers {
                self.scope(helper)?;
                self.entry(helper)?;
            }
            Ok(())
        }
    }

    fn require_answer(
        answers: &[(OperationId, HelperAnswer)],
        operation: OperationId,
        kind: StaticInterruptOperation,
    ) -> Result<(), &'static str> {
        if kind == StaticInterruptOperation::Install {
            return Err("an installation operation is not an arrival token helper");
        }
        let index = answers
            .binary_search_by_key(&operation, |(id, _)| *id)
            .map_err(|_| "interrupt helper has no admitted installation association")?;
        let (expected, answer) = &answers[index].1;
        if kind != *expected {
            return Err("interrupt helper association has a changed kind or duplicate root");
        }
        *answer
    }

    pub struct PreparedInstalledInterruptHelpers {
        psi: TerminalPsiIdentity,
        cohort_psi: TerminalPsiIdentity,
        answers: Vec<(OperationId, HelperAnswer)>,
    }
    impl PreparedInstalledInterruptHelpers {
        pub fn new(
            psi: TerminalPsiIdentity,
            module: &TerminalModule,
            cohort: &AdmittedInstalledInterruptCohort,
        ) -> Self {
            if cohort.helpers().is_empty() {
                return Self {
                    psi,
                    cohort_psi: cohort.psi(),
                    answers: Vec::new(),
                };
            }
            let prepared = PreparedHelperCohort::new(cohort);
            let operations = Rows::new(module.operations().map(|row| (row.id, row)));
            let registrations = Rows::new(
                module
                    .installed_interrupt_registrations
                    .iter()
                    .map(|row| (row.operation, row)),
            );
            let callbacks =
                Associations::new(module.installed_interrupt_registrations.iter().flat_map(
                    |row| {
                        row.callbacks.iter().map(move |callback| {
                            ((row.operation, callback.application_index), callback)
                        })
                    },
                ));
            let mut answers = Vec::with_capacity(prepared.helpers.0.len());
            for (operation, helpers) in &prepared.helpers.0 {
                let Some(first) = helpers.first() else {
                    continue;
                };
                let kind = first.kind();
                let answer = (|| {
                    prepared.require(*operation, kind)?;
                    let [source] = operations.get(*operation) else {
                        return Err("interrupt helper lost its unique Terminal operation");
                    };
                    let OperationKind::BoundaryCall { boundary, .. } = source.kind else {
                        return Err("interrupt helper is no longer its admitted boundary call");
                    };
                    if module
                        .boundary_machine(boundary)
                        .and_then(|row| row.declaration.as_ref())
                        .and_then(|row| row.execution.static_interrupt_operation())
                        != Some(kind)
                    {
                        return Err("interrupt helper changed its authenticated operation kind");
                    }
                    for helper in helpers {
                        let scope = prepared.scope(helper)?;
                        let [registration] = registrations.get(scope.operation()) else {
                            return Err(
                                "interrupt helper lost its unique retained installation request",
                            );
                        };
                        let [source] = operations.get(scope.operation()) else {
                            return Err(
                                "interrupt helper installation has no unique source operation",
                            );
                        };
                        if !matches!(&source.kind, OperationKind::BoundaryCall { boundary, arguments, structural_arguments, .. }
                            if *boundary == scope.boundary() && arguments.is_empty() && structural_arguments.as_slice() == scope.arguments())
                            || registration.installer != scope.boundary()
                        {
                            return Err(
                                "interrupt helper installation changed its boundary or retained operands",
                            );
                        }
                        let callback = callbacks
                            .unique(scope.operation(), helper.callback_application())
                            .ok_or("interrupt helper lost its designated acknowledgement entry")?;
                        let entry = prepared.entry(helper)?;
                        if !callback.entry_admitted
                            || callback.registrar != scope.operation()
                            || entry.placement.static_machine_ordinal
                                != callback.static_machine_ordinal
                            || entry.placement.canonical_requirement_overload
                                != callback.requirement
                            || entry.placement.contract_commitment != callback.body.contract
                            || entry.callback_function.caller() != psi
                            || entry.callback_function.callback_thunk_placement_index()
                                != Some(entry.placement_index)
                        {
                            return Err(
                                "interrupt helper changed its admitted callback identity or contract",
                            );
                        }
                    }
                    Ok(())
                })();
                answers.push((*operation, (kind, answer)));
            }
            Self {
                psi,
                cohort_psi: cohort.psi(),
                answers,
            }
        }
        pub fn verify(
            &self,
            operation: OperationId,
            kind: StaticInterruptOperation,
        ) -> Result<(), &'static str> {
            if self.psi != self.cohort_psi {
                return Err("interrupt helper cohort belongs to another Terminal program");
            }
            require_answer(&self.answers, operation, kind)
        }
    }

    pub(crate) fn validate_target(
        target: &target_operations::TargetOperationPlan,
        module: &TerminalModule,
        cohort: &AdmittedInstalledInterruptCohort,
        abstract_plan: &abstract_operations::AbstractOperationPlan,
    ) -> Result<(), crate::LegalizationError> {
        let fail = |operation, reason| crate::LegalizationError::InstalledInterruptAssociation {
            operation,
            reason,
        };
        if cohort.psi() != target.psi {
            return Err(fail(
                None,
                "interrupt helper cohort belongs to another Terminal program",
            ));
        }
        let helpers = PreparedInstalledInterruptHelpers::new(target.psi, module, cohort);
        let sources = PreparedInstallationSources::new(cohort, abstract_plan);
        for row in target
            .functions
            .iter()
            .flat_map(|function| function.graph.operations())
        {
            if let target_operations::TargetUnitOperation::StaticInterrupt {
                psi_operation,
                boundary,
                operation,
                token,
                ..
            } = row
            {
                helpers
                    .verify(*psi_operation, *operation)
                    .map_err(|reason| fail(Some(*psi_operation), reason))?;
                if !sources.boundary_calls(*psi_operation).iter().any(|source|
                    matches!(source, abstract_operations::AbstractOperation::BoundaryCall { psi_operation: actual_operation, boundary: actual, arguments, structural_arguments, .. }
                        if actual_operation == psi_operation && actual == boundary && arguments.is_empty() && structural_arguments.as_slice() == std::slice::from_ref(token))) {
                    return Err(fail(Some(*psi_operation), "interrupt helper changed its exact boundary or token operand"));
                }
            }
        }
        crate::interrupt_installation::validate_target(target, module, &sources)
    }
}
pub use installed_interrupt_geometry::InstalledInterruptOperandLayout;
pub(crate) mod interrupt_installation {
    use crate::installed_interrupts::PreparedInstallationSources;
    use target_operations::{InstallationCallRole, TargetOperationPlan, TargetUnitOperation};

    pub(crate) fn validate_target(
        target: &TargetOperationPlan,
        module: &terminal_psi::TerminalModule,
        source: &PreparedInstallationSources<'_>,
    ) -> Result<(), crate::LegalizationError> {
        let fail = |operation, reason| crate::LegalizationError::InstalledInterruptAssociation {
            operation: Some(operation),
            reason,
        };
        for row in target
            .functions
            .iter()
            .flat_map(|function| function.graph.operations())
        {
            let TargetUnitOperation::InstallStaticInterrupt {
                psi_operation,
                boundary,
                arguments,
                calls,
                ..
            } = row
            else {
                continue;
            };
            let Some(scope) = source.scope(*psi_operation) else {
                return Err(fail(
                    *psi_operation,
                    "installation has no unique admitted Scope",
                ));
            };
            if source.psi() != target.psi
                || scope.boundary() != *boundary
                || scope.arguments() != arguments
                || calls[0].role != InstallationCallRole::Validate
                || calls[0].callee != scope.validator()
                || calls[1].role != InstallationCallRole::Work
                || calls[1].callee != scope.work()
            {
                return Err(fail(
                    *psi_operation,
                    "installation changed its admitted operands or selected helper calls",
                ));
            }
            if module
                .boundary_machine(*boundary)
                .and_then(|row| row.declaration.as_ref())
                .and_then(|row| row.execution.static_interrupt_operation())
                != Some(language_core::static_interrupts::StaticInterruptOperation::Install)
                || !source.boundary_calls(*psi_operation).iter().any(|row| {
                    matches!(row,
                    abstract_operations::AbstractOperation::BoundaryCall {
                        psi_operation: operation, boundary: declared, arguments: scalars,
                        structural_arguments, ..
                    } if operation == psi_operation && declared == boundary && scalars.is_empty()
                        && structural_arguments.as_slice() == arguments)
                })
            {
                return Err(fail(
                    *psi_operation,
                    "installation changed its authenticated source operation",
                ));
            }
        }
        Ok(())
    }
}
pub use installed_interrupts::PreparedInstalledInterruptHelpers;
mod legalization;
mod legalization_descriptor_inputs;
mod legalization_function_inputs;
mod legalization_reference_custody;
mod legalization_scalar_graph;
mod legalization_source;
mod legalization_target;
mod optimized;
mod selection;
mod selection_access_lifecycles;
mod selection_call_abi;
mod selection_edge_transfers;
mod selection_inputs;
mod selection_interrupts;
mod selection_scalar_graph;
mod selection_structural;
mod selection_views_and_bytes;
mod structural_inputs;

pub use structural_inputs::structural_reference_input::{
    PointerSlot, ProjectedByteField, ReferenceSourceGeometry, RuntimeElement, projected_byte_field,
    reference_formation_geometry,
};

pub(crate) use legalization::{
    LegalizationError, ValidatedLegalizedOperations, legalize_target_operations,
};
pub use optimized::{
    OptimizedSelectionPipelineError, StagedOptimizedSelectedInstructions,
    StagedOptimizedSelectionCustodyReceipt, TargetSelectionInput,
    stage_optimized_instruction_selection,
};
pub(crate) use selection::{SelectedInstructionError, select_instructions};
pub use selection::{ValidatedSelectedInstructions, selected_instruction_plan_identity};

pub use legalization::{
    LegalizationError as TargetOperationAuditError, verify_target_operation_plan,
};
