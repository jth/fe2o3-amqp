use fe2o3_amqp::{
    transaction::{TransactionDischarge, TransactionalRetirement, OwnedTransaction},
    types::primitives::Value,
    Connection, Delivery, Receiver, Session,
};

#[tokio::main]
async fn main() {
    let mut connection = Connection::open("connection-1", "amqp://localhost:5672")
        .await
        .unwrap();
    let mut session = Session::begin(&mut connection).await.unwrap();
    let mut receiver = Receiver::attach(&mut session, "rust-recver-1", "q1")
        .await
        .unwrap();

    let delivery: Delivery<Value> = receiver.recv().await.unwrap();

    // Transactionally retiring
    let mut txn = OwnedTransaction::declare(&mut session, "controller", None).await.unwrap();
    txn.accept(&mut receiver, &delivery).await.unwrap();
    txn.commit().await.unwrap();

    receiver.close().await.unwrap();
    session.close().await.unwrap();
    connection.close().await.unwrap();
}
