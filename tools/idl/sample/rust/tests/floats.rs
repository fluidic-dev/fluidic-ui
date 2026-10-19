//! A float that is NaN or infinite has no canonical text.

use fluidic_ui_idl_sample::{Canonical, Sample};

#[test]
fn nan_and_infinity_are_not_encoded() {
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(v.encode().is_err(), "{v} was encoded");
        assert!((v as f32).encode().is_err(), "{v} was encoded as an f32");
    }
}

#[test]
fn a_value_holding_nan_is_not_encoded() {
    let text = r#"{"flag":false,"small":0,"delta":0,"id":0,"offset":0,"ratio":0,"scale":0,"label":"","contract":"0.0","tags":[],"grid":[],"by_name":{},"align":"start","reason":"done","type":""}"#;
    let mut sample = Sample::decode(text.as_bytes()).expect("decoded");
    sample.scale = f64::NAN;
    assert!(sample.encode().is_err());
}
