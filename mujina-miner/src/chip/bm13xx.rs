mod crc;

use bitvec::prelude::*;
use bytes::{Buf, BytesMut, BufMut};
use std::io;
use tokio_util::codec::{Encoder, Decoder};

use crate::tracing::prelude::*;
use crc::*;


#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum Register {
    ChipAddress = 0,
}

impl TryFrom<u8> for Register {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            value if value == Register::ChipAddress  as u8 => Ok(Register::ChipAddress),
            _ => Err(()),
        }
    }
}

pub enum Command {
    ReadRegister { all: bool, address: u8, register: Register },
}

struct CommandFieldBuilder {
    field: u8,
}

#[repr(u8)]
enum CommandFieldType {
    // Job = 1,
    Command = 2,
}

#[repr(u8)]
enum CommandFieldCmd {
    // SetAddress = 0,
    // WriteRegisterOrJob = 1,
    ReadRegister = 2,
    // ChainInactive = 3,
}

impl CommandFieldBuilder {
    fn new() -> Self {
        Self { field: 0 }
    }

    fn with_type(mut self, command_type: CommandFieldType) -> Self {
        let view = self.field.view_bits_mut::<Lsb0>();
        view[5..7].store(command_type as u8);
        self
    }

    fn with_type_for_command(self, command: &Command) -> Self {
        self.with_type(
            match command {
                Command::ReadRegister {..} => CommandFieldType::Command,
            }
        )
    }

    fn with_all(mut self, all: &bool) -> Self {
        let view = self.field.view_bits_mut::<Lsb0>();
        view[4..5].store(*all as u8);
        self
    }

    fn with_all_for_command(self, command: &Command) -> Self {
        self.with_all(
            match command {
                Command::ReadRegister {all, ..} => all,
            }
        )
    }

    fn with_cmd(mut self, cmd: CommandFieldCmd) -> Self {
        let view = self.field.view_bits_mut::<Lsb0>();
        view[0..4].store(cmd as u8);
        self
    }

    fn with_cmd_for_command(self, command: &Command) -> Self {
        self.with_cmd(
            match command {
                Command::ReadRegister {..} => CommandFieldCmd::ReadRegister,
            }
        )
    }

    fn for_command(self, command: &Command) -> Self {
        self.with_type_for_command(command)
            .with_all_for_command(command)
            .with_cmd_for_command(command)
    }

    fn build(self) -> u8 {
        self.field
    }
}

#[derive(Default)]
pub struct FrameCodec {
    // Controls whether to use the alternative frame format required when version rolling 
    // is enabled. When true, uses version rolling compatible format. (default: false)
    version_rolling: bool,
}

impl Encoder<Command> for FrameCodec {
    type Error = io::Error;

    fn encode(&mut self, command: Command, dst: &mut BytesMut) -> Result<(), Self::Error> {
        const COMMAND_PREAMBLE: &[u8] = &[0x55, 0xaa];
        dst.put_slice(COMMAND_PREAMBLE);

        let command_field = CommandFieldBuilder::new().for_command(&command).build();
        dst.put_u8(command_field);

        match command {
            Command::ReadRegister { all: _, address, register } => {
                const LENGTH: u8 = 5;
                dst.put_u8(LENGTH);
                dst.put_u8(address);
                dst.put_u8(register as u8);
            }
        }

        let crc = crc5(&dst[2..]);
        dst.put_u8(crc);

        Ok(())
    }
}

pub enum Response {
    RegisterValue { value: u32, chip: u8, register: Register },
    Nonce,
}

#[repr(u8)]
enum ResponseType {
    Command = 0,
    Job = 4,
}

impl TryFrom<u8> for ResponseType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            value if value == ResponseType::Command as u8 => Ok(ResponseType::Command),
            value if value == ResponseType::Job as u8 => Ok(ResponseType::Job),
            _ => Err(()),
        }
    }
}

impl Decoder for FrameCodec {
    type Item = Response;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Return Ok(Item) with a valid frame, or Ok(None) if to be called again, potentially with
        // more data. Returning an Error causes the stream to be terminated, so don't do that.
        //
        // There are three cases:
        //
        // 1. More data needed
        // 2. Invalid frame
        // 3. Valid frame
        //
        // In the case of an invalid frame, consume the first byte and request another call by
        // returning Ok(None). In the case of a valid frame, consume that frame's worth of bytes.

        const PREAMBLE: &[u8] = &[0xaa, 0x55];
        const NONROLLING_FRAME_LEN: usize = PREAMBLE.len() + 7;
        const ROLLING_FRAME_LEN: usize = PREAMBLE.len() + 9;

        let frame_len = if self.version_rolling {
            ROLLING_FRAME_LEN
        } else {
            NONROLLING_FRAME_LEN
        };

        if src.len() < frame_len {
            return Ok(None);
        }

        let mut prospect = src.clone();  // avoid consuming real buffer as we provisionally parse

        if prospect.get_u8() != PREAMBLE[0] {
            src.advance(1);
            return Ok(None);
        }

        if prospect.get_u8() != PREAMBLE[1] {
            src.advance(1);
            return Ok(None);
        }

        if crc5_is_valid(&src[PREAMBLE.len()..]) {
            src.advance(frame_len);
        } else {
            src.advance(1);
            return Ok(None);
        }

        let kind_and_crc = prospect[prospect.len() - 1].view_bits::<Lsb0>();
        let kind = kind_and_crc[5..].load::<u8>();

        Ok(match ResponseType::try_from(kind) {
            Ok(ResponseType::Command) => {
                let value = prospect.get_u32();
                let chip = prospect.get_u8();
                let register_address = prospect.get_u8();

                if let Ok(register) = Register::try_from(register_address) {
                    Some(Response::RegisterValue { value, chip, register })
                } else {
                    warn!("Unknown register 0x{:x} in response from chip.", register_address);
                    None
                }
            },
            Ok(ResponseType::Job) => {
                Some(Response::Nonce)
            },
            _ => {
                warn!("Unknown response type 0x{:x} from chip.", kind);
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_read_register() {
        assert_frame_eq(
            Command::ReadRegister {
                all: true,
                address: 0,
                register: Register::ChipAddress,
            },
            &[0x55, 0xaa, 0x52, 0x05, 0x00, 0x00, 0x0a],
        );
    }

    #[test]
    fn response_register_value() { 
        let wire = &[0xaa, 0x55, 0x13, 0x70, 0x00, 0x00, 0x00, 0x00, 0x06];
        let result = decode_frame(wire);
        
        if let Some(Response::RegisterValue { value, chip, register }) = result {
            eprint!("value {:x}", &value);
            assert_eq!(value, 0x1370_0000);
            assert_eq!(chip, 0x00);
            assert_eq!(register, Register::ChipAddress);
        } else {
            panic!();
        }
    }

    fn as_hex(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<String>>()
            .join(" ")
    }

    fn assert_frame_eq(cmd: Command, expect: &[u8]) {
        let mut codec = FrameCodec::default();
        let mut frame = BytesMut::new();
        codec.encode(cmd, &mut frame).unwrap();
        if frame != expect {
            panic!(
                "mismatch!\nexpected: {}\nactual: {}",
                as_hex(expect),
                as_hex(&frame[..])
            )
        }
    }

    fn decode_frame(frame: &[u8]) -> Option<Response> {
        let mut buf = BytesMut::from(frame);
        let mut codec = FrameCodec::default();
        codec.decode(&mut buf).unwrap()
    }
}
