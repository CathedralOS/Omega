use super::*;
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationAccountingError,
    EvaluationMeasurementIdentity, EvaluationResource, TerminalStorageScheduleIdentity,
};

fn sponsor(cells: u64) -> BuildEvaluationSponsor {
    BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(100, 8, 2, 2, cells, 100, 100, 100).unwrap(),
        EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        },
    )
    .unwrap()
}

#[test]
fn payload_is_funded_before_reading_elements_and_refusal_preserves_inspection() {
    let sponsor = sponsor(2);
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let mut value = PackedScalarArrayValue::collect(
        StructuralTypeId::new(1).unwrap(),
        Vec::new(),
        2,
        [TerminalScalarValue::Boolean(false); 2].map(Ok),
        Some(&account),
    )
    .unwrap();
    let inspected = value.clone();
    assert_eq!(sponsor.live_storage_cells(), 2);
    assert!(matches!(
        value.set_element(0, TerminalScalarValue::Boolean(true)),
        Err(TerminalInterpretError::Storage(
            EvaluationAccountingError::AllowanceExhausted(EvaluationResource::LiveStorage)
        ))
    ));
    assert_eq!(value.elements().unwrap(), inspected.elements().unwrap());
    assert_eq!(sponsor.live_storage_cells(), 2);
    assert!(value.snapshot().is_err());
    let consumed = std::cell::Cell::new(false);
    assert!(
        ScalarArrayStorage::collect(
            1,
            std::iter::once_with(|| {
                consumed.set(true);
                Ok(TerminalScalarValue::Boolean(true))
            }),
            Some(&account)
        )
        .is_err()
    );
    assert!(
        !consumed.get(),
        "refusal must precede payload evaluation/allocation"
    );
    drop(inspected);
    value
        .set_element(0, TerminalScalarValue::Boolean(true))
        .unwrap();
    assert_eq!(sponsor.live_storage_cells(), 2, "unique writes do not copy");
    drop(value);
    assert_eq!(sponsor.live_storage_cells(), 0);
}

#[test]
fn snapshots_and_detached_writes_own_their_overlap_until_last_observation() {
    let sponsor = sponsor(4);
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let mut value = PackedScalarArrayValue::collect(
        StructuralTypeId::new(1).unwrap(),
        Vec::new(),
        2,
        [TerminalScalarValue::Boolean(false); 2].map(Ok),
        Some(&account),
    )
    .unwrap();
    value.storage_identity = Some(7);
    let copied = value.snapshot().unwrap();
    assert_eq!(copied.storage_identity, None);
    assert_eq!(copied.elements(), value.elements().unwrap());
    assert_eq!(sponsor.live_storage_cells(), 4);
    drop(copied);
    let inspected = value.clone();
    value
        .set_element(0, TerminalScalarValue::Boolean(true))
        .unwrap();
    assert_eq!(value.storage_identity, Some(7));
    assert_eq!(
        inspected.elements().unwrap(),
        [TerminalScalarValue::Boolean(false); 2]
    );
    assert_eq!(sponsor.live_storage_cells(), 4);
    assert_eq!(sponsor.peak_live_cells(), 4);
    drop(value);
    assert_eq!(sponsor.live_storage_cells(), 2);
    drop(inspected);
    assert_eq!(sponsor.live_storage_cells(), 0);
}

#[test]
fn empty_and_incomplete_payloads_do_not_leak_reservations() {
    let sponsor = sponsor(2);
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let empty = ScalarArrayStorage::<TerminalScalarValue>::collect(0, [], Some(&account)).unwrap();
    assert!(empty.elements().is_empty());
    assert_eq!(sponsor.live_storage_cells(), 0);
    assert!(ScalarArrayStorage::<TerminalScalarValue>::collect(2, [], Some(&account)).is_err());
    assert_eq!(sponsor.live_storage_cells(), 0);
    assert!(
        ScalarArrayStorage::collect(0, [Ok(TerminalScalarValue::Boolean(false))], Some(&account))
            .is_err()
    );
    assert_eq!(sponsor.live_storage_cells(), 0);
}

#[derive(Debug)]
struct OriginalCell {
    value: std::cell::Cell<TerminalScalarValue>,
    faulted: std::cell::Cell<bool>,
}

impl crate::scalar_storage::TerminalScalarBacking for OriginalCell {
    fn read(&self) -> Result<TerminalScalarValue, crate::effects::TerminalEffectRejection> {
        if self.faulted.get() {
            return Err(crate::effects::TerminalEffectRejection::new(
                "original cell unavailable",
            ));
        }
        Ok(self.value.get())
    }

    fn write(
        &self,
        value: TerminalScalarValue,
    ) -> Result<(), crate::effects::TerminalEffectRejection> {
        if self.faulted.get() {
            return Err(crate::effects::TerminalEffectRejection::new(
                "original cell unavailable",
            ));
        }
        self.value.set(value);
        Ok(())
    }
}

fn byte(value: u8) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(
            semantic_vocabulary::IntegerSign::Unsigned,
            8,
        )
        .unwrap(),
        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(value)),
    }
}

fn original(value: u8) -> std::rc::Rc<OriginalCell> {
    std::rc::Rc::new(OriginalCell {
        value: std::cell::Cell::new(byte(value)),
        faulted: std::cell::Cell::new(false),
    })
}

fn array() -> PackedScalarArrayValue {
    PackedScalarArrayValue::collect(
        StructuralTypeId::new(1).unwrap(),
        Vec::new(),
        2,
        [Ok(byte(7)), Ok(byte(1))],
        None,
    )
    .unwrap()
}

#[test]
fn resident_mutation_detaches_local_cells_but_preserves_the_original_endpoint() {
    let mut value = array();
    let home = original(7);
    value
        .install_resident(0, ScalarCell::resident(byte(7), home.clone()).unwrap())
        .unwrap();
    value.storage_identity = Some(19);
    let snapshot = value.snapshot().unwrap();
    let inspected = value.clone();
    value.set_element(0, byte(4)).unwrap();
    value.set_element(1, byte(9)).unwrap();
    assert_eq!(value.storage_identity, Some(19));
    assert_eq!(snapshot.storage_identity, None);
    assert_eq!(snapshot.elements(), [byte(7), byte(1)]);
    assert_eq!(inspected.elements().unwrap(), [byte(4), byte(1)]);
    assert_eq!(value.elements().unwrap(), [byte(4), byte(9)]);
    assert_eq!(value.cell(0), inspected.cell(0));
    home.value.set(byte(23));
    assert_eq!(value.element(0).unwrap(), Some(byte(23)));
    assert_eq!(inspected.element(0).unwrap(), Some(byte(23)));
    assert_eq!(snapshot.elements()[0], byte(7));
    assert_eq!(value.element(2).unwrap(), None);
    assert!(value.set_element(2, byte(1)).is_err());
    assert!(
        value
            .set_element(0, TerminalScalarValue::Boolean(true))
            .is_err()
    );
    assert_eq!(home.value.get(), byte(23));
}

#[test]
fn resident_installation_cannot_redirect_or_change_an_established_cell() {
    let mut value = array();
    let home = original(7);
    assert!(
        value
            .install_resident(0, ScalarCell::Local(byte(7)))
            .is_err()
    );
    assert!(
        value
            .install_resident(0, ScalarCell::resident(byte(9), original(9)).unwrap())
            .is_err()
    );
    let resident = ScalarCell::resident(byte(7), home.clone()).unwrap();
    value.install_resident(0, resident.clone()).unwrap();
    value.install_resident(0, resident).unwrap();
    assert!(
        value
            .install_resident(0, ScalarCell::resident(byte(7), original(7)).unwrap())
            .is_err()
    );
    value.set_element(0, byte(31)).unwrap();
    assert_eq!(home.value.get(), byte(31));
}

#[test]
fn resident_preflight_funds_detachment_without_publishing_contents() {
    let sponsor = sponsor(4);
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let mut value = PackedScalarArrayValue::collect(
        StructuralTypeId::new(1).unwrap(),
        Vec::new(),
        2,
        [Ok(byte(7)), Ok(byte(1))],
        Some(&account),
    )
    .unwrap();
    let inspected = value.clone();
    let home = original(7);
    let resident = ScalarCell::resident(byte(7), home.clone()).unwrap();
    value.prepare_resident_install(0, &resident).unwrap();
    assert_eq!(sponsor.live_storage_cells(), 4);
    assert!(matches!(value.cell(0), Some(ScalarCell::Local(_))));
    assert!(matches!(inspected.cell(0), Some(ScalarCell::Local(_))));
    assert_eq!(value.elements().unwrap(), inspected.elements().unwrap());
    // The allowance is full. Installation must use the already funded cells.
    value.install_resident(0, resident).unwrap();
    assert_eq!(sponsor.live_storage_cells(), 4);
    home.value.set(byte(47));
    assert_eq!(value.element(0).unwrap(), Some(byte(47)));
    assert_eq!(inspected.element(0).unwrap(), Some(byte(7)));
    let further_inspection = value.clone();
    let same_resident = value.cell(0).unwrap().clone();
    assert!(matches!(
        value.prepare_resident_install(0, &same_resident),
        Err(TerminalInterpretError::Storage(
            EvaluationAccountingError::AllowanceExhausted(EvaluationResource::LiveStorage)
        ))
    ));
    assert_eq!(value.cell(0), further_inspection.cell(0));
    assert_eq!(value.element(0).unwrap(), Some(byte(47)));
    assert_eq!(sponsor.live_storage_cells(), 4);
    drop(further_inspection);
    drop(inspected);
    drop(value);
    assert_eq!(sponsor.live_storage_cells(), 0);
}

#[test]
fn retained_packed_cells_keep_the_scalar_wire_format_and_decode_as_local_snapshots() {
    let mut value = array();
    let home = original(7);
    value
        .install_resident(0, ScalarCell::resident(byte(7), home.clone()).unwrap())
        .unwrap();
    home.value.set(byte(37));
    let limits = compiler_storage::RecordLimits::default();
    let portable = value.snapshot().unwrap();
    let encoded = compiler_storage::encode(portable.elements.as_ref(), limits).unwrap();
    assert_eq!(
        encoded,
        compiler_storage::encode(&vec![byte(37), byte(1)], limits).unwrap()
    );
    let restored: ScalarArrayStorage<TerminalScalarValue> =
        compiler_storage::decode(&encoded, limits).unwrap();
    home.value.set(byte(41));
    assert_eq!(restored.elements(), [byte(37), byte(1)]);
    assert_eq!(value.element(0).unwrap(), Some(byte(41)));
    home.faulted.set(true);
    assert!(value.elements().is_err());
    assert!(value.element(0).is_err());
    assert!(value.snapshot().is_err());
}

#[test]
fn byte_observation_borrows_live_packed_cells_and_propagates_original_read_failure() {
    let mut value = array();
    let home = original(7);
    value
        .install_resident(0, ScalarCell::resident(byte(7), home.clone()).unwrap())
        .unwrap();
    let bytes = crate::TerminalByteRead::cells(value.cells()).unwrap();
    assert_eq!(bytes.len(), 2);
    assert_eq!(bytes.read(0).unwrap(), 7);
    home.value.set(byte(43));
    assert_eq!(bytes.read(0).unwrap(), 43);
    let mut output = [0; 2];
    bytes.copy_to(&mut output).unwrap();
    assert_eq!(output, [43, 1]);
    home.faulted.set(true);
    assert!(bytes.read(0).is_err());
    assert!(bytes.copy_to(&mut output).is_err());
    assert!(crate::TerminalByteRead::cells(value.cells()).is_err());
    assert_eq!(bytes.len(), 2);
}

#[test]
fn portable_array_transfer_reuses_its_lease_and_runtime_snapshot_is_send() {
    fn assert_send_sync<Value: Send + Sync>() {}
    assert_send_sync::<TerminalScalarArrayValue>();
    assert_send_sync::<TerminalScalarArrayResult>();
    let sponsor = sponsor(4);
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let owned = TerminalScalarArrayValue::collect(
        StructuralTypeId::new(1).unwrap(),
        Vec::new(),
        2,
        [Ok(byte(7)), Ok(byte(1))],
        Some(&account),
    )
    .unwrap();
    assert_eq!(sponsor.live_storage_cells(), 2);
    let mut runtime = owned.into_runtime().unwrap();
    assert_eq!(
        sponsor.live_storage_cells(),
        2,
        "unique transfer retains its lease"
    );
    let home = original(7);
    runtime
        .install_resident(0, ScalarCell::resident(byte(7), home.clone()).unwrap())
        .unwrap();
    home.value.set(byte(59));
    runtime.storage_identity = Some(29);
    let portable = runtime.snapshot().unwrap();
    assert_eq!(sponsor.live_storage_cells(), 4);
    assert_eq!(portable.storage_identity, None);
    assert_eq!(portable.elements(), [byte(59), byte(1)]);
    let bytes =
        compiler_storage::encode(&portable, compiler_storage::RecordLimits::default()).unwrap();
    let restored: TerminalScalarArrayValue =
        compiler_storage::decode(&bytes, compiler_storage::RecordLimits::default()).unwrap();
    home.value.set(byte(61));
    assert_eq!(portable.elements(), restored.elements());
    assert_eq!(restored.elements()[0], byte(59));
    assert_eq!(runtime.element(0).unwrap(), Some(byte(61)));
    drop(runtime);
    assert_eq!(sponsor.live_storage_cells(), 2);
    drop(portable);
    assert_eq!(sponsor.live_storage_cells(), 0);
}
