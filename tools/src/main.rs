use std::{
    io::{self, Write},
    path::Path,
};

use blaze_pk::types::TdfMap;

use flate2::{write::ZlibEncoder, Compression};
use tokio::fs::{create_dir_all, read, read_dir, write};

#[tokio::main]
async fn main() {
    let root = Path::new("tools/input/tlk");
    let out_root = Path::new("tools/output/tlk");
    process_language_files(root, out_root).await.unwrap();

    let coal_path = Path::new("tools/input/coalesced.bin");
    let out_path = Path::new("tools/output/coalesced.dmap");
    process_coalesced(coal_path, out_path).await.unwrap();
}

pub async fn process_coalesced(path: &Path, out: &Path) -> io::Result<()> {
    let bytes = read(path).await?;
    println!("Non Compressed length: {}", bytes.len());
    let compressed = compress_coalesced(&bytes)?;
    println!("Compressed length: {}", compressed.len());
    let encoded = encode_coalesced(&bytes, &compressed);
    let map = base64_chunk_map(encoded);
    write_dmap(&out, map).await?;
    Ok(())
}

pub fn encode_coalesced(original: &[u8], compressed: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(16 + compressed.len());
    output.push('N' as u8);
    output.push('I' as u8);
    output.push('B' as u8);
    output.push('C' as u8);
    output.extend_from_slice(&1u32.to_le_bytes());
    output.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    output.extend_from_slice(&(original.len() as u32).to_le_bytes());
    output.extend_from_slice(compressed);
    output
}

pub fn compress_coalesced(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut encode = ZlibEncoder::new(Vec::new(), Compression::new(6));
    encode.write_all(&bytes)?;
    encode.finish()
}

pub async fn process_language_files(root: &Path, out_root: &Path) -> io::Result<()> {
    let mut files = read_dir(root).await?;
    while let Some(file) = files.next_entry().await? {
        let file_name = file.file_name();
        let file_name = file_name.to_string_lossy();
        if !file_name.ends_with(".tlk") {
            continue;
        }
        let mut out_file_name = file_name.to_string();
        out_file_name.push_str(".dmap");
        let out_path = out_root.join(out_file_name);
        let map = encode_talk_file(&file.path()).await?;
        write_dmap(&out_path, map).await?;
    }
    Ok(())
}

pub async fn write_dmap(path: &Path, map: TdfMap<String, String>) -> io::Result<()> {
    let mut out = String::new();
    for (key, value) in map.iter() {
        out.push_str(&key);
        out.push('=');
        out.push_str(&value);
        out.push('\n');
    }

    // Pop last new line
    out.pop();

    if let Some(path) = path.parent() {
        if !path.exists() {
            create_dir_all(path).await?;
        }
    }

    write(path, out).await?;
    Ok(())
}

pub async fn encode_talk_file(path: &Path) -> io::Result<TdfMap<String, String>> {
    let bytes = read(path).await?;
    Ok(base64_chunk_map(bytes))
}

pub fn base64_chunk_map(bytes: Vec<u8>) -> TdfMap<String, String> {
    const CHUNK_LENGTH: usize = 255;

    let mut output = TdfMap::new();
    let value = base64::encode(bytes);
    let length = value.len();

    let mut chars = value.chars();
    let mut position = 0;
    let mut index = 0;

    while position < length {
        let mut char_count = 0;
        for char in chars.by_ref().take(CHUNK_LENGTH) {
            char_count += char.len_utf8();
        }

        let value = &value[position..position + char_count];
        position += char_count;
        output.insert(format!("CHUNK_{index}"), value);
        index += 1;
    }
    output.insert("CHUNK_SIZE", "255");
    output.insert("DATA_SIZE", length.to_string());
    output.order();
    output
}
