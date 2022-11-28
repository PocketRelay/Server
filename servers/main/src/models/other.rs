use blaze_pk::{codec::Codec, tag::ValueType, tagging::*};

/// Structure for the default response to offline game reporting
pub struct GameReportResponse;

impl Codec for GameReportResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_var_int_list_empty(output, "DATA");
        tag_zero(output, "EROR");
        tag_zero(output, "FNL");
        tag_zero(output, "GHID");
        tag_zero(output, "GRID");
    }
}

/// Structure for the default response to assocated lists
pub struct AssocListResponse;

impl Codec for AssocListResponse {
    fn encode(&self, output: &mut Vec<u8>) {
        tag_list_start(output, "LMAP", ValueType::Group, 1);
        {
            tag_group_start(output, "INFO");

            tag_triple(output, "BOID", &(0x19, 0x1, 0x74b09c4));
            tag_u8(output, "FLGS", 4);

            {
                tag_group_start(output, "LID");
                tag_str(output, "LNM", "friendList");
                tag_u8(output, "TYPE", 1);
                tag_group_end(output);
            }

            tag_u8(output, "LMS", 0xC8);
            tag_u8(output, "PRID", 0);

            tag_group_end(output);
        }
        tag_zero(output, "OFRC");
        tag_zero(output, "TOCT");
        tag_group_end(output);
    }
}
