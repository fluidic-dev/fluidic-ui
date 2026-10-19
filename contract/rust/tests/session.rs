//! The session's messages decode to the values they mean and encode back to
//! the same text.

use fluidic_ui_schema::{
    AppMessage, Bye, ByeReason, Canonical, Hello, HostMessage, PlatformFacts, SafetyLimits,
    SdkInfo, Version, Welcome,
};

fn version(major: u32, minor: u32) -> Version {
    Version { major, minor }
}

fn hello(versions: Vec<Version>) -> Hello {
    Hello {
        versions,
        app_id: "dev.fluidic.corpus".into(),
        sdk: SdkInfo {
            name: "corpus".into(),
            version: "0".into(),
        },
    }
}

fn bye(reason: ByeReason) -> Bye {
    Bye { reason }
}

/// Decodes `text`, checks it is `want` and that it encodes back to `text`.
fn assert_message<T: Canonical + PartialEq + std::fmt::Debug>(text: &str, want: T) {
    let got = T::decode(text.as_bytes()).unwrap_or_else(|e| panic!("{text}: {e}"));
    assert_eq!(got, want, "{text}");
    assert_eq!(got.encode().unwrap(), text);
}

#[test]
fn the_apps_messages() {
    let cases = [
        (
            r#"{"hello":{"versions":["0.1"],"app_id":"dev.fluidic.corpus","sdk":{"name":"corpus","version":"0"}}}"#,
            AppMessage::Hello(hello(vec![version(0, 1)])),
        ),
        (
            r#"{"hello":{"versions":["9.0"],"app_id":"dev.fluidic.corpus","sdk":{"name":"corpus","version":"0"}}}"#,
            AppMessage::Hello(hello(vec![version(9, 0)])),
        ),
        (
            r#"{"hello":{"versions":["0.1","0.7","9.0"],"app_id":"dev.fluidic.corpus","sdk":{"name":"corpus","version":"0"}}}"#,
            AppMessage::Hello(hello(vec![version(0, 1), version(0, 7), version(9, 0)])),
        ),
        (
            r#"{"bye":{"reason":"done"}}"#,
            AppMessage::Bye(bye(ByeReason::Done)),
        ),
    ];
    for (text, want) in cases {
        assert_message(text, want);
    }
}

#[test]
fn the_hosts_messages() {
    let cases = [
        (
            r#"{"bye":{"reason":"not_hello_first"}}"#,
            HostMessage::Bye(bye(ByeReason::NotHelloFirst)),
        ),
        (
            r#"{"bye":{"reason":"second_hello"}}"#,
            HostMessage::Bye(bye(ByeReason::SecondHello)),
        ),
        (
            r#"{"bye":{"reason":"unreadable_frame"}}"#,
            HostMessage::Bye(bye(ByeReason::UnreadableFrame)),
        ),
        (
            r#"{"bye":{"reason":{"no_common_version":{"host_versions":["0.1"]}}}}"#,
            HostMessage::Bye(bye(ByeReason::NoCommonVersion {
                host_versions: vec![version(0, 1)],
            })),
        ),
        (
            r#"{"welcome":{"version":"0.1","session_id":18446744073709551615,"platform":{},"limits":{}}}"#,
            HostMessage::Welcome(Welcome {
                version: version(0, 1),
                session_id: u64::MAX,
                platform: PlatformFacts {},
                limits: SafetyLimits {},
            }),
        ),
    ];
    for (text, want) in cases {
        assert_message(text, want);
    }
}
