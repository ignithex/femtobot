use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[derive(Clone, Debug)]
pub struct InboundMessage {
    pub channel: String,
    pub chat_id: String,
    pub sender_id: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct OutboundMessage {
    pub channel: String,
    pub chat_id: String,
    pub content: String,
}

#[derive(Clone)]
pub struct MessageBus {
    inbound_tx: mpsc::Sender<InboundMessage>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    inbound_rx: Arc<Mutex<mpsc::Receiver<InboundMessage>>>,
}

pub struct BusHandle {
    pub outbound_rx: Arc<Mutex<mpsc::Receiver<OutboundMessage>>>,
}

impl MessageBus {
    pub fn new() -> (Self, BusHandle) {
        let (inbound_tx, inbound_rx) = mpsc::channel(100);
        let (outbound_tx, outbound_rx) = mpsc::channel(100);

        let inbound_rx = Arc::new(Mutex::new(inbound_rx));
        let outbound_rx = Arc::new(Mutex::new(outbound_rx));

        let bus = MessageBus {
            inbound_tx,
            outbound_tx,
            inbound_rx: inbound_rx.clone(),
        };

        let handle = BusHandle { outbound_rx };
        (bus, handle)
    }

    pub async fn publish_inbound(&self, msg: InboundMessage) {
        let _ = self.inbound_tx.send(msg).await;
    }

    pub async fn publish_outbound(&self, msg: OutboundMessage) {
        let _ = self.outbound_tx.send(msg).await;
    }

    pub async fn consume_inbound(&self) -> Option<InboundMessage> {
        let mut rx = self.inbound_rx.lock().await;
        rx.recv().await
    }
}
