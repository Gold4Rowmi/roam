//! What a release signature covers. Shared by the updater and examples/sign.rs.

pub const PUBLIC_KEY: [u8; 32] = [
    44, 171, 236, 83, 145, 84, 128, 213, 68, 137, 222, 180, 81, 213, 131, 74,
    111, 245, 50, 172, 51, 248, 91, 168, 69, 133, 154, 85, 243, 0, 11, 23,
];

/// Binding the version stops an old signed exe being served as a newer release.
pub fn message(version: &str, exe: &[u8]) -> Vec<u8> {
    [format!("roam {version}\n").as_bytes(), exe].concat()
}
