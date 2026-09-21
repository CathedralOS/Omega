//! The projected-receiver ranked-callee pin: `Engine::run` calls the
//! ranked `Cursor::step` through `self.cursor`, borrowing the field as the
//! callee's whole receiver. Source checking admits the composition and the
//! interpreted run observes the callee's write on the projected referent.

use super::fixture_roster;
use crate::{CheckedCompileRequest, compile_reviewed_repository_fixture, interpret, pass_canary};

#[test]
fn ranked_callee_projected_receiver_compiles_and_observes_the_projected_write() {
    let canary = pass_canary(fixture_roster::RANKED_CALLEE_PROJECTED_RECEIVER_COMPILE);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("ranked callee on a projected receiver should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.error, None,
        "ranked callee on a projected receiver should interpret"
    );
    assert_eq!(
        interpreted.exit_code, 70,
        "the callee's write must be visible on the caller's projected referent"
    );
}
