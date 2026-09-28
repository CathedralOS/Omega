use crate::tests::termination::progress_mutation::assert_subjects;
use crate::tests::termination::progress_mutation::fixture_with_extra;

#[test]
fn constructed_record_does_not_lend_a_sibling_field_its_premise() {
    let program = fixture_with_extra(
        "let holder: Holder = Holder { scheduler: replacement.scheduler, spare: context.scheduler };
         context.scheduler = holder.spare;",
        true,
        false,
        "data Holder { scheduler: SchedulerHandle; spare: SchedulerHandle; }",
    );
    assert_subjects(&program, &["context"]);
}
