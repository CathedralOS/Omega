//! The selected normalized-foreign-call record both instruction sets' template
//! tests admit, the case those tests derive from it, and the callback roster
//! they wire into it.
//!
//! The record carries physical call inputs only; source ownership and
//! declarations stay the template tests' independent responsibility. Only a
//! target's own facts differ between instruction sets — which target, how many
//! parameters its integer registers take before the first floating one, the
//! aggregate its fourth result shape names, and the locator family its import is
//! spelled in — so those are arguments and the construction is stated once.
//!
//! The encoded effects a roster row charges, the callback destination
//! vocabulary naming where a materialized address lands, and the byte counts
//! that vocabulary's materialization groups charge stay with the instruction
//! set; [`case`] and [`callback_fixture`] take them as arguments so the
//! scaffolding around them is stated once.
//!
//! The template bodies both instruction sets run end to end are stated here
//! too. Each admission asserts the same retained operands, the same evaluated
//! effects, the same exact byte extent, and the same fixup refusal of a mutated
//! group, a swapped destination, or a stale effect; an instruction set answers
//! only the [`Entrances`] it enters through — its row, its declaration, its
//! register vocabulary, its encoder and validator, its callback destination
//! vocabulary and byte table, and the terminal call bytes those groups
//! precede.

use calling_conventions::{
    CallSignature, CallbackBinderRequirement, CallbackMaterialization,
    CallbackMaterializationContext, CallbackRequirementId, CallingPolicy, LayoutPlanId,
    LayoutSlotId, MachineRegister, NativeCallbackDemand, NativeParameterApplication,
    NativeParameterId, NativePlace, ValueLocation, ValueShape,
};
use function_identity::MachineFunctionIdentity;
use register_model::{
    RegisterInstructionConstraint, RegisterOperandAccess, RegisterViewId,
    ValidatedPhysicalRegisterModel,
};
use selected_instructions::{
    MachineAlternativeFamily, MachineAlternativeKey, MachineEffectDeclaration,
    MachineEncodedEffects, MachineSizeKnowledge, SelectedInstructionId, SelectedInstructionKind,
    SelectedNormalizedForeignCall,
};
use semantic_vocabulary::{
    BoundaryMachineId, IeeeFloatFormat, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType,
    StructuralTypeId, ValueId,
};
use target::{
    ForeignLocatorCandidate, NativeTarget, NormalizedForeignLocator, TargetProfile,
    normalize_foreign_locator,
};
use target_operations::{
    NormalizedForeignCallBinding, NormalizedForeignCallMechanism, ProviderExecutionBinding,
    ProviderPlanReportIdentity, TargetBoundaryResult, TargetNativeCallbackArgument,
    TargetStructuralHomeLayout, TargetStructuralHomeOrigin, TargetStructuralHomeRequirement,
    TargetUnitScalarArgumentSource, TargetUnitScalarCallArgument, TargetUnitScalarHomeRequirement,
};

/// The PE import the Windows fixture names.
pub fn pe_locator() -> NormalizedForeignLocator {
    normalize_foreign_locator(
        ForeignLocatorCandidate::PeByName {
            library: b"KERNEL32.dll".to_vec(),
            export: b"Leaf".to_vec(),
        },
        TargetProfile::WindowsX64,
    )
    .unwrap()
}

/// The symbol-versioned ELF import the Linux fixture names.
pub fn elf_locator(profile: TargetProfile) -> NormalizedForeignLocator {
    normalize_foreign_locator(
        ForeignLocatorCandidate::ElfVersioned {
            object: b"libc.so.6".to_vec(),
            symbol: b"leaf".to_vec(),
            version: b"GLIBC_2.0".to_vec(),
        },
        profile,
    )
    .unwrap()
}

/// The Mach-O dylib import the Darwin fixture names.
pub fn mach_o_locator() -> NormalizedForeignLocator {
    normalize_foreign_locator(
        ForeignLocatorCandidate::MachODylibSymbol {
            install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
            symbol: b"_leaf".to_vec(),
        },
        TargetProfile::MacosArm64,
    )
    .unwrap()
}

/// One selected normalized-foreign-call record for `target`, carrying
/// `input_layout` parameters and `result`'s shaped result. `integer_registers`
/// is the target's integer-argument register count and `aggregate` the shape
/// its fourth result names.
pub fn record(
    target: NativeTarget,
    integer_registers: usize,
    aggregate: ValueShape,
    locator: NormalizedForeignLocator,
    input_layout: usize,
    result: usize,
) -> SelectedNormalizedForeignCall {
    let (general, floating) = if input_layout < integer_registers {
        (input_layout, 0)
    } else {
        (
            (input_layout - integer_registers) / 8,
            (input_layout - integer_registers) % 8 + 1,
        )
    };
    let parameters = std::iter::repeat_n(ValueShape::integer(8, 8), general)
        .chain(std::iter::repeat_n(ValueShape::float(8), floating))
        .collect::<Vec<_>>();
    let result_shape = match result {
        0 => None,
        1 => Some(ValueShape::integer(8, 8)),
        2 => Some(ValueShape::float(8)),
        3 => Some(ValueShape::integer(16, 8)),
        4 => Some(aggregate),
        5 => Some(ValueShape::integer(24, 8)),
        _ => panic!("test result"),
    };
    let boundary_entry_plan = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters,
            result: result_shape,
        },
    )
    .unwrap()
    .plan()
    .clone();
    let commitment = task_plans::SameStackProviderPlanCommitment::from_digest([0x42; 32]);
    let identities =
        terminal_psi::TerminalIdentityTable::from(vec![String::new(), "Foreign::leaf".to_owned()]);
    let requirement_index = identities
        .index_of("Foreign::leaf")
        .expect("test requirement identity must intern");
    let same_stack_contribution = task_plans::admit_same_stack_contribution(
        task_plans::SameStackContributionAdmissionCandidate {
            provider_plan_report_identity: 0xA1,
            provider_plan_commitment: commitment,
            requirement_identity: requirement_index,
            receipt: task_plans::SameStackContributionAdmissionReceiptId::from_normalized_identity(
                0xA2,
            )
            .unwrap(),
            bytes: 64,
            alignment: 16,
        },
        0xA1,
        commitment,
        requirement_index,
    )
    .unwrap();
    let operation = OperationId::new(7).unwrap();
    let scalar_type = |floating| {
        if floating {
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)
        } else {
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
        }
    };
    let scalar_arguments = boundary_entry_plan
        .call
        .parameters
        .iter()
        .enumerate()
        .map(|(position, placement)| TargetUnitScalarCallArgument {
            parameter_index: position as u32,
            source: TargetUnitScalarArgumentSource::Parameter {
                parameter_index: position as u32,
                source_value: ValueId::new(100 + position as u64).unwrap(),
                scalar_type: scalar_type(position >= general),
            },
            placement: placement.clone(),
        })
        .collect();
    let completion = match result_shape {
        None => TargetBoundaryResult::Unit,
        Some(shape) if result <= 2 => {
            TargetBoundaryResult::Scalar(TargetUnitScalarHomeRequirement {
                defining_operation: operation,
                source_value: ValueId::new(8).unwrap(),
                scalar_type: scalar_type(result == 2),
                shape,
            })
        }
        Some(shape) => TargetBoundaryResult::Structural(TargetStructuralHomeRequirement {
            origin: TargetStructuralHomeOrigin::OperationResult {
                operation,
                result: terminal_psi::StructuralOperationResult {
                    place: PlaceId::new(9).unwrap(),
                    structural_type: StructuralTypeId::new(2).unwrap(),
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                },
            },
            layout: TargetStructuralHomeLayout::Aggregate(ValueShape::integer(
                shape.byte_size,
                shape.alignment,
            )),
        }),
    };
    SelectedNormalizedForeignCall {
        instruction: SelectedInstructionId(4),
        invocation_site: operation.into(),
        call: legalized_operations::LegalizedNormalizedForeignCall {
            boundary: BoundaryMachineId::new(3).unwrap(),
            provider_execution: ProviderExecutionBinding::from_execution_record(
                ProviderPlanReportIdentity::new(0xA1).unwrap(),
                1,
                2,
                3,
                4,
            )
            .unwrap(),
            binding: NormalizedForeignCallBinding {
                boundary_entry_plan,
                mechanism: NormalizedForeignCallMechanism::Import {
                    locator,
                    same_stack_contribution,
                },
            },
            scalar_arguments,
            structural_arguments: Vec::new(),
            structural_indices: Vec::new(),
            result: completion,
            callbacks: Vec::new(),
        },
        effect: optimization_unit::EffectLink {
            input: 1,
            output: 2,
        },
        ownership: Vec::new(),
    }
}

/// Everything a template test hands its encoder, in the order both instruction
/// sets' test modules already destructure: the validated physical model, the
/// selected kind and alternative, the roster row's operand columns, the effects
/// that row's call declaration charges, and the record they were derived from.
pub type Case = (
    ValidatedPhysicalRegisterModel,
    SelectedInstructionKind,
    MachineAlternativeKey,
    Vec<RegisterViewId>,
    Vec<RegisterOperandAccess>,
    MachineEncodedEffects,
    SelectedNormalizedForeignCall,
);

/// Derive one encoder-test case from `record`.
///
/// The kind, the alternative, the roster row the record evaluates to, that
/// row's operand columns, and the record itself are target-independent; only
/// the row lookup (`constraint`) and the encoded effects of the row under
/// `physical` (`effects`) belong to the instruction set.
pub fn case(
    physical: ValidatedPhysicalRegisterModel,
    record: SelectedNormalizedForeignCall,
    constraint: impl Fn(&SelectedNormalizedForeignCall) -> Option<RegisterInstructionConstraint>,
    effects: impl Fn(
        &RegisterInstructionConstraint,
        &ValidatedPhysicalRegisterModel,
    ) -> MachineEncodedEffects,
) -> Case {
    let kind = SelectedInstructionKind::NormalizedForeignCall {
        boundary: BoundaryMachineId::new(3).unwrap(),
        ordinal: 2,
    };
    let alternative = MachineAlternativeKey {
        family: MachineAlternativeFamily::NormalizedForeignCall,
        variant: 0,
    };
    let row = constraint(&record).expect("fixture record must evaluate its roster row");
    let (operands, accesses) = operand_columns(&row);
    let effects = effects(&row, &physical);
    (
        physical,
        kind,
        alternative,
        operands,
        accesses,
        effects,
        record,
    )
}

/// The roster row's operand view and access columns, in roster order.
pub fn operand_columns(
    row: &RegisterInstructionConstraint,
) -> (Vec<RegisterViewId>, Vec<RegisterOperandAccess>) {
    let views = row
        .operands
        .iter()
        .map(|operand| {
            operand
                .fixed_view
                .expect("fixture row pins every operand view")
        })
        .collect();
    let accesses = row.operands.iter().map(|operand| operand.access).collect();
    (views, accesses)
}

/// The materialization byte counts one instruction set charges for its two
/// callback destination forms: a register and a record field.
#[derive(Debug, Clone, Copy)]
pub struct CallbackByteCounts {
    pub register: u16,
    pub field: u16,
}

/// One wired private callback destination, named the way every instruction set
/// can read it: the register its address materializes in and, for a record
/// field, the byte offset of the private slot inside that record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackPlacement {
    pub register: MachineRegister,
    pub field_byte_offset: Option<usize>,
}

/// The wired callback fixture: the record that carries the callbacks, each
/// destination in the caller's own vocabulary, the materialization byte count
/// the caller's table charges for it, and the physical placement the fixture
/// drew it from — all in roster order.
pub struct CallbackFixture<Destination> {
    pub record: SelectedNormalizedForeignCall,
    pub destinations: Vec<Destination>,
    pub byte_counts: Vec<u16>,
    pub placements: Vec<CallbackPlacement>,
}

/// Wire the four-callback fixture into `record`: a private parameter slot, a
/// record field, another parameter slot, and another field of the same root
/// record, in that roster order, over the record's own evaluated
/// boundary-entry plan.
///
/// `into_destination` names each placement in the caller's vocabulary — the
/// fixture never lands an address in a memory destination, because its callback
/// roots are register resident — and `counts` is the caller's own per-form
/// materialization extent table. The two private parameter slots are
/// materialized inside the call, so the record keeps only the field root as an
/// explicit input; source custody for that root is checked upstream.
pub fn callback_fixture<Destination>(
    mut record: SelectedNormalizedForeignCall,
    into_destination: impl Fn(CallbackPlacement) -> Destination,
    counts: CallbackByteCounts,
) -> CallbackFixture<Destination> {
    let root = NativeParameterId::new(81).unwrap();
    let places = [
        NativePlace::Parameter(NativeParameterId::new(82).unwrap()),
        NativePlace::Field {
            parameter: root,
            layout: LayoutPlanId::new(83).unwrap(),
            field_path: vec![LayoutSlotId::new(1).unwrap()],
        },
        NativePlace::Parameter(NativeParameterId::new(84).unwrap()),
        NativePlace::Field {
            parameter: root,
            layout: LayoutPlanId::new(83).unwrap(),
            field_path: vec![LayoutSlotId::new(2).unwrap()],
        },
    ];
    let requirement = CallbackRequirementId::new(85).unwrap();
    let context = CallbackMaterializationContext {
        binders: (1..=4)
            .map(|identity| CallbackBinderRequirement {
                binder: calling_conventions::StaticMachineBinderId::new(identity).unwrap(),
                requirement,
            })
            .collect(),
        demands: places
            .iter()
            .cloned()
            .map(|destination| NativeCallbackDemand {
                destination,
                requirement,
            })
            .collect(),
    };
    let plan = &mut record.call.binding.boundary_entry_plan;
    plan.call.callback_materializations = places
        .iter()
        .cloned()
        .enumerate()
        .map(|(position, destination)| CallbackMaterialization {
            binder: context.binders[position].binder,
            destination,
        })
        .collect();
    let continuation = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
    };
    let mut destinations = Vec::with_capacity(places.len());
    let mut byte_counts = Vec::with_capacity(places.len());
    let mut placements = Vec::with_capacity(places.len());
    for (placement_index, place) in places.into_iter().enumerate() {
        let (parameter, native_ordinal, field_byte_offset) = match place {
            NativePlace::Parameter(parameter) => (parameter, placement_index / 2 + 1, None),
            NativePlace::Field { parameter, .. } => (parameter, 0, Some((placement_index / 2) * 8)),
        };
        let placement = plan.call.parameters[native_ordinal].clone();
        let [ValueLocation::Register { register, .. }] = placement.locations.as_slice() else {
            panic!("fixture callback root must be register resident")
        };
        destinations.push(into_destination(CallbackPlacement {
            register: *register,
            field_byte_offset,
        }));
        placements.push(CallbackPlacement {
            register: *register,
            field_byte_offset,
        });
        byte_counts.push(if field_byte_offset.is_some() {
            counts.field
        } else {
            counts.register
        });
        record.call.callbacks.push(TargetNativeCallbackArgument {
            registrar_invokes_callback: true,
            terminal_operation: record
                .invocation_site
                .operation()
                .expect("callback is owned by a returning operation"),
            placement_index,
            callback_function: MachineFunctionIdentity::callback_thunk(
                continuation,
                placement_index,
            )
            .unwrap(),
            destination: place,
            field_byte_offset,
            application: NativeParameterApplication {
                parameter,
                native_ordinal: native_ordinal as u32,
                shape: placement.shape,
                placement,
            },
            registrar_boundary_entry_plan: plan.clone(),
            registrar_context: context.clone(),
            registrar_application_commitment: [0x66; 32],
        });
    }
    record.call.scalar_arguments.truncate(1);
    CallbackFixture {
        record,
        destinations,
        byte_counts,
        placements,
    }
}

/// One instruction set's terminal call bytes, and where its fixup patches the
/// import field and measures the next-instruction reference, both relative to
/// the call's first byte.
#[derive(Debug, Clone)]
pub struct CallBytes {
    pub bytes: Vec<u8>,
    pub patch_byte_offset: u16,
    pub reference_byte_offset: u16,
}

/// The unresolved fixup record one instruction set's templates carry.
pub type ForeignFixup<Kind, State, Callback> = crate::Fixup<Kind, State, Callback>;

/// One instruction set's encoder entrance: the selected inputs a template is
/// encoded from, the callback destinations in the caller's vocabulary, and the
/// record they were derived from.
pub type Entrance<Destination, Kind, State, Callback> =
    fn(
        NativeTarget,
        &ValidatedPhysicalRegisterModel,
        SelectedInstructionKind,
        MachineAlternativeKey,
        &[RegisterViewId],
        &[RegisterOperandAccess],
        &MachineEncodedEffects,
        &[Destination],
        &SelectedNormalizedForeignCall,
    ) -> Result<crate::Template<ForeignFixup<Kind, State, Callback>>, crate::Error>;

/// One instruction set's validator entrance: the same selected inputs as
/// [`Entrance`], plus the bytes, fixup, and callback destinations a caller
/// claims the encoder produced.
pub type Validation<Destination, Kind, State, Callback> =
    fn(
        NativeTarget,
        &ValidatedPhysicalRegisterModel,
        SelectedInstructionKind,
        MachineAlternativeKey,
        &[RegisterViewId],
        &[RegisterOperandAccess],
        &MachineEncodedEffects,
        &[u8],
        ForeignFixup<Kind, State, Callback>,
        &[Destination],
        &SelectedNormalizedForeignCall,
    ) -> Result<crate::Template<ForeignFixup<Kind, State, Callback>>, crate::Error>;

/// Everything an instruction set answers the shared template bodies, and
/// nothing else: the roster row and declaration its own effects owner
/// evaluates, the physical view its model names a machine register by, its
/// encoder and validator, the canonical fixup constructor and the columns of
/// one callback materialization, and the terminal call bytes plus the patch
/// offsets those materialization groups precede.
pub struct Entrances<Destination, Kind, State, Callback> {
    pub call: CallBytes,
    pub constraint: fn(&SelectedNormalizedForeignCall) -> Option<RegisterInstructionConstraint>,
    pub declaration: fn(
        &SelectedNormalizedForeignCall,
        &RegisterInstructionConstraint,
    ) -> Option<MachineEffectDeclaration>,
    pub fixed_view: fn(&ValidatedPhysicalRegisterModel, MachineRegister) -> Option<RegisterViewId>,
    pub encode: Entrance<Destination, Kind, State, Callback>,
    pub validate: Validation<Destination, Kind, State, Callback>,
    /// The canonical unresolved fixup for one `{boundary, ordinal}` row.
    pub fixup:
        fn(BoundaryMachineId, u32, &[Destination]) -> Option<ForeignFixup<Kind, State, Callback>>,
    /// One callback materialization as `(destination, byte offset, byte count)`.
    pub fixup_row: fn(&Callback) -> (Destination, u16, u16),
    /// Displace one materialization group, leaving the others intact.
    pub advance: fn(&mut Callback),
}

/// One selected case bound to the instruction set that encodes it, so a shared
/// body names only the effects, bytes, fixup, and destinations it asserts
/// about rather than restating the nine selected inputs each time.
struct Subject<'a, Destination, Kind, State, Callback> {
    entrances: &'a Entrances<Destination, Kind, State, Callback>,
    target: NativeTarget,
    physical: ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operands: Vec<RegisterViewId>,
    accesses: Vec<RegisterOperandAccess>,
    effects: MachineEncodedEffects,
    record: SelectedNormalizedForeignCall,
}

impl<'a, Destination, Kind, State, Callback> Subject<'a, Destination, Kind, State, Callback> {
    fn new(entrances: &'a Entrances<Destination, Kind, State, Callback>, case: Case) -> Self {
        let (physical, kind, alternative, operands, accesses, effects, record) = case;
        let target = record
            .call
            .binding
            .native_target()
            .expect("fixture record names its target");
        Self {
            entrances,
            target,
            physical,
            kind,
            alternative,
            operands,
            accesses,
            effects,
            record,
        }
    }

    /// Re-point the subject at the retained row's own columns and alternative.
    fn bind_row(
        &mut self,
        row: &RegisterInstructionConstraint,
        alternative: MachineAlternativeKey,
    ) {
        let (operands, accesses) = operand_columns(row);
        self.alternative = alternative;
        self.operands = operands;
        self.accesses = accesses;
    }

    fn attempt(
        &self,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        accesses: &[RegisterOperandAccess],
        effects: &MachineEncodedEffects,
        destinations: &[Destination],
    ) -> Result<crate::Template<ForeignFixup<Kind, State, Callback>>, crate::Error> {
        self.attempt_at(
            self.target,
            kind,
            alternative,
            operands,
            accesses,
            effects,
            destinations,
        )
    }

    fn attempt_at(
        &self,
        target: NativeTarget,
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operands: &[RegisterViewId],
        accesses: &[RegisterOperandAccess],
        effects: &MachineEncodedEffects,
        destinations: &[Destination],
    ) -> Result<crate::Template<ForeignFixup<Kind, State, Callback>>, crate::Error> {
        (self.entrances.encode)(
            target,
            &self.physical,
            kind,
            alternative,
            operands,
            accesses,
            effects,
            destinations,
            &self.record,
        )
    }

    fn encode(
        &self,
        effects: &MachineEncodedEffects,
        destinations: &[Destination],
    ) -> Result<crate::Template<ForeignFixup<Kind, State, Callback>>, crate::Error> {
        self.attempt(
            self.kind,
            self.alternative,
            &self.operands,
            &self.accesses,
            effects,
            destinations,
        )
    }

    /// Encode this record against substituted operand columns, which is how a
    /// body proves the columns are not interchangeable.
    fn encode_with(
        &self,
        operands: &[RegisterViewId],
        accesses: &[RegisterOperandAccess],
        effects: &MachineEncodedEffects,
        destinations: &[Destination],
    ) -> Result<crate::Template<ForeignFixup<Kind, State, Callback>>, crate::Error> {
        self.attempt(
            self.kind,
            self.alternative,
            operands,
            accesses,
            effects,
            destinations,
        )
    }

    fn validate(
        &self,
        bytes: &[u8],
        fixup: ForeignFixup<Kind, State, Callback>,
        effects: &MachineEncodedEffects,
        destinations: &[Destination],
    ) -> Result<crate::Template<ForeignFixup<Kind, State, Callback>>, crate::Error> {
        (self.entrances.validate)(
            self.target,
            &self.physical,
            self.kind,
            self.alternative,
            &self.operands,
            &self.accesses,
            effects,
            bytes,
            fixup,
            destinations,
            &self.record,
        )
    }

    /// The exact extent of `byte_counts`' materialization groups plus this
    /// instruction set's terminal call bytes.
    fn extent(&self, byte_counts: &[u16]) -> u16 {
        let call = u16::try_from(self.entrances.call.bytes.len()).expect("call bytes fit a u16");
        byte_counts.iter().sum::<u16>() + call
    }
}

/// The callback-bearing record's retained operands, evaluated effects, exact
/// bytes, and unresolved fixups, under both of one target's alternate ABIs.
///
/// `into_destination` and `counts` are the caller's callback destination
/// vocabulary and per-form byte table; every assertion around them is the same
/// on both instruction sets.
pub fn retained_callbacks<Destination, Kind, State, Callback>(
    entrances: &Entrances<Destination, Kind, State, Callback>,
    inputs: impl Fn(bool, usize, usize) -> Case,
    into_destination: fn(CallbackPlacement) -> Destination,
    counts: CallbackByteCounts,
) where
    Destination: Copy + PartialEq + std::fmt::Debug,
    Kind: Clone + PartialEq + std::fmt::Debug,
    State: Clone + PartialEq + std::fmt::Debug,
    Callback: Clone + PartialEq + std::fmt::Debug,
{
    for alternate_abi in [false, true] {
        let mut subject = Subject::new(entrances, inputs(alternate_abi, 3, 1));
        let original = (entrances.constraint)(&subject.record)
            .expect("fixture record evaluates its roster row");
        let CallbackFixture {
            record,
            destinations,
            byte_counts,
            placements,
        } = callback_fixture(subject.record.clone(), into_destination, counts);
        subject.record = record;
        let scratch_register = subject
            .record
            .call
            .binding
            .boundary_entry_plan
            .call
            .policy
            .callback_field_scratch();
        let row =
            (entrances.constraint)(&subject.record).expect("callback record evaluates its row");
        assert_eq!(row.key, original.key);
        assert_eq!(row.operands.len(), 2);
        assert_eq!(row.operands[0], original.operands[0]);
        assert_eq!(row.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(row.operands[1].fixed_view, original.operands[3].fixed_view);
        for register in [
            placements[0].register,
            placements[2].register,
            scratch_register,
        ] {
            let view = (entrances.fixed_view)(&subject.physical, register)
                .expect("fixture placement is an ABI-visible register");
            let view = subject
                .physical
                .model()
                .view(view)
                .expect("fixed view is in the model");
            assert!(
                view.units
                    .iter()
                    .all(|unit| row.implicit_uses.contains(unit))
            );
        }
        let declaration = (entrances.declaration)(&subject.record, &row)
            .expect("callback row retains its declaration");
        let alternative = &declaration.alternatives[0];
        assert_eq!(alternative.key.variant, 3);
        subject.bind_row(&row, alternative.key);
        let materialized: u16 = byte_counts.iter().sum();
        let extent = subject.extent(&byte_counts);
        assert_eq!(alternative.size, MachineSizeKnowledge::ExactBytes(extent));
        assert_eq!(alternative.encoded.external_operand_reads, vec![0]);
        assert_eq!(alternative.encoded.external_operand_writes, vec![1]);
        let template = subject
            .encode(&alternative.encoded, &destinations)
            .expect("callback record encodes");
        assert_eq!(template.bytes().len(), usize::from(extent));
        assert_eq!(
            &template.bytes()[usize::from(materialized)..],
            entrances.call.bytes.as_slice()
        );
        let fixup = template.fixup();
        assert_eq!(fixup.opcode_byte_offset, materialized);
        assert_eq!(
            fixup.patch_byte_offset,
            materialized + entrances.call.patch_byte_offset
        );
        assert_eq!(
            fixup.reference_byte_offset,
            materialized + entrances.call.reference_byte_offset
        );
        assert_eq!(fixup.patch_byte_width, 4);
        assert_eq!(fixup.callbacks.len(), 4);
        let mut byte_offset = 0;
        for (position, callback) in fixup.callbacks.iter().enumerate() {
            let (destination, materialization, byte_count) = (entrances.fixup_row)(callback);
            assert_eq!(destination, destinations[position]);
            assert_eq!(materialization, byte_offset);
            assert_eq!(byte_count, byte_counts[position]);
            byte_offset += byte_count;
        }
        assert_eq!(byte_offset, materialized);
        let replay = |bytes: &[u8], fixup, effects: &MachineEncodedEffects, destinations: &[_]| {
            subject.validate(bytes, fixup, effects, destinations)
        };
        assert!(
            replay(
                template.bytes(),
                template.fixup().clone(),
                &alternative.encoded,
                &destinations
            )
            .is_ok()
        );
        let mut changed_bytes = template.bytes().to_vec();
        changed_bytes[0] ^= 1;
        assert!(
            replay(
                &changed_bytes,
                template.fixup().clone(),
                &alternative.encoded,
                &destinations
            )
            .is_err()
        );
        let mut changed_fixup = template.fixup().clone();
        (entrances.advance)(&mut changed_fixup.callbacks[1]);
        assert!(
            replay(
                template.bytes(),
                changed_fixup,
                &alternative.encoded,
                &destinations
            )
            .is_err()
        );
        let mut changed_destinations = destinations.clone();
        changed_destinations.swap(0, 2);
        assert!(
            replay(
                template.bytes(),
                template.fixup().clone(),
                &alternative.encoded,
                &changed_destinations
            )
            .is_err()
        );
        let mut changed_effects = alternative.encoded.clone();
        changed_effects.implicit_unit_uses.clear();
        assert!(
            replay(
                template.bytes(),
                template.fixup().clone(),
                &changed_effects,
                &destinations
            )
            .is_err()
        );
        let mut changed_row = row.clone();
        changed_row.implicit_uses.clear();
        assert!(
            (entrances.declaration)(&subject.record, &changed_row).is_none(),
            "a row that lost its implicit uses cannot retain the declaration"
        );
        let mut changed_record = subject.record.clone();
        changed_record.call.callbacks[0].application.native_ordinal = 0;
        assert!((entrances.constraint)(&changed_record).is_none());
        let mut changed_record = subject.record.clone();
        changed_record.call.callbacks[1].field_byte_offset = None;
        assert!((entrances.constraint)(&changed_record).is_none());
        let mut changed_record = subject.record.clone();
        changed_record.call.callbacks[0].terminal_operation = OperationId::new(99).unwrap();
        assert!((entrances.declaration)(&changed_record, &row).is_none());
    }
}

/// Every result shape — direct, hidden-aggregate, and structural — retains one
/// template, and a stale effect or a dropped parameter location is refused.
pub fn direct_and_hidden_aggregate_results_share_the_import_template<
    Destination,
    Kind,
    State,
    Callback,
>(
    entrances: &Entrances<Destination, Kind, State, Callback>,
    inputs: impl Fn(bool, usize, usize) -> Case,
) where
    Destination: Copy + PartialEq + std::fmt::Debug,
    Kind: PartialEq + std::fmt::Debug,
    State: PartialEq + std::fmt::Debug,
    Callback: PartialEq + std::fmt::Debug,
{
    for alternate_abi in [false, true] {
        let mut shared_key = None;
        for result in 0..=5 {
            let subject = Subject::new(entrances, inputs(alternate_abi, 11, result));
            let row =
                (entrances.constraint)(&subject.record).expect("fixture record evaluates its row");
            if let Some(key) = shared_key {
                assert_eq!(row.key, key);
            } else {
                shared_key = Some(row.key);
            }
            let declaration = (entrances.declaration)(&subject.record, &row)
                .expect("fixture row retains its declaration");
            assert_eq!(declaration.alternatives[0].encoded, subject.effects);
            let template = subject
                .encode(&subject.effects, &[])
                .expect("fixture record encodes");
            assert_eq!(template.bytes(), entrances.call.bytes.as_slice());
            let mut corrupted = subject.effects.clone();
            corrupted.implicit_unit_clobbers.clear();
            assert_eq!(
                subject.encode(&corrupted, &[]),
                Err(crate::Error::EffectMismatch)
            );
            let mut changed_row = row.clone();
            changed_row.operands.last_mut().unwrap().access = RegisterOperandAccess::UseDef;
            assert!(
                (entrances.declaration)(&subject.record, &changed_row).is_none(),
                "a row that changed its result custody cannot retain the declaration"
            );
            let mut changed_record = subject.record.clone();
            changed_record
                .call
                .binding
                .boundary_entry_plan
                .call
                .parameters[0]
                .locations
                .clear();
            assert!((entrances.constraint)(&changed_record).is_none());
        }
    }
}

/// A callback-free template is exactly the terminal call bytes over the
/// canonical fixup, and every selected input round-trips.
pub fn template_has_exact_bytes_and_fixup<Destination, Kind, State, Callback>(
    entrances: &Entrances<Destination, Kind, State, Callback>,
    inputs: impl Fn(bool, usize, usize) -> Case,
) where
    Destination: Copy + PartialEq + std::fmt::Debug,
    Kind: Clone + PartialEq + std::fmt::Debug,
    State: Clone + PartialEq + std::fmt::Debug,
    Callback: Clone + PartialEq + std::fmt::Debug,
{
    let subject = Subject::new(entrances, inputs(false, 1, 1));
    let template = subject
        .encode(&subject.effects, &[])
        .expect("fixture record encodes");
    assert_eq!(template.bytes(), entrances.call.bytes.as_slice());
    assert_eq!(template.kind(), subject.kind);
    assert_eq!(template.alternative(), subject.alternative);
    assert_eq!(template.operand_views(), subject.operands.as_slice());
    assert_eq!(template.effects(), &subject.effects);
    let fixup = template.fixup().clone();
    assert_eq!(
        fixup,
        (entrances.fixup)(BoundaryMachineId::new(3).unwrap(), 2, &[])
            .expect("canonical fixup constructs")
    );
    assert_eq!(fixup.opcode_byte_offset, 0);
    assert_eq!(fixup.patch_byte_offset, entrances.call.patch_byte_offset);
    assert_eq!(
        fixup.reference_byte_offset,
        entrances.call.reference_byte_offset
    );
    assert_eq!(fixup.patch_byte_width, 4);
}

/// Every evaluated boundary-entry plan selects its own exact operand columns
/// under one template, and a substituted view, a flipped role, or another
/// result's columns are refused.
///
/// `layouts` is the number of parameter counts one target's plan admits and
/// `substitute` a view naming neither the input nor the result bank.
pub fn evaluated_plans_select_exact_operands_and_effects_under_one_template<
    Destination,
    Kind,
    State,
    Callback,
>(
    entrances: &Entrances<Destination, Kind, State, Callback>,
    inputs: impl Fn(bool, usize, usize) -> Case,
    layouts: std::ops::Range<usize>,
    substitute: &str,
) where
    Destination: Copy + PartialEq + std::fmt::Debug,
    Kind: PartialEq + std::fmt::Debug,
    State: PartialEq + std::fmt::Debug,
    Callback: PartialEq + std::fmt::Debug,
{
    for input_layout in layouts {
        for result in 0..3 {
            let subject = Subject::new(entrances, inputs(false, input_layout, result));
            let template = subject
                .encode(&subject.effects, &[])
                .expect("admitted plan encodes");
            assert_eq!(template.operand_views(), subject.operands.as_slice());
            // A different result choice is a valid row, but cannot retain
            // this row's external writes and remaining caller-save effects.
            let (_, _, _, wrong, wrong_accesses, _, _) =
                inputs(false, input_layout, (result + 1) % 3);
            assert_eq!(
                subject.encode_with(&wrong, &wrong_accesses, &subject.effects, &[]),
                Err(crate::Error::OperandViewMismatch)
            );
            if !subject.operands.is_empty() {
                // This view belongs to neither the input nor result bank.
                let mut substituted = subject.operands.clone();
                substituted[0] = subject.physical.model().view_named(substitute).unwrap().id;
                assert_eq!(
                    subject.encode_with(&substituted, &subject.accesses, &subject.effects, &[]),
                    Err(crate::Error::OperandViewMismatch)
                );
                // Flip the actual role: result-only rows start with Def,
                // whereas nonempty input layouts start with Use. If the
                // new roles describe another valid row, stale effects reject.
                let mut swapped_accesses = subject.accesses.clone();
                swapped_accesses[0] = match subject.accesses[0] {
                    RegisterOperandAccess::Use => RegisterOperandAccess::Def,
                    RegisterOperandAccess::Def => RegisterOperandAccess::Use,
                    RegisterOperandAccess::UseDef => unreachable!(),
                };
                assert!(
                    subject
                        .encode_with(&subject.operands, &swapped_accesses, &subject.effects, &[])
                        .is_err()
                );
            }
        }
    }
}

/// One admission rejects a wrong instruction kind, every target this
/// instruction set does not encode, a foreign alternative family, a
/// substituted `{boundary, ordinal}`, a non-canonical terminal word, and a
/// stale effect.
///
/// `unsupported` is the set of targets the other instruction set owns, which
/// is this one's evidence of target admission; `malformed` is a word that
/// differs from the canonical placeholder.
pub fn rejects_wrong_kind_target_alternative_and_fixup<Destination, Kind, State, Callback>(
    entrances: &Entrances<Destination, Kind, State, Callback>,
    inputs: impl Fn(bool, usize, usize) -> Case,
    unsupported: &[NativeTarget],
    malformed: &[u8],
) where
    Destination: Copy + PartialEq + std::fmt::Debug,
    Kind: Clone + PartialEq + std::fmt::Debug,
    State: Clone + PartialEq + std::fmt::Debug,
    Callback: Clone + PartialEq + std::fmt::Debug,
{
    let subject = Subject::new(entrances, inputs(false, 1, 1));
    assert_eq!(
        subject.attempt(
            SelectedInstructionKind::CallScalar {
                target: selected_instructions::SelectedCallTarget::Machine(
                    semantic_vocabulary::MachineId::new(4).unwrap(),
                ),
            },
            subject.alternative,
            &subject.operands,
            &subject.accesses,
            &subject.effects,
            &[],
        ),
        Err(crate::Error::InstructionKindMismatch)
    );
    for target in unsupported {
        assert_eq!(
            subject.attempt_at(
                *target,
                subject.kind,
                subject.alternative,
                &subject.operands,
                &subject.accesses,
                &subject.effects,
                &[],
            ),
            Err(crate::Error::UnsupportedTarget)
        );
    }
    assert_eq!(
        subject.attempt(
            subject.kind,
            MachineAlternativeKey {
                family: MachineAlternativeFamily::CallScalar,
                variant: 0,
            },
            &subject.operands,
            &subject.accesses,
            &subject.effects,
            &[],
        ),
        Err(crate::Error::AlternativeMismatch)
    );
    let template = subject
        .encode(&subject.effects, &[])
        .expect("fixture record encodes");
    // A substituted boundary or ordinal cannot validate.
    let mut foreign = template.fixup().clone();
    foreign.boundary = BoundaryMachineId::new(9).unwrap();
    assert_eq!(
        subject.validate(template.bytes(), foreign, &subject.effects, &[]),
        Err(crate::Error::FixupMismatch)
    );
    let mut foreign = template.fixup().clone();
    foreign.ordinal = 7;
    assert_eq!(
        subject.validate(template.bytes(), foreign, &subject.effects, &[]),
        Err(crate::Error::FixupMismatch)
    );
    // The canonical placeholder is the only admitted word.
    assert_eq!(
        subject.validate(malformed, template.fixup().clone(), &subject.effects, &[]),
        Err(crate::Error::MalformedTemplate)
    );
    // A complete effect mismatch rejects.
    let mut wrong_effects = subject.effects.clone();
    wrong_effects.implicit_unit_clobbers.clear();
    assert_eq!(
        subject.encode(&wrong_effects, &[]),
        Err(crate::Error::EffectMismatch)
    );
}
