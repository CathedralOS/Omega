//! A callback takes Views<S> by value. Its two slots each store a shared reference.
use super::*;

#[test]
fn a_record_of_references_keeps_its_witness_access_and_extent() {
    let mut program = readers();
    let mut names = std::mem::take(&mut program.identities).into_builder();
    let views = names.intern("declaration of Views");
    let slots = names.intern("slots");
    let slots_declaration = names.intern("declaration of Views.slots");
    for (witness, name, reference, array, record) in
        [(2, "hidden", 8, 9, 10), (4, "File", 11, 12, 13)]
    {
        let argument = format!("named(name({name}))");
        program.types.structural_types[witness - 1].identity = names.intern(&argument);
        let mut reference_row = byte_view_descriptor(reference, None);
        reference_row.identity = names.intern(&format!("reference to {name}"));
        reference_row.shape = StructuralTypeShape::Reference {
            referent: ty(witness as u64),
            access: StructuralAccess::SharedBorrow,
        };
        let mut array_row = byte_view_descriptor(array, None);
        array_row.identity = names.intern(&format!("two references to {name}"));
        array_row.shape = StructuralTypeShape::FixedArray {
            element: ty(reference),
            length: terminal_psi::ArrayExtent::Literal(2),
        };
        let mut field = declaration(record, StructuralFieldType::Structural(ty(array)));
        field.identity = slots;
        field.declaration_identity = slots_declaration;
        let mut record_row = byte_view_descriptor(record, None);
        record_row.identity = names.intern(&format!("generic(name(Views),{argument})"));
        record_row.declaration_identity = views;
        record_row.shape = StructuralTypeShape::Record {
            fields: vec![field],
            storage: StructuralRecordStorage::Packed,
        };
        program
            .types
            .structural_types
            .extend([reference_row, array_row, record_row]);
        let contract = &mut program.execution.callables.contracts[witness - 1];
        let mut parameter = param(1, ty(record));
        parameter.access = StructuralAccess::Owned;
        contract.structural_parameters = vec![parameter];
        contract.proof.structural_parameters = contract.structural_parameters.clone();
    }
    program.identities = names.build();
    let admitted = |program: &TerminalModule| {
        crate::packages::opened_callable_admits(program, &[ty(2)], callable(2), callable(4))
    };
    assert!(admitted(&program));
    for (index, shape) in [
        (
            10,
            StructuralTypeShape::Reference {
                referent: ty(5),
                access: StructuralAccess::SharedBorrow,
            },
        ),
        (
            10,
            StructuralTypeShape::Reference {
                referent: ty(4),
                access: StructuralAccess::MutableBorrow,
            },
        ),
        (
            11,
            StructuralTypeShape::FixedArray {
                element: ty(11),
                length: terminal_psi::ArrayExtent::Literal(1),
            },
        ),
    ] {
        let mut crossed = program.clone();
        crossed.types.structural_types[index].shape = shape;
        assert!(!admitted(&crossed));
    }
    // Keeping both layouts equal cannot authorize inline storage of the witness.
    for (row, witness) in [(8, 2), (11, 4)] {
        program.types.structural_types[row].shape = StructuralTypeShape::FixedArray {
            element: ty(witness),
            length: terminal_psi::ArrayExtent::Literal(2),
        };
    }
    assert!(!admitted(&program));
}

#[test]
fn two_constructions_store_arrays_of_their_own_references() {
    let mut program = readers();
    add_two_constructions(&mut program);
    program.machines[0].blocks[0].operations.drain(..2);
    program.execution.packages.sites.drain(..2);
    for (reference, array) in [(6, 10), (8, 11), (9, 12)] {
        let reference_row = &mut program.types.structural_types[reference - 1];
        let StructuralTypeShape::Reference { access, .. } = &mut reference_row.shape else {
            panic!("reader state must be a reference")
        };
        *access = StructuralAccess::SharedBorrow;
        let mut row = byte_view_descriptor(array, None);
        row.shape = StructuralTypeShape::FixedArray {
            element: ty(reference as u64),
            length: terminal_psi::ArrayExtent::Literal(2),
        };
        program.types.structural_types.push(row);
    }
    let StructuralTypeShape::Record { fields, .. } = &mut program.types.structural_types[6].shape
    else {
        panic!("reader must be a record")
    };
    fields[0].field_type = StructuralFieldType::Structural(ty(10));
    let formals = &mut program.machines[0].structural_parameters;
    formals[2].structural_type = ty(11);
    formals[3].structural_type = ty(12);
    check_package_sites(&program)
        .expect("both readers store two references to their chosen witness");
    program.machines[0].structural_parameters[3].structural_type = ty(11);
    assert_eq!(
        check_package_sites(&program).err(),
        Some("package field differs from the shared witness substitution")
    );
}
