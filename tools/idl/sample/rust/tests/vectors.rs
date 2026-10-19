//! Runs the shared vectors in `../vectors.txt` against the generated Rust.

use fluidic_ui_idl_sample::{
    Alignment, AppMessage, ByeReason, Canonical, Choice, Group, HostMessage, Keywords, Node,
    Nothing, Sample, SdkInfo, Shape, Tally, Version,
};

/// The unknown case's name, for the types that have one.
trait Unknown {
    fn unknown(&self) -> Option<&str> {
        None
    }
}

impl Unknown for ByeReason {
    fn unknown(&self) -> Option<&str> {
        match self {
            ByeReason::Unknown(name) => Some(name),
            _ => None,
        }
    }
}

impl Unknown for Alignment {
    fn unknown(&self) -> Option<&str> {
        match self {
            Alignment::Unknown(name) => Some(name),
            _ => None,
        }
    }
}

impl Unknown for AppMessage {
    fn unknown(&self) -> Option<&str> {
        match self {
            AppMessage::Unknown(name) => Some(name),
            _ => None,
        }
    }
}

impl Unknown for HostMessage {
    fn unknown(&self) -> Option<&str> {
        match self {
            HostMessage::Unknown(name) => Some(name),
            _ => None,
        }
    }
}

impl Unknown for Sample {}
impl Unknown for SdkInfo {}
impl Unknown for Choice {}
impl Unknown for Tally {}
impl Unknown for Node {}
impl Unknown for Nothing {}
impl Unknown for Shape {}
impl Unknown for Group {}
impl Unknown for Keywords {}
impl Unknown for bool {}
impl Unknown for u32 {}
impl Unknown for i32 {}
impl Unknown for u64 {}
impl Unknown for i64 {}
impl Unknown for f32 {}
impl Unknown for f64 {}
impl Unknown for String {}
impl Unknown for Version {}

/// Runs one vector; returns what went wrong, if anything.
fn run<T: Canonical + Unknown + std::fmt::Debug>(fields: &[&str]) -> Result<(), String> {
    let kind = fields[0];
    let text = fields[2];
    let decoded = T::decode(text.as_bytes());
    match kind {
        "refuse" => match decoded {
            Ok(v) => Err(format!("decoded to {v:?}")),
            Err(_) => Ok(()),
        },
        "round" | "normal" => {
            let v = decoded.map_err(|e| format!("not decoded: {e}"))?;
            let want = if kind == "round" { text } else { fields[3] };
            let got = v.encode().map_err(|e| format!("not encoded: {e}"))?;
            if got == want {
                Ok(())
            } else {
                Err(format!("encoded as {got}"))
            }
        }
        "unknown" | "noencode" => {
            let v = decoded.map_err(|e| format!("not decoded: {e}"))?;
            if kind == "unknown" && v.unknown() != Some(fields[3]) {
                return Err(format!("decoded to {v:?}"));
            }
            match v.encode() {
                Ok(got) => Err(format!("encoded as {got}")),
                Err(_) => Ok(()),
            }
        }
        _ => Err(format!("no such kind `{kind}`")),
    }
}

#[test]
fn shared_vectors() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../vectors.txt");
    let text = std::fs::read_to_string(path).expect("vectors.txt");
    let mut failures = Vec::new();
    let mut count = 0;
    for (n, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        count += 1;
        let fields: Vec<&str> = line.split('\t').collect();
        let result = match fields[1] {
            "sample" => run::<Sample>(&fields),
            "sdk_info" => run::<SdkInfo>(&fields),
            "nothing" => run::<Nothing>(&fields),
            "node" => run::<Node>(&fields),
            "choice" => run::<Choice>(&fields),
            "tally" => run::<Tally>(&fields),
            "bye_reason" => run::<ByeReason>(&fields),
            "shape" => run::<Shape>(&fields),
            "group" => run::<Group>(&fields),
            "keywords" => run::<Keywords>(&fields),
            "alignment" => run::<Alignment>(&fields),
            "app_message" => run::<AppMessage>(&fields),
            "host_message" => run::<HostMessage>(&fields),
            "bool" => run::<bool>(&fields),
            "u32" => run::<u32>(&fields),
            "i32" => run::<i32>(&fields),
            "u64" => run::<u64>(&fields),
            "i64" => run::<i64>(&fields),
            "f32" => run::<f32>(&fields),
            "f64" => run::<f64>(&fields),
            "string" => run::<String>(&fields),
            "version" => run::<Version>(&fields),
            other => Err(format!("no such type `{other}`")),
        };
        if let Err(why) = result {
            failures.push(format!("vectors.txt:{}: {line}\n    {why}", n + 1));
        }
    }
    assert!(count > 0, "no vectors");
    assert!(
        failures.is_empty(),
        "{} of {count} vectors failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
