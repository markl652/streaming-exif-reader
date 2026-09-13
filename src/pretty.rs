use crate::exif::{ExifData, IfdEntry, Value};

pub fn print_report(data: &ExifData) {
    println!(
        "byte order: {}",
        if data.little_endian { "little-endian (Intel)" } else { "big-endian (Motorola)" }
    );
    print_ifd("IFD0", &data.ifd0, ifd0_tag_name);
    if !data.exif_ifd.is_empty() {
        print_ifd("EXIF sub-IFD", &data.exif_ifd, exif_tag_name);
    }
    if !data.gps_ifd.is_empty() {
        print_ifd("GPS IFD", &data.gps_ifd, gps_tag_name);
    }
}

fn print_ifd(label: &str, entries: &[IfdEntry], name_fn: fn(u16) -> Option<&'static str>) {
    println!("{label}: {} entries", entries.len());
    for entry in entries {
        let name = name_fn(entry.tag)
            .map(str::to_string)
            .unwrap_or_else(|| format!("tag 0x{:04X}", entry.tag));
        println!("  {name:<18} {}", format_value(&entry.value));
    }
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Ascii(s) => s.clone(),
        Value::Short(vals) => join(vals),
        Value::Long(vals) => join(vals),
        Value::Rational(vals) => vals
            .iter()
            .map(|(n, d)| format!("{n}/{d}"))
            .collect::<Vec<_>>()
            .join(", "),
        Value::Unknown { field_type, count } => {
            format!("<unsupported type {field_type}, {count} values>")
        }
    }
}

fn join<T: std::fmt::Display>(vals: &[T]) -> String {
    vals.iter().map(T::to_string).collect::<Vec<_>>().join(", ")
}

/// Names for the handful of IFD0 tags most photos actually carry.
/// Anything else prints as a raw tag number for now.
fn ifd0_tag_name(tag: u16) -> Option<&'static str> {
    match tag {
        0x010F => Some("Make"),
        0x0110 => Some("Model"),
        0x0112 => Some("Orientation"),
        0x011A => Some("XResolution"),
        0x011B => Some("YResolution"),
        0x0128 => Some("ResolutionUnit"),
        0x0131 => Some("Software"),
        0x0132 => Some("DateTime"),
        0x013B => Some("Artist"),
        0x8298 => Some("Copyright"),
        0x8769 => Some("ExifIFDPointer"),
        0x8825 => Some("GPSInfoIFDPointer"),
        _ => None,
    }
}

/// Names for the EXIF sub-IFD tags most photos actually carry.
fn exif_tag_name(tag: u16) -> Option<&'static str> {
    match tag {
        0x829A => Some("ExposureTime"),
        0x829D => Some("FNumber"),
        0x8822 => Some("ExposureProgram"),
        0x8827 => Some("ISOSpeedRatings"),
        0x9000 => Some("ExifVersion"),
        0x9003 => Some("DateTimeOriginal"),
        0x9004 => Some("DateTimeDigitized"),
        0x9201 => Some("ShutterSpeedValue"),
        0x9202 => Some("ApertureValue"),
        0x9204 => Some("ExposureBiasValue"),
        0x9207 => Some("MeteringMode"),
        0x9209 => Some("Flash"),
        0x920A => Some("FocalLength"),
        0xA002 => Some("PixelXDimension"),
        0xA003 => Some("PixelYDimension"),
        0xA405 => Some("FocalLengthIn35mmFilm"),
        0xA406 => Some("SceneCaptureType"),
        _ => None,
    }
}

/// Names for the GPS IFD tags most photos actually carry.
fn gps_tag_name(tag: u16) -> Option<&'static str> {
    match tag {
        0x0000 => Some("GPSVersionID"),
        0x0001 => Some("GPSLatitudeRef"),
        0x0002 => Some("GPSLatitude"),
        0x0003 => Some("GPSLongitudeRef"),
        0x0004 => Some("GPSLongitude"),
        0x0005 => Some("GPSAltitudeRef"),
        0x0006 => Some("GPSAltitude"),
        0x0007 => Some("GPSTimeStamp"),
        0x001D => Some("GPSDateStamp"),
        _ => None,
    }
}
