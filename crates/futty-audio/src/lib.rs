use std::{collections::HashMap, error::Error, io::Cursor, sync::Arc, time::Duration};

use kira::{
    AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Tween,
    sound::{
        FromFileError, PlaybackState,
        streaming::{StreamingSoundData, StreamingSoundHandle},
    },
};
use uuid::Uuid;

pub type AudioResult<T> = Result<T, Box<dyn Error>>;
type Voice = StreamingSoundHandle<FromFileError>;
const MAX_VOICES: usize = 24;

struct Clip {
    bytes: Arc<[u8]>,
    duration: Duration,
}

impl Clip {
    fn read(extension: &str, bytes: &[u8]) -> AudioResult<Self> {
        let expected = match extension {
            "wav" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE"),
            "ogg" => bytes.starts_with(b"OggS"),
            _ => false,
        };
        if !expected {
            return Err("Audio file does not match its WAV or OGG extension".into());
        }
        let bytes: Arc<[u8]> = bytes.into();
        let probed = StreamingSoundData::from_cursor(Cursor::new(bytes.clone()))?;
        let duration = probed.duration();
        if duration.is_zero() {
            return Err("Audio clip is empty".into());
        }
        Ok(Self { bytes, duration })
    }

    fn sound(&self, gain: f32, looping: bool) -> AudioResult<StreamingSoundData<FromFileError>> {
        let data = StreamingSoundData::from_cursor(Cursor::new(self.bytes.clone()))?
            .volume(decibels(gain));
        Ok(if looping { data.loop_region(..) } else { data })
    }
}

fn decibels(gain: f32) -> Decibels {
    if gain == 0.0 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * gain.log10())
    }
}

#[derive(Default)]
pub struct Audio {
    clips: HashMap<Uuid, Clip>,
    manager: Option<AudioManager<DefaultBackend>>,
    unavailable: Option<String>,
    preview: Option<Voice>,
    voices: Vec<(Uuid, Voice)>,
}

impl Audio {
    pub fn load(&mut self, id: Uuid, extension: &str, bytes: &[u8]) -> AudioResult<()> {
        self.clips.insert(id, Clip::read(extension, bytes)?);
        Ok(())
    }

    pub fn duration(&self, id: Uuid) -> Option<Duration> {
        self.clips.get(&id).map(|clip| clip.duration)
    }

    pub fn clear(&mut self) {
        self.stop();
        self.clips.clear();
    }

    pub fn preview(&mut self, clip: Uuid) -> AudioResult<()> {
        self.stop_preview();
        self.unavailable = None;
        let sound = self.sound(clip, 0.65, false)?;
        let handle = self.manager()?.play(sound)?;
        self.preview = Some(handle);
        Ok(())
    }

    pub fn stop_preview(&mut self) {
        if let Some(mut voice) = self.preview.take() {
            voice.stop(Tween::default());
        }
    }

    pub fn preview_active(&self) -> bool {
        self.preview
            .as_ref()
            .is_some_and(|voice| voice.state() != PlaybackState::Stopped)
    }

    pub fn play(&mut self, object: Uuid, clip: Uuid, gain: f32, looping: bool) -> AudioResult<()> {
        if self.voices.len() == MAX_VOICES
            || (looping && self.voices.iter().any(|(owner, _)| *owner == object))
        {
            return Ok(());
        }
        let sound = self.sound(clip, gain, looping)?;
        let handle = self.manager()?.play(sound)?;
        self.voices.push((object, handle));
        Ok(())
    }

    pub fn update(&mut self) -> Option<String> {
        let mut error = self.preview.as_mut().and_then(|voice| voice.pop_error());
        if !self.preview_active() {
            self.preview = None;
        }
        self.voices.retain_mut(|(_, voice)| {
            if let Some(next) = voice.pop_error() {
                error.get_or_insert(next);
            }
            voice.state() != PlaybackState::Stopped
        });
        error.map(|error| error.to_string())
    }

    pub fn stop(&mut self) {
        self.stop_preview();
        for (_, mut voice) in self.voices.drain(..) {
            voice.stop(Tween::default());
        }
        self.unavailable = None;
    }

    pub fn stop_object(&mut self, object: Uuid) {
        self.voices.retain_mut(|(owner, voice)| {
            if *owner != object {
                return true;
            }
            voice.stop(Tween::default());
            false
        });
    }

    fn sound(
        &self,
        id: Uuid,
        gain: f32,
        looping: bool,
    ) -> AudioResult<StreamingSoundData<FromFileError>> {
        if !(0.0..=1.0).contains(&gain) {
            return Err("Invalid volume".into());
        }
        self.clips
            .get(&id)
            .ok_or("Audio clip is missing")?
            .sound(gain, looping)
    }

    fn manager(&mut self) -> AudioResult<&mut AudioManager<DefaultBackend>> {
        if let Some(reason) = &self.unavailable {
            return Err(reason.clone().into());
        }
        if self.manager.is_none() {
            match AudioManager::new(AudioManagerSettings::default()) {
                Ok(manager) => self.manager = Some(manager),
                Err(error) => {
                    let reason = format!("Audio output unavailable: {error}");
                    self.unavailable = Some(reason.clone());
                    return Err(reason.into());
                }
            }
        }
        Ok(self.manager.as_mut().expect("initialized above"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAV: &[u8] = include_bytes!("../../../examples/audio-playground/Chime.wav");
    const OGG: &[u8] = include_bytes!("../../../examples/audio-playground/Chime.ogg");

    #[test]
    fn cached_wav_and_vorbis_are_independently_playable() {
        for (extension, bytes) in [("wav", WAV), ("ogg", OGG)] {
            let clip = Clip::read(extension, bytes).unwrap();
            assert!((clip.duration.as_secs_f32() - 0.54).abs() < 0.08);
            let once = clip.sound(0.5, false).unwrap();
            let repeating = clip.sound(1.0, true).unwrap();
            assert!(once.settings.loop_region.is_none());
            assert!(repeating.settings.loop_region.is_some());
            assert_eq!(once.settings.volume, kira::Value::Fixed(decibels(0.5)));
        }
        assert!(Clip::read("ogg", WAV).is_err());
        assert!(Clip::read("wav", &WAV[..12]).is_err());
        assert_eq!(decibels(0.0), Decibels::SILENCE);
    }

    #[test]
    #[ignore = "requires a local audio output device"]
    fn output_device_advances_preview() {
        let mut audio = Audio::default();
        let asset = Uuid::new_v4();
        audio.load(asset, "wav", WAV).unwrap();
        audio.preview(asset).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            assert!(audio.update().is_none());
            let playing = audio
                .preview
                .as_ref()
                .expect("audio ended before advancing");
            if playing.position() > 0.03 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "audio output did not advance"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        audio.stop();
        assert!(!audio.preview_active());
    }
}
