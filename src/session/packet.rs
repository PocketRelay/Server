#![allow(unused)]

use crate::utils::components::{get_command_name, get_component_name};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::{fmt::Debug, sync::Arc};
use std::{io, ops::Deref};
use tdf::{
    serialize_vec, DecodeResult, TdfDeserialize, TdfDeserializer, TdfSerialize, TdfStringifier,
};
use tokio_util::codec::{Decoder, Encoder};

/// The different types of packets
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    /// ID counted request packets (0x00)
    Request = 0x00,
    /// Packets responding to requests (0x10)
    Response = 0x10,
    /// Unique packets coming from the server (0x20)
    Notify = 0x20,
    /// Error packets (0x30)
    Error = 0x30,
}

/// From u8 implementation to convert bytes back into
/// PacketTypes
impl From<u8> for PacketType {
    fn from(value: u8) -> Self {
        match value {
            0x00 => PacketType::Request,
            0x10 => PacketType::Response,
            0x20 => PacketType::Notify,
            0x30 => PacketType::Error,
            // Default type fallback to request
            _ => PacketType::Request,
        }
    }
}

/// Structure of packet header which comes before the
/// packet content and describes it.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PacketHeader {
    /// The component of this packet
    pub component: u16,
    /// The command of this packet
    pub command: u16,
    /// A possible error this packet contains (zero is none)
    pub error: u16,
    /// The type of this packet
    pub ty: PacketType,
    /// The unique ID of this packet (Notify packets this is just zero)
    pub id: u16,
}

impl PacketHeader {
    /// Creates a notify header for the provided component and command
    ///
    /// `component` The component to use
    /// `command`   The command to use
    pub const fn notify(component: u16, command: u16) -> Self {
        Self {
            component,
            command,
            error: 0,
            ty: PacketType::Notify,
            id: 0,
        }
    }

    /// Creates a request header for the provided id, component
    /// and command
    ///
    /// `id`        The packet ID
    /// `component` The component to use
    /// `command`   The command to use
    pub const fn request(id: u16, component: u16, command: u16) -> Self {
        Self {
            component,
            command,
            error: 0,
            ty: PacketType::Request,
            id,
        }
    }

    /// Creates a response to the provided packet header by
    /// changing the type of the header
    pub const fn response(&self) -> Self {
        self.with_type(PacketType::Response)
    }

    /// Copies the header contents changing its Packet Type
    ///
    /// `ty` The new packet type
    pub const fn with_type(&self, ty: PacketType) -> Self {
        Self {
            component: self.component,
            command: self.command,
            error: self.error,
            ty,
            id: self.id,
        }
    }

    /// Copies the header contents changing its Packet Type
    pub const fn with_error(&self, error: u16) -> Self {
        Self {
            component: self.component,
            command: self.command,
            error,
            ty: PacketType::Error,
            id: self.id,
        }
    }

    /// Checks if the component and command of this packet header matches
    /// that of the other packet header
    ///
    /// `other` The packet header to compare to
    pub fn path_matches(&self, other: &PacketHeader) -> bool {
        self.component.eq(&other.component) && self.command.eq(&other.command)
    }

    /// Encodes the contents of this header appending to the
    /// output source
    ///
    /// `dst`    The dst to append the bytes to
    /// `length` The length of the content after the header
    pub fn write(&self, dst: &mut BytesMut, length: usize) {
        let is_extended = length > 0xFFFF;
        dst.put_u16(length as u16);
        dst.put_u16(self.component);
        dst.put_u16(self.command);
        dst.put_u16(self.error);
        dst.put_u8(self.ty as u8);
        dst.put_u8(if is_extended { 0x10 } else { 0x00 });
        dst.put_u16(self.id);
        if is_extended {
            dst.put_u8(((length & 0xFF000000) >> 24) as u8);
            dst.put_u8(((length & 0x00FF0000) >> 16) as u8);
        }
    }

    /// Attempts to read the packet header from the provided
    /// source bytes returning None if there aren't enough bytes
    ///
    /// `src` The bytes to read from
    pub fn read(src: &mut BytesMut) -> Option<(PacketHeader, usize)> {
        if src.len() < 12 {
            return None;
        }

        let mut length = src.get_u16() as usize;
        let component = src.get_u16();
        let command = src.get_u16();
        let error = src.get_u16();
        let ty = src.get_u8();
        // If we encounter 0x10 here then the packet contains extended length
        // bytes so its longer than a u16::MAX length
        let is_extended = src.get_u8() == 0x10;
        let id = src.get_u16();

        if is_extended {
            // We need another two bytes for the extended length
            if src.len() < 2 {
                return None;
            }
            length += src.get_u16() as usize;
        }

        let ty = PacketType::from(ty);
        let header = PacketHeader {
            component,
            command,
            error,
            ty,
            id,
        };
        Some((header, length))
    }
}

/// Structure for Blaze packets contains the contents of the packet
/// and the header for identification.
///
/// Packets can be cloned with little memory usage increase because
/// the content is stored as Bytes.
#[derive(Debug, Clone)]
pub struct Packet {
    /// The packet header
    pub header: PacketHeader,
    /// The packet encoded byte contents
    pub contents: Bytes,
}

fn serialize_bytes<V>(value: &V) -> Bytes
where
    V: TdfSerialize,
{
    Bytes::from(serialize_vec(value))
}

impl Packet {
    /// Creates a new packet from the provided header and contents
    pub const fn new(header: PacketHeader, contents: Bytes) -> Self {
        Self { header, contents }
    }

    /// Creates a new packet from the provided header with empty content
    #[inline]
    pub const fn new_empty(header: PacketHeader) -> Self {
        Self::new(header, Bytes::new())
    }

    #[inline]
    pub const fn new_request(id: u16, component: u16, command: u16, contents: Bytes) -> Packet {
        Self::new(PacketHeader::request(id, component, command), contents)
    }

    #[inline]
    pub const fn new_response(packet: &Packet, contents: Bytes) -> Self {
        Self::new(packet.header.response(), contents)
    }

    #[inline]
    pub const fn new_error(packet: &Packet, error: u16, contents: Bytes) -> Self {
        Self::new(packet.header.with_error(error), contents)
    }

    #[inline]
    pub const fn new_notify(component: u16, command: u16, contents: Bytes) -> Packet {
        Self::new(PacketHeader::notify(component, command), contents)
    }

    #[inline]
    pub const fn request_empty(id: u16, component: u16, command: u16) -> Packet {
        Self::new_empty(PacketHeader::request(id, component, command))
    }

    #[inline]
    pub const fn response_empty(packet: &Packet) -> Self {
        Self::new_empty(packet.header.response())
    }

    #[inline]
    pub const fn error_empty(packet: &Packet, error: u16) -> Packet {
        Self::new_empty(packet.header.with_error(error))
    }

    #[inline]
    pub const fn notify_empty(component: u16, command: u16) -> Packet {
        Self::new_empty(PacketHeader::notify(component, command))
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
        let (header, length) = PacketHeader::read(src)?;

        if src.len() < length {
            return None;
        }

        let contents = src.split_to(length);
        Some(Self {
            header,
            contents: contents.freeze(),
        })
    }

    pub fn write(&self, dst: &mut BytesMut) {
        let contents = &self.contents;
        self.header.write(dst, contents.len());
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

    /// Decide whether to display the contents of the packet
    pub minified: bool,
}

impl<'a> Debug for PacketDebug<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Append basic header information
        let header = &self.packet.header;

        let component_name = get_component_name(header.component);
        let command_name = get_command_name(
            header.component,
            header.command,
            matches!(&header.ty, PacketType::Notify),
        );

        match (component_name, command_name) {
            (Some(component), Some(command)) => {
                writeln!(f, "Component: {}({})", component, command)?;
            }
            (Some(component), None) => {
                writeln!(f, "Component: {}({:#06x})", component, header.command)?;
            }
            _ => {
                writeln!(
                    f,
                    "Component: {:#06x}({:#06x})",
                    header.component, header.command
                )?;
            }
        }

        writeln!(f, "Type: {:?}", header.ty)?;

        if !matches!(&header.ty, PacketType::Notify) {
            writeln!(f, "ID: {}", &header.id)?;
        }

        if let PacketType::Error = &header.ty {
            writeln!(f, "Error: {:#06x}", &header.error)?;
        }

        // Skip remaining if the message shouldn't contain its content
        if self.minified {
            return Ok(());
        }

        let mut r = TdfDeserializer::new(&self.packet.contents);
        let mut out = String::new();
        out.push_str("{\n");
        let mut str = TdfStringifier::new(r, &mut out);

        // Stringify the content or append error instead
        if !str.stringify() {
            writeln!(f, "Content Error: Content was malformed or not parsible")?;
            writeln!(f, "Partial Content: {}", out)?;
            writeln!(f, "Raw: {:?}", &self.packet.contents)?;
            return Ok(());
        }

        if out.len() == 2 {
            // Remove new line if nothing else was appended
            out.pop();
        }

        out.push('}');

        write!(f, "Content: {}", out)
    }
}
