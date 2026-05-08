pub(super) fn dispatch_label(machine: &str, state: &str) -> String {
    let mut label = String::from("omega_state_");
    label.push_str(&sanitize_label_part(machine));
    label.push('_');
    label.push_str(&sanitize_label_part(state));
    label
}

fn sanitize_label_part(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}
