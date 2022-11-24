use blaze_pk::{codec::Reader, packet::Packet, tag::Tag};

pub mod codec;
pub mod components;
pub mod errors;

/// Decodes the provided packet into its string representation and appends
/// the value to the provided output prefixed by Content: if an error
/// occurs while decoding the raw values and decoding error will
/// be appended to the output
///
/// `packet` The packet to decode
/// `output` The output to append to
pub fn append_packet_decoded(packet: &Packet, output: &mut String) {
    let mut reader = Reader::new(&packet.contents);
    let mut out = String::new();
    out.push_str("{\n");
    if let Err(err) = Tag::stringify(&mut reader, &mut out, 1) {
        output.push_str("\nExtra: Content was malformed");
        output.push_str(&format!("\nError: {:?}", err));

        output.push_str("\nnPartial Content: ");
        output.push_str(&out);

        output.push_str(&format!("\nRaw: {:?}", &packet.contents));
        return;
    }
    if out.len() == 2 {
        // Remove new line if nothing else was appended
        out.pop();
    }
    out.push('}');
    output.push_str("\nContent: ");
    output.push_str(&out);
}
