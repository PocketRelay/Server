use std::io::Write;

use base64ct::{Base64, Encoding};
use flate2::{write::ZlibEncoder, Compression};
use tdf::TdfMap;

/// Type of a base64 chunks map
pub type ChunkMap = TdfMap<String, String>;

/// Converts to provided slice of bytes into an ordered TdfMap where
/// the keys are the chunk index and the values are the bytes encoded
/// as base64 chunks. The map contains a CHUNK_SIZE key which states
/// how large each chunk is and a DATA_SIZE key indicating the total
/// length of the chunked value
pub fn create_base64_map(bytes: &[u8]) -> ChunkMap {
    // The size of the chunks
    const CHUNK_LENGTH: usize = 255;

    let encoded: String = Base64::encode_string(bytes);
    let length = encoded.len();
    let mut output: ChunkMap = TdfMap::with_capacity((length / CHUNK_LENGTH) + 2);

    let mut index = 0;
    let mut offset = 0;

    while offset < length {
        let o1 = offset;
        offset += CHUNK_LENGTH;

        let slice = if offset < length {
            &encoded[o1..offset]
        } else {
            &encoded[o1..]
        };

        output.insert(format!("CHUNK_{}", index), slice.to_string());
        index += 1;
    }

    output.insert("CHUNK_SIZE".to_string(), CHUNK_LENGTH.to_string());
    output.insert("DATA_SIZE".to_string(), length.to_string());
    output
}

/// Generates a compressed coalesced from the provided bytes
pub fn generate_coalesced(bytes: &[u8]) -> std::io::Result<ChunkMap> {
    let compressed: Vec<u8> = {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(6));
        encoder.write_all(bytes)?;
        encoder.finish()?
    };

    let mut encoded = Vec::with_capacity(16 + compressed.len());
    encoded.extend_from_slice(b"NIBC");
    encoded.extend_from_slice(&1u32.to_le_bytes());
    encoded.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    encoded.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    encoded.extend_from_slice(&compressed);
    Ok(create_base64_map(&encoded))
}
