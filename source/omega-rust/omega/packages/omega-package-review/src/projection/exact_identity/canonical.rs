pub(crate) fn canonical_digest_label(kind: &str, digest: [u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut label = String::with_capacity(kind.len() + 1 + digest.len() * 2);
    label.push_str(kind);
    label.push(':');
    for byte in digest {
        let _ = write!(label, "{byte:02x}");
    }
    label
}

pub(crate) fn framed_identity(label: &str, children: &[String]) -> String {
    use std::fmt::Write as _;

    let mut identity = String::new();
    let _ = write!(identity, "{}:{label}", label.len());
    for child in children {
        let _ = write!(identity, "{}:{child}", child.len());
    }
    identity
}
