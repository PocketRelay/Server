use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::{fmt::Debug, sync::Arc};
use std::{io, ops::Deref};
use tdf::{
    serialize_vec, DecodeResult, TdfDeserialize, TdfDeserializer, TdfSerialize, TdfStringifier,
};
use tokio_util::codec::{Decoder, Encoder};

use crate::utils::components::{get_command_name, get_component_name};

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

impl Packet {
    /// Creates a packet from its raw components
    ///
    /// `header`   The packet header
    /// `contents` The encoded packet contents
    pub fn raw(header: PacketHeader, contents: Vec<u8>) -> Self {
        Self {
            header,
            contents: Bytes::from(contents),
        }
    }

    /// Creates a packet from its raw components
    /// where the contents are empty
    ///
    /// `header` The packet header
    pub const fn raw_empty(header: PacketHeader) -> Self {
        Self {
            header,
            contents: Bytes::new(),
        }
    }

    /// Creates a packet responding to the provided packet.
    /// Clones the header of the request packet and changes
    /// the type to repsonse
    ///
    /// `packet`   The packet to respond to
    /// `contents` The contents to encode for the packet
    pub fn response<C: TdfSerialize>(packet: &Packet, contents: C) -> Self {
        Self {
            header: packet.header.response(),
            contents: Bytes::from(serialize_vec(&contents)),
        }
    }

    /// Creates a packet responding to the current packet.
    /// Clones the header of the request packet and changes
    /// the type to repsonse
    ///
    /// `packet`   The packet to respond to
    /// `contents` The contents to encode for the packet
    pub fn respond<C: TdfSerialize>(&self, contents: C) -> Self {
        Self::response(self, contents)
    }

    /// Creates a response packet responding to the provided packet
    /// but with raw contents that have already been encoded.
    ///
    /// `packet`   The packet to respond to
    /// `contents` The raw encoded packet contents
    pub fn response_raw(packet: &Packet, contents: Vec<u8>) -> Self {
        Self {
            header: packet.header.response(),
            contents: Bytes::from(contents),
        }
    }

    /// Creates a response packet responding to the provided packet
    /// but with empty contents.
    ///
    /// `packet` The packet to respond to
    pub const fn response_empty(packet: &Packet) -> Self {
        Self {
            header: packet.header.response(),
            contents: Bytes::new(),
        }
    }

    /// Creates a response packet responding to the provided packet
    /// but with empty contents.
    ///
    /// `packet`   The packet to respond to
    /// `contents` The contents to encode for the packet
    pub const fn respond_empty(&self) -> Self {
        Self::response_empty(self)
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error and contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    /// `contents` The response contents
    pub fn error<C: TdfSerialize>(packet: &Packet, error: u16, contents: C) -> Self {
        Self {
            header: packet.header.with_error(error),
            contents: Bytes::from(serialize_vec(&contents)),
        }
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error and contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    /// `contents` The response contents
    pub fn respond_error<C: TdfSerialize>(&self, error: u16, contents: C) -> Self {
        Self::error(self, error, contents)
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error and raw encoded contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    /// `contents` The raw encoded contents
    pub fn error_raw(packet: &Packet, error: u16, contents: Vec<u8>) -> Self {
        Self {
            header: packet.header.with_error(error),
            contents: Bytes::from(contents),
        }
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error with empty contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    pub const fn error_empty(packet: &Packet, error: u16) -> Packet {
        Self {
            header: packet.header.with_error(error),
            contents: Bytes::new(),
        }
    }

    /// Creates a error respond packet responding to the provided
    /// packet with the provided error with empty contents
    ///
    /// `packet`   The packet to respond to
    /// `error`    The response error value
    pub const fn respond_error_empty(&self, error: u16) -> Packet {
        Self::error_empty(self, error)
    }

    /// Creates a notify packet for the provided component with the
    /// provided contents.
    ///
    /// `component` The packet component to use for the header
    /// `contents`  The contents of the packet to encode
    pub fn notify<C: TdfSerialize>(component: u16, command: u16, contents: C) -> Packet {
        Self {
            header: PacketHeader::notify(component, command),
            contents: Bytes::from(serialize_vec(&contents)),
        }
    }

    /// Creates a notify packet for the provided component with the
    /// provided raw encoded contents.
    ///
    /// `component` The packet component
    /// `contents`  The encoded packet contents
    pub fn notify_raw(component: u16, command: u16, contents: Vec<u8>) -> Packet {
        Self {
            header: PacketHeader::notify(component, command),
            contents: Bytes::from(contents),
        }
    }

    /// Creates a notify packet for the provided component with
    /// empty contents
    ///
    /// `component` The packet component
    pub fn notify_empty(component: u16, command: u16) -> Packet {
        Self {
            header: PacketHeader::notify(component, command),
            contents: Bytes::new(),
        }
    }

    /// Creates a new request packet from the provided id, component, and contents
    ///
    /// `id`        The packet id
    /// `component` The packet component
    /// `contents`  The packet contents
    pub fn request<C: TdfSerialize>(id: u16, component: u16, command: u16, contents: C) -> Packet {
        let (component, command) = component.values();
        Self {
            header: PacketHeader::request(id, component, command),
            contents: Bytes::from(serialize_vec(&contents)),
        }
    }

    /// Creates a new request packet from the provided id, component
    /// with raw encoded contents
    ///
    /// `id`        The packet id
    /// `component` The packet component
    /// `contents`  The raw encoded contents
    pub fn request_raw(id: u16, component: u16, command: u16, contents: Vec<u8>) -> Packet {
        Self {
            header: PacketHeader::request(id, component, command),
            contents: Bytes::from(contents),
        }
    }

    /// Creates a new request packet from the provided id, component
    /// with empty contents
    ///
    /// `id`        The packet id
    /// `component` The packet component
    /// `contents`  The packet contents
    pub fn request_empty(id: u16, component: u16, command: u16) -> Packet {
        Self {
            header: PacketHeader::request(id, component, command),
            contents: Bytes::new(),
        }
    }

    /// Attempts to decode the contents bytes of this packet into the
    /// provided Codec type value.
    pub fn decode<'de, C: TdfDeserialize<'de>>(&'de self) -> DecodeResult<C> {
        let mut r = TdfDeserializer::new(&self.contents);
        C::deserialize(&mut r)
    }

    /// Attempts to read a packet from the provided
    /// bytes source
    ///
    /// `src` The bytes to read from
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

    /// Writes the contents and header of the packet
    /// onto the dst source of bytes
    ///
    /// `dst` The destination buffer
    pub fn write(&self, dst: &mut BytesMut) {
        let contents = &self.contents;
        self.header.write(dst, contents.len());
        dst.extend_from_slice(contents);
    }
}

/// Tokio codec for encoding and decoding packets
pub struct PacketCodec;

/// Decoder implementation
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

/// Encoder implementation for owned packets
impl Encoder<Packet> for PacketCodec {
    type Error = io::Error;

    fn encode(&mut self, item: Packet, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.write(dst);
        Ok(())
    }
}

/// Encoder implementation for borrowed packets
impl Encoder<&Packet> for PacketCodec {
    type Error = io::Error;

    fn encode(&mut self, item: &Packet, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.write(dst);
        Ok(())
    }
}

/// Encoder implementation for arc reference packets
impl Encoder<Arc<Packet>> for PacketCodec {
    type Error = io::Error;

    fn encode(&mut self, item: Arc<Packet>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.write(dst);
        Ok(())
    }
}

/// Structure wrapping a from request type to include a packet
/// header to allow the response type to be created
pub struct Request<T: FromRequest> {
    /// The decoded request type
    pub req: T,
    /// The packet header from the request
    pub header: PacketHeader,
}

/// Deref implementation so that the request fields can be
/// directly accessed
impl<T: FromRequest> Deref for Request<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.req
    }
}

impl<T: FromRequest> Request<T> {
    /// Creates a response from the provided response type value
    /// returning a Response structure which can be used as a Route
    /// repsonse
    ///
    /// `res` The into response type implementation
    pub fn response<E>(&self, res: E) -> Response
    where
        E: TdfSerialize,
    {
        Response(Packet {
            header: self.header.response(),
            contents: Bytes::from(res.encode_bytes()),
        })
    }
}

/// Wrapping structure for raw Bytes structures that can
/// be used as packet response
pub struct PacketBody(Bytes);

impl<T> From<T> for PacketBody
where
    T: TdfSerialize,
{
    fn from(value: T) -> Self {
        let bytes = serialize_vec(&value);
        let bytes = Bytes::from(bytes);
        PacketBody(bytes)
    }
}

/// Type for route responses that have already been turned into
/// packets usually for lifetime reasons
pub struct Response(Packet);

impl IntoResponse for Response {
    /// Simply provide the already compute response
    fn into_response(self, _req: &Packet) -> Packet {
        self.0
    }
}

impl IntoResponse for PacketBody {
    fn into_response(self, req: &Packet) -> Packet {
        Packet {
            header: req.header.response(),
            contents: self.0,
        }
    }
}

impl<T: FromRequest> FromRequest for Request<T> {
    fn from_request(req: &Packet) -> DecodeResult<Self> {
        let inner = T::from_request(req)?;
        let header = req.header;
        Ok(Self { req: inner, header })
    }
}

/// Trait implementing by structures which can be created from a request
/// packet and is used for the arguments on routing functions
pub trait FromRequest: Sized + Send + 'static {
    /// Takes the value from the request returning a decode result of
    /// whether the value could be created
    ///
    /// `req` The request packet
    fn from_request(req: &Packet) -> DecodeResult<Self>;
}

impl<D> FromRequest for D
where
    for<'de> D: TdfDeserialize<'de> + Send + 'de,
{
    fn from_request(req: &Packet) -> DecodeResult<Self> {
        req.decode()
    }
}

/// Trait for a type that can be converted into a packet
/// response using the header from the request packet
pub trait IntoResponse: 'static {
    /// Into packet conversion
    fn into_response(self, req: &Packet) -> Packet;
}

/// Into response imeplementation for encodable responses
/// which just calls res.respond
impl<E> IntoResponse for E
where
    E: TdfSerialize + 'static,
{
    fn into_response(self, req: &Packet) -> Packet {
        req.respond(self)
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
        let mut str = TdfStringifier::new(r, &mut out);

        out.push_str("{\n");

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
