use std::{env, sync::Arc};

use dotenv::dotenv;
use fe2o3_amqp::{
    sasl_profile::SaslProfile, types::primitives::Value, Connection, Receiver, Session,
};
use rustls::OwnedTrustAnchor;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let hostname = env::var("HOST_NAME").unwrap();
    let port = 5671;
    let sa_key_name = env::var("SHARED_ACCESS_KEY_NAME").unwrap();
    let sa_key_value = env::var("SHARED_ACCESS_KEY_VALUE").unwrap();
    let topic_name = env::var("TOPIC_NAME").unwrap();
    let topic_subscription = env::var("TOPIC_SUBSCRIPTION").unwrap();

    let mut root_store = rustls::RootCertStore::empty();
    root_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));
    let config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let stream = TcpStream::connect((&hostname[..], port)).await.unwrap();
    let domain = hostname.as_str().try_into().unwrap();
    let connector = TlsConnector::from(Arc::new(config));
    let tls_stream = connector.connect(domain, stream).await.unwrap();

    let mut connection = Connection::builder()
        .container_id("rust-receiver-connection-1")
        .hostname(&hostname[..])
        .sasl_profile(SaslProfile::Plain {
            username: sa_key_name,
            password: sa_key_value,
        })
        .open_with_stream(tls_stream)
        .await
        .unwrap();
    let mut session = Session::begin(&mut connection).await.unwrap();

    let address = format!("{}/Subscriptions/{}", topic_name, topic_subscription);
    let mut receiver = Receiver::attach(&mut session, "rust-topic-receiver", address)
        .await
        .unwrap();

    for _ in 0..3 {
        // All of the Microsoft AMQP clients represent the event body as an uninterpreted bag of bytes.
        let delivery = receiver.recv::<Value>().await.unwrap();
        let msg = std::str::from_utf8(&delivery.try_as_data().unwrap()[..]).unwrap();
        println!("Received: {:?}", msg);
        receiver.accept(&delivery).await.unwrap();
    }

    receiver.close().await.unwrap();
    session.end().await.unwrap();
    connection.close().await.unwrap();
}
