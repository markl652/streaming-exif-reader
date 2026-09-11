use std::io::{self, Read};

/// The six bytes that mark the start of an EXIF payload inside an APP1
/// segment, right before the TIFF header.
pub const EXIF_SIGNATURE: &[u8] = b"Exif\0\0";

pub const MARKER_APP1: u8 = 0xE1;

const MARKER_EOI: u8 = 0xD9;
const MARKER_SOS: u8 = 0xDA;
const MARKER_RST_FIRST: u8 = 0xD0;
const MARKER_RST_LAST: u8 = 0xD7;
const MARKER_TEM: u8 = 0x01;
const MARKER_SOI: u8 = 0xD8;

#[derive(Debug)]
pub enum JpegError {
    Io(io::Error),
    NotAJpeg,
    Truncated,
}

impl std::fmt::Display for JpegError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JpegError::Io(e) => write!(f, "i/o error: {e}"),
            JpegError::NotAJpeg => write!(f, "not a JPEG file (bad SOI marker)"),
            JpegError::Truncated => write!(f, "file ended in the middle of a segment"),
        }
    }
}

impl From<io::Error> for JpegError {
    fn from(e: io::Error) -> Self {
        JpegError::Io(e)
    }
}

pub struct Segment {
    pub marker: u8,
    pub data: Vec<u8>,
}

/// Walks a JPEG file one marker segment at a time.
///
/// This never buffers the entropy-coded scan data that follows the SOS
/// marker: that data has no fixed size (it can be the bulk of a multi
/// megabyte photo) and is the reason a naive "read the whole file into a
/// Vec<u8>" parser doesn't scale to batch-processing a large photo
/// library. Segment payloads before SOS are metadata and are bounded to
/// 65533 bytes by the JPEG spec, so those are read in full and handed
/// back as owned buffers.
pub struct SegmentReader<R: Read> {
    inner: R,
    done: bool,
}

impl<R: Read> SegmentReader<R> {
    pub fn new(mut inner: R) -> Result<Self, JpegError> {
        let mut soi = [0u8; 2];
        inner.read_exact(&mut soi)?;
        if soi != [0xFF, MARKER_SOI] {
            return Err(JpegError::NotAJpeg);
        }
        Ok(SegmentReader { inner, done: false })
    }

    /// Returns the next segment, or `None` once the entropy-coded scan
    /// (or end of file) is reached.
    pub fn next_segment(&mut self) -> Result<Option<Segment>, JpegError> {
        if self.done {
            return Ok(None);
        }
        loop {
            let marker = match self.read_marker()? {
                Some(m) => m,
                None => {
                    self.done = true;
                    return Ok(None);
                }
            };

            if marker == MARKER_EOI {
                self.done = true;
                return Ok(None);
            }

            if marker == MARKER_SOS {
                // Stop rather than skip: the bytes after SOS are
                // byte-stuffed (0xFF 0x00 stands for a literal 0xFF), so
                // finding the next real marker means scanning every byte
                // of the scan data anyway. We don't need anything past
                // this point, so we just stop reading.
                self.done = true;
                return Ok(None);
            }

            if marker == MARKER_TEM || (MARKER_RST_FIRST..=MARKER_RST_LAST).contains(&marker) {
                continue; // standalone markers, no length field
            }

            let length = self.read_u16_be()?;
            if length < 2 {
                return Err(JpegError::Truncated);
            }
            let payload_len = (length - 2) as usize;
            let mut data = vec![0u8; payload_len];
            self.inner.read_exact(&mut data)?;
            return Ok(Some(Segment { marker, data }));
        }
    }

    fn read_byte(&mut self) -> Result<Option<u8>, JpegError> {
        let mut b = [0u8; 1];
        match self.inner.read_exact(&mut b) {
            Ok(()) => Ok(Some(b[0])),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
            Err(e) => Err(JpegError::Io(e)),
        }
    }

    fn read_marker(&mut self) -> Result<Option<u8>, JpegError> {
        loop {
            let b = match self.read_byte()? {
                Some(b) => b,
                None => return Ok(None),
            };
            if b != 0xFF {
                continue; // markers are always preceded by 0xFF
            }
            let next = match self.read_byte()? {
                Some(b) => b,
                None => return Ok(None),
            };
            if next == 0x00 || next == 0xFF {
                continue; // stuffing byte or padding, not a real marker
            }
            return Ok(Some(next));
        }
    }

    fn read_u16_be(&mut self) -> Result<u16, JpegError> {
        let mut buf = [0u8; 2];
        self.inner.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }
}
