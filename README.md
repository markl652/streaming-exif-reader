# streaming-exif-reader

A JPEG/EXIF metadata reader that never loads the image data into memory.

## The problem

Most tools that read EXIF metadata (camera make and model, exposure
settings, timestamps, GPS) do it by reading the whole JPEG file into a
`Vec<u8>` and then poking around in it. That's fine for one photo. It
falls over when you point the same code at a folder of a few thousand
photos from a phone or camera, or at video-adjacent JPEGs in the tens of
megabytes: you end up holding gigabytes of pixel data in RAM just to read
a few hundred bytes of text sitting near the start of the file.

The metadata you actually want lives in a JPEG APP1 marker segment near
the start of the file, capped at 65,533 bytes by the JPEG spec itself.
Everything after the SOS (start of scan) marker is compressed pixel
data, and that's the part that can be enormous.

## What this does

`streaming-exif-reader` walks a JPEG file marker by marker using a
bounded amount of memory:

- It reads segments one at a time directly from a `Read` stream (a
  `BufReader<File>` in the CLI, but any `Read` implementor works).
- It stops as soon as it hits the SOS marker, before the entropy-coded
  scan data. That data is never read, skipped, or buffered — the reader
  just returns `None` from that point on.
- The APP1 segment holding the `Exif\0\0` signature is kept around for
  parsing; every other segment's payload is read into a small,
  spec-bounded buffer and then dropped. If the Exif data doesn't fit in
  one 65533-byte segment, the continuation APP1 segments some encoders
  emit afterward are stitched back together automatically.

The EXIF payload itself (TIFF header plus IFD0) is then parsed with
bounds-checked offsets, so a truncated or malformed file produces an
error instead of a panic or an out-of-bounds read.

## Usage

```
$ cargo run -- photo.jpg
byte order: little-endian (Intel)
IFD0: 6 entries
  Make               Canon
  Model              Canon EOS 90D
  Orientation        1
  XResolution        350/1
  YResolution        350/1
  DateTime           2026:03:14 09:22:31
```

If the file has no EXIF segment, or isn't a JPEG at all, you get a plain
message on stdout or an error on stderr instead of a stack trace.

## Current scope

This is an early skeleton. It currently:

- only understands JPEG containers (no TIFF, PNG, HEIC yet)
- parses IFD0, the EXIF sub-IFD, and the GPS IFD (no thumbnail IFD yet)
- decodes ASCII, SHORT, LONG, and RATIONAL field types; anything else is
  reported as present but unsupported rather than silently dropped

See the roadmap in the project notes for what's planned next.

## Building

No external dependencies — standard library only.

```
cargo build --release
```
