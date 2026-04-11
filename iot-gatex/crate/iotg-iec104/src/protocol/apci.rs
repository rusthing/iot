use bytes::{Bytes, BytesMut, BufMut};
use std::io;

pub const START: u8 = 0x68;
pub const HEADER: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    I { send_sn: u16, recv_sn: u16, apdu: Bytes },
    S { recv_sn: u16 },
    U(UType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UType {
    StartDtAct, StartDtCon,
    StopDtAct,  StopDtCon,
    TestFrAct,  TestFrCon,
}

impl UType {
    fn ctrl0(self) -> u8 {
        match self {
            Self::StartDtAct => 0x07, Self::StartDtCon => 0x0B,
            Self::StopDtAct  => 0x13, Self::StopDtCon  => 0x23,
            Self::TestFrAct  => 0x43, Self::TestFrCon  => 0x83,
        }
    }
    fn from_ctrl0(b: u8) -> Option<Self> {
        match b {
            0x07 => Some(Self::StartDtAct), 0x0B => Some(Self::StartDtCon),
            0x13 => Some(Self::StopDtAct),  0x23 => Some(Self::StopDtCon),
            0x43 => Some(Self::TestFrAct),  0x83 => Some(Self::TestFrCon),
            _ => None,
        }
    }
}

impl Frame {
    pub fn encode(&self) -> Bytes {
        let mut b = BytesMut::new();
        match self {
            Frame::I { send_sn, recv_sn, apdu } => {
                b.put_u8(START);
                b.put_u8((4 + apdu.len()) as u8);
                b.put_u8((send_sn << 1) as u8);
                b.put_u8((send_sn >> 7) as u8);
                b.put_u8((recv_sn << 1) as u8);
                b.put_u8((recv_sn >> 7) as u8);
                b.put_slice(apdu);
            }
            Frame::S { recv_sn } => {
                b.put_u8(START); b.put_u8(4);
                b.put_u8(0x01); b.put_u8(0x00);
                b.put_u8((recv_sn << 1) as u8);
                b.put_u8((recv_sn >> 7) as u8);
            }
            Frame::U(u) => {
                b.put_u8(START); b.put_u8(4);
                b.put_u8(u.ctrl0());
                b.put_slice(&[0u8; 3]);
            }
        }
        b.freeze()
    }

    pub fn decode(buf: &[u8]) -> io::Result<Option<(Frame, usize)>> {
        if buf.len() < HEADER { return Ok(None); }
        if buf[0] != START {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad start byte"));
        }
        let apdu_len = buf[1] as usize;
        if buf.len() < 2 + apdu_len { return Ok(None); }
        let total = 2 + apdu_len;
        let (c0, c1, c2, c3) = (buf[2], buf[3], buf[4], buf[5]);

        let frame = if c0 & 0x01 == 0 {
            Frame::I {
                send_sn: ((c0 as u16) >> 1) | ((c1 as u16) << 7),
                recv_sn: ((c2 as u16) >> 1) | ((c3 as u16) << 7),
                apdu: Bytes::copy_from_slice(&buf[HEADER..total]),
            }
        } else if c0 & 0x03 == 0x01 {
            Frame::S { recv_sn: ((c2 as u16) >> 1) | ((c3 as u16) << 7) }
        } else {
            match UType::from_ctrl0(c0) {
                Some(u) => Frame::U(u),
                None => return Err(io::Error::new(io::ErrorKind::InvalidData, "bad U-frame")),
            }
        };
        Ok(Some((frame, total)))
    }
}
