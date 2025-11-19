mod discoveries_and_topics_qos;
mod consumer_impl;

use std::{env};
use std::time::Duration;
use dust_dds::infrastructure::type_support::DdsType;
use mycellium_computing::{
    provides,
    consumes,
};
use mycellium_computing::core::application::Application;
use mycellium_computing::core::application::consumer::Consumer;
use dust_dds::std_runtime::StdRuntime;

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
#[provides(StdRuntime, [
    RequestResponse("sum", CalculatorRequest, Number),
    RequestResponse("multiply", CalculatorRequest, Number),
    RequestResponse("divide", CalculatorRequest, Number),
    RequestResponse("exponentiate", CalculatorRequest, Number),
])]
struct TwoNumbersCalculator;


#[derive(Default)]
#[provides(StdRuntime, [
    RequestResponse("addition", CalculatorRequest, Number)
])]
struct AddTwoInts;


//#[consumes([("add_two_ints", CalculatorRequest, Number)])]
struct CalculatorProxy;

impl AddTwoIntsProviderTrait for AddTwoInts {
    async fn addition(&self, input: CalculatorRequest) -> Number {
        Number { value: input.a + input.b }
    }
}

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

async fn provider() {
    let tick_duration = Duration::from_nanos(1_000_000_000 / HERTZ);
    let mut app = Application::new(
        0, "JustASumService", tick_duration
    ).await;

    app.register_provider::<TwoNumbersCalculator>().await;
    app.register_provider::<AddTwoInts>().await;

    app.run_forever().await;
}

async fn consumer() {

}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 1 {
        consumer().await;
    } else if args[0] == "provider" {
        provider().await;
    }
}