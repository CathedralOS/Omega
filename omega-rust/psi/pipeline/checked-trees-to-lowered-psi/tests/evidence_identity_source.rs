//! Fixtures shared by the evidence identity source tests.

#[path = "evidence_identity_source/runtime_unit_proof_outputs.rs"]
mod runtime_unit_proof_outputs;
#[path = "evidence_identity_source/scalar_lanes_and_callee_identities.rs"]
mod scalar_lanes_and_callee_identities;
#[path = "evidence_identity_source/source_projections.rs"]
mod source_projections;

const FORWARDED_SOURCE: &str = r#"
    trait Evidence<T> {
        machine witness(value: T);
    }

    proposition ready<T>() evidence Evidence<T>;

    data Token { value: u64; }
    data Root {}
    machine Root::forward(first: Token, second: Token)
    requires incoming_first: ready<i32>()
    requires incoming_second: ready<i32>()
    ensures outgoing_first: ready<i32>()
    ensures outgoing_second: ready<i32>()
    {
        outgoing_first = incoming_first;
        outgoing_second = incoming_second;
    }
"#;

const PRODUCED_SOURCE: &str = r#"
    trait Evidence<T> {
        machine witness(value: T);
    }

    proposition ready<T>() evidence Evidence<T>;

    ConcreteEvidence: satisfies Evidence<i32> {
        machine witness(value: i32) {}
    }

    data Token { value: u64; }
    data Root {}
    machine Root::produce(first: Token, second: Token)
    ensures
        outgoing: ready<i32>()
    {
        outgoing = ConcreteEvidence;
    }
"#;

const EMPTY_PRODUCER_SOURCE: &str = r#"
    trait Evidence<T> {}

    proposition ready<T>() evidence Evidence<T>;

    ConcreteEvidence: satisfies Evidence<i32> {}

    data Root {}
    machine Root::produce()
    ensures outgoing: ready<i32>()
    {
        outgoing = ConcreteEvidence;
    }
"#;

const PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::produce()
    ensures outgoing: ready()
    {
        outgoing = ConcreteEvidence;
    }

    machine Root::relay()
    ensures relayed: ready()
    {
        let (; outgoing: local) = Root::produce();
        relayed = local;
    }
"#;

const ARGUMENTED_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition carries(value: i32) evidence Evidence;
    data Root {}

    machine Root::produce(value: i32)
    requires incoming: carries(value)
    ensures copied: carries(value)
    {
        copied = incoming;
    }

    machine Root::relay()
    requires source: carries(7)
    ensures relayed: carries(7)
    {
        let (; copied: local) = Root::produce(7; source);
        relayed = local;
    }
"#;

const STATIC_REQUIREMENT_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;

    trait Producer {
        machine Self::produce(&self)
        requires public_in: ready()
        ensures public_out: ready();
    }

    data Token {}

    TokenProducer: Token satisfies Producer {
        machine produce(&self)
        requires local_in: ready()
        ensures public_out: ready()
        ensures private_out: ready()
        {
            public_out = local_in;
            private_out = local_in;
        }
    }

    data Root {}

    machine Root::invoke<Element, Order: Element satisfies Producer>(
        &self,
        value: &Element
    )
    requires incoming: ready()
    {
        let (; public_out: result) = Order::produce(value; incoming);
    }

    machine Root::caller(&self, value: &Token)
    requires incoming: ready()
    {
        self.invoke<Token, TokenProducer>(value; incoming);
    }
"#;

const PLURAL_STATIC_REQUIREMENT_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;

    trait Producer {
        machine Self::produce(&self)
        requires public_first: ready()
        requires public_second: ready()
        ensures first_copy: ready()
        ensures second_copy: ready()
        ensures third_copy: ready();
    }

    data Token {}

    TokenProducer: Token satisfies Producer {
        machine produce(&self)
        requires local_first: ready()
        requires local_second: ready()
        ensures first_copy: ready()
        ensures second_copy: ready()
        ensures third_copy: ready()
        {
            first_copy = local_first;
            second_copy = local_first;
            third_copy = local_second;
        }
    }

    data Root {}

    machine Root::invoke<Element, Order: Element satisfies Producer>(
        &self,
        value: &Element
    )
    requires incoming_first: ready()
    requires incoming_second: ready()
    {
        let (; first_copy: first, second_copy: second, third_copy: third) =
            Order::produce(value; incoming_first, incoming_second);
    }

    machine Root::caller(&self, value: &Token)
    requires incoming_first: ready()
    requires incoming_second: ready()
    {
        self.invoke<Token, TokenProducer>(value; incoming_first, incoming_second);
    }
"#;

const STATIC_REQUIREMENT_TRAIT_DEFAULT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;

    trait Producer {
        machine Self::produce(&self)
        requires public_in: ready()
        ensures public_out: ready()
        {
            public_out = public_in;
        }
    }

    data Token {}
    TokenProducer: Token satisfies Producer {}

    data Root {}
    machine Root::invoke<Element, Order: Element satisfies Producer>(
        &self,
        value: &Element
    )
    requires incoming: ready()
    {
        let (; public_out: result) = Order::produce(value; incoming);
    }

    machine Root::caller(&self, value: &Token)
    requires incoming: ready()
    {
        self.invoke<Token, TokenProducer>(value; incoming);
    }
"#;

const STATIC_REQUIREMENT_RUNTIME_BASELINE_SOURCE: &str = r#"
    trait Producer {
        machine Self::produce(&self);
    }

    data Token {}

    TokenProducer: Token satisfies Producer {
        machine produce(&self) {}
    }

    data Root {}

    machine Root::invoke<Element, Order: Element satisfies Producer>(
        &self,
        value: &Element
    ) {
        Order::produce(value);
    }

    machine Root::caller(&self, value: &Token) {
        self.invoke<Token, TokenProducer>(value);
    }
"#;

const STATIC_REQUIREMENT_I32_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;

    trait Producer {
        machine Self::produce() -> i32
        requires public_in: ready()
        ensures public_out: ready();
    }

    data Token {}
    TokenProducer: Token satisfies Producer {
        machine produce() -> i32
        requires local_in: ready()
        requires 17i32 == 17i32
        ensures public_out: ready()
        ensures 17i32 == 17i32
        {
            public_out = local_in;
            17i32
        }
    }

    machine invoke<Element, Order: Element satisfies Producer>() -> i32
    requires incoming: ready()
    requires 17i32 == 17i32
    ensures 17i32 == 17i32
    {
        let (value; public_out: result) = Order::produce(; incoming);
        value
    }

    machine caller() -> i32
    requires incoming: ready()
    requires 17i32 == 17i32
    ensures 17i32 == 17i32
    {
        invoke<Token, TokenProducer>(; incoming)
    }
"#;

const STATIC_REQUIREMENT_I32_RUNTIME_BASELINE_SOURCE: &str = r#"
    machine produce() -> i32
    requires 17i32 == 17i32
    ensures 17i32 == 17i32
    {
        17i32
    }

    machine caller() -> i32
    requires 17i32 == 17i32
    ensures 17i32 == 17i32
    {
        let value: i32 = produce();
        value
    }
"#;

const STATIC_REQUIREMENT_BOOL_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;

    trait Producer {
        machine Self::produce() -> bool
        requires public_in: ready()
        ensures public_out: ready();
    }

    data Token {}
    TokenProducer: Token satisfies Producer {
        machine produce() -> bool
        requires local_in: ready()
        requires true == true
        ensures public_out: ready()
        ensures true == true
        {
            public_out = local_in;
            true
        }
    }

    machine invoke<Element, Order: Element satisfies Producer>() -> bool
    requires incoming: ready()
    requires true == true
    ensures true == true
    {
        let (value; public_out: result) = Order::produce(; incoming);
        value
    }

    machine caller() -> bool
    requires incoming: ready()
    requires true == true
    ensures true == true
    {
        invoke<Token, TokenProducer>(; incoming)
    }
"#;

const STATIC_REQUIREMENT_BOOL_RUNTIME_BASELINE_SOURCE: &str = r#"
    machine produce() -> bool
    requires true == true
    ensures true == true
    {
        true
    }

    machine caller() -> bool
    requires true == true
    ensures true == true
    {
        let value: bool = produce();
        value
    }
"#;

const ORDINARY_ATTACHED_SCALAR_SOURCE: &str = r#"
    data Root {}

    machine Root::f(value: i64) -> bool
    requires true == true
    ensures true == true
    {
        true
    }
"#;

const DUPLICATE_ARGUMENTED_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    data Root {}

    machine Root::produce()
    requires first: ready()
    requires second: ready()
    ensures copied: ready()
    {
        copied = second;
    }

    machine Root::relay()
    requires source: ready()
    ensures relayed: ready()
    {
        let (; copied: local) = Root::produce(; source, source);
        relayed = local;
    }
"#;

const RUNTIME_UNIT_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::touch() {}

    machine Root::produce()
    ensures outgoing: ready()
    {
        Root::touch();
        outgoing = ConcreteEvidence;
    }

    machine Root::relay()
    ensures relayed: ready()
    {
        let (; outgoing: local) = Root::produce();
        relayed = local;
    }
"#;

const COPY_AND_DISCARD_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::produce()
    ensures copied: ready()
    ensures discarded: ready()
    { copied = ConcreteEvidence; discarded = ConcreteEvidence; }

    machine Root::relay()
    ensures first: ready()
    ensures second: ready()
    {
        let (; copied: local, discarded: _) = Root::produce();
        first = local;
        second = local;
    }
"#;

const MULTI_FIELD_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::produce()
    ensures first: ready()
    ensures second: ready()
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
    }

    machine Root::relay()
    ensures relayed_first: ready()
    ensures relayed_second: ready()
    {
        let (; second: local_second, first: local_first) = Root::produce();
        relayed_first = local_first;
        relayed_second = local_second;
    }
"#;

const RUNTIME_VALUE_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    machine warmup() -> bool
    requires true == true
    ensures true == true
    { true }

    machine produce() -> bool
    requires true == true
    ensures true == true
    ensures first: ready()
    ensures second: ready()
    {
        first = ConcreteEvidence;
        second = ConcreteEvidence;
        true
    }

    machine relay() -> bool
    requires true == true
    ensures true == true
    ensures relayed_first: ready()
    ensures relayed_second: ready()
    {
        let warmed: bool = warmup();
        let (local_value; second: local_second, first: local_first) = produce();
        relayed_first = local_first;
        relayed_second = local_second;
        local_value
    }
"#;

const REPEATED_MULTI_FIELD_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::produce()
    ensures first: ready()
    ensures second: ready()
    { first = ConcreteEvidence; second = ConcreteEvidence; }

    machine Root::relay()
    ensures first_one: ready()
    ensures first_two: ready()
    ensures second_one: ready()
    ensures second_two: ready()
    {
        let (; first: local_first_one, second: local_first_two) = Root::produce();
        first_one = local_first_one;
        first_two = local_first_two;
        let (; first: local_second_one, second: local_second_two) = Root::produce();
        second_one = local_second_one;
        second_two = local_second_two;
    }
"#;

const REPEATED_PROOF_OUTPUT_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::produce()
    ensures outgoing: ready()
    {
        outgoing = ConcreteEvidence;
    }

    machine Root::relay()
    ensures first: ready()
    ensures second: ready()
    {
        let (; outgoing: local_first) = Root::produce();
        first = local_first;
        let (; outgoing: local_second) = Root::produce();
        second = local_second;
    }
"#;

const DISTINCT_PROOF_OUTPUT_PRODUCERS_SOURCE: &str = r#"
    trait Evidence {}
    proposition ready() evidence Evidence;
    ConcreteEvidence: satisfies Evidence {}

    data Root {}
    machine Root::produce_first()
    ensures outgoing: ready()
    { outgoing = ConcreteEvidence; }

    machine Root::produce_second()
    ensures outgoing: ready()
    { outgoing = ConcreteEvidence; }

    machine Root::relay()
    ensures first: ready()
    ensures second: ready()
    {
        let (; outgoing: local_first) = Root::produce_first();
        first = local_first;
        let (; outgoing: local_second) = Root::produce_second();
        second = local_second;
    }
"#;

const PROJECTED_SOURCE: &str = r#"
    trait Parent<T> {
        machine modulus(value: T) -> i32;
    }
    trait Evidence<T>: Parent<T> {}

    proposition ready<T>() evidence Evidence<T>;
    proposition selected<machine Witness>();
    proposition chosen<machine Witness>() = selected<Witness>();

    data Root {}
    machine Root::project()
    requires first: ready<i32>()
    requires second: ready<i32>()
    requires selected<first.modulus>()
    requires selected<first.modulus>()
    requires chosen<first.modulus>()
    requires selected<second.modulus>()
    {
    }

    machine Root::forward()
    requires incoming: ready<i32>()
    requires selected<incoming.modulus>()
    ensures outgoing: ready<i32>()
    {
        outgoing = incoming;
    }
"#;
