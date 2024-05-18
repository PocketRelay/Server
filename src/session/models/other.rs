use tdf::{ObjectId, TdfSerialize, TdfType};

use crate::utils::components::association_lists::ASSOC_LIST_REF;

/// Structure for the default response to offline game reporting
pub struct GameReportResponse;

impl TdfSerialize for GameReportResponse {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_var_int_list_empty(b"DATA");
        w.tag_zero(b"EROR");
        w.tag_zero(b"FNL");
        w.tag_zero(b"GHID");
        w.tag_zero(b"GRID");
    }
}

/// Structure for the default response to associated lists
pub struct AssocListResponse;

impl TdfSerialize for AssocListResponse {
    fn serialize<S: tdf::TdfSerializer>(&self, w: &mut S) {
        w.tag_list_start(b"LMAP", TdfType::Group, 1);
        w.group_body(|w| {
            w.group(b"INFO", |w| {
                w.tag_alt(
                    b"BOID",
                    ObjectId::new(ASSOC_LIST_REF, 0x74b09c4 /* ID of friends list? */),
                );
                w.tag_u8(b"FLGS", 4);

                w.group(b"LID", |w| {
                    w.tag_str(b"LNM", "friendList");
                    w.tag_u8(b"TYPE", 1);
                });

                w.tag_u8(b"LMS", 0xC8);
                w.tag_u8(b"PRID", 0);
            });
            w.tag_zero(b"OFRC");
            w.tag_zero(b"TOCT");
        });
    }
}
