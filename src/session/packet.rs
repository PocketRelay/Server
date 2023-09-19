use crate::utils::components::{
    component_key, get_command_name, get_component_name, OMIT_PACKET_CONTENTS,
};
use bitflags::bitflags;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::fmt::Debug;
use std::io;
use tdf::{prelude::*, serialize_vec};
use tokio_util::codec::{Decoder, Encoder};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    /// Request to a server
    Request = 0x0,
    /// Response to a request
    Response = 0x1,
    /// Async notification from the server
    Notify = 0x2,
    /// Error response from the server
    Error = 0x3,
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct PacketOptions: u8 {
        const NONE = 0x0;
        /// Frame length is extended from 16bits to 32bits
        const JUMBO_FRAME = 0x1;
        const HAS_CONTEXT = 0x2;
        const IMMEDIATE = 0x4;
        const JUMBO_CONTEXT = 0x8;
    }
}

impl From<u8> for FrameType {
    fn from(value: u8) -> Self {
        match value {
            0x0 => FrameType::Request,
            0x1 => FrameType::Response,
            0x2 => FrameType::Notify,
            0x3 => FrameType::Error,
            _ => FrameType::Request,
        }
    }
}

/// Framing structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FireFrame {
    /// The component that should handle this frame
    pub component: u16,
    /// The command this frame is for
    pub command: u16,
    /// Error code if present, otherwise zero
    pub error: u16,
    /// The type of frame
    pub ty: FrameType,
    /// Additional options assocaited with this frame
    pub options: PacketOptions,
    /// Sequence number for tracking request and response mappings
    pub seq: u16,
}

impl FireFrame {
    const MIN_HEADER_SIZE: usize = 12;
    const JUMBO_SIZE: usize = std::mem::size_of::<u16>();

    pub const fn notify(component: u16, command: u16) -> Self {
        Self {
            component,
            command,
            error: 0,
            ty: FrameType::Notify,
            options: PacketOptions::NONE,
            seq: 0,
        }
    }

    pub const fn request(seq: u16, component: u16, command: u16) -> Self {
        Self {
            component,
            command,
            error: 0,
            ty: FrameType::Request,
            options: PacketOptions::NONE,
            seq,
        }
    }

    pub const fn response(&self) -> Self {
        self.with_type(FrameType::Response)
    }

    pub const fn with_type(&self, ty: FrameType) -> Self {
        Self {
            component: self.component,
            command: self.command,
            error: self.error,
            ty,
            options: PacketOptions::NONE,
            seq: self.seq,
        }
    }

    pub const fn with_error(&self, error: u16) -> Self {
        Self {
            component: self.component,
            command: self.command,
            error,
            ty: FrameType::Error,
            options: PacketOptions::NONE,
            seq: self.seq,
        }
    }

    pub fn path_matches(&self, other: &FireFrame) -> bool {
        self.component.eq(&other.component) && self.command.eq(&other.command)
    }

    pub fn write(&self, dst: &mut BytesMut, length: usize) {
        let mut options = self.options;
        if length > 0xFFFF {
            options |= PacketOptions::JUMBO_FRAME;
        }

        dst.put_u16(length as u16);
        dst.put_u16(self.component);
        dst.put_u16(self.command);
        dst.put_u16(self.error);
        dst.put_u8((self.ty as u8) << 4);
        dst.put_u8(options.bits() << 4);
        dst.put_u16(self.seq);

        if options.contains(PacketOptions::JUMBO_FRAME) {
            // Put the extended length (The next 16 bits of the value to make the 32bit length)
            dst.put_u16((length >> 16) as u16);
        }
    }

    pub fn read(src: &mut BytesMut) -> Option<(FireFrame, usize)> {
        if src.len() < Self::MIN_HEADER_SIZE {
            return None;
        }

        let mut length = src.get_u16() as usize;
        let component = src.get_u16();
        let command = src.get_u16();
        let error = src.get_u16();
        let ty = src.get_u8() >> 4;
        let options = src.get_u8() >> 4;
        let options = PacketOptions::from_bits_retain(options);
        let seq = src.get_u16();

        if options.contains(PacketOptions::JUMBO_FRAME) {
            // We need another two bytes for the extended length
            if src.len() < Self::JUMBO_SIZE {
                return None;
            }
            let ext_length = (src.get_u16() as usize) << 16;
            length |= ext_length;
        }

        let ty = FrameType::from(ty);
        let header = FireFrame {
            component,
            command,
            error,
            ty,
            options,
            seq,
        };
        Some((header, length))
    }
}

#[derive(Debug, Clone)]
pub struct Packet {
    /// The frame preceeding this packet
    pub frame: FireFrame,
    /// The encoded contents of the packet
    pub contents: Bytes,
}

fn serialize_bytes<V>(value: &V) -> Bytes
where
    V: TdfSerialize,
{
    Bytes::from(serialize_vec(value))
}

#[allow(unused)]
impl Packet {
    /// Creates a new packet from the provided header and contents
    pub const fn new(header: FireFrame, contents: Bytes) -> Self {
        Self {
            frame: header,
            contents,
        }
    }

    /// Creates a new packet from the provided header with empty content
    #[inline]
    pub const fn new_empty(header: FireFrame) -> Self {
        Self::new(header, Bytes::new())
    }

    #[inline]
    pub const fn new_request(id: u16, component: u16, command: u16, contents: Bytes) -> Packet {
        Self::new(FireFrame::request(id, component, command), contents)
    }

    #[inline]
    pub const fn new_response(packet: &Packet, contents: Bytes) -> Self {
        Self::new(packet.frame.response(), contents)
    }

    #[inline]
    pub const fn new_error(packet: &Packet, error: u16, contents: Bytes) -> Self {
        Self::new(packet.frame.with_error(error), contents)
    }

    #[inline]
    pub const fn new_notify(component: u16, command: u16, contents: Bytes) -> Packet {
        Self::new(FireFrame::notify(component, command), contents)
    }

    #[inline]
    pub const fn request_empty(id: u16, component: u16, command: u16) -> Packet {
        Self::new_empty(FireFrame::request(id, component, command))
    }

    #[inline]
    pub const fn response_empty(packet: &Packet) -> Self {
        Self::new_empty(packet.frame.response())
    }

    #[inline]
    pub const fn error_empty(packet: &Packet, error: u16) -> Packet {
        Self::new_empty(packet.frame.with_error(error))
    }

    #[inline]
    pub const fn notify_empty(component: u16, command: u16) -> Packet {
        Self::new_empty(FireFrame::notify(component, command))
    }

    #[inline]
    pub fn response<V>(packet: &Packet, contents: V) -> Self
    where
        V: TdfSerialize,
    {
        Self::new_response(packet, serialize_bytes(&contents))
    }

    #[inline]
    pub fn error<V>(packet: &Packet, error: u16, contents: V) -> Self
    where
        V: TdfSerialize,
    {
        Self::new_error(packet, error, serialize_bytes(&contents))
    }

    #[inline]
    pub fn notify<V>(component: u16, command: u16, contents: V) -> Packet
    where
        V: TdfSerialize,
    {
        Self::new_notify(component, command, serialize_bytes(&contents))
    }

    #[inline]
    pub fn request<V>(id: u16, component: u16, command: u16, contents: V) -> Packet
    where
        V: TdfSerialize,
    {
        Self::new_request(id, component, command, serialize_bytes(&contents))
    }

    /// Attempts to deserialize the packet contents as the provided type
    pub fn deserialize<'de, V>(&'de self) -> DecodeResult<V>
    where
        V: TdfDeserialize<'de>,
    {
        let mut r = TdfDeserializer::new(&self.contents);
        V::deserialize(&mut r)
    }

    pub fn read(src: &mut BytesMut) -> Option<Self> {
        let (frame, length) = FireFrame::read(src)?;

        if src.len() < length {
            return None;
        }

        let contents = src.split_to(length);
        Some(Self {
            frame,
            contents: contents.freeze(),
        })
    }

    pub fn write(&self, dst: &mut BytesMut) {
        let contents = &self.contents;
        self.frame.write(dst, contents.len());
        dst.extend_from_slice(contents);
    }
}

/// Tokio codec for encoding and decoding packets
pub struct PacketCodec;

impl Decoder for PacketCodec {
    type Error = io::Error;
    type Item = Packet;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut read_src = src.clone();
        let result = Packet::read(&mut read_src);

        if result.is_some() {
            *src = read_src;
        }

        Ok(result)
    }
}

impl Encoder<Packet> for PacketCodec {
    type Error = io::Error;

    fn encode(&mut self, item: Packet, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.write(dst);
        Ok(())
    }
}

/// Wrapper over a packet structure to provde debug logging
/// with names resolved for the component
pub struct PacketDebug<'a> {
    /// Reference to the packet itself
    pub packet: &'a Packet,
}

impl<'a> Debug for PacketDebug<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Append basic header information
        let header = &self.packet.frame;

        let key = component_key(header.component, header.command);

        let is_notify = matches!(&header.ty, FrameType::Notify);
        let is_error = matches!(&header.ty, FrameType::Error);

        let component_name = get_component_name(header.component).unwrap_or("Unknown");
        let command_name = get_command_name(key, is_notify).unwrap_or("Unkown");

        write!(f, "{:?}", header.ty)?;

        if is_error {
            // Write sequence number and error for errors
            write!(f, " ({}, E?{:#06x})", header.seq, header.error)?;
        } else if !is_notify {
            // Write sequence number of sequenced types
            write!(f, " ({})", header.seq)?;
        }

        writeln!(
            f,
            ": {}->{} ({:#06x}->{:#06x})",
            component_name, command_name, header.component, header.command
        )?;

        let omit_content = OMIT_PACKET_CONTENTS.contains(&key);

        writeln!(f, "Options: {:?}", header.options)?;

        // Skip remaining if the message shouldn't contain its content
        if omit_content {
            return Ok(());
        }

        write!(f, "Content: ")?;

        let r = TdfDeserializer::new(&self.packet.contents);
        let mut str = TdfStringifier::new(r, f);

        if !str.stringify() {
            // Write the raw content if stringify doesn't complete
            writeln!(&mut str.w, "Raw: {:?}", &self.packet.contents)?;
        }

        Ok(())
    }
}
