use dust_dds::{infrastructure::type_support::DdsType, std_runtime::StdRuntime};
use mycelium_computing::{consumes, provides};

#[derive(DdsType)]
struct ArithmeticRequest {
    a: f32,
    b: f32,
}

#[derive(DdsType)]
struct Number {
    value: f32,
}

#[provides(StdRuntime, [
    RequestResponse("add_two_ints", ArithmeticRequest, Number)
])]
struct CalculatorProvider;

impl CalculatorProviderProviderTrait for CalculatorProvider {
    async fn add_two_ints(request: ArithmeticRequest) -> Number {
        println!("Adding {} and {}", request.a, request.b);
        Number {
            value: request.a + request.b,
        }
    }
}

#[consumes(StdRuntime, [
    RequestResponse("add_two_ints", ArithmeticRequest, Number),
])]
struct CalculatorConsumer;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
    use mycelium_computing::core::application::Application;
    use smol::Timer;

    use crate::{
        ArithmeticRequest, CalculatorConsumer, CalculatorConsumerResponseTrait, CalculatorProvider,
    };

    #[test]
    fn test_function() {
        let handle = std::thread::spawn(|| {
            smol::block_on(async {
                let participant_factory = DomainParticipantFactoryAsync::get_instance();
                let mut app = Application::new(150, "test_application", participant_factory).await;
                app.register_provider::<CalculatorProvider>().await;

                Timer::after(Duration::new(2, 0)).await;
            });
        });

        smol::block_on(async {
            let factory = DomainParticipantFactoryAsync::get_instance();

            let participant = factory
                .create_participant(
                    150,
                    dust_dds::infrastructure::qos::QosKind::Default,
                    dust_dds::listener::NO_LISTENER,
                    dust_dds::infrastructure::status::NO_STATUS,
                )
                .await
                .unwrap();

            let subscriber = participant
                .create_subscriber(
                    dust_dds::infrastructure::qos::QosKind::Default,
                    dust_dds::listener::NO_LISTENER,
                    dust_dds::infrastructure::status::NO_STATUS,
                )
                .await
                .unwrap();

            let publisher = participant
                .create_publisher(
                    dust_dds::infrastructure::qos::QosKind::Default,
                    dust_dds::listener::NO_LISTENER,
                    dust_dds::infrastructure::status::NO_STATUS,
                )
                .await
                .unwrap();

            let consumer = CalculatorConsumer::init(&participant, &subscriber, &publisher).await;
            let request = ArithmeticRequest { a: 1.0, b: 2.0 };

            let expected_result = 3.0;

            let result = consumer
                .add_two_ints(
                    request,
                    dust_dds::dcps::infrastructure::time::Duration::new(2, 0),
                )
                .await
                .unwrap();

            assert_eq!(
                result.value, expected_result,
                "The provider responded with an incorrect result"
            );
        });

        handle.join().unwrap();
    }
}
