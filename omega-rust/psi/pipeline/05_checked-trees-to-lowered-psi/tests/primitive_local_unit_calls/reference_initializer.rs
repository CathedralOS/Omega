//! Readable primitive inputs initialize distinct locals and immutable snapshots.

pub(super) fn source(scalar: &str) -> String {
    format!(
        "machine replace(destination: &mut {scalar}, value: {scalar}) {{
            destination = value;
        }}
        machine observe(output: &mut {scalar}, input: &{scalar},
            initial_output: &mut {scalar}, replacement: {scalar}) {{
            let mut scratch: {scalar} = input;
            let before: {scalar} = scratch;
            replace(&mut scratch, replacement);
            output = scratch;
            initial_output = before;
        }}"
    )
}
