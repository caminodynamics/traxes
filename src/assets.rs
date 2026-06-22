// Embedded demo assets for v0.2 release
// These assets are compiled into the binary to enable standalone demo execution

pub const ALLOW_PAYLOAD: &str = include_str!("../payloads/test_allowed_t3medium.json");
pub const DENY_PAYLOAD: &str = include_str!("../payloads/test_denied_m5large.json");
