use std::time::Duration;
use dust_dds::infrastructure::type_support::DdsType;
use mycellium_computing::{
    provides,
};
use mycellium_computing::core::application::Application;

const HERTZ: u64 = 360;

#[derive(DdsType)]
struct CalculatorRequest {
    a: f64,
    b: f64,
}

#[derive(DdsType)]
struct Number {
    value: f64,
}

#[derive(Default)]
#[provides([
    ("sum", CalculatorRequest, Number),
    ("multiply", CalculatorRequest, Number),
    ("divide", CalculatorRequest, Number),
    ("exponentiate", CalculatorRequest, Number),
])]
struct TwoNumbersCalculator;

impl TwoNumbersCalculatorProviderTrait for TwoNumbersCalculator {
    async fn sum(&self, input: CalculatorRequest) -> Number {
        Number { value: input.a + input.b }
    }

    async fn multiply(&self, input: CalculatorRequest) -> Number {
        Number { value: input.a * input.b }
    }

    async fn divide(&self, input: CalculatorRequest) -> Number {
        if input.b == 0.0 {
            Number { value: f64::NAN }
        } else {
            Number { value: input.a / input.b }
        }
    }

    async fn exponentiate(&self, input: CalculatorRequest) -> Number {
        Number { value: input.a.powf(input.b) }
    }
}


#[tokio::main]
async fn main() {
    let tick_duration = Duration::from_nanos(1_000_000_000 / HERTZ);
    let mut app = Application::new(
        0, "JustASumService", tick_duration
    ).await;

    app.register_provider::<TwoNumbersCalculator>().await;

    app.run_forever().await;
}