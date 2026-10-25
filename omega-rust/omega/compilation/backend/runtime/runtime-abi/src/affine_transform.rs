//! The host C ABI of CoreGraphics' six-double affine record.
//! Guest declaration identities and storage remain with the Terminal adapter.

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostAffineTransform {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    #[link_name = "CGAffineTransformMake"]
    fn cg_affine_transform_make(
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        tx: f64,
        ty: f64,
    ) -> HostAffineTransform;
}

pub fn host_affine_transform(components: [f64; 6]) -> Option<HostAffineTransform> {
    #[cfg(target_os = "macos")]
    {
        let [a, b, c, d, tx, ty] = components;
        // SAFETY: CGFloat is C double on the supported 64-bit Mac hosts.
        // repr(C) retains the six ordered doubles and the actual indirect C
        // return. This constructor neither retains backing nor has partial
        // arithmetic preconditions; it returns the supplied coefficients.
        Some(unsafe { cg_affine_transform_make(a, b, c, d, tx, ty) })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = components;
        None
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn actual_c_indirect_result_retains_six_fields_and_earlier_call() {
        assert_eq!(std::mem::size_of::<HostAffineTransform>(), 48);
        assert_eq!(std::mem::align_of::<HostAffineTransform>(), 8);
        let first = host_affine_transform([1.25, -2.5, 3.75, -4.0, 5.5, -6.25]).unwrap();
        let second = host_affine_transform([-11.0, 12.5, -13.25, 14.75, -15.5, 16.0]).unwrap();
        assert_eq!(
            first,
            HostAffineTransform {
                a: 1.25,
                b: -2.5,
                c: 3.75,
                d: -4.0,
                tx: 5.5,
                ty: -6.25
            }
        );
        assert_eq!(
            second,
            HostAffineTransform {
                a: -11.0,
                b: 12.5,
                c: -13.25,
                d: 14.75,
                tx: -15.5,
                ty: 16.0
            }
        );
    }
}
