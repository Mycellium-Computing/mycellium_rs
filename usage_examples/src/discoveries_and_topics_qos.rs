use std::io::{stdin, stdout, Write};
use std::thread;
use std::time::Duration;
use dust_dds::{
    domain::domain_participant_factory::DomainParticipantFactory,
    listener::NO_LISTENER,
    infrastructure::{qos::QosKind, status::NO_STATUS, type_support::DdsType},
};
use dust_dds::domain::domain_participant::DomainParticipant;
use dust_dds::infrastructure::qos::{DataReaderQos, DataWriterQos, TopicQos};
use dust_dds::infrastructure::qos_policy::{
    DurabilityQosPolicy, DurabilityQosPolicyKind,
    HistoryQosPolicy, HistoryQosPolicyKind,
};
use dust_dds::infrastructure::sample_info::{ANY_INSTANCE_STATE, ANY_SAMPLE_STATE, ANY_VIEW_STATE};
use dust_dds::std_runtime::StdRuntime;
use dust_dds::topic_definition::topic_description::TopicDescription;

#[derive(DdsType, Debug)]
struct HelloWorldType {
    id: u8,
    msg: String,
}

fn main() {
    let domain_id = 0;
    let participant_factory = DomainParticipantFactory::get_instance();

    let mut topic_qos = TopicQos::default();

    topic_qos.durability = DurabilityQosPolicy {
        kind: DurabilityQosPolicyKind::Volatile
    };

    topic_qos.history = HistoryQosPolicy {
        kind: HistoryQosPolicyKind::KeepLast(1)
    };

    let participant = participant_factory
        .create_participant(domain_id, QosKind::Default, NO_LISTENER, NO_STATUS)
        .unwrap();

    let topic = participant
        .create_topic::<HelloWorldType>("HelloWorld", "HelloWorldType", QosKind::Specific(topic_qos), NO_LISTENER, NO_STATUS)
        .unwrap();

    print!("Enter 1 to use as publisher and 0 to use as listener: ");
    let _ = stdout().flush();
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();

    if input.trim() == "1" {
        publisher(participant, topic);
    } else {
        listener(participant, topic);
    }
}

fn publisher(participant: DomainParticipant<StdRuntime>, topic: TopicDescription<StdRuntime>) {
    let mut qos_writer = DataWriterQos::default();

    qos_writer.durability = DurabilityQosPolicy {
        kind: DurabilityQosPolicyKind::Volatile,
    };

    qos_writer.history = HistoryQosPolicy {
        kind: HistoryQosPolicyKind::KeepLast(1)
    };

    let publisher = participant
        .create_publisher(QosKind::Default, NO_LISTENER, NO_STATUS)
        .unwrap();

    let writer = publisher
        .create_datawriter::<HelloWorldType>(&topic, QosKind::Specific(qos_writer), NO_LISTENER, NO_STATUS)
        .unwrap();

    let mut i = 0;
    loop {
        let hello_world = HelloWorldType {
            id: i,
            msg: "Hello world!".to_string(),
        };
        writer.write(&hello_world, None).unwrap();
        i += 1;
        thread::sleep(Duration::from_millis(500));
    }
}

fn listener(participant: DomainParticipant<StdRuntime>, topic: TopicDescription<StdRuntime>) {
    let mut qos_reader = DataReaderQos::default();
    qos_reader.durability = DurabilityQosPolicy {
        kind: DurabilityQosPolicyKind::Volatile,
    };

    qos_reader.history = HistoryQosPolicy {
        kind: HistoryQosPolicyKind::KeepLast(1),
    };

    let subscriber = participant
        .create_subscriber(QosKind::Default, NO_LISTENER, NO_STATUS)
        .unwrap();

    let reader = subscriber
        .create_datareader::<HelloWorldType>(&topic, QosKind::Specific(qos_reader), NO_LISTENER, NO_STATUS)
        .unwrap();

    loop {
        let samples = reader
            .take(1, ANY_SAMPLE_STATE, ANY_VIEW_STATE, ANY_INSTANCE_STATE);

        if let Ok(hello_world_samples) = samples {
            if !hello_world_samples.is_empty() {
                if let Ok(data) = hello_world_samples[0].data() {
                    println!("Received: {:?}", data);
                }
            }
        }

        thread::sleep(Duration::from_millis(1500));
    }
}
