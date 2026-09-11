mod exif;
mod jpeg;
mod pretty;

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let path = match args.next() {
        Some(p) => p,
        None => {
            eprintln!("usage: streaming-exif-reader <path-to-jpeg>");
            return ExitCode::FAILURE;
        }
    };

    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("could not open {}: {e}", path.to_string_lossy());
            return ExitCode::FAILURE;
        }
    };

    let mut segments = match jpeg::SegmentReader::new(BufReader::new(file)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    loop {
        let segment = match segments.next_segment() {
            Ok(Some(s)) => s,
            Ok(None) => {
                println!("no EXIF metadata found before the image data started");
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        };

        if segment.marker != jpeg::MARKER_APP1 || !segment.data.starts_with(jpeg::EXIF_SIGNATURE) {
            continue; // APP1 can also carry XMP; skip anything that isn't Exif
        }

        return match exif::parse(&segment.data) {
            Ok(data) => {
                pretty::print_report(&data);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("invalid EXIF data: {e}");
                ExitCode::FAILURE
            }
        };
    }
}
