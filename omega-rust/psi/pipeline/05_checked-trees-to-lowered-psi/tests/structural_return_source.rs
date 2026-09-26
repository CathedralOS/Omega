//! Fixtures shared by the structural return source tests.

#[path = "structural_return_source/boundary_arguments.rs"]
mod boundary_arguments;
#[path = "structural_return_source/claim_transfers_and_returns.rs"]
mod claim_transfers_and_returns;
#[path = "structural_return_source/projected_claims.rs"]
mod projected_claims;
#[path = "structural_return_source/projected_source_mutations.rs"]
mod projected_source_mutations;
#[path = "structural_return_source/service_reach.rs"]
mod service_reach;
#[path = "structural_return_source/unit_cleanup_custody.rs"]
mod unit_cleanup_custody;

use language_semantics::{Multiplicity, PermissionClaimIdentity};
use proof_admission::AdmissionProfile;
use terminal_codec::{decode_module, encode_module, encode_proof_section};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError, TerminalScalarValue,
    TerminalStructuralResult, TerminalStructuralValue,
};
use terminal_psi::Terminator;

const SOURCE: &str = r#"
    data ByteUnit {}
    data CountedQuantity<Unit> { magnitude: u64; }
    trait Content<A> {
        machine project(subject: &Self) -> A;
    }

    data Region [linear] { length: u64; }
    data Scratch { marker: u64; }
    data EmptyScratch {}
    data NominalScratch {}
    machine NominalScratch::drop(&mut self) {}
    domain Region::Owned;
    machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
    satisfies Content<CountedQuantity<ByteUnit>>::project
    {
        CountedQuantity { magnitude: region.length }
    }

    data Main {}
    machine Main::forward(region: Region in Owned) -> Region in Owned {
        region
    }
    machine Main::through_call(region: Region in Owned) -> Region in Owned {
        Main::forward(region)
    }
    machine Main::forward_and_drop(region: Region in Owned, scratch: Scratch) -> Region in Owned {
        region
    }
    machine Main::forward_with_local(region: Region in Owned) -> Region in Owned {
        let scratch: EmptyScratch = EmptyScratch {};
        region
    }
    machine Main::forward_with_local_and_drop(
        region: Region in Owned,
        scratch: Scratch
    ) -> Region in Owned {
        let local: EmptyScratch = EmptyScratch {};
        region
    }
    machine Main::local_partial_value(region: Region in Owned) -> Region in Owned {
        let scratch: Scratch = Scratch { marker: 1 };
        region
    }
    machine Main::local_nominal_cleanup(region: Region in Owned) -> Region in Owned {
        let scratch: NominalScratch = NominalScratch {};
        region
    }
    machine Main::forward_with_two_locals(region: Region in Owned) -> Region in Owned {
        let first: EmptyScratch = EmptyScratch {};
        let second: EmptyScratch = EmptyScratch {};
        region
    }
    machine Main::forward_and_drop_two(
        region: Region in Owned,
        first: Scratch,
        second: Scratch
    ) -> Region in Owned {
        region
    }
    machine Main::forward_with_local_and_drop_two(
        region: Region in Owned,
        first: Scratch,
        second: Scratch
    ) -> Region in Owned {
        let local: EmptyScratch = EmptyScratch {};
        region
    }
    machine Main::through_local(region: Region in Owned) -> Region in Owned {
        let forwarded: Region in Owned = region;
        forwarded
    }
    machine Main::contracted(region: Region in Owned) -> Region in Owned
    requires
        region in Region::Owned
    {
        region
    }
    machine Main::local_control(region: Region in Owned) -> Region in Owned {
        let scratch: EmptyScratch = EmptyScratch {};
        transition { _ -> next(region) }
        state next(region: Region in Owned) -> Region in Owned { region }
    }
    machine Main::main(&mut self) {}
"#;

const INDEXED_CUSTODY_SOURCE: &str = r#"
    boundary trait PortIo {}
    pub data Receipt [linear] { value: u64; }

    boundary machine Receipt::settle(self)
    reaches PortIo
    ensures true;

    data Root {}
    machine Root::enter(receipts: [Receipt; 2])
    reaches PortIo
    {
        Receipt::settle(receipts[0]);
        Receipt::settle(receipts[1]);
    }
"#;

const RESULT_BOUNDARY_CUSTODY_SOURCE: &str = r#"
    boundary trait PortIo {}
    pub data Receipt [linear] { value: u64; }

    boundary machine Receipt::settle(self) -> bool
    reaches PortIo
    ensures true;

    data Root {}
    machine Root::enter(receipt: Receipt) -> bool
    reaches PortIo
    {
        let accepted: bool = receipt.settle();
        accepted
    }
"#;

const RESULT_BOUNDARY_CONTENT_CUSTODY_SOURCE: &str = r#"
    data ByteUnit {}
    data CountedQuantity<Unit> { magnitude: u64; }
    trait Content<A> {
        machine project(subject: &Self) -> A;
    }

    pub data Region [linear] { length: u64; }
    domain Region::Owned;
    machine Owned::content(region: &Region) -> CountedQuantity<ByteUnit>
    satisfies Content<CountedQuantity<ByteUnit>>::project
    {
        CountedQuantity { magnitude: region.length }
    }

    boundary trait PortIo {}

    boundary machine Region::retire(self) -> bool
    reaches PortIo
    ensures true;

    boundary machine Region::discard(self)
    reaches PortIo;

    data Root {}
    machine Root::enter(region: Region in Owned) -> bool
    reaches PortIo
    {
        let accepted: bool = region.retire();
        accepted
    }

    machine Root::exit(region: Region in Owned)
    reaches PortIo
    {
        region.discard();
    }
"#;

const RESULT_BOUNDARY_BOUNDED_REACH_SOURCE: &str = r#"
    boundary trait MachineControl {}
    boundary trait PortIo {}

    boundary trait InterruptCompletion {
        machine complete() -> bool
        reaches <= MachineControl + PortIo;
    }

    data Root {}
    machine Root::enter() -> bool
    reaches InterruptCompletion + MachineControl + PortIo
    invokes InterruptCompletion;
    {
        let accepted: bool = InterruptCompletion::complete();
        accepted
    }
"#;

const ORDINARY_INDEXED_CUSTODY_SOURCE: &str = r#"
    boundary trait PortIo {}
    pub data Receipt [linear] { value: u64; }

    boundary machine Receipt::settle(self)
    reaches PortIo
    ensures true;

    data Helper {}
    machine Helper::run(receipt: Receipt)
    reaches PortIo
    {
        Receipt::settle(receipt);
    }

    data Root {}
    machine Root::enter(receipts: [Receipt; 2])
    reaches PortIo
    {
        Helper::run(receipts[0]);
        Helper::run(receipts[1]);
    }
"#;

const UNIT_AFFINE_LOCAL_SOURCE: &str = r#"
    data Empty {}
    data Token { value: u64; }
    data Root {}

    machine Root::cleanup(first: Token, second: Token) {
        let one: Empty = Empty {};
        let two: Empty = Empty {};
    }
"#;

const UNIT_AFFINE_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 3];
        values[0] = Empty {};
        values[1] = Empty {};
    }
"#;

const UNIT_AFFINE_WIDER_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 4];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
    }
"#;

const UNIT_AFFINE_DEEPER_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 5];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
    }
"#;

const UNIT_AFFINE_DEEPEST_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 6];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
    }
"#;

const UNIT_AFFINE_SEVEN_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 7];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
    }
"#;

const UNIT_AFFINE_EIGHT_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 8];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
    }
"#;

const UNIT_AFFINE_NINE_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 9];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
    }
"#;

const UNIT_AFFINE_TEN_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 10];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
    }
"#;

const UNIT_AFFINE_ELEVEN_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 11];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
    }
"#;

const UNIT_AFFINE_TWELVE_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 12];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
    }
"#;

const UNIT_AFFINE_THIRTEEN_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 13];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
    }
"#;

const UNIT_AFFINE_FOURTEEN_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 14];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
    }
"#;

const UNIT_AFFINE_FIFTEEN_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 15];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
        values[13] = Empty {};
    }
"#;

const UNIT_AFFINE_SIXTEEN_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 16];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
        values[13] = Empty {};
        values[14] = Empty {};
    }
"#;

const UNIT_AFFINE_SEVENTEEN_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 17];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
    }
"#;

const UNIT_AFFINE_EIGHTEEN_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 18];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
    }
"#;

const UNIT_AFFINE_NINETEEN_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 19];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
    }
"#;

const UNIT_AFFINE_TWENTY_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 20];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
    }
"#;

const UNIT_AFFINE_TWENTY_ONE_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 21];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
        values[19] = Empty {};
    }
"#;

const UNIT_AFFINE_TWENTY_TWO_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 22];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
        values[19] = Empty {};
        values[20] = Empty {};
    }
"#;

const UNIT_AFFINE_TWENTY_THREE_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 23];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
        values[19] = Empty {};
        values[20] = Empty {};
        values[21] = Empty {};
    }
"#;

const UNIT_AFFINE_TWENTY_FOUR_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 24];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
        values[19] = Empty {};
        values[20] = Empty {};
        values[21] = Empty {};
        values[22] = Empty {};
    }
"#;

const UNIT_AFFINE_TWENTY_FIVE_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 25];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
        values[19] = Empty {};
        values[20] = Empty {};
        values[21] = Empty {};
        values[22] = Empty {};
        values[23] = Empty {};
    }
"#;

const UNIT_AFFINE_TWENTY_SIX_CONSTRUCTION_PREFIX_SOURCE: &str = r#"
    data Empty {}
    data Root {}

    machine Root::cleanup_prefix() {
        let mut values: [Empty; 26];
        values[0] = Empty {};
        values[1] = Empty {};
        values[2] = Empty {};
        values[3] = Empty {};
        values[4] = Empty {};
        values[5] = Empty {};
        values[6] = Empty {};
        values[7] = Empty {};
        values[8] = Empty {};
        values[9] = Empty {};
        values[10] = Empty {};
        values[11] = Empty {};
        values[12] = Empty {};
        values[13] = Empty {};
        values[14] = Empty {};
        values[15] = Empty {};
        values[16] = Empty {};
        values[17] = Empty {};
        values[18] = Empty {};
        values[19] = Empty {};
        values[20] = Empty {};
        values[21] = Empty {};
        values[22] = Empty {};
        values[23] = Empty {};
        values[24] = Empty {};
    }
"#;

#[derive(Default)]
struct RejectSecondEffect {
    accepted: usize,
}

impl TerminalEffectHandler for RejectSecondEffect {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        if self.accepted == 1 {
            return Err(TerminalEffectRejection::new(
                "reject second indexed settlement",
            ));
        }
        self.accepted += 1;
        Ok(())
    }
}

struct ResultBoundaryHandler {
    reject: bool,
}

impl TerminalEffectHandler for ResultBoundaryHandler {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Ok(())
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<terminal_interpreter::TerminalEffectResult, TerminalEffectRejection> {
        if self.reject {
            return Err(TerminalEffectRejection::new("provider rejected settlement"));
        }
        assert!(matches!(
            effect,
            TerminalEffect::BoundaryCall {
                result: terminal_psi::BoundaryMachineResult::Scalar(
                    semantic_vocabulary::ScalarType::Boolean,
                ),
                ..
            }
        ));
        Ok(terminal_interpreter::TerminalEffectResult::Scalar(
            TerminalScalarValue::Boolean(true),
        ))
    }
}

fn checked_source() -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(SOURCE)
}

fn checked_result_boundary_source() -> checked_trees::CheckedTrees {
    crate::front_end::checked_program(RESULT_BOUNDARY_CUSTODY_SOURCE)
}
