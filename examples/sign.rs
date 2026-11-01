//! Release signing, kept out of the shipped exe.
//!   sign gen <keyfile>      make a key; refuses to overwrite one, prints the public half for src/signing.rs
//!   sign <exe> <version>    write <exe>.sig with the hex key in ROAM_SIGNING_KEY
#[path = "../src/signing.rs"]
mod signing;

use chacha20poly1305::aead::{rand_core::RngCore, OsRng};
use ed25519_dalek::{Signer, SigningKey};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["gen", path] => {
            let mut seed = [0u8; 32];
            OsRng.fill_bytes(&mut seed);
            let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
            let mut f = std::fs::File::options().write(true).create_new(true).open(path).expect("key file must not exist yet");
            std::io::Write::write_all(&mut f, hex.as_bytes()).unwrap();
            println!("{:?}", SigningKey::from_bytes(&seed).verifying_key().to_bytes());
        }
        [exe, version] => {
            let hex = std::env::var("ROAM_SIGNING_KEY").expect("ROAM_SIGNING_KEY is not set");
            let hex = hex.trim();
            assert_eq!(hex.len(), 64, "ROAM_SIGNING_KEY must be 64 hex characters");
            let seed: Vec<u8> = (0..32).map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap()).collect();
            let key = SigningKey::from_bytes(seed.as_slice().try_into().unwrap());
            // a release signed by the wrong key would be refused by every installed copy
            assert_eq!(key.verifying_key().to_bytes(), signing::PUBLIC_KEY, "key does not match src/signing.rs");
            let sig = key.sign(&signing::message(version, &std::fs::read(exe).unwrap()));
            std::fs::write(format!("{exe}.sig"), sig.to_bytes()).unwrap();
        }
        _ => panic!("usage: sign gen <keyfile> | sign <exe> <version>"),
    }
}
