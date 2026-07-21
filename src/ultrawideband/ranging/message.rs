//! Message types and logic for ultra wideband ranging.

/// Ranging frame sentinel.
const SENTINEL: u8 = 0xA7;

const MSG_LENGTH_POLL: usize = 3;
const MSG_LENGTH_RESPONSE: usize = 3;
const MSG_LENGTH_FINAL: usize = 27;

pub enum DecodedMessage {
    Poll {
        msg_id: u8,
    },
    Response {
        msg_id: u8,
    },
    Final {
        msg_id: u8,
        time_tx_poll: u64,
        time_rx_response: u64,
        time_tx_final: u64,
    },
}

/// Two-way ranging message types.
#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum MessageType {
    Poll = 1,
    Response = 2,
    Final = 3,
}

impl MessageType {
    #[must_use]
    pub fn decode(frame: &[u8]) -> Option<DecodedMessage> {
        if let Some(([sentinel, msg_type, msg_id], times)) = frame.split_first_chunk::<3>()
            && *sentinel == SENTINEL
        {
            match (frame.len(), Self::try_from(*msg_type)) {
                (MSG_LENGTH_POLL, Ok(Self::Poll)) => Some(DecodedMessage::Poll { msg_id: *msg_id }),
                (MSG_LENGTH_RESPONSE, Ok(Self::Response)) => {
                    Some(DecodedMessage::Response { msg_id: *msg_id })
                }
                (MSG_LENGTH_FINAL, Ok(Self::Final)) => {
                    let mut time_chunks = times
                        .as_chunks::<8>()
                        .0
                        .iter()
                        .map(|time| u64::from_le_bytes(*time));

                    let time_tx_poll = time_chunks.next()?;
                    let time_rx_response = time_chunks.next()?;
                    let time_tx_final = time_chunks.next()?;
                    let decoded_message = DecodedMessage::Final {
                        msg_id: *msg_id,
                        time_tx_poll,
                        time_rx_response,
                        time_tx_final,
                    };

                    Some(decoded_message)
                }
                (_, _) => None,
            }
        } else {
            None
        }
    }

    #[must_use]
    pub const fn encode_poll(msg_id: u8) -> [u8; MSG_LENGTH_POLL] {
        [SENTINEL, Self::Poll as u8, msg_id]
    }

    #[must_use]
    pub const fn encode_response(msg_id: u8) -> [u8; MSG_LENGTH_RESPONSE] {
        [SENTINEL, Self::Response as u8, msg_id]
    }

    #[must_use]
    pub const fn encode_final(
        msg_id: u8,
        time_tx_poll: u64,
        time_rx_response: u64,
        time_tx_final: u64,
    ) -> [u8; MSG_LENGTH_FINAL] {
        let [p0, p1, p2, p3, p4, p5, p6, p7] = time_tx_poll.to_le_bytes();
        let [r0, r1, r2, r3, r4, r5, r6, r7] = time_rx_response.to_le_bytes();
        let [f0, f1, f2, f3, f4, f5, f6, f7] = time_tx_final.to_le_bytes();

        #[rustfmt::skip]
        let frame = [
            SENTINEL,
            Self::Final as u8,
            msg_id,
            p0, p1, p2, p3, p4, p5, p6, p7,
            r0, r1, r2, r3, r4, r5, r6, r7,
            f0, f1, f2, f3, f4, f5, f6, f7,
        ];
        frame
    }
}

impl TryFrom<u8> for MessageType {
    type Error = &'static str;

    fn try_from(msg_type: u8) -> Result<Self, Self::Error> {
        match msg_type {
            1 => Ok(Self::Poll),
            2 => Ok(Self::Response),
            3 => Ok(Self::Final),
            _ => Err("Invalid message type"),
        }
    }
}
