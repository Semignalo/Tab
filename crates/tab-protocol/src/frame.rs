//! Framing sesi: `u32` big-endian panjang + body.
//!
//! Header dan body ditulis dalam satu panggilan `write_all` lewat buffer yang dipakai ulang,
//! supaya dengan `TCP_NODELAY` satu frame benar-benar menjadi satu paket dan tidak ada
//! tambahan round-trip pada jalur input.

use crate::MAX_FRAME;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const LEN_PREFIX: usize = 4;

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("frame dengan panjang nol tidak sah")]
    Empty,
    #[error("frame {size} byte melebihi batas {limit} byte")]
    TooLarge { size: usize, limit: usize },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Tulis `len` + `body` ke `out` (isi `out` diganti).
pub fn encode(body: &[u8], out: &mut Vec<u8>) -> Result<(), FrameError> {
    if body.is_empty() {
        return Err(FrameError::Empty);
    }
    if body.len() > MAX_FRAME {
        return Err(FrameError::TooLarge {
            size: body.len(),
            limit: MAX_FRAME,
        });
    }
    out.clear();
    out.reserve(LEN_PREFIX + body.len());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
    Ok(())
}

/// Penulis frame dengan buffer yang dipakai ulang antar pemanggilan.
#[derive(Debug, Default)]
pub struct FrameWriter {
    scratch: Vec<u8>,
}

impl FrameWriter {
    pub fn new() -> Self {
        Self {
            scratch: Vec::with_capacity(2048),
        }
    }

    pub async fn write<W: AsyncWrite + Unpin>(
        &mut self,
        w: &mut W,
        body: &[u8],
    ) -> Result<(), FrameError> {
        encode(body, &mut self.scratch)?;
        w.write_all(&self.scratch).await?;
        Ok(())
    }
}

/// Baca satu frame ke `buf` (isi `buf` diganti) dan kembalikan panjangnya.
///
/// Frame yang melewati batas menghasilkan error dan pemanggil **wajib** menutup koneksi:
/// stream sudah tidak bisa disinkronkan ulang setelah header yang tidak masuk akal.
pub async fn read_frame<R: AsyncRead + Unpin>(
    r: &mut R,
    buf: &mut Vec<u8>,
) -> Result<usize, FrameError> {
    let mut header = [0u8; LEN_PREFIX];
    r.read_exact(&mut header).await?;
    let len = u32::from_be_bytes(header) as usize;
    if len == 0 {
        return Err(FrameError::Empty);
    }
    if len > MAX_FRAME {
        return Err(FrameError::TooLarge {
            size: len,
            limit: MAX_FRAME,
        });
    }
    buf.clear();
    buf.resize(len, 0);
    r.read_exact(buf).await?;
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_prefixes_big_endian_length() {
        let mut out = Vec::new();
        encode(&[0xaa, 0xbb, 0xcc], &mut out).unwrap();
        assert_eq!(out, vec![0, 0, 0, 3, 0xaa, 0xbb, 0xcc]);
    }

    #[test]
    fn encode_rejects_empty_and_oversized() {
        let mut out = Vec::new();
        assert!(matches!(encode(&[], &mut out), Err(FrameError::Empty)));
        let big = vec![0u8; MAX_FRAME + 1];
        assert!(matches!(
            encode(&big, &mut out),
            Err(FrameError::TooLarge { .. })
        ));
    }

    #[tokio::test]
    async fn roundtrip_many_frames_over_one_stream() {
        let bodies: Vec<Vec<u8>> = vec![vec![1], vec![2; 100], vec![3; MAX_FRAME]];
        let mut wire = Vec::new();
        let mut writer = FrameWriter::new();
        for b in &bodies {
            writer.write(&mut wire, b).await.unwrap();
        }

        let mut cursor = std::io::Cursor::new(wire);
        let mut buf = Vec::new();
        for expected in &bodies {
            let n = read_frame(&mut cursor, &mut buf).await.unwrap();
            assert_eq!(n, expected.len());
            assert_eq!(&buf, expected);
        }
        // Stream habis: pembacaan berikutnya harus gagal, bukan menggantung.
        assert!(read_frame(&mut cursor, &mut buf).await.is_err());
    }

    #[tokio::test]
    async fn read_rejects_oversized_header_without_reading_body() {
        let mut wire = Vec::new();
        wire.extend_from_slice(&((MAX_FRAME + 1) as u32).to_be_bytes());
        let mut cursor = std::io::Cursor::new(wire);
        let mut buf = Vec::new();
        assert!(matches!(
            read_frame(&mut cursor, &mut buf).await,
            Err(FrameError::TooLarge { .. })
        ));
    }
}
