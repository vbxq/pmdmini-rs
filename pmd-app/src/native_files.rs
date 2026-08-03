// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use pmd_core::{FileProvider, Metadata, Player, CHANNEL_COUNT};

#[derive(Default)]
pub struct MemoryFiles {
    files: HashMap<String, Vec<u8>>,
}

pub struct LoadedSong {
    pub name: String,
    pub directory: Option<String>,
    pub files: MemoryFiles,
    pub player: Player,
    pub metadata: Metadata,
}

impl MemoryFiles {
    pub fn from_main_path(path: &Path) -> Result<(String, Vec<u8>, Self), String> {
        let main = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
        let mut files = Self::default();
        files.insert_path(path, main.clone());

        if let Some(parent) = path.parent() {
            if let Ok(entries) = fs::read_dir(parent) {
                for entry in entries.flatten() {
                    let candidate = entry.path();
                    if candidate.is_file() && candidate != path {
                        if let Ok(bytes) = fs::read(&candidate) {
                            files.insert_path(&candidate, bytes);
                        }
                    }
                }
            }
        }

        let main_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
            .ok_or_else(|| format!("{} has no valid filename", path.display()))?;
        Ok((main_name, main, files))
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    fn insert_path(&mut self, path: &Path, bytes: Vec<u8>) {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return;
        };
        self.files.insert(normalize_name(name), bytes);
    }
}

pub fn load_song(
    path: &Path,
    sample_rate: u32,
    loop_count: Option<u32>,
    muted: [bool; CHANNEL_COUNT],
) -> Result<LoadedSong, String> {
    let (name, main, files) = MemoryFiles::from_main_path(path)?;
    let mut player = Player::new(sample_rate);
    player.set_loop_count(loop_count);
    for (channel, &is_muted) in muted.iter().enumerate() {
        player.set_channel_mute(channel, is_muted);
    }
    player
        .load(&main, &files)
        .map_err(|error| format!("PMD load failed: {error:?}"))?;
    let metadata = player.metadata().clone();
    let directory = path
        .parent()
        .and_then(|parent| parent.to_str())
        .map(ToOwned::to_owned);
    Ok(LoadedSong {
        name,
        directory,
        files,
        player,
        metadata,
    })
}

impl FileProvider for MemoryFiles {
    fn get(&self, name: &str) -> Option<&[u8]> {
        self.files.get(&normalize_name(name)).map(Vec::as_slice)
    }
}

fn normalize_name(name: &str) -> String {
    let basename = name.rsplit(['/', '\\']).next().unwrap_or(name);
    basename.to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{load_song, normalize_name, MemoryFiles};
    use pmd_core::{FileProvider, CHANNEL_COUNT};
    use std::fs;

    #[test]
    fn names_are_case_insensitive_and_basename_based() {
        assert_eq!(normalize_name("dir\\bank.ppc"), "BANK.PPC");

        let mut files = MemoryFiles::default();
        files.files.insert(String::from("BANK.PPC"), vec![1, 2, 3]);
        assert_eq!(files.get("bank.ppc"), Some(&[1, 2, 3][..]));
        assert_eq!(files.get("C:/music/bank.PPC"), Some(&[1, 2, 3][..]));
    }

    #[test]
    fn selected_file_loader_copies_synthetic_neighbors() {
        let directory =
            std::env::temp_dir().join(format!("pmdmini-native-files-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create synthetic test directory");
        let main_path = directory.join("song.M");
        fs::write(&main_path, [0x01, 0x18, 0x00]).expect("write synthetic main file");
        fs::write(directory.join("BANK.PPC"), [9, 8, 7]).expect("write synthetic companion");

        let (name, main, files) =
            MemoryFiles::from_main_path(&main_path).expect("load synthetic directory");
        assert_eq!(name, "song.M");
        assert_eq!(main, [0x01, 0x18, 0x00]);
        assert_eq!(files.file_count(), 2);
        assert_eq!(files.get("bank.ppc"), Some(&[9, 8, 7][..]));

        let mut muted = [false; CHANNEL_COUNT];
        muted[0] = true;
        let loaded = load_song(&main_path, 44_100, Some(2), muted).expect("parse synthetic song");
        assert_eq!(loaded.name, "song.M");
        assert_eq!(loaded.files.file_count(), 2);
        assert_eq!(loaded.player.playback_status().loop_limit, Some(2));
        assert_eq!(loaded.player.channel_muted(0), Some(true));

        fs::remove_dir_all(&directory).expect("remove synthetic test directory");
    }
}
