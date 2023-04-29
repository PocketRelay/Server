use blaze_pk::{codec::Encodable, tag::TdfType};

/// Structure for the default response to offline game reporting
pub struct GameReportResponse;

impl Encodable for GameReportResponse {
    fn encode(&self, writer: &mut blaze_pk::writer::TdfWriter) {
        writer.tag_var_int_list_empty(b"DATA");
        writer.tag_zero(b"EROR");
        writer.tag_zero(b"FNL");
        writer.tag_zero(b"GHID");
        writer.tag_zero(b"GRID");
    }
}

/// Structure for the default response to assocated lists
pub struct AssocListResponse;

impl Encodable for AssocListResponse {
    fn encode(&self, writer: &mut blaze_pk::writer::TdfWriter) {
        writer.tag_list_start(b"LMAP", TdfType::Group, 1);

        writer.group(b"INFO", |writer| {
            writer.tag_triple(b"BOID", (0x19, 0x1, 0x74b09c4));
            writer.tag_u8(b"FLGS", 4);

            writer.group(b"LID", |writer| {
                writer.tag_str(b"LNM", "friendList");
                writer.tag_u8(b"TYPE", 1);
            });

            writer.tag_u8(b"LMS", 0xC8);
            writer.tag_u8(b"PRID", 0);
        });
        writer.tag_zero(b"OFRC");
        writer.tag_zero(b"TOCT");
        writer.tag_group_end();
    }
}
