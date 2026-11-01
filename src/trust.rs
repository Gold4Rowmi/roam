//! The PCs this one has connected to, kept so they can be found again without retyping the code.
//! One "CODE NAME" per line in trusted.txt beside roam.code. Whoever holds the code is trusted:
//! anyone who knows it can pair, so Forget only protects you if that code is not reused elsewhere.
use std::path::PathBuf;

const MAX: usize = 8; // shortcut: only the 8 most recent PCs are kept, raise it if someone needs more

pub fn dir() -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join(r"ROWMI\Roam")
}

fn parse(s: &str) -> Vec<(String, String)> {
    s.lines().filter_map(|l| {
        let (code, name) = l.split_once(' ')?;
        (code.len() == 10 && !name.trim().is_empty()).then(|| (code.to_string(), name.trim().to_string()))
    }).collect()
}

/// `name` first, replacing an older entry for the same PC (it was paired again with a new code).
fn with(mut v: Vec<(String, String)>, code: &str, name: &str) -> Vec<(String, String)> {
    v.retain(|(_, n)| n != name);
    v.insert(0, (code.to_string(), name.to_string()));
    v.truncate(MAX);
    v
}

pub fn load() -> Vec<(String, String)> {
    parse(&std::fs::read_to_string(dir().join("trusted.txt")).unwrap_or_default())
}

fn save(v: &[(String, String)]) {
    let text: String = v.iter().map(|(c, n)| format!("{c} {n}\n")).collect();
    let _ = std::fs::create_dir_all(dir());
    let _ = std::fs::write(dir().join("trusted.txt"), text); // fine if it can't be saved
}

pub fn remember(code: &str, name: &str) { save(&with(load(), code, name)) }

pub fn forget(name: &str) {
    let mut v = load();
    v.retain(|(_, n)| n != name);
    save(&v);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_first_one_per_name_and_capped() {
        let c = |i: u8| format!("CODE{i:06}");
        let mut v = with(vec![], &c(1), "WORK-PC");
        v = with(v, &c(2), "HOME PC");
        v = with(v, &c(3), "WORK-PC"); // paired again with a new code
        assert_eq!(v, [(c(3), "WORK-PC".to_string()), (c(2), "HOME PC".to_string())]);
        for i in 10..30 { v = with(v, &c(i), &format!("PC{i}")) }
        assert_eq!(v.len(), MAX);
        assert_eq!(v[0].1, "PC29");
    }

    #[test]
    fn parse_skips_damaged_lines_and_keeps_names_with_spaces() {
        let v = parse("ABCDEFGHJK WORK-PC\nshort X\nABCDEFGHJK\n\nQWERT23456 My Laptop\n");
        assert_eq!(v, [("ABCDEFGHJK".to_string(), "WORK-PC".to_string()), ("QWERT23456".to_string(), "My Laptop".to_string())]);
    }
}
