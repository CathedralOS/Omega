use crate::tests::termination::progress_mutation::aliases::carriers::borrowed::assert_input_premise;
use crate::tests::termination::progress_mutation::aliases::carriers::borrowed::source;
use crate::tests::termination::progress_mutation::check_source;

mod adversarial;
mod selectors;

fn direct_source(access: &str, body: &str) -> String {
    source(
        access,
        "",
        "let borrowed: &Context = select(carrier);
         transition { _ -> wait_context(borrowed) }",
        &format!("machine select(carrier: &{access}Carrier) -> &Context {{ {body} }}"),
    )
}

#[test]
fn a_direct_result_can_load_an_owned_input_carriers_reference() {
    let source =
        direct_source("", "carrier.context").replace("carrier: &Carrier)", "carrier: Carrier)");
    assert_input_premise(&check_source(&source));
}
