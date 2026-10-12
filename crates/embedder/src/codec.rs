//! Lo `StandardMessageCodec` di Flutter, per quel poco che serve all'embedder.
//!
//! Formato (services/message_codecs.dart): un byte di tipo, poi il valore in
//! little-endian; le dimensioni sotto 254 stanno in un byte. Una risposta di
//! successo del `StandardMethodCodec` è il byte 0 seguito dal valore.

const INT32: u8 = 3;
const INT64: u8 = 4;
const MAP: u8 = 13;

fn write_size(out: &mut Vec<u8>, size: usize) {
    match (u8::try_from(size), u16::try_from(size)) {
        (Ok(byte), _) if byte < 254 => out.push(byte),
        (_, Ok(short)) => {
            out.push(254);
            out.extend_from_slice(&short.to_le_bytes());
        }
        _ => {
            out.push(255);
            out.extend_from_slice(&u32::try_from(size).unwrap_or(u32::MAX).to_le_bytes());
        }
    }
}

fn write_int(out: &mut Vec<u8>, value: i64) {
    match i32::try_from(value) {
        Ok(small) => {
            out.push(INT32);
            out.extend_from_slice(&small.to_le_bytes());
        }
        Err(_) => {
            out.push(INT64);
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
}

/// Risposta di successo con una mappa intero → intero.
pub fn success_int_map(entries: impl ExactSizeIterator<Item = (i64, i64)>) -> Vec<u8> {
    let mut out = vec![0, MAP];
    write_size(&mut out, entries.len());
    for (key, value) in entries {
        write_int(&mut out, key);
        write_int(&mut out, value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_dart() {
        assert_eq!(success_int_map(std::iter::empty()), [0, MAP, 0]);
        // 0x70004 sta in 32 bit; 0x10000001b no.
        let encoded = success_int_map([(0x7_0004, 0x1_0000_001b)].into_iter());
        let mut expected = vec![0, MAP, 1, INT32];
        expected.extend_from_slice(&0x7_0004i32.to_le_bytes());
        expected.push(INT64);
        expected.extend_from_slice(&0x1_0000_001bi64.to_le_bytes());
        assert_eq!(encoded, expected);
    }
}
