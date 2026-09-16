use crate::jpeg::EXIF_SIGNATURE;

#[derive(Debug)]
pub enum ExifError {
    NotExif,
    BadTiffHeader,
    Truncated,
    BadOffset,
    UnsupportedType(u16),
}

impl std::fmt::Display for ExifError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExifError::NotExif => write!(f, "APP1 segment does not start with the Exif signature"),
            ExifError::BadTiffHeader => write!(f, "malformed TIFF header (bad byte order or magic number)"),
            ExifError::Truncated => write!(f, "IFD entry points outside the segment data"),
            ExifError::BadOffset => write!(f, "IFD entry offset is invalid or overflows"),
            ExifError::UnsupportedType(t) => write!(f, "field type {t} is not recognized"),
        }
    }
}

/// IFD0 tag whose value is the byte offset (from the start of the TIFF
/// header) of the EXIF sub-IFD.
const TAG_EXIF_IFD_POINTER: u16 = 0x8769;
/// IFD0 tag whose value is the byte offset of the GPS IFD.
const TAG_GPS_IFD_POINTER: u16 = 0x8825;

#[derive(Debug)]
pub struct ExifData {
    pub little_endian: bool,
    pub ifd0: Vec<IfdEntry>,
    pub exif_ifd: Vec<IfdEntry>,
    pub gps_ifd: Vec<IfdEntry>,
}

#[derive(Debug)]
pub struct IfdEntry {
    pub tag: u16,
    pub field_type: u16,
    pub count: u32,
    pub value: Value,
}

#[derive(Debug)]
pub enum Value {
    Ascii(String),
    Short(Vec<u16>),
    Long(Vec<u32>),
    Rational(Vec<(u32, u32)>),
    Unknown { field_type: u16, count: u32 },
}

/// Parses the contents of a JPEG APP1 segment as EXIF/TIFF data,
/// validating every offset against the segment's own bounds so a
/// corrupt or hostile file can't make us read past the buffer.
pub fn parse(app1_data: &[u8]) -> Result<ExifData, ExifError> {
    if app1_data.len() < EXIF_SIGNATURE.len() || &app1_data[..EXIF_SIGNATURE.len()] != EXIF_SIGNATURE {
        return Err(ExifError::NotExif);
    }
    let tiff = &app1_data[EXIF_SIGNATURE.len()..];
    if tiff.len() < 8 {
        return Err(ExifError::Truncated);
    }

    let little_endian = match &tiff[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Err(ExifError::BadTiffHeader),
    };

    let magic = read_u16(tiff, 2, little_endian)?;
    if magic != 42 {
        return Err(ExifError::BadTiffHeader);
    }

    let ifd0_offset = read_u32(tiff, 4, little_endian)? as usize;
    let ifd0 = parse_ifd(tiff, ifd0_offset, little_endian)?;

    let exif_ifd = match ifd_pointer(&ifd0, TAG_EXIF_IFD_POINTER) {
        Some(offset) => parse_ifd(tiff, offset, little_endian)?,
        None => Vec::new(),
    };
    let gps_ifd = match ifd_pointer(&ifd0, TAG_GPS_IFD_POINTER) {
        Some(offset) => parse_ifd(tiff, offset, little_endian)?,
        None => Vec::new(),
    };

    Ok(ExifData { little_endian, ifd0, exif_ifd, gps_ifd })
}

/// Looks up an IFD0 entry that points at another IFD (EXIF sub-IFD or
/// GPS IFD) and returns its offset into the TIFF data, if present.
fn ifd_pointer(ifd0: &[IfdEntry], tag: u16) -> Option<usize> {
    ifd0.iter().find(|e| e.tag == tag).and_then(|e| match &e.value {
        Value::Long(vals) => vals.first().map(|&v| v as usize),
        _ => None,
    })
}

fn parse_ifd(tiff: &[u8], offset: usize, le: bool) -> Result<Vec<IfdEntry>, ExifError> {
    let entry_count = read_u16(tiff, offset, le)? as usize;
    let mut entries = Vec::with_capacity(entry_count);

    for i in 0..entry_count {
        let entry_offset = checked_add(checked_add(offset, 2)?, i * 12)?;
        let tag = read_u16(tiff, entry_offset, le)?;
        let field_type = read_u16(tiff, checked_add(entry_offset, 2)?, le)?;
        let count = read_u32(tiff, checked_add(entry_offset, 4)?, le)?;
        let raw = slice(tiff, checked_add(entry_offset, 8)?, 4)?;
        let raw: [u8; 4] = [raw[0], raw[1], raw[2], raw[3]];

        let value = read_value(tiff, field_type, count, &raw, le)?;
        entries.push(IfdEntry { tag, field_type, count, value });
    }

    Ok(entries)
}

fn read_value(tiff: &[u8], field_type: u16, count: u32, raw: &[u8; 4], le: bool) -> Result<Value, ExifError> {
    let elem_size = type_size(field_type).ok_or(ExifError::UnsupportedType(field_type))?;
    let total = elem_size.checked_mul(count as usize).ok_or(ExifError::BadOffset)?;

    let data: Vec<u8> = if total <= 4 {
        raw[..total].to_vec()
    } else {
        let offset = if le {
            u32::from_le_bytes(*raw)
        } else {
            u32::from_be_bytes(*raw)
        } as usize;
        slice(tiff, offset, total)?.to_vec()
    };

    match field_type {
        2 => {
            let s = String::from_utf8_lossy(&data).trim_end_matches('\0').to_string();
            Ok(Value::Ascii(s))
        }
        3 => Ok(Value::Short(
            data.chunks_exact(2)
                .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
                .collect(),
        )),
        4 => Ok(Value::Long(
            data.chunks_exact(4)
                .map(|c| {
                    let b = [c[0], c[1], c[2], c[3]];
                    if le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) }
                })
                .collect(),
        )),
        5 => Ok(Value::Rational(
            data.chunks_exact(8)
                .map(|c| {
                    let n = [c[0], c[1], c[2], c[3]];
                    let d = [c[4], c[5], c[6], c[7]];
                    if le {
                        (u32::from_le_bytes(n), u32::from_le_bytes(d))
                    } else {
                        (u32::from_be_bytes(n), u32::from_be_bytes(d))
                    }
                })
                .collect(),
        )),
        _ => Ok(Value::Unknown { field_type, count }),
    }
}

/// Byte size of one element of a TIFF field type, per the EXIF spec's
/// type table. Only types 1-12 are defined.
fn type_size(field_type: u16) -> Option<usize> {
    match field_type {
        1 | 2 | 6 | 7 => Some(1),  // BYTE, ASCII, SBYTE, UNDEFINED
        3 | 8 => Some(2),          // SHORT, SSHORT
        4 | 9 | 11 => Some(4),     // LONG, SLONG, FLOAT
        5 | 10 | 12 => Some(8),    // RATIONAL, SRATIONAL, DOUBLE
        _ => None,
    }
}

fn checked_add(a: usize, b: usize) -> Result<usize, ExifError> {
    a.checked_add(b).ok_or(ExifError::BadOffset)
}

fn slice(buf: &[u8], start: usize, len: usize) -> Result<&[u8], ExifError> {
    let end = checked_add(start, len)?;
    buf.get(start..end).ok_or(ExifError::Truncated)
}

fn read_u16(buf: &[u8], offset: usize, le: bool) -> Result<u16, ExifError> {
    let b = slice(buf, offset, 2)?;
    Ok(if le { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) })
}

fn read_u32(buf: &[u8], offset: usize, le: bool) -> Result<u32, ExifError> {
    let b = slice(buf, offset, 4)?;
    let bytes = [b[0], b[1], b[2], b[3]];
    Ok(if le { u32::from_le_bytes(bytes) } else { u32::from_be_bytes(bytes) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_u16(v: &mut Vec<u8>, val: u16, le: bool) {
        v.extend_from_slice(if le { &val.to_le_bytes() } else { &val.to_be_bytes() });
    }

    fn push_u32(v: &mut Vec<u8>, val: u32, le: bool) {
        v.extend_from_slice(if le { &val.to_le_bytes() } else { &val.to_be_bytes() });
    }

    /// Builds a full APP1 payload (Exif signature + TIFF header + IFD0)
    /// with three entries: an inline SHORT, an inline ASCII string, and
    /// an ASCII string too long to fit inline, forcing an out-of-line
    /// read through the offset field. Same layout in both byte orders,
    /// only the multi-byte fields' encoding changes.
    fn sample_app1(le: bool) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(EXIF_SIGNATURE);
        v.extend_from_slice(if le { b"II" } else { b"MM" });
        push_u16(&mut v, 42, le);
        push_u32(&mut v, 8, le); // IFD0 offset, right after the header

        push_u16(&mut v, 3, le); // entry count

        // Orientation: SHORT, count 1, value inline.
        push_u16(&mut v, 0x0112, le);
        push_u16(&mut v, 3, le);
        push_u32(&mut v, 1, le);
        push_u16(&mut v, 1, le);
        push_u16(&mut v, 0, le); // padding to fill the 4-byte value slot

        // Make: ASCII, count 3, "Co\0" inline.
        push_u16(&mut v, 0x010F, le);
        push_u16(&mut v, 2, le);
        push_u32(&mut v, 3, le);
        v.extend_from_slice(b"Co\0");
        v.push(0);

        // Model: ASCII, count 6, too big to inline, offset 46 points
        // just past the end of this IFD0 structure.
        push_u16(&mut v, 0x0110, le);
        push_u16(&mut v, 2, le);
        push_u32(&mut v, 6, le);
        push_u32(&mut v, 46, le);

        v.extend_from_slice(b"Canon\0");
        v
    }

    fn single_entry_app1(field_type: u16, count: u32, raw: [u8; 4], le: bool) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(EXIF_SIGNATURE);
        v.extend_from_slice(if le { b"II" } else { b"MM" });
        push_u16(&mut v, 42, le);
        push_u32(&mut v, 8, le);
        push_u16(&mut v, 1, le);
        push_u16(&mut v, 0x00FF, le); // arbitrary tag
        push_u16(&mut v, field_type, le);
        push_u32(&mut v, count, le);
        v.extend_from_slice(&raw);
        v
    }

    fn ascii_value<'a>(ifd0: &'a [IfdEntry], tag: u16) -> &'a str {
        match &ifd0.iter().find(|e| e.tag == tag).unwrap().value {
            Value::Ascii(s) => s,
            other => panic!("expected Ascii for tag {tag:#06X}, got {other:?}"),
        }
    }

    #[test]
    fn parses_little_endian_ifd0() {
        let data = parse(&sample_app1(true)).unwrap();
        assert!(data.little_endian);
        assert_eq!(data.ifd0.len(), 3);
        match &data.ifd0.iter().find(|e| e.tag == 0x0112).unwrap().value {
            Value::Short(vals) => assert_eq!(vals, &[1]),
            other => panic!("expected Short, got {other:?}"),
        }
        assert_eq!(ascii_value(&data.ifd0, 0x010F), "Co");
        assert_eq!(ascii_value(&data.ifd0, 0x0110), "Canon");
    }

    #[test]
    fn parses_big_endian_ifd0() {
        let data = parse(&sample_app1(false)).unwrap();
        assert!(!data.little_endian);
        assert_eq!(data.ifd0.len(), 3);
        match &data.ifd0.iter().find(|e| e.tag == 0x0112).unwrap().value {
            Value::Short(vals) => assert_eq!(vals, &[1]),
            other => panic!("expected Short, got {other:?}"),
        }
        assert_eq!(ascii_value(&data.ifd0, 0x010F), "Co");
        assert_eq!(ascii_value(&data.ifd0, 0x0110), "Canon");
    }

    #[test]
    fn rejects_missing_exif_signature() {
        match parse(b"not an exif app1 payload") {
            Err(ExifError::NotExif) => {}
            other => panic!("expected NotExif, got {other:?}"),
        }
    }

    #[test]
    fn rejects_bad_byte_order_marker() {
        let mut v = Vec::new();
        v.extend_from_slice(EXIF_SIGNATURE);
        v.extend_from_slice(&[b'X', b'X', 0, 0, 0, 0, 0, 0]);
        match parse(&v) {
            Err(ExifError::BadTiffHeader) => {}
            other => panic!("expected BadTiffHeader, got {other:?}"),
        }
    }

    #[test]
    fn rejects_tiff_header_shorter_than_eight_bytes() {
        let mut v = Vec::new();
        v.extend_from_slice(EXIF_SIGNATURE);
        v.extend_from_slice(b"II\x2A\x00");
        match parse(&v) {
            Err(ExifError::Truncated) => {}
            other => panic!("expected Truncated, got {other:?}"),
        }
    }

    #[test]
    fn rejects_ifd0_offset_outside_the_segment() {
        let mut v = Vec::new();
        v.extend_from_slice(EXIF_SIGNATURE);
        v.extend_from_slice(b"II");
        push_u16(&mut v, 42, true);
        push_u32(&mut v, 1000, true); // nothing at that offset
        match parse(&v) {
            Err(ExifError::Truncated) => {}
            other => panic!("expected Truncated, got {other:?}"),
        }
    }

    #[test]
    fn recognized_type_without_a_decoder_becomes_unknown_value() {
        // Field type 7 is UNDEFINED: a valid TIFF type with a known
        // element size, but we don't decode it into anything specific.
        let data = parse(&single_entry_app1(7, 2, [0xAA, 0xBB, 0, 0], true)).unwrap();
        match &data.ifd0[0].value {
            Value::Unknown { field_type, count } => {
                assert_eq!(*field_type, 7);
                assert_eq!(*count, 2);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unrecognized_field_type() {
        match parse(&single_entry_app1(13, 1, [0, 0, 0, 0], true)) {
            Err(ExifError::UnsupportedType(13)) => {}
            other => panic!("expected UnsupportedType(13), got {other:?}"),
        }
    }
}
