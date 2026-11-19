use eframe::egui;
use egui::RichText;
use futty_core::{Asset, AssetKind};
use futty_runtime::{AudioListener, AudioSource, AudioStart, Object, Runtime};
use glam::Vec3;
use uuid::Uuid;

use crate::{Editor, MUTED};

impl Editor {
    pub(crate) fn load_audio_playground(&mut self) {
        let bytes = include_bytes!("../../../examples/audio-playground/Chime.wav");
        let clip = Uuid::new_v4();
        let mut audio = futty_audio::Audio::default();
        if let Err(error) = audio.load(clip, "wav", bytes) {
            self.report(format!("Audio Playground could not open: {error}"), true);
            return;
        }
        let mut scene = crate::physics::playground();
        scene.name = "Audio Playground".into();
        scene.assets.push(Asset {
            id: clip,
            name: "Chime".into(),
            kind: AssetKind::Wav,
        });
        let trigger = scene
            .entities
            .iter_mut()
            .find(|object| object.body.is_some_and(|body| body.sensor))
            .expect("the physics playground includes a trigger");
        trigger.audio_source = Some(AudioSource {
            start: AudioStart::OnTriggerEnter,
            ..AudioSource::new(clip)
        });
        let selected = trigger.uuid;
        self.clear_assets();
        self.audio = audio;
        self.asset_bytes.insert(clip, bytes.to_vec());
        self.doc = crate::document::Document::new(scene, None);
        self.doc.selection.replace(Some(selected));
        self.viewport.camera.eye = Vec3::new(8.3, 6.2, 10.7);
        self.viewport.camera.look_at(Vec3::Y * 1.9);
        self.report(
            "Audio Playground ready. Press Play to hear the trigger chime.",
            false,
        );
    }
}

pub(crate) fn pump(audio: &mut futty_audio::Audio, runtime: &mut Runtime) -> Option<String> {
    for object in runtime.drain_stopped_audio() {
        audio.stop_object(object);
    }
    let volume = runtime.audio_gain();
    let mut issue = None;
    for cue in runtime.drain_audio() {
        if let Err(error) = audio.play(
            cue.object,
            cue.source.clip,
            cue.source.volume * volume,
            cue.source.looping,
        ) {
            issue.get_or_insert_with(|| error.to_string());
        }
    }
    issue
}

pub(crate) fn inspector(
    ui: &mut egui::Ui,
    object: &mut Object,
    assets: &[Asset],
    listener_available: bool,
) {
    egui::CollapsingHeader::new("Audio")
        .default_open(object.audio_source.is_some() || object.audio_listener.is_some())
        .show(ui, |ui| {
            let mut clip = object.audio_source.map(|source| source.clip);
            let selected = assets
                .iter()
                .find(|asset| Some(asset.id) == clip)
                .map_or("None", |asset| asset.name.as_str());
            ui.label("Audio source");
            egui::ComboBox::from_id_salt("audio-clip")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut clip, None, "None");
                    for asset in assets.iter().filter(|asset| asset.kind.is_audio()) {
                        ui.selectable_value(&mut clip, Some(asset.id), &asset.name);
                    }
                });
            object.audio_source = clip.map(|clip| AudioSource {
                clip,
                ..object
                    .audio_source
                    .unwrap_or_else(|| AudioSource::new(clip))
            });
            if let Some(source) = &mut object.audio_source {
                ui.add(egui::Slider::new(&mut source.volume, 0.0..=1.0).text("Volume"));
                ui.checkbox(&mut source.looping, "Loop");
                egui::ComboBox::from_id_salt("audio-start")
                    .selected_text(source.start.label())
                    .show_ui(ui, |ui| {
                        for mode in [
                            AudioStart::OnPlay,
                            AudioStart::OnTriggerEnter,
                            AudioStart::Manual,
                        ] {
                            ui.selectable_value(&mut source.start, mode, mode.label());
                        }
                    });
            }
            ui.separator();
            let mut enabled = object.audio_listener.is_some();
            if ui
                .add_enabled(
                    listener_available,
                    egui::Checkbox::new(&mut enabled, "Audio listener"),
                )
                .changed()
            {
                object.audio_listener = enabled.then_some(AudioListener { volume: 1.0 });
            }
            if let Some(listener) = &mut object.audio_listener {
                ui.add(egui::Slider::new(&mut listener.volume, 0.0..=1.0).text("Master volume"));
            } else if !listener_available {
                ui.label(RichText::new("The scene already has an audio listener.").color(MUTED));
            }
        });
}
