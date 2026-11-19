use eframe::egui;
use futty_assets::{AnimationSample, Clip};
use futty_runtime::{Animation, Object};
use uuid::Uuid;

pub(crate) struct Preview {
    pub object: Uuid,
    clip: u32,
    seconds: f32,
    playing: bool,
}

impl Preview {
    pub fn sample(&self, animation: Option<Animation>) -> Option<AnimationSample> {
        animation
            .filter(|animation| animation.clip == self.clip)
            .map(|_| AnimationSample {
                clip: self.clip,
                seconds: f64::from(self.seconds),
                looping: false,
            })
    }

    pub fn tick(&mut self, dt: f32, object: &Object, clips: &[Clip]) -> bool {
        let Some(animation) = object
            .animation
            .filter(|animation| animation.clip == self.clip && self.playing)
        else {
            return false;
        };
        let Some(clip) = clips.iter().find(|clip| clip.index == self.clip) else {
            return false;
        };
        self.seconds += dt.clamp(0.0, 0.1) * animation.speed;
        if animation.looping && clip.duration > 0.0 {
            self.seconds %= clip.duration;
        } else if self.seconds >= clip.duration {
            self.seconds = clip.duration;
            self.playing = false;
        }
        true
    }
}

pub(crate) fn inspector(
    ui: &mut egui::Ui,
    object: &mut Object,
    clips: &[Clip],
    preview: &mut Option<Preview>,
) {
    if clips.is_empty() && object.animation.is_none() {
        return;
    }
    ui.collapsing("Animation", |ui| {
        let mut index = object.animation.map(|animation| animation.clip);
        let label = index.map_or("Bind pose", |index| {
            clips
                .iter()
                .find(|clip| clip.index == index)
                .map_or("Missing clip", |clip| clip.name.as_str())
        });
        egui::ComboBox::from_label("Start clip")
            .selected_text(label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut index, None, "Bind pose");
                for clip in clips {
                    ui.selectable_value(
                        &mut index,
                        Some(clip.index),
                        format!("{} · {}", clip.index, clip.name),
                    );
                }
            });
        if index != object.animation.map(|animation| animation.clip) {
            object.animation = index.map(|clip| Animation {
                clip,
                ..object.animation.unwrap_or_default()
            });
            *preview = None;
            ui.ctx().request_repaint();
        }
        let Some(animation) = &mut object.animation else {
            return;
        };
        let Some(clip) = clips.iter().find(|clip| clip.index == animation.clip) else {
            return;
        };
        ui.horizontal(|ui| {
            ui.checkbox(&mut animation.looping, "Loop");
            ui.label("Speed");
            ui.add(
                egui::DragValue::new(&mut animation.speed)
                    .range(0.05..=8.0)
                    .speed(0.05)
                    .suffix("×"),
            );
        });
        if !ui.is_enabled() || clip.duration <= 0.0 {
            return;
        }
        if !preview
            .as_ref()
            .is_some_and(|preview| preview.object == object.uuid && preview.clip == clip.index)
        {
            *preview = Some(Preview {
                object: object.uuid,
                clip: clip.index,
                seconds: 0.0,
                playing: false,
            });
        }
        let preview = preview.as_mut().unwrap();
        ui.horizontal(|ui| {
            if ui
                .button(if preview.playing { "Pause" } else { "Preview" })
                .clicked()
            {
                if preview.seconds >= clip.duration {
                    preview.seconds = 0.0;
                }
                preview.playing = !preview.playing;
                ui.ctx().request_repaint();
            }
            if ui
                .add(egui::Slider::new(&mut preview.seconds, 0.0..=clip.duration).suffix(" s"))
                .changed()
            {
                preview.playing = false;
                ui.ctx().request_repaint();
            }
        });
    });
}
