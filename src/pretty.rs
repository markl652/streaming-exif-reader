use crate::exif::{ExifData, Value};

pub fn print_report(data: &ExifData) {
    println!(
        "byte order: {}",
        if data.little_endian { "little-endian (Intel)" } else { "big-endian (Motorola)" }
    );
    println!("IFD0: {} entries", data.ifd0.len());
    for entry in &data.ifd0 {
        let name = tag_name(entry.tag)
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
fn tag_name(tag: u16) -> Option<&'static str> {
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
        _ => None,
    }
}
