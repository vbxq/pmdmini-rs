// SPDX-License-Identifier: AGPL-3.0-only

use pmd_core::CHANNEL_COUNT;

pub const SAVE_INTERVAL_SECONDS: f64 = 2.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlayMode {
    #[default]
    Single,

    RepeatOne,

    RepeatAll,

    Shuffle,
}

impl PlayMode {
    pub fn label(self) -> &'static str {
        match self {
            PlayMode::Single => "SINGLE",
            PlayMode::RepeatOne => "REP.ONE",
            PlayMode::RepeatAll => "REP.ALL",
            PlayMode::Shuffle => "RANDOM",
        }
    }

    pub fn next(self) -> Self {
        match self {
            PlayMode::Single => PlayMode::RepeatOne,
            PlayMode::RepeatOne => PlayMode::RepeatAll,
            PlayMode::RepeatAll => PlayMode::Shuffle,
            PlayMode::Shuffle => PlayMode::Single,
        }
    }

    fn parse(text: &str) -> Self {
        match text {
            "REP.ONE" => PlayMode::RepeatOne,
            "REP.ALL" => PlayMode::RepeatAll,
            "RANDOM" => PlayMode::Shuffle,
            _ => PlayMode::Single,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub volume: f32,

    pub loop_count: Option<u32>,

    pub play_mode: PlayMode,

    pub muted: [bool; CHANNEL_COUNT],

    pub zoom: Option<u32>,

    pub playlist: Vec<String>,

    pub last_directory: Option<String>,

    dirty: bool,

    last_write: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            loop_count: None,
            play_mode: PlayMode::default(),
            muted: [false; CHANNEL_COUNT],
            zoom: None,
            playlist: Vec::new(),
            last_directory: None,
            dirty: false,
            last_write: 0.0,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut settings = Self::default();
        let Some(text) = read_storage() else {
            return settings;
        };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "volume" => {
                    if let Ok(volume) = value.trim().parse::<f32>() {
                        settings.volume = volume.clamp(0.0, 1.0);
                    }
                }
                "loops" => {
                    settings.loop_count = match value.trim() {
                        "infinite" => None,
                        other => other.parse().ok(),
                    }
                }
                "mode" => settings.play_mode = PlayMode::parse(value.trim()),
                "muted" => {
                    for (slot, flag) in settings.muted.iter_mut().zip(value.trim().chars()) {
                        *slot = flag == '1';
                    }
                }
                "zoom" => settings.zoom = value.trim().parse().ok(),
                "directory" => settings.last_directory = Some(value.trim().to_owned()),
                "track" => settings.playlist.push(value.trim().to_owned()),
                _ => {}
            }
        }
        settings
    }

    pub fn save(&mut self) {
        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn flush_if_due(&mut self, now: f64) {
        if self.dirty && now - self.last_write >= SAVE_INTERVAL_SECONDS {
            self.flush(now);
        }
    }

    pub fn flush(&mut self, now: f64) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        self.last_write = now;
        write_storage(&self.serialise());
    }

    fn serialise(&self) -> String {
        let mut out = String::new();
        out.push_str("# pmdmini settings\n");
        out.push_str(&format!("volume={}\n", self.volume));
        match self.loop_count {
            Some(count) => out.push_str(&format!("loops={count}\n")),
            None => out.push_str("loops=infinite\n"),
        }
        out.push_str(&format!("mode={}\n", self.play_mode.label()));
        let muted: String = self
            .muted
            .iter()
            .map(|flag| if *flag { '1' } else { '0' })
            .collect();
        out.push_str(&format!("muted={muted}\n"));
        if let Some(zoom) = self.zoom {
            out.push_str(&format!("zoom={zoom}\n"));
        }
        if let Some(directory) = &self.last_directory {
            out.push_str(&format!("directory={directory}\n"));
        }
        for entry in &self.playlist {
            if !entry.contains('\n') {
                out.push_str(&format!("track={entry}\n"));
            }
        }
        out
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn settings_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config"))
        })?;
    Some(base.join("pmdmini").join("settings.conf"))
}

#[cfg(not(target_arch = "wasm32"))]
fn read_storage() -> Option<String> {
    std::fs::read_to_string(settings_path()?).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn write_storage(text: &str) {
    let Some(path) = settings_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, text);
}

#[cfg(target_arch = "wasm32")]
fn read_storage() -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()??
        .get_item("pmdmini.settings")
        .ok()?
}

#[cfg(target_arch = "wasm32")]
fn write_storage(text: &str) {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok())
        .flatten()
    {
        let _ = storage.set_item("pmdmini.settings", text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_cycle_returns_to_where_it_started() {
        let mut mode = PlayMode::Single;
        for _ in 0..4 {
            mode = mode.next();
        }
        assert_eq!(mode, PlayMode::Single);
    }

    #[test]
    fn every_mode_has_a_caption_that_fits_the_status_band() {
        for mode in [
            PlayMode::Single,
            PlayMode::RepeatOne,
            PlayMode::RepeatAll,
            PlayMode::Shuffle,
        ] {
            assert!(!mode.label().is_empty());
            assert!(mode.label().len() <= 8, "{} is too wide", mode.label());
            assert_eq!(PlayMode::parse(mode.label()), mode, "round trip failed");
        }
    }

    #[test]
    fn settings_survive_a_round_trip_through_their_stored_form() {
        let mut settings = Settings {
            volume: 0.5,
            loop_count: Some(3),
            play_mode: PlayMode::Shuffle,
            zoom: Some(2),
            playlist: vec![String::from("/songs/A.M"), String::from("/songs/B.M2")],
            last_directory: Some(String::from("/songs")),
            ..Settings::default()
        };
        settings.muted[4] = true;
        let text = settings.serialise();

        let mut restored = Settings::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "volume" => restored.volume = value.trim().parse().unwrap(),
                "loops" => {
                    restored.loop_count = match value.trim() {
                        "infinite" => None,
                        other => other.parse().ok(),
                    }
                }
                "mode" => restored.play_mode = PlayMode::parse(value.trim()),
                "muted" => {
                    for (slot, flag) in restored.muted.iter_mut().zip(value.trim().chars()) {
                        *slot = flag == '1';
                    }
                }
                "zoom" => restored.zoom = value.trim().parse().ok(),
                "directory" => restored.last_directory = Some(value.trim().to_owned()),
                "track" => restored.playlist.push(value.trim().to_owned()),
                _ => {}
            }
        }
        assert_eq!(restored, settings);
    }

    #[test]
    fn an_infinite_loop_count_round_trips_as_a_word_rather_than_a_number() {
        let settings = Settings::default();
        assert!(settings.serialise().contains("loops=infinite"));
    }

    #[test]
    fn settings_reach_the_disk_and_come_back() {
        let root = std::env::temp_dir().join(format!("pmdmini-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        std::env::set_var("XDG_CONFIG_HOME", &root);

        let mut settings = Settings {
            volume: 0.25,
            loop_count: Some(4),
            play_mode: PlayMode::RepeatAll,
            playlist: vec![String::from("/songs/A.M")],
            ..Settings::default()
        };
        settings.muted[7] = true;
        settings.save();
        settings.flush(1_000.0);

        let back = Settings::load();
        assert_eq!(back.volume, 0.25);
        assert_eq!(back.loop_count, Some(4));
        assert_eq!(back.play_mode, PlayMode::RepeatAll);
        assert!(back.muted[7]);
        assert_eq!(back.playlist, vec![String::from("/songs/A.M")]);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn writing_is_deferred_and_rate_limited() {
        let mut settings = Settings::default();
        settings.flush_if_due(0.0);
        assert!(!settings.dirty, "a clean state needs no write");

        settings.save();
        assert!(settings.dirty);
        settings.flush_if_due(SAVE_INTERVAL_SECONDS - 0.1);
        assert!(settings.dirty, "the interval had not elapsed yet");
    }
}
