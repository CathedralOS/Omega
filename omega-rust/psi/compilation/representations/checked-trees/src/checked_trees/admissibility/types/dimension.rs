macro_rules! acceptance_dimensions {
    ($( { $variant:ident => $name:literal } ),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum AcceptanceDimension {
            $( $variant ),+
        }

        impl AcceptanceDimension {
            pub const ALL: [Self; acceptance_dimensions!(@count $( $variant )+)] = [
                $( Self::$variant ),+
            ];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $name ),+
                }
            }
        }
    };
    (@count $( $variant:ident )+) => {
        <[()]>::len(&[$(acceptance_dimensions!(@unit $variant)),+])
    };
    (@unit $variant:ident) => { () };
}

acceptance_dimensions! {
    { Borrow => "borrow" },
    { Proof => "proof" },
    { ServiceReach => "service_reach" },
    { Suspension => "suspension" },
    { Blocking => "blocking" },
    { Boundaries => "boundaries" },
    { Termination => "termination" },
}
