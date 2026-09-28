use super::later_results::SCALAR_HELPERS;
use super::{
    ObserveResults, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalEffectResult,
};

pub(super) fn source(completion: &str) -> String {
    let host = if completion.contains("Host::finish") {
        " + Host"
    } else {
        ""
    };
    format!(
        r#"
        {SCALAR_HELPERS}
        pub data Token {{ flag: bool; }}
        boundary trait Factory {{
            machine create(first: u16, second: u16) -> Token reaches Factory;
        }}
        boundary trait Sink {{
            machine consume(token: Token, count: u16) reaches Sink;
            machine measure(token: Token, first: u16, second: u16) -> u16 reaches Sink;
        }}
        machine forward(token: Token, count: u16) -> Token {{ token }}
        data Main {{}}
        machine Main::consume(token: Token, count: u16) reaches Sink {{
            Sink::consume(token, count);
        }}
        machine Main::measure(token: Token, count: u16) -> u16 reaches Sink {{
            let result: u16 = Sink::measure(token, count, 17u16);
            result
        }}
        machine Main::main() reaches Factory + Sink{host} {{
            let prefix: u16 = 5u16;
            let first: Token = Factory::create(identity16(prefix), 7u16);
            let spare: Token = Factory::create(11u16, 13u16);
            {completion}
        }}
        "#
    )
}

#[derive(Default)]
pub(super) struct ObserveMoves {
    pub(super) results: ObserveResults,
    pub(super) produced: Vec<u64>,
    pub(super) consumed: Vec<u64>,
}

impl TerminalEffectHandler for ObserveMoves {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        panic!("result-bearing boundary handler required")
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            structural_arguments,
            ..
        } = effect
        else {
            panic!("boundary effect")
        };
        self.consumed.extend(
            structural_arguments
                .iter()
                .map(|value| value.opaque_identity),
        );
        let mut result = self.results.handle_effect_result(effect)?;
        if let TerminalEffectResult::Structural(value) = &mut result {
            value.opaque_identity += self.produced.len() as u64;
            self.produced.push(value.opaque_identity);
        }
        Ok(result)
    }
}
