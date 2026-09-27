use crate::tests::termination::progress_mutation::CONTEXT_FIXTURE;

mod exposure;
mod transfer;

fn fixture_source(access: &str, body: &str, extra: &str) -> String {
    format!(
        "{CONTEXT_FIXTURE}
         data Carrier {{ context: &{access}Context; }}
         machine replace(mut carrier: Carrier, replacement: &Context) -> u64
         requires carrier.context.scheduler in WeakFair
         requires replacement.scheduler in WeakFair
         {{ {body} }}
         {extra}"
    )
}
