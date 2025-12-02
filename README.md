# Mycelium Computing

A Rust framework for building modular, distributed applications using DDS (Data Distribution Service) middleware. This framework simplifies the creation of provider-consumer communication patterns through intuitive procedural macros.

## Overview

Mycelium Computing enables developers to create distributed systems with minimal boilerplate by leveraging Rust's procedural macro system. It abstracts the complexity of DDS-based communication, allowing you to focus on your application logic.

### Key Features

- **Declarative Macros**: Use `#[provides]` and `#[consumes]` attributes to define providers and consumers
- **Multiple Communication Patterns**:
  - **RequestResponse**: Traditional request-response with configurable timeout
  - **Response**: One-way response pattern (no input required)
  - **Continuous**: Streaming/pub-sub pattern for real-time data
- **Type-Safe**: Leverages Rust's type system with compile-time verification
- **DDS-Based**: Built on top of Dust DDS for reliable, high-performance communication
- **Async-First**: Fully asynchronous API design

## Project Structure

```
modular_architecture/
├── core/                    # Main library (mycelium_computing)
│   └── src/
│       ├── core/           # Application, listeners, and messages
│       └── utils/          # Utilities (ID generator, storage)
├── macros/                  # Procedural macros library
│   └── src/
│       ├── provider/       # #[provides] macro implementation
│       └── consumer/       # #[consumes] macro implementation
├── tests/                   # Integration tests
├── benchmarking/           # Performance benchmarks
├── usage_examples/         # Example implementations
└── docs/                   # Documentation
```

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
mycelium_computing = { path = "path/to/core" }
dust_dds = "0.13.0"
```

## Quick Start

### Defining Message Types

First, define your message types using the `DdsType` derive macro:

```rust
use dust_dds::infrastructure::type_support::DdsType;

#[derive(DdsType, Debug)]
pub struct ArithmeticRequest {
    pub a: f32,
    pub b: f32,
}

#[derive(DdsType, Debug)]
pub struct Number {
    pub value: f32,
}
```

### Creating a Provider

Use the `#[provides]` macro to define a service provider:

```rust
use mycelium_computing::provides;
use dust_dds::std_runtime::StdRuntime;

#[provides(StdRuntime, [
    RequestResponse("add_two_ints", ArithmeticRequest, Number),
    Continuous("stream_data", SensorData)
])]
struct CalculatorProvider;

// Implement the generated trait
impl CalculatorProviderProviderTrait for CalculatorProvider {
    async fn add_two_ints(request: ArithmeticRequest) -> Number {
        Number {
            value: request.a + request.b,
        }
    }
}
```

### Creating a Consumer

Use the `#[consumes]` macro to define a service consumer:

```rust
use mycelium_computing::consumes;

#[consumes(StdRuntime, [
    RequestResponse("add_two_ints", ArithmeticRequest, Number),
    Continuous("stream_data", SensorData)
])]
struct CalculatorConsumer;

// For continuous data, implement the callback trait
impl CalculatorConsumerContinuosTrait for CalculatorConsumer {
    async fn stream_data(data: SensorData) {
        println!("Received sensor data: {:?}", data);
    }
}
```

### Running Provider and Consumer

**Provider Application:**

```rust
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use mycelium_computing::core::application::Application;

async fn run_provider() {
    let factory = DomainParticipantFactoryAsync::get_instance();
    let mut app = Application::new(0, "CalculatorService", factory).await;
    
    // Register provider and get handle for continuous data
    let continuous_handle = app.register_provider::<CalculatorProvider>().await;
    
    // Publish continuous data when needed
    continuous_handle.stream_data(&SensorData { /* ... */ }).await;
    
    // Keep provider running
    app.run_forever().await;
}
```

**Consumer Application:**

```rust
async fn run_consumer() {
    let factory = DomainParticipantFactoryAsync::get_instance();
    
    let participant = factory
        .create_participant(0, QosKind::Default, NO_LISTENER, NO_STATUS)
        .await.unwrap();
        
    let subscriber = participant
        .create_subscriber(QosKind::Default, NO_LISTENER, NO_STATUS)
        .await.unwrap();
        
    let publisher = participant
        .create_publisher(QosKind::Default, NO_LISTENER, NO_STATUS)
        .await.unwrap();
    
    // Initialize the consumer proxy
    let consumer = CalculatorConsumer::init(&participant, &subscriber, &publisher).await;
    
    // Make a request with timeout
    let result = consumer
        .add_two_ints(
            ArithmeticRequest { a: 1.0, b: 2.0 },
            Duration::new(10, 0),
        )
        .await;
    
    match result {
        Some(response) => println!("Result: {}", response.value),
        None => println!("Request timed out"),
    }
}
```

## Communication Patterns

### RequestResponse

A bidirectional pattern where the consumer sends a request and waits for a response:

```rust
#[provides(StdRuntime, [
    RequestResponse("service_name", RequestType, ResponseType)
])]
```

### Response

A pattern where the provider returns data without requiring input:

```rust
#[provides(StdRuntime, [
    Response("get_status", StatusResponse)
])]
```

### Continuous

A pub-sub pattern for streaming data from provider to consumers:

```rust
#[provides(StdRuntime, [
    Continuous("telemetry", TelemetryData)
])]
```

## Architecture

The framework follows a provider-consumer architecture built on DDS:

```
┌──────────────┐     ┌─────────────────┐     ┌──────────────┐
│   Provider   │────>│  DDS Middleware │<────│   Consumer   │
│              │<────│  (Topics/QoS)   │────>│              │
└──────────────┘     └─────────────────┘     └──────────────┘
```

Key components:
- **Application**: Manages DDS participants, publishers, and subscribers
- **ProviderTrait**: Generated trait that providers must implement
- **Proxy**: Generated struct for consumers to interact with providers
- **Listeners**: Handle incoming messages and route to implementations

## Running Tests

```bash
cargo test --workspace
```

## Running Benchmarks

```bash
# Request-Response benchmark
cargo run --bin request_response_provider
cargo run --bin request_response_consumer

# Continuous data benchmark
cargo run --bin continuous_provider
cargo run --bin continuous_consumer
```

## Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| dust_dds | 0.13.0 | DDS middleware implementation |
| futures | 0.3.31 | Async utilities |
| futures-timer | 3.0.3 | Async timing |
| proc-macro2 | 1.0.103 | Procedural macro support |
| quote | 1.0.41 | Code generation |
| syn | 2.0.108 | Rust syntax parsing |

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

```
Copyright 2025 Juan David Guevara Arévalo

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.
```

## Author

**Juan David Guevara Arévalo**

---

*Built with ❤️ using Rust and DDS*
