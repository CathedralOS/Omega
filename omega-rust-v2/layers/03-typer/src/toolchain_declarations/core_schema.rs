//! Public reflection and access-policy data used in Omega package signatures.
use super::core_interface::{CoreInterface, FieldType::*, Shape};

const CALLABLE_IDENTITY: &[(&str, super::core_interface::FieldType)] = &[
    ("declaration_identity", Text),
    ("callable_identity", Text),
    ("source_revision", Text),
];

pub(super) fn define(core: &mut CoreInterface) {
    core.data("reflect::TypeSchema", true, true, &[("storage", Text)]);
    core.data("reflect::CallableInfo", true, true, &[]);
    core.case("reflect::CallableInfo", "Machine", CALLABLE_IDENTITY);
    core.case("reflect::CallableInfo", "Requirement", CALLABLE_IDENTITY);
    core.data(
        "reflect::FieldInfo",
        true,
        false,
        &[
            ("member_identity", Text),
            ("case_identity", Text),
            ("owner_identity", Text),
            ("owner_application", Text),
            ("name", Text),
            ("qualified_type", Text),
            ("declaration_position", Natural),
            ("has_stable_number", Boolean),
            ("is_erased", Boolean),
            ("stable_number", Natural),
            ("source_revision", Text),
        ],
    );
    core.data("Exposure", false, false, &[]);
    for case in ["Exported", "BindingPrivate"] {
        core.case("Exposure", case, &[]);
    }
    core.data("ExternalRead", false, false, &[]);
    for case in ["None", "Read", "Take"] {
        core.case("ExternalRead", case, &[]);
    }
    core.data(
        "AtomicOperations",
        false,
        true,
        &[
            ("load", Boolean),
            ("store", Boolean),
            ("fetch_add", Boolean),
            ("fetch_sub", Boolean),
            ("fetch_xor", Boolean),
            ("fetch_or", Boolean),
            ("fetch_and", Boolean),
            ("swap", Boolean),
            ("compare_exchange", Boolean),
            ("compare_exchange_once", Boolean),
            ("try_exchange", Boolean),
            ("try_exchange_once", Boolean),
        ],
    );
    core.data("FieldAccess", false, false, &[]);
    core.case("FieldAccess", "Inaccessible", &[]);
    core.case(
        "FieldAccess",
        "Stable",
        &[
            ("read", Boolean),
            ("write", Boolean),
            ("exposure", Named("Exposure")),
        ],
    );
    core.case(
        "FieldAccess",
        "External",
        &[
            ("read", Named("ExternalRead")),
            ("write", Boolean),
            ("exposure", Named("Exposure")),
        ],
    );
    core.case(
        "FieldAccess",
        "Atomic",
        &[
            ("operations", Named("AtomicOperations")),
            ("exposure", Named("Exposure")),
        ],
    );
    core.data(
        "AccessFieldEntry",
        true,
        false,
        &[
            ("field", Applied("reflect::FieldInfo")),
            ("access", Named("FieldAccess")),
        ],
    );
    core.data("BoundaryReach", false, false, &[("services", Array(&Natural, 32)), ("service_count", Natural)]);
    core.data("Issuance", true, false, &[]);
    core.case("Issuance", "Only", &[("routes", Slice(&Applied("reflect::CallableInfo")))]);
    core.case("Issuance", "Unrestricted", &[]);
    core.data("AccessRegion", true, false, &[]);
    core.case("AccessRegion", "Subject", &[("schema", Applied("reflect::TypeSchema"))]);
    core.case("AccessRegion", "Field", &[("field", Applied("reflect::FieldInfo"))]);
    core.data(
        "AccessCallerRule",
        true,
        false,
        &[
            ("region", Applied("AccessRegion")),
            ("callers", Slice(&Applied("reflect::CallableInfo"))),
        ],
    );
    core.data(
        "AccessPlan",
        true,
        false,
        &[
            ("schema", Applied("reflect::TypeSchema")),
            ("issuance", Applied("Issuance")),
            ("fields", Array(&Applied("AccessFieldEntry"), 32)),
            ("field_count", Natural),
            ("readers", Array(&Applied("AccessCallerRule"), 32)),
            ("reader_count", Natural),
            ("writers", Array(&Applied("AccessCallerRule"), 32)),
            ("writer_count", Natural),
            ("reach", Named("BoundaryReach")),
        ],
    );
    core.add("AccessPolicy", omega_parser::NodeKind::Trait, false, Shape::Trait);
    core.machine("AccessPolicy::plan", true, &[Applied("reflect::TypeSchema")], Applied("AccessPlan"));
    core.machine(
        "access::new_plan",
        true,
        &[Shared(&Applied("reflect::TypeSchema")), Applied("Issuance")],
        Applied("AccessPlan"),
    );
    core.machine("Nat::Descending", false, &[Natural], Natural);
    core.machine("Nat::BoundedDistance", false, &[Natural, Natural], Natural);
}
