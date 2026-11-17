//! Combat emits bounded, cloneable events; only the GUI opens an audio device.
use crate::{game::Game, model::config::Result};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source, buffer::SamplesBuffer};
use std::{fs::File, path::Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Melee,
    Gunshot,
    Crossbow,
    Reload,
    Throw,
    Explosion,
    FlashBang,
}

impl Sound {
    fn index(self) -> usize {
        self as usize
    }
}

pub struct SoundPlayer {
    stream: OutputStream,
    clips: [SamplesBuffer; 7],
    playing: Vec<Sink>,
}

impl SoundPlayer {
    pub fn open(root: &Path) -> Result<Self> {
        let clips = Self::load_clips(root)?;
        let mut stream = OutputStreamBuilder::open_default_stream()?;
        stream.log_on_drop(false);
        Ok(Self {
            stream,
            clips,
            playing: Vec::new(),
        })
    }

    fn load_clips(root: &Path) -> Result<[SamplesBuffer; 7]> {
        let load = |name: &str| -> Result<SamplesBuffer> {
            let decoder = Decoder::try_from(File::open(root.join("data/sound").join(name))?)?;
            let channels = decoder.channels();
            let rate = decoder.sample_rate();
            Ok(SamplesBuffer::new(
                channels,
                rate,
                decoder.collect::<Vec<_>>(),
            ))
        };
        Ok([
            load("melee_attack.wav")?,
            load("gunshot.wav")?,
            load("crossbow.wav")?,
            load("reload.wav")?,
            load("throw.wav")?,
            load("explosion.wav")?,
            load("flashbang.wav")?,
        ])
    }

    pub fn play(&mut self, sound: Sound) {
        self.playing.retain(|sink| !sink.empty());
        if self.playing.len() >= 16 {
            return;
        }
        let sink = Sink::connect_new(self.stream.mixer());
        sink.set_volume(0.45);
        sink.append(self.clips[sound.index()].clone());
        self.playing.push(sink);
    }
}

impl Game {
    pub(crate) fn queue_sound(&mut self, sound: Sound) {
        if self.sounds.len() < 64 {
            self.sounds.push_back(sound);
        }
    }

    pub fn take_sounds(&mut self) -> impl Iterator<Item = Sound> + '_ {
        self.sounds.drain(..)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_sound_only_follows_success_and_visible_actors() {
        let mut game = Game::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let p = game.player_id;
        let weapon = game.spawn("item.glock", None, Some(p), 1).unwrap();
        game.equip_auto(p, weapon).unwrap();
        assert!(game.reload(p).is_err());
        assert_eq!(game.take_sounds().count(), 0);
        game.spawn("ammo.pistol", None, Some(p), 20).unwrap();
        game.reload(p).unwrap();
        assert_eq!(game.take_sounds().collect::<Vec<_>>(), vec![Sound::Reload]);
        assert!(game.reload(p).is_err());
        assert_eq!(game.take_sounds().count(), 0);
        let origin = game.position(p).unwrap();
        let visible = game
            .spawn("monster.ghoul_archer", Some(origin.offset(2, 0)), None, 1)
            .unwrap();
        game.reload(visible).unwrap();
        assert_eq!(game.take_sounds().collect::<Vec<_>>(), vec![Sound::Reload]);
        let hidden = game
            .spawn(
                "monster.ghoul_archer",
                Some(crate::model::config::Pos { x: 40, y: 20, z: 0 }),
                None,
                1,
            )
            .unwrap();
        game.reload(hidden).unwrap();
        assert_eq!(game.take_sounds().count(), 0);
    }

    #[test]
    fn shipped_clips_decode_without_an_audio_device() {
        let clips = SoundPlayer::load_clips(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        for clip in clips {
            assert_eq!(clip.channels(), 1);
            assert_eq!(clip.sample_rate(), 44_100);
            let samples: Vec<_> = clip.collect();
            assert!(!samples.is_empty());
            assert!(samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
            assert!(samples.iter().any(|s| s.abs() > 0.1));
        }
        assert!(SoundPlayer::load_clips(Path::new("/nonexistent/my-rogue-sounds")).is_err());
    }
}
