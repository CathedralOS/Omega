//! A scalar member of an owned local array's element (`cells[i].low`) reads
//! through the selector and the element's static member steps, as a
//! parameter array's flat element read does. Terminal verification resolves
//! the leaf from the local's own declaration, so a read naming another member
//! than the one authored is the verifier's to reject.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_psi::{OperationKind, StructuralPathSegment};

fn lowered(returned: &str) -> lowered_psi::LoweredPsi {
    let source = format!(
        "data Cell {{ high: u8; low: u8; }}
         machine pick(index: u64) -> u8 requires index < 2 {{
             let cells: [Cell; 2] = [Cell {{ high: 1, low: 2 }}, Cell {{ high: 3, low: 4 }}];
             {returned}
         }}"
    );
    let checked = crate::front_end::checked_program_result(&source)
        .unwrap_or_else(|errors| panic!("{errors:#?}"));
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("pick"))
        .unwrap_or_else(|error| panic!("{returned}: {error:?}"))
}

fn verify(module: &terminal_psi::TerminalModule, lowered: &lowered_psi::LoweredPsi) -> bool {
    let proof = terminal_codec::decode_proof_bundle(
        &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap(),
    )
    .unwrap();
    terminal_verifier::verify_module(module, &proof, &AdmissionProfile::default()).is_ok()
}

fn member_read(module: &mut terminal_psi::TerminalModule) -> &mut Vec<StructuralPathSegment> {
    module.machines[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::PrimitiveScalarRead { path, .. }
                if matches!(path.last(), Some(StructuralPathSegment::Field(_))) =>
            {
                Some(path)
            }
            _ => None,
        })
        .expect("the member read projects through the element")
}

#[test]
fn owned_local_element_members_read_through_the_selector() {
    for (returned, member) in [("cells[index].low", "low"), ("cells[1].high", "high")] {
        let lowered = lowered(returned);
        let mut module = terminal_codec::decode_module(
            &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        )
        .unwrap();
        assert!(verify(&module, &lowered), "{returned} verifies");
        let path = member_read(&mut module);
        assert_eq!(path.len(), 2, "{returned}: selector then member");
        assert_eq!(
            path.last(),
            Some(&StructuralPathSegment::Field(member.to_owned()))
        );
        // Naming a member the element does not declare cannot verify.
        *path.last_mut().unwrap() = StructuralPathSegment::Field("missing".to_owned());
        assert!(
            !verify(&module, &lowered),
            "{returned}: forged member rejects"
        );
    }
}
