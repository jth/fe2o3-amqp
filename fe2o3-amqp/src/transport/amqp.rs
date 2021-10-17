use bytes::{Buf, BufMut, BytesMut};
use serde_amqp::{de::Deserializer, read::IoReader};
use fe2o3_amqp_types::performatives::{
    Attach, Begin, Close, Detach, Disposition, End, Flow, Open, Performative, Transfer,
};
use serde::{ser::Serialize, Deserialize};
use tokio_util::codec::{Decoder, Encoder};

use crate::error::EngineError;

use super::FRAME_TYPE_AMQP;

#[derive(Debug)]
pub struct Frame {
    pub channel: u16,
    pub body: FrameBody,
}

impl Frame {
    pub fn new(channel: impl Into<u16>, body: FrameBody) -> Self {
        Self {
            channel: channel.into(),
            body,
        }
    }

    pub fn channel(&self) -> u16 {
        self.channel
    }

    pub fn body(&self) -> &FrameBody {
        &self.body
    }

    pub fn into_body(self) -> FrameBody {
        self.body
    }

    pub fn empty() -> Self {
        Self {
            channel: 0,
            body: FrameBody::empty(),
        }
    }
}

pub struct FrameCodec {}

impl Encoder<Frame> for FrameCodec {
    type Error = EngineError;

    fn encode(&mut self, item: Frame, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // AMQP frame ignores extended header, thus doff should always be 2
        dst.put_u8(2); // doff
        dst.put_u8(FRAME_TYPE_AMQP); // frame type
        dst.put_u16(item.channel);

        // encode frame body
        let mut encoder = FrameBodyCodec {};
        encoder.encode(item.body, dst)
    }
}

impl Decoder for FrameCodec {
    type Item = Frame;
    type Error = EngineError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let doff = src.get_u8();
        let ftype = src.get_u8();
        let channel = src.get_u16();

        // check type byte
        if ftype != FRAME_TYPE_AMQP {
            return Err(EngineError::Message(
                "Only AMQP frame is implemented for now",
            ));
        }

        match doff {
            2 => {}
            // 0..=1 => return Err(EngineError::MalformedFrame),
            _ => return Err(EngineError::MalformedFrame),
        }

        // decode body
        let mut decoder = FrameBodyCodec {};
        let body = match decoder.decode(src)? {
            Some(b) => b,
            None => return Ok(None),
        };
        Ok(Some(Frame { channel, body }))
    }
}

#[derive(Debug)]
pub enum FrameBody {
    Open(Open),
    Begin(Begin),
    Attach(Attach),
    Flow(Flow),
    Transfer {
        performative: Transfer,
        payload: Option<BytesMut>,
    },
    Disposition(Disposition),
    Detach(Detach),
    End(End),
    Close(Close),
    // An empty frame used only for heartbeat
    Empty,
}

impl FrameBody {
    /// The payload will be ignored unless the performative is Transfer
    pub fn from_parts(performative: Performative, payload: Option<BytesMut>) -> Self {
        match performative {
            Performative::Open(performative) => FrameBody::Open (performative),
            Performative::Begin(performative) => FrameBody::Begin (performative),
            Performative::Attach(performative) => FrameBody::Attach (performative),
            Performative::Flow(performative) => FrameBody::Flow (performative),
            Performative::Transfer(performative) => FrameBody::Transfer {
                performative,
                payload,
            },
            Performative::Disposition(performative) => FrameBody::Disposition (performative),
            Performative::Detach(performative) => FrameBody::Detach (performative),
            Performative::End(performative) => FrameBody::End (performative),
            Performative::Close(performative) => FrameBody::Close (performative),
        }
    }

    pub fn open(performative: Open) -> Self {
        Self::Open (performative)
    }

    pub fn begin(performative: Begin) -> Self {
        Self::Begin (performative)
    }

    pub fn attach(performative: Attach) -> Self {
        Self::Attach (performative)
    }

    pub fn flow(performative: Flow) -> Self {
        Self::Flow (performative)
    }

    pub fn transfer(performative: Transfer, payload: Option<BytesMut>) -> Self {
        Self::Transfer {
            performative,
            payload,
        }
    }

    pub fn disposition(performative: Disposition) -> Self {
        Self::Disposition (performative)
    }

    pub fn detach(performative: Detach) -> Self {
        Self::Detach (performative)
    }

    pub fn end(performative: End) -> Self {
        Self::End (performative)
    }

    pub fn close(performative: Close) -> Self {
        Self::Close (performative)
    }

    pub fn empty() -> Self {
        Self::Empty
    }
}

pub struct FrameBodyCodec {}

impl Encoder<FrameBody> for FrameBodyCodec {
    type Error = EngineError;

    fn encode(&mut self, item: FrameBody, dst: &mut BytesMut) -> Result<(), Self::Error> {
        use serde_amqp::ser::Serializer;

        let mut serializer = Serializer::from(dst.writer());
        match item {
            FrameBody::Open (performative) => performative.serialize(&mut serializer),
            FrameBody::Begin (performative) => performative.serialize(&mut serializer),
            FrameBody::Attach (performative) => performative.serialize(&mut serializer),
            FrameBody::Flow (performative) => performative.serialize(&mut serializer),
            FrameBody::Transfer {
                performative,
                payload,
            } => {
                performative.serialize(&mut serializer)?;
                if let Some(payload) = payload {
                    dst.put(payload);
                }
                Ok(())
            }
            FrameBody::Disposition (performative) => performative.serialize(&mut serializer),
            FrameBody::Detach (performative) => performative.serialize(&mut serializer),
            FrameBody::End (performative) => performative.serialize(&mut serializer),
            FrameBody::Close (performative) => performative.serialize(&mut serializer),
            FrameBody::Empty => Ok(()),
        }
        .map_err(Into::into)
    }
}

impl Decoder for FrameBodyCodec {
    type Item = FrameBody;
    type Error = EngineError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() == 0 {
            return Ok(Some(FrameBody::Empty));
        }
        let reader = IoReader::new(src.reader());
        let mut deserializer = Deserializer::new(reader);
        let performative: Performative = Deserialize::deserialize(&mut deserializer)?;

        let frame_body = match performative {
            Performative::Transfer(performative) => {
                let payload = if src.has_remaining() {
                    Some(src.split())
                } else {
                    None
                };
                FrameBody::Transfer {
                    performative,
                    payload,
                }
            }
            p @ _ => FrameBody::from_parts(p, None),
        };

        Ok(Some(frame_body))
    }
}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;
    use fe2o3_amqp_types::performatives::Open;
    use tokio_util::codec::{Decoder, Encoder};

    use super::{Frame, FrameBody, FrameBodyCodec, FrameCodec};

    #[test]
    fn test_encoding_frame_body() {
        let open = Open {
            container_id: "1234".into(),
            hostname: Some("127.0.0.1".into()),
            max_frame_size: 100.into(),
            channel_max: 9.into(),
            idle_time_out: Some(10),
            outgoing_locales: None,
            incoming_locales: None,
            offered_capabilities: None,
            desired_capabilities: None,
            properties: None,
        };

        let body = FrameBody::open(open);

        let mut encoder = FrameBodyCodec {};
        let mut dst = BytesMut::new();
        encoder.encode(body, &mut dst).unwrap();
        println!("{:?}", dst);
    }

    #[test]
    fn test_encoding_empty_frame() {
        let empty = Frame::empty();
        let mut encoder = FrameCodec {};
        let mut dst = BytesMut::new();
        encoder.encode(empty, &mut dst).unwrap();
        println!("{:x?}", dst);
    }

    #[test]
    fn test_decode_empty_frame() {
        let mut decoder = FrameCodec {};
        let mut src = BytesMut::from(&[0x02, 0x00, 0x00, 0x00][..]);
        let frame = decoder.decode(&mut src).unwrap();
        println!("{:?}", frame);
    }
}