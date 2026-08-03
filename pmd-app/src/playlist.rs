// SPDX-License-Identifier: AGPL-3.0-only

use crate::settings::PlayMode;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub path: String,

    pub name: String,

    pub seconds: Option<u32>,
}

impl Entry {
    pub fn from_path(path: &str) -> Self {
        let name = path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(path)
            .to_uppercase();
        Self {
            path: path.to_owned(),
            name,
            seconds: None,
        }
    }

    pub fn duration_label(&self) -> String {
        match self.seconds {
            Some(seconds) => format!("{:02}:{:02}", seconds / 60 % 100, seconds % 60),
            None => String::from("--:--"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Playlist {
    entries: Vec<Entry>,

    playing: Option<usize>,

    cursor: usize,

    steps: u32,
}

impl Playlist {
    pub fn from_paths<I: IntoIterator<Item = String>>(paths: I) -> Self {
        Self {
            entries: paths
                .into_iter()
                .map(|path| Entry::from_path(&path))
                .collect(),
            ..Self::default()
        }
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn playing(&self) -> Option<usize> {
        self.playing
    }

    pub fn cursor(&self) -> usize {
        self.cursor.min(self.entries.len().saturating_sub(1))
    }

    pub fn current(&self) -> Option<&Entry> {
        self.playing.and_then(|index| self.entries.get(index))
    }

    pub fn paths(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect()
    }

    pub fn extend(&mut self, paths: &[String]) -> usize {
        let mut added = 0;
        for path in paths {
            if self.entries.iter().any(|entry| entry.path == *path) {
                continue;
            }
            self.entries.push(Entry::from_path(path));
            added += 1;
        }
        added
    }

    pub fn remove(&mut self, index: usize) {
        if index >= self.entries.len() {
            return;
        }
        self.entries.remove(index);
        self.playing = match self.playing {
            Some(playing) if playing == index => None,
            Some(playing) if playing > index => Some(playing - 1),
            other => other,
        };
        self.cursor = self.cursor.min(self.entries.len().saturating_sub(1));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.playing = None;
        self.cursor = 0;
    }

    pub fn move_cursor(&mut self, delta: i32) {
        if self.entries.is_empty() {
            self.cursor = 0;
            return;
        }
        let last = self.entries.len() as i32 - 1;
        self.cursor = (self.cursor as i32 + delta).clamp(0, last) as usize;
    }

    pub fn set_cursor(&mut self, index: usize) {
        self.cursor = index.min(self.entries.len().saturating_sub(1));
    }

    pub fn set_playing(&mut self, index: usize) {
        if index < self.entries.len() {
            self.playing = Some(index);
            self.cursor = index;
        }
    }

    pub fn record_length(&mut self, seconds: u32) {
        if let Some(entry) = self.playing.and_then(|index| self.entries.get_mut(index)) {
            entry.seconds = Some(seconds);
        }
    }

    pub fn next_under(&mut self, mode: PlayMode) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        let current = self.playing.unwrap_or(0);
        self.steps = self.steps.wrapping_add(1);
        match mode {
            PlayMode::RepeatOne => Some(current),
            PlayMode::Single => (current + 1 < self.entries.len()).then_some(current + 1),
            PlayMode::RepeatAll => Some((current + 1) % self.entries.len()),
            PlayMode::Shuffle => Some(self.shuffled(current)),
        }
    }

    pub fn previous(&self) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        let current = self.playing.unwrap_or(0);
        Some((current + self.entries.len() - 1) % self.entries.len())
    }

    pub fn next(&self) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        let current = self.playing.unwrap_or(0);
        Some((current + 1) % self.entries.len())
    }

    fn shuffled(&self, current: usize) -> usize {
        let len = self.entries.len();
        if len <= 1 {
            return 0;
        }

        let mut state = self
            .steps
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223)
            .wrapping_add(current as u32);
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let pick = (state >> 8) as usize % (len - 1);
        if pick >= current {
            pick + 1
        } else {
            pick
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(count: usize) -> Playlist {
        Playlist::from_paths((0..count).map(|index| format!("/songs/TRACK{index}.M")))
    }

    #[test]
    fn a_display_name_is_the_uppercased_basename() {
        let entry = Entry::from_path("/music/th5/op.m2");
        assert_eq!(entry.name, "OP.M2");
        assert_eq!(entry.path, "/music/th5/op.m2");
        assert_eq!(entry.duration_label(), "--:--");
    }

    #[test]
    fn a_known_length_prints_as_minutes_and_seconds() {
        let entry = Entry {
            seconds: Some(125),
            ..Entry::from_path("A.M")
        };
        assert_eq!(entry.duration_label(), "02:05");
    }

    #[test]
    fn adding_the_same_folder_twice_does_not_double_the_list() {
        let mut playlist = list(0);
        let paths = vec![String::from("/a/A.M"), String::from("/a/B.M")];
        assert_eq!(playlist.extend(&paths), 2);
        assert_eq!(playlist.extend(&paths), 0);
        assert_eq!(playlist.len(), 2);
    }

    #[test]
    fn single_play_stops_at_the_end_and_repeat_all_wraps() {
        let mut playlist = list(3);
        playlist.set_playing(2);
        assert_eq!(playlist.next_under(PlayMode::Single), None);
        playlist.set_playing(2);
        assert_eq!(playlist.next_under(PlayMode::RepeatAll), Some(0));
        playlist.set_playing(2);
        assert_eq!(playlist.next_under(PlayMode::RepeatOne), Some(2));
    }

    #[test]
    fn shuffle_never_returns_the_entry_it_was_given() {
        let mut playlist = list(8);
        for start in 0..8 {
            playlist.set_playing(start);
            for _ in 0..12 {
                let pick = playlist.next_under(PlayMode::Shuffle).expect("a pick");
                assert_ne!(pick, start, "shuffle repeated the current track");
                assert!(pick < 8);
            }
        }
    }

    #[test]
    fn shuffle_of_a_single_entry_list_returns_that_entry() {
        let mut playlist = list(1);
        playlist.set_playing(0);
        assert_eq!(playlist.next_under(PlayMode::Shuffle), Some(0));
    }

    #[test]
    fn an_empty_list_never_advances() {
        let mut playlist = list(0);
        for mode in [
            PlayMode::Single,
            PlayMode::RepeatOne,
            PlayMode::RepeatAll,
            PlayMode::Shuffle,
        ] {
            assert_eq!(playlist.next_under(mode), None);
        }
        assert_eq!(playlist.next(), None);
        assert_eq!(playlist.previous(), None);
        assert_eq!(playlist.cursor(), 0);
    }

    #[test]
    fn removing_the_playing_entry_clears_the_playing_index() {
        let mut playlist = list(3);
        playlist.set_playing(1);
        playlist.remove(1);
        assert_eq!(playlist.playing(), None);
        assert_eq!(playlist.len(), 2);
    }

    #[test]
    fn removing_an_earlier_entry_shifts_the_playing_index_down() {
        let mut playlist = list(3);
        playlist.set_playing(2);
        playlist.remove(0);
        assert_eq!(playlist.playing(), Some(1));
    }

    #[test]
    fn removing_past_the_end_is_ignored_rather_than_panicking() {
        let mut playlist = list(2);
        playlist.remove(9);
        assert_eq!(playlist.len(), 2);
    }

    #[test]
    fn the_cursor_stays_inside_the_list() {
        let mut playlist = list(3);
        playlist.move_cursor(-5);
        assert_eq!(playlist.cursor(), 0);
        playlist.move_cursor(99);
        assert_eq!(playlist.cursor(), 2);
        playlist.clear();
        assert_eq!(playlist.cursor(), 0);
    }

    #[test]
    fn a_measured_length_lands_on_the_entry_that_was_playing() {
        let mut playlist = list(2);
        playlist.set_playing(1);
        playlist.record_length(90);
        assert_eq!(playlist.entries()[1].seconds, Some(90));
        assert_eq!(playlist.entries()[0].seconds, None);
    }
}
