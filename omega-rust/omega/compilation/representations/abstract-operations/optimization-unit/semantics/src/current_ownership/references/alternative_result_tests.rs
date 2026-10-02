//! Independent current-IR reference replay controls. Mutated functions exercise
//! these receiving helpers, not a claim that the whole mutated unit is valid.

use super::*;
use crate::tests::indexed_byte_fields::indexed_field_unit;
use crate::tests::support::id;
use crate::unit_validation::references::validate_function_references;
use abstract_operations::AbstractFunctionResult;
use semantic_vocabulary::OperationId;
use terminal_psi::{
    StructuralMultiplicity, StructuralOperationResult, StructuralPathSegment,
    StructuralReferenceResultSource, StructuralResultDeclaration, StructuralTypeShape,
};

pub(super) fn selected_record() -> (
    PsiOptimizationFunction,
    Vec<StructuralTypeDeclaration>,
    StructuralOperationResult,
) {
    let unit = indexed_field_unit();
    let mut function = unit
        .functions
        .into_iter()
        .find(|function| function.machine == unit.entry)
        .unwrap();
    let mut declarations = unit.structural_types.to_vec();
    let record = id(9200, StructuralTypeId::new);
    let array = id(9201, StructuralTypeId::new);
    let carrier = id(9202, StructuralTypeId::new);
    declarations.extend([
        StructuralTypeDeclaration {
            id: record,
            identity: "Holder".into(),
            shape: StructuralTypeShape::Record { fields: vec![] },
        },
        StructuralTypeDeclaration {
            id: array,
            identity: "Holder[3]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: record,
                length: 3,
            },
        },
        StructuralTypeDeclaration {
            id: carrier,
            identity: "&mut Holder".into(),
            shape: StructuralTypeShape::Reference {
                referent: record,
                access: StructuralAccess::MutableBorrow,
            },
        },
    ]);
    let parameter = &mut function.structural_parameters[0];
    parameter.structural_type = array;
    parameter.access = StructuralAccess::MutableBorrow;
    parameter.multiplicity = StructuralMultiplicity::Unrestricted;
    parameter.qualifications.clear();
    parameter.projected_qualifications.clear();
    let root = parameter.place;
    function.entry_claim_declarations.clear();
    function.content_entry_claims.clear();
    function.blocks.truncate(1);
    function.blocks[0].structural_parameters.clear();
    // Keep one existing node shell only as current-IR test construction scratch.
    function.blocks[0].nodes.truncate(1);
    let result = StructuralOperationResult {
        place: id(9203, PlaceId::new),
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: vec![],
        projected_qualifications: vec![],
        qualification_establishments: vec![],
        claims: vec![],
    };
    let candidates = [0, 1].map(|position| StructuralArgument {
        place: root,
        path: vec![StructuralPathSegment::FixedIndex(position)],
        access: StructuralAccess::MutableBorrow,
    });
    function.result = AbstractFunctionResult::Structural(StructuralResultDeclaration {
        place: id(9204, PlaceId::new),
        structural_type: carrier,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: vec![],
        projected_qualifications: vec![],
        reference_sources: vec![StructuralReferenceResultSource {
            path: vec![],
            sources: candidates.to_vec(),
        }],
    });
    function.blocks[0].nodes[0].operation = O::EstablishReference {
        psi_operation: id(9205, OperationId::new),
        result: result.clone(),
        source: candidates[0].clone(),
    };
    (function, declarations, result)
}

pub(super) fn call(function: &PsiOptimizationFunction, result: &StructuralOperationResult) -> O {
    O::CallStructural {
        psi_operation: id(9210, OperationId::new),
        callee: function.machine,
        arguments: vec![],
        structural_arguments: vec![StructuralArgument {
            place: function.structural_parameters[0].place,
            path: vec![],
            access: StructuralAccess::MutableBorrow,
        }],
        dynamic_arguments: vec![],
        claim_transfers: vec![],
        result: AbstractStructuralCallResult::Structural {
            result: result.clone(),
            returned_claim_transfers: vec![],
            selected_evidence: vec![],
        },
        requirement_obligations: vec![],
        crash_continuations: vec![],
    }
}

#[test]
fn alternative_result_signature_replays_every_candidate() {
    let (function, declarations, _) = selected_record();
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    let check = |function: &PsiOptimizationFunction| {
        validate_function_references(
            function,
            &BTreeMap::from([(function.machine, function)]),
            &types,
        )
    };
    check(&function).unwrap();
    for corruption in [
        "empty",
        "reordered",
        "duplicate",
        "foreign",
        "outside",
        "shared",
        "leaf",
    ] {
        let mut changed = function.clone();
        let AbstractFunctionResult::Structural(result) = &mut changed.result else {
            unreachable!()
        };
        let mapping = &mut result.reference_sources[0];
        match corruption {
            "empty" => mapping.sources.clear(),
            "reordered" => mapping.sources.reverse(),
            "duplicate" => mapping.sources[1] = mapping.sources[0].clone(),
            "foreign" => mapping.sources[1].place = id(9300, PlaceId::new),
            "outside" => mapping.sources[1].path[0] = StructuralPathSegment::FixedIndex(3),
            "shared" => mapping.sources[1].access = StructuralAccess::SharedBorrow,
            "leaf" => mapping.path.push(StructuralPathSegment::FixedIndex(0)),
            _ => unreachable!(),
        }
        assert!(check(&changed).is_err(), "{corruption}");
    }
}

#[test]
fn alternative_result_returns_require_exact_reachable_coverage() {
    let (function, declarations, result) = selected_record();
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    let sources = &function.result.structural().unwrap().reference_sources[0].sources;
    let mut observed = vec![];
    for source in sources {
        let mut live = vec![];
        let operation = O::EstablishReference {
            psi_operation: id(9205, OperationId::new),
            result: result.clone(),
            source: source.clone(),
        };
        apply_operation(
            &function,
            &BTreeMap::new(),
            &types,
            function.entry,
            0,
            &operation,
            &mut live,
        )
        .unwrap();
        transfer_return(
            &function,
            &types,
            function.entry,
            result.place,
            &mut live,
            &mut observed,
        )
        .unwrap();
        assert!(live.is_empty());
    }
    validate_return_coverage(&function, &observed).unwrap();
    assert!(
        validate_return_coverage(&function, &observed[..1]).is_err(),
        "unused declared alternative"
    );
    assert!(
        validate_return_coverage(&function, &[]).is_err(),
        "no returning origin"
    );

    let mut foreign = sources[0].clone();
    foreign.path[0] = StructuralPathSegment::FixedIndex(2);
    let operation = O::EstablishReference {
        psi_operation: id(9205, OperationId::new),
        result: result.clone(),
        source: foreign,
    };
    let mut live = vec![];
    apply_operation(
        &function,
        &BTreeMap::new(),
        &types,
        function.entry,
        0,
        &operation,
        &mut live,
    )
    .unwrap();
    assert!(
        transfer_return(
            &function,
            &types,
            function.entry,
            result.place,
            &mut live,
            &mut vec![]
        )
        .is_err(),
        "undeclared same-type sibling"
    );
}

#[test]
fn alternative_call_keeps_one_carrier_and_suspends_each_possible_origin() {
    let (function, declarations, result) = selected_record();
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    let functions = BTreeMap::from([(function.machine, &function)]);
    let mut live = vec![];
    apply_operation(
        &function,
        &functions,
        &types,
        function.entry,
        0,
        &call(&function, &result),
        &mut live,
    )
    .unwrap();
    assert_eq!(live.len(), 1);
    let sources = &function.result.structural().unwrap().reference_sources[0].sources;
    for source in sources {
        assert!(normalized_source(&function, &types, function.entry, 1, &live, source).is_err());
    }
    let mut sibling = sources[0].clone();
    sibling.path[0] = StructuralPathSegment::FixedIndex(2);
    assert!(normalized_source(&function, &types, function.entry, 1, &live, &sibling).is_ok());
    let argument = StructuralArgument {
        place: result.place,
        path: vec![StructuralPathSegment::Referent],
        access: StructuralAccess::MutableBorrow,
    };
    assert_eq!(
        normalized_source(&function, &types, function.entry, 1, &live, &argument)
            .unwrap()
            .0,
        live[0].root
    );
    let mut observed = vec![];
    transfer_return(
        &function,
        &types,
        function.entry,
        result.place,
        &mut live,
        &mut observed,
    )
    .unwrap();
    validate_return_coverage(&function, &observed).unwrap();
}

#[test]
fn alternative_result_rejects_descendant_singleton_recursion() {
    let (mut function, declarations, result) = selected_record();
    let mut descendant = function.clone();
    descendant.machine = id(9400, MachineId::new);
    let AbstractFunctionResult::Structural(signature) = &mut descendant.result else {
        unreachable!()
    };
    signature.reference_sources[0].sources.truncate(1);
    function.blocks[0].nodes[0].operation = call(&descendant, &result);
    descendant.blocks[0].nodes[0].operation = call(&descendant, &result);
    let types = declarations.iter().map(|row| (row.id, row)).collect();
    let functions = BTreeMap::from([
        (function.machine, &function),
        (descendant.machine, &descendant),
    ]);
    assert!(validate_function_references(&function, &functions, &types).is_err());
    descendant.blocks[0].nodes.clear();
    let functions = BTreeMap::from([
        (function.machine, &function),
        (descendant.machine, &descendant),
    ]);
    validate_function_references(&function, &functions, &types).unwrap();
}
