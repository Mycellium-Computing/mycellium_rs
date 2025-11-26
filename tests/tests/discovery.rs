#[cfg(test)]
mod tests {
    use std::{
        io::{self, Write},
        time::Duration,
    };

    use dust_dds::{
        dds_async::{
            data_reader::DataReaderAsync, domain_participant_factory::DomainParticipantFactoryAsync,
        },
        infrastructure::{
            qos::QosKind,
            sample_info::{ANY_INSTANCE_STATE, ANY_SAMPLE_STATE, ANY_VIEW_STATE},
            status::{NO_STATUS, StatusKind},
            type_support::DdsType,
        },
        listener::NO_LISTENER,
        std_runtime::StdRuntime,
        subscription::data_reader_listener::DataReaderListener,
    };
    use smol::Timer;

    #[derive(DdsType, Debug)]
    struct ARandomMessage<'a> {
        field1: &'a str,
        field2: i32,
    }

    const TOPIC_NAME: &str = "ARandomMessageTopic";
    const TOPIC_TYPE: &str = "ARandomMessage";

    struct MatchedSubscriber;

    impl<T> DataReaderListener<StdRuntime, T> for MatchedSubscriber
    where
        T: Send + 'static,
    {
        async fn on_subscription_matched(
            &mut self,
            _the_reader: DataReaderAsync<StdRuntime, T>,
            _status: dust_dds::infrastructure::status::SubscriptionMatchedStatus,
        ) {
            println!("Matched Status: {:?}", _status);
            io::stdout().flush().expect("Failed to flush stdout");
        }

        async fn on_liveliness_changed(
            &mut self,
            _the_reader: DataReaderAsync<StdRuntime, T>,
            _status: dust_dds::infrastructure::status::LivelinessChangedStatus,
        ) {
            println!("Changed Status: {:?}", _status);
            io::stdout().flush().expect("Failed to flush stdout");
        }
    }

    async fn consumer() {
        let factory = DomainParticipantFactoryAsync::get_instance();
        let participant = factory
            .create_participant(0, QosKind::Default, NO_LISTENER, NO_STATUS)
            .await
            .unwrap();

        let topic = participant
            .create_topic::<ARandomMessage>(
                TOPIC_NAME,
                TOPIC_TYPE,
                QosKind::Default,
                NO_LISTENER,
                NO_STATUS,
            )
            .await
            .unwrap();

        let subscriber = participant
            .create_subscriber(QosKind::Default, NO_LISTENER, NO_STATUS)
            .await
            .unwrap();

        let _reader = subscriber
            .create_datareader::<ARandomMessage>(
                &topic,
                QosKind::Default,
                Some(MatchedSubscriber),
                &[
                    StatusKind::SubscriptionMatched,
                    StatusKind::LivelinessChanged,
                    StatusKind::LivelinessLost,
                ],
            )
            .await
            .unwrap();

        Timer::after(Duration::from_secs(30)).await;
    }

    async fn provider() {
        let factory = DomainParticipantFactoryAsync::get_instance();
        let participant = factory
            .create_participant(0, QosKind::Default, NO_LISTENER, NO_STATUS)
            .await
            .unwrap();

        let topic = participant
            .create_topic::<ARandomMessage>(
                TOPIC_NAME,
                TOPIC_TYPE,
                QosKind::Default,
                NO_LISTENER,
                NO_STATUS,
            )
            .await
            .unwrap();

        let publisher = participant
            .create_publisher(QosKind::Default, NO_LISTENER, NO_STATUS)
            .await
            .unwrap();

        let _writer = publisher
            .create_datawriter::<ARandomMessage>(&topic, QosKind::Default, NO_LISTENER, NO_STATUS)
            .await
            .unwrap();

        Timer::after(Duration::from_secs(5)).await;
        println!("Provider finished");
    }

    #[test]
    fn test() {
        let a = std::thread::spawn(|| {
            smol::block_on(consumer());
        });
        let b = std::thread::spawn(|| {
            smol::block_on(provider());
        });

        a.join().unwrap();
        b.join().unwrap();
    }
}
