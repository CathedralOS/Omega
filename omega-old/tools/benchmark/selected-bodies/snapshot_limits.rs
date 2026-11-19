// The scalar workload exceeded the default 4,194,304-element export allowance
// at 28,160,139 bytes. These inputs govern private benchmark snapshots only.
pub const ENVIRONMENT: &str = "OMEGA_SELECTED_BODY_SNAPSHOT_LIMITS";
pub const DEFAULT: &str = "268435456,2147483648,134217728,128";

pub fn parse(configuration: &str) -> Result<[usize; 4], String> {
    let mut fields = configuration.split(',');
    let mut values = [0; 4];
    for value in &mut values {
        *value = fields
            .next()
            .ok_or("snapshot limits require bytes,allocation,elements,depth")?
            .parse::<usize>()
            .map_err(|_| "snapshot limits must be positive integers")?;
    }
    if fields.next().is_some() {
        return Err("snapshot limits require exactly four fields".into());
    }
    let maxima = [
        512 * 1024 * 1024,
        4_usize * 1024 * 1024 * 1024,
        256 * 1024 * 1024,
        128,
    ];
    if values
        .iter()
        .zip(maxima)
        .any(|(value, maximum)| *value == 0 || *value > maximum)
    {
        return Err("snapshot limits exceed private maxima: 512 MiB bytes, 4 GiB allocation, 268435456 elements, depth 128".into());
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_remain_finite_and_complete() {
        assert_eq!(
            parse(DEFAULT).unwrap(),
            [268_435_456, 2_147_483_648, 134_217_728, 128]
        );
        for invalid in [
            "",
            "1,2,3",
            "1,2,3,4,5",
            "0,1,1,1",
            "1,1,1,129",
            "536870913,1,1,1",
            "-1,1,1,1",
        ] {
            assert!(parse(invalid).is_err(), "{invalid}");
        }
    }
}
