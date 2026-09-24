//! Fixtures shared by the unit cleanup tests: the nominal cleanup sources.

#[path = "unit_cleanup/contextual_and_multi_root_cleanups.rs"]
mod contextual_and_multi_root_cleanups;
#[path = "unit_cleanup/shared_executables_and_call_cleanups.rs"]
mod shared_executables_and_call_cleanups;

const SOURCE: &str = r#"
    data Token {}
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const SCALAR_SOURCE: &str = r#"
    data Token { flag: bool; tag: u8; delta: i16; payload: u64; address: addr; }
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const CONTEXTUAL_SOURCE: &str = r#"
    data Token { ready: bool; }
    machine Token::drop(&mut self)
    requires self.ready
    {}

    data Root {}
    machine Root::enter(token: Token)
    requires token.ready
    {}
"#;

const FINITE_CONTEXTUAL_SOURCE: &str = r#"
    data Token { ready: bool; audited: bool; armed: bool; }
    machine Token::drop(&mut self)
    requires
        self.armed;
        self.ready
    {}

    data Root {}
    machine Root::enter(token: Token)
    requires
        token.armed;
        token.audited;
        token.ready
    {}
"#;

const CALLER_ONLY_CONTEXTUAL_SOURCE: &str = r#"
    data Token { observed: bool; }
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(token: Token)
    requires token.observed
    {}
"#;

const TWO_ROOT_SOURCE: &str = r#"
    data Token {}
    machine Token::drop(&mut self) {}

    data Root {}
    machine Root::enter(first: Token, second: Token) {}
"#;

const TWO_ROOT_CONTEXTUAL_SOURCE: &str = r#"
    data Token { ready: bool; }
    machine Token::drop(&mut self)
    requires self.ready
    {}

    data Root {}
    machine Root::enter(first: Token, second: Token)
    requires first.ready, second.ready
    {}
"#;

const TWO_ROOT_ONE_EXECUTABLE_SOURCE: &str = r#"
    data FirstHelper {}
    machine FirstHelper::touch() {}
    data SecondHelper {}
    machine SecondHelper::touch() {}
    data ThirdHelper {}
    machine ThirdHelper::touch() {}

    data First {}
    machine First::drop(&mut self) {
        FirstHelper::touch();
        SecondHelper::touch();
        ThirdHelper::touch();
    }
    data Second {}
    machine Second::drop(&mut self) {}

    data Root {}
    machine Root::enter(first: First, second: Second) {}
"#;

const TWO_ROOT_TWO_EXECUTABLE_SOURCE: &str = r#"
    data FirstHelper {}
    machine FirstHelper::touch() {}
    data SecondHelper {}
    machine SecondHelper::touch() {}

    data First {}
    machine First::drop(&mut self) { FirstHelper::touch(); }
    data Second {}
    machine Second::drop(&mut self) { SecondHelper::touch(); }

    data Root {}
    machine Root::enter(first: First, second: Second) {}
"#;

const TWO_ROOT_SHARED_EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }

    data Root {}
    machine Root::enter(first: Token, second: Token) {}
"#;

const THREE_ROOT_DISTINCT_SOURCE: &str = r#"
    data First {}
    machine First::drop(&mut self) {}
    data Second {}
    machine Second::drop(&mut self) {}
    data Third {}
    machine Third::drop(&mut self) {}

    data Root {}
    machine Root::enter(first: First, second: Second, third: Third) {}
"#;

const THREE_ROOT_SHARED_EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token {}
    machine Token::drop(&mut self) { Helper::touch(); }

    data Root {}
    machine Root::enter(first: Token, second: Token, third: Token) {}
"#;

const EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token { flag: bool; }
    machine Token::drop(&mut self) {
        Helper::touch();
    }

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const CONTEXTUAL_EXECUTABLE_SOURCE: &str = r#"
    data Helper {}
    machine Helper::touch() {}

    data Token { ready: bool; padding: u8; }
    machine Token::drop(&mut self)
    requires self.ready
    {
        Helper::touch();
    }

    data Root {}
    machine Root::enter(first: Token, second: Token)
    requires second.ready, first.ready
    {}
"#;

const TWO_CALL_SOURCE: &str = r#"
    data First {}
    machine First::touch() {}
    data Second {}
    machine Second::touch() {}

    data Token { flag: bool; }
    machine Token::drop(&mut self) {
        First::touch();
        Second::touch();
    }

    data Root {}
    machine Root::enter(token: Token) {}
"#;

const THREE_CALL_SOURCE: &str = r#"
    data First {}
    machine First::touch() {}
    data Second {}
    machine Second::touch() {}
    data Third {}
    machine Third::touch() {}

    data Token { flag: bool; }
    machine Token::drop(&mut self) {
        First::touch();
        Second::touch();
        Third::touch();
    }

    data Root {}
    machine Root::enter(token: Token) {}
"#;
