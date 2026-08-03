// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;

use pmd_core::FileProvider;

#[derive(Debug)]
pub struct MemoryBundle {
    files: HashMap<String, Vec<u8>>,
    main_key: String,
    main_name: String,
}

impl MemoryBundle {
    pub fn from_named_bytes<I>(files: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = (String, Vec<u8>)>,
    {
        let mut stored = HashMap::new();
        let mut main = None;

        for (name, bytes) in files {
            let key = normalize_name(&name);
            if key.is_empty() {
                continue;
            }
            stored.entry(key.clone()).or_insert(bytes);

            if let Some(priority) = main_priority(&key) {
                let replace = main
                    .as_ref()
                    .map(|candidate: &(u8, String, String)| priority < candidate.0)
                    .unwrap_or(true);
                if replace {
                    main = Some((priority, key, name));
                }
            }
        }

        let Some((_, main_key, main_name)) = main else {
            return Err(String::from(
                "the selected bundle contains no .M, .M2, or .MZ main file",
            ));
        };

        Ok(Self {
            files: stored,
            main_key,
            main_name,
        })
    }

    pub fn main_bytes(&self) -> &[u8] {
        self.files
            .get(&self.main_key)
            .map(Vec::as_slice)
            .expect("main file is inserted when the bundle is constructed")
    }

    pub fn main_name(&self) -> &str {
        &self.main_name
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn song_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .files
            .keys()
            .filter(|name| main_priority(name).is_some())
            .cloned()
            .collect();
        names.sort();
        names
    }

    pub fn select_main(&mut self, name: &str) -> bool {
        let key = normalize_name(name);
        if main_priority(&key).is_none() || !self.files.contains_key(&key) {
            return false;
        }
        self.main_name.clone_from(&key);
        self.main_key = key;
        true
    }
}

impl FileProvider for MemoryBundle {
    fn get(&self, name: &str) -> Option<&[u8]> {
        self.files.get(&normalize_name(name)).map(Vec::as_slice)
    }
}

fn normalize_name(name: &str) -> String {
    name.rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .to_ascii_uppercase()
}

fn main_priority(name: &str) -> Option<u8> {
    match name.rsplit_once('.')?.1 {
        "M" => Some(0),
        "M2" => Some(1),
        "MZ" => Some(2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_name, MemoryBundle};
    use pmd_core::FileProvider;

    #[test]
    fn names_are_case_insensitive_and_basename_based() {
        let bundle = MemoryBundle::from_named_bytes([
            (String::from("music\\song.M"), vec![1, 2, 3]),
            (String::from("banks/BANK.PPC"), vec![9, 8, 7]),
        ])
        .expect("recognized main file");

        assert_eq!(normalize_name("dir\\bank.ppc"), "BANK.PPC");
        assert_eq!(bundle.main_name(), "music\\song.M");
        assert_eq!(bundle.main_bytes(), [1, 2, 3]);
        assert_eq!(bundle.get("bank.ppc"), Some(&[9, 8, 7][..]));
        assert_eq!(bundle.get("C:/music/BANK.PPC"), Some(&[9, 8, 7][..]));
        assert_eq!(bundle.file_count(), 2);
    }

    #[test]
    fn main_extension_priority_is_deterministic() {
        let bundle = MemoryBundle::from_named_bytes([
            (String::from("late.MZ"), vec![3]),
            (String::from("middle.M2"), vec![2]),
            (String::from("first.M"), vec![1]),
        ])
        .expect("recognized main file");

        assert_eq!(bundle.main_name(), "first.M");
        assert_eq!(bundle.main_bytes(), [1]);
    }

    #[test]
    fn every_main_file_in_a_drop_becomes_a_playlist_entry() {
        let mut bundle = MemoryBundle::from_named_bytes([
            (String::from("folder/B.M"), vec![2]),
            (String::from("folder/A.M"), vec![1]),
            (String::from("folder/BANK.PPC"), vec![9]),
            (String::from("folder/C.M2"), vec![3]),
        ])
        .expect("recognized main file");

        assert_eq!(bundle.song_names(), ["A.M", "B.M", "C.M2"]);
        assert!(bundle.select_main("B.M"));
        assert_eq!(bundle.main_bytes(), [2]);
        assert!(bundle.select_main("c.m2"), "selection is case-insensitive");
        assert_eq!(bundle.main_bytes(), [3]);
        assert!(!bundle.select_main("BANK.PPC"), "a companion is not a song");
        assert!(!bundle.select_main("MISSING.M"));
        assert_eq!(bundle.main_bytes(), [3], "a failed switch changes nothing");
    }

    #[test]
    fn missing_main_file_is_rejected() {
        let error = MemoryBundle::from_named_bytes([(String::from("BANK.PPC"), vec![1])])
            .expect_err("a companion alone is not a song");
        assert!(error.contains("no .M"));
    }
}
