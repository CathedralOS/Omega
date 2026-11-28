use super::aggregate_result_input::{owned_parameter_uses_local_home, parameter_homes};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use legalized_operations::native_geometry::{NativeFunction, NativeGeometry};
use legalized_operations::*;
use operation_evidence::provenance::EffectLink;
use target_lowering::test_support::{
    HOST, block_id, case_id, edge, machine, op, parameter_at, place, sum_type, ty,
};
use target_vocabulary::{
    CallSiteOwner, NativeCallOrigin, NativeCallTarget, TargetStructuralArgument,
    TargetStructuralArgumentSource, TargetStructuralParameter,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralPathSegment, TerminalIdentityTableBuilder,
};

#[test]
fn entry_and_dispatch_agree_on_which_parameter_uses_need_storage() {
    use StructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};
    let mut names = TerminalIdentityTableBuilder::default();
    let mut primitive = sum_type(2, names.intern("Scalar"), &mut names, &[]);
    primitive.shape = terminal_psi::StructuralTypeShape::PrimitiveScalar(
        semantic_vocabulary::ScalarType::Boolean,
    );
    let types = vec![
        sum_type(1, names.intern("Choice"), &mut names, &[]),
        primitive,
    ];
    let shape = ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(HOST()),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .unwrap();
    let placement = call_plan.parameters[0].clone();
    // Parameter positions are dense even when the semantic place numbers are not.
    let cases = [
        // Place, entry access, type, byte size, expected entry home.
        (99, Owned, ty(1), 8, true),    // Projection.
        (5100, Owned, ty(1), 8, false), // Root membership.
        (4, SharedBorrow, ty(1), 8, false),
        (8, MutableBorrow, ty(1), 8, false),
        (21, WriteOnlyBorrow, ty(1), 8, false),
        (13, Owned, ty(2), 8, false), // Ineligible primitive.
        (81, Owned, ty(1), 8, false), // Cleanup forwards its register.
        (76, Owned, ty(1), 8, true),  // Cleanup needs a different byte range.
        (41, Owned, ty(1), 8, true),  // Case dispatch.
        (2, Owned, ty(1), 0, true),   // Entry can give an empty value an address.
        (23, Owned, ty(1), 8, true),  // Jump binding.
        (61, Owned, ty(1), 8, true),  // Borrowed at a normal call.
        (88, Owned, ty(1), 8, true),  // Borrowed at a call that never returns.
        (55, Owned, ty(1), 8, false), // Owned at a normal call.
        (72, Owned, ty(1), 8, false), // Unused.
        (73, Owned, ty(1), 8, false), // Stack-backed argument.
        (74, Owned, ty(1), 8, false), // Register forwarded to indirect transport.
        (75, Owned, ty(1), 8, true),  // Indirect transport needs different bytes.
        (77, Owned, ty(1), 8, true),  // Cleanup borrows the incoming value.
    ];
    let parameters: Vec<_> = cases
        .iter()
        .enumerate()
        .map(|(position, &(id, access, structural_type, byte_size, _))| {
            let mut semantic = parameter_at(id, structural_type);
            semantic.access = access;
            semantic.position = position as u32;
            let target = TargetStructuralParameter {
                place: semantic.place,
                structural_type,
                access,
                multiplicity: semantic.multiplicity,
                projected_qualifications: vec![],
                shape: ValueShape::integer(byte_size, 8),
                placement: placement.clone(),
            };
            LegalizedCallUnitParameter { semantic, target }
        })
        .collect();
    let stack = calling_conventions::ValuePlacement {
        shape,
        locations: vec![calling_conventions::ValueLocation::stack(0, 0, 8, 8)],
    };
    let indirect = calling_conventions::ValuePlacement {
        shape,
        locations: vec![calling_conventions::ValueLocation::indirect(
            calling_conventions::IndirectPointerLocation::Stack {
                stack_byte_offset: 0,
                alignment: 8,
            },
            Some(8),
            8,
            8,
        )],
    };
    let argument = |id, access| StructuralArgument {
        place: place(id),
        access,
        path: vec![],
    };
    let transport = |id, access, offset, source, destination| LegalizedScalarArgument::Structural {
        semantic: argument(id, access),
        indices: vec![],
        target: TargetStructuralArgument {
            place: place(id),
            access,
            path: vec![],
            root_structural_type: ty(1),
            structural_type: ty(1),
            shape,
            source_byte_offset: offset,
            fixed_array_length: None,
            element_stride: None,
            indices: vec![],
            source: TargetStructuralArgumentSource::Placement(source),
            destination,
        },
    };
    let call_argument =
        |id, access, offset| transport(id, access, offset, placement.clone(), placement.clone());
    let call = |arguments| LegalizedScalarCall {
        suspension_crossing: None,
        arguments,
        call_plan: call_plan.clone(),
        source: NativeCallOrigin::Authored,
        target: NativeCallTarget::Machine(machine()),
        result_placement: None,
        structural_result: None,
        claim_transfers: vec![],
        requirement_obligations: vec![],
        crash_continuations: vec![],
        descriptor_origins: vec![],
        access_bindings: vec![],
    };
    let effect = EffectLink {
        input: 0,
        output: 0,
    };
    let instruction = |id, kind| LegalizedScalarInstruction {
        operation: op(id),
        result: None,
        kind,
        work: vec![],
        effect,
        ownership: vec![],
    };
    let membership = |id, path| {
        instruction(
            id,
            LegalizedScalarInstructionKind::StructuralCaseMembership {
                source: place(id),
                path,
                case: case_id(1),
                case_tag: 0,
                tag_byte_offset: 0,
                indices: vec![],
                pointer_slots: vec![],
            },
        )
    };
    let block = |id, instructions, terminator| LegalizedScalarBlock {
        id: block_id(id),
        parameters: vec![],
        structural_parameters: Default::default(),
        instructions,
        terminator,
    };
    let mut instructions: Vec<_> = [99, 4, 8, 21, 13, 2]
        .map(|id| membership(id, vec![StructuralPathSegment::Referent]))
        .into_iter()
        .collect();
    instructions.push(membership(5100, vec![]));
    instructions.push(instruction(
        100,
        LegalizedScalarInstructionKind::Call(call(vec![
            call_argument(61, SharedBorrow, 0),
            call_argument(55, Owned, 1),
        ])),
    ));
    let geometry = NativeGeometry::new(&types, HOST()).unwrap();
    let dispatch = LegalizedScalarTerminator::StructuralCase {
        psi_block: block_id(2),
        source: LegalizedStructuralCaseSource::Parameter {
            declaration: parameters[8].semantic.clone(),
        },
        path: vec![],
        indices: vec![],
        root_shape: shape,
        byte_offset: 0,
        layout: geometry.sum_layout(ty(1)).unwrap(),
        pointer_slots: vec![],
        region_byte_size: 8,
        cases: vec![],
        effect,
        ownership: vec![],
    };
    let jump = LegalizedScalarTerminator::Jump {
        successor: LegalizedScalarSuccessor {
            edge: edge(1),
            target: block_id(2),
            bindings: vec![],
            work: vec![],
            structural_bindings: vec![LegalizedStructuralBinding {
                semantic: operation_operands::values::AbstractStructuralBinding {
                    parameter: place(211),
                    argument: argument(23, Owned),
                },
                indices: vec![],
            }],
            cleanup_calls: vec![(
                CallSiteOwner::CleanupAction {
                    edge: edge(1),
                    action_ordinal: 0,
                },
                call(vec![
                    call_argument(81, Owned, 0),
                    call_argument(76, Owned, 1),
                    transport(73, Owned, 1, stack, placement.clone()),
                    transport(74, Owned, 0, placement.clone(), indirect.clone()),
                    transport(75, Owned, 1, placement.clone(), indirect),
                    call_argument(77, SharedBorrow, 0),
                ]),
            )],
        },
        effect,
        ownership: vec![],
    };
    let terminal_call = LegalizedScalarTerminator::InvokeNoNormal {
        psi_edge: edge(3),
        call: call(vec![call_argument(88, SharedBorrow, 0)]),
        declared_result: operation_operands::values::AbstractFunctionResult::Unit,
        work: vec![],
        effect,
        ownership: vec![],
    };
    let function = LegalizedScalarFunction {
        machine: machine(),
        attachment: None,
        provenance: Default::default(),
        call_plan,
        parameters: vec![],
        entry_block: block_id(1),
        structural: Some(LegalizedStructuralContract {
            result: None,
            structural_types: types.clone().into(),
            parameters,
            structural_places: vec![],
            entry_claims: vec![],
            published_service_ceiling: vec![],
        }),
        blocks: vec![
            block(1, instructions, jump),
            block(2, vec![], dispatch),
            block(3, vec![], terminal_call),
        ],
    };
    let source = NativeFunction::new(&function, &geometry);
    let signature = source.structural.as_ref().unwrap();
    let required = parameter_homes(&source, signature);
    for (index, &(id, access, _, byte_size, needs_home)) in cases.iter().enumerate() {
        assert_eq!(required[index], needs_home, "entry for place {id}");
        let declaration = &signature.parameters[index].semantic;
        assert_eq!(
            owned_parameter_uses_local_home(&source, declaration, &required),
            (access == Owned).then_some(needs_home && byte_size != 0),
            "dispatch for place {id}"
        );
    }
    let mut mismatched = signature.parameters[8].semantic.clone();
    mismatched.place = place(99);
    assert_eq!(
        owned_parameter_uses_local_home(&source, &mismatched, &required),
        None
    );
}
