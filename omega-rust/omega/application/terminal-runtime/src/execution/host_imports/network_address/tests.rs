use super::{AddressValues, OwnedAddress};
use runtime_abi::HostInternetAddress;
use terminal_psi::StructuralMultiplicity;

#[test]
fn stored_application_and_multiplicity_are_checked_before_consumption() {
    let application = [7; 32];
    for multiplicity in [
        StructuralMultiplicity::Affine,
        StructuralMultiplicity::Linear,
        StructuralMultiplicity::Unrestricted,
    ] {
        let mut values = AddressValues {
            values: vec![Some(OwnedAddress {
                application,
                multiplicity,
                value: HostInternetAddress::make(10, 66051).unwrap(),
            })],
        };
        assert!(values.argument(0, [8; 32], multiplicity).is_err());
        for other in [
            StructuralMultiplicity::Affine,
            StructuralMultiplicity::Linear,
            StructuralMultiplicity::Unrestricted,
        ] {
            if other != multiplicity {
                assert!(values.argument(0, application, other).is_err());
            }
        }
        assert_eq!(
            values
                .argument(0, application, multiplicity)
                .unwrap()
                .host_number(),
            Some(66051)
        );
        let again = values.argument(0, application, multiplicity);
        if multiplicity != StructuralMultiplicity::Unrestricted {
            assert!(again.is_err());
            // A later result does not recycle the consumed ordinal.
            values.values.push(Some(OwnedAddress {
                application,
                multiplicity,
                value: HostInternetAddress::make(10, 1027).unwrap(),
            }));
            assert!(values.argument(0, application, multiplicity).is_err());
            assert_eq!(
                values
                    .argument(1, application, multiplicity)
                    .unwrap()
                    .host_number(),
                Some(1027)
            );
        } else {
            assert_eq!(again.unwrap().host_number(), Some(66051));
        }
    }
}
