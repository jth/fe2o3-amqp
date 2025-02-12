# Service Bus Example

This is a simple example showcasing how to send message to and receiver message from Azure Service Bus.

## Queue

To run the example, the following environment variables must be set in a `.env` file:

- `HOST_NAME=<namespace>.servicebus.windows.net`
- `SHARED_ACCESS_KEY_NAME=<SharedAccessKeyName>`
- `SHARED_ACCESS_KEY_VALUE=<SharedAccessKey>`
- `QUEUE_NAME=<queue>`

Replace the fields wrapped in `<>` with the corresponding values for your Service Bus instance.

Then you can run the example with

```sh
cargo run --bin queue_sender
```

OR

```sh
cargo run --bin queue_receiver
```

```sh
cargo run --bin queue_dlq --features "dlq"
```

## Topic

To run the example, the following environment variables must be set in a `.env` file:

- `HOST_NAME=<namespace>.servicebus.windows.net`
- `SHARED_ACCESS_KEY_NAME=<SharedAccessKeyName>`
- `SHARED_ACCESS_KEY_VALUE=<SharedAccessKey>`
- `TOPIC_NAME=<topic>`
- `TOPIC_SUBSCRIPTION=<subscription>`

Replace the fields wrapped in `<>` with the corresponding values for your Service Bus instance.

Then you can run the example with

```sh
cargo run --bin topic_sender
```

OR

```sh
cargo run --bin topic_receiver
```
