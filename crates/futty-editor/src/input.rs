use eframe::egui::{self, Event, Key, PointerButton};
use futty_runtime::{ActionState, GameInput, InputAction};
use serde::{Deserialize, Serialize};

const STORAGE_KEY: &str = "futty.game-input.v1";

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Binding {
    Key(Key),
    Mouse(PointerButton),
}

impl Binding {
    fn label(self) -> &'static str {
        match self {
            Self::Key(key) => key.name(),
            Self::Mouse(PointerButton::Primary) => "Mouse 1",
            Self::Mouse(PointerButton::Secondary) => "Mouse 2",
            Self::Mouse(PointerButton::Middle) => "Mouse 3",
            Self::Mouse(PointerButton::Extra1) => "Mouse 4",
            Self::Mouse(PointerButton::Extra2) => "Mouse 5",
        }
    }

    fn read(self, input: &egui::InputState, mouse: bool) -> ActionState {
        match self {
            Self::Key(key) => ActionState {
                held: input.key_down(key),
                pressed: input.events.iter().any(|event| {
                    matches!(event, Event::Key { key: actual, pressed: true, repeat: false, .. } if *actual == key)
                }),
                released: input.key_released(key),
            },
            Self::Mouse(button) if mouse => ActionState {
                held: input.pointer.button_down(button),
                pressed: input.pointer.button_pressed(button),
                released: input.pointer.button_released(button),
            },
            Self::Mouse(_) => ActionState::default(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct InputSettings {
    bindings: [Binding; InputAction::ALL.len()],
    sensitivity: f32,
}

impl Default for InputSettings {
    fn default() -> Self {
        Self {
            bindings: InputAction::ALL.map(|action| {
                Binding::Key(match action {
                    InputAction::MoveForward => Key::W,
                    InputAction::MoveBackward => Key::S,
                    InputAction::MoveLeft => Key::A,
                    InputAction::MoveRight => Key::D,
                    InputAction::Jump => Key::Space,
                    InputAction::Fire => return Binding::Mouse(PointerButton::Primary),
                    InputAction::Reload => Key::R,
                    InputAction::Interact => Key::E,
                    InputAction::Retry => Key::Enter,
                })
            }),
            sensitivity: 1.0,
        }
    }
}

impl InputSettings {
    pub(crate) fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        storage
            .and_then(|storage| eframe::get_value::<Self>(storage, STORAGE_KEY))
            .filter(|settings| (0.1..=4.0).contains(&settings.sensitivity))
            .unwrap_or_default()
    }

    pub(crate) fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, self);
    }

    pub(crate) fn label(&self, action: InputAction) -> &'static str {
        self.bindings[action as usize].label()
    }

    pub(crate) fn sample(&self, input: &egui::InputState, mouse: bool) -> GameInput {
        let [forward, backward, left, right] = [
            InputAction::MoveForward,
            InputAction::MoveBackward,
            InputAction::MoveLeft,
            InputAction::MoveRight,
        ];
        let actions = self.bindings.map(|binding| binding.read(input, mouse));
        let delta = if mouse {
            input
                .pointer
                .motion()
                .unwrap_or_else(|| input.pointer.delta())
        } else {
            egui::Vec2::ZERO
        };
        GameInput {
            forward: actions[forward as usize].held,
            backward: actions[backward as usize].held,
            left: actions[left as usize].held,
            right: actions[right as usize].held,
            held: Key::ALL
                .iter()
                .copied()
                .filter(|key| input.key_down(*key))
                .map(Key::name)
                .collect(),
            actions,
            mouse_delta: glam::vec2(delta.x, delta.y) * self.sensitivity,
            ..Default::default()
        }
    }

    pub(crate) fn toggle_shortcut(ctx: &egui::Context, open: &mut bool) {
        if ctx.input_mut(|input| {
            Binding::Key(Key::F10).read(input, false).pressed
                && input.consume_key(egui::Modifiers::NONE, Key::F10)
        }) {
            *open = !*open;
        }
    }

    pub(crate) fn window(&mut self, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new("Game Controls")
            .open(open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("game-bindings")
                    .num_columns(2)
                    .show(ui, |ui| {
                        for action in InputAction::ALL {
                            let binding = &mut self.bindings[action as usize];
                            ui.label(action.name());
                            egui::ComboBox::from_id_salt(action.name())
                                .selected_text(binding.label())
                                .width(125.0)
                                .show_ui(ui, |ui| {
                                    for button in [
                                        PointerButton::Primary,
                                        PointerButton::Secondary,
                                        PointerButton::Middle,
                                        PointerButton::Extra1,
                                        PointerButton::Extra2,
                                    ] {
                                        let choice = Binding::Mouse(button);
                                        ui.selectable_value(binding, choice, choice.label());
                                    }
                                    ui.separator();
                                    for &key in Key::ALL
                                        .iter()
                                        .filter(|&&key| !matches!(key, Key::Escape | Key::F10))
                                    {
                                        let choice = Binding::Key(key);
                                        ui.selectable_value(binding, choice, choice.label());
                                    }
                                });
                            ui.end_row();
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Mouse sensitivity");
                    ui.add(
                        egui::DragValue::new(&mut self.sensitivity)
                            .range(0.1..=4.0)
                            .speed(0.02)
                            .suffix("×"),
                    );
                });
                if ui.button("Reset to Defaults").clicked() {
                    *self = Self::default();
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_bindings_preserve_held_keys_and_a_click_between_frames() {
        let mut settings = InputSettings::default();
        settings.bindings[InputAction::MoveForward as usize] = Binding::Key(Key::ArrowUp);
        settings.sensitivity = 2.0;
        let settings: InputSettings = ron::from_str(&ron::to_string(&settings).unwrap()).unwrap();
        let ctx = egui::Context::default();
        let key = |pressed| Event::Key {
            key: Key::ArrowUp,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let click = |pressed| Event::PointerButton {
            pos: egui::Pos2::ZERO,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let frame = |events| egui::RawInput {
            events,
            ..Default::default()
        };

        ctx.run_ui(
            frame(vec![
                key(true),
                click(true),
                click(false),
                Event::MouseMoved(egui::vec2(3.0, -2.0)),
            ]),
            |ui| {
                let game = ui.input(|input| settings.sample(input, true));
                assert_eq!(
                    game.action(InputAction::MoveForward),
                    ActionState {
                        held: true,
                        pressed: true,
                        released: false
                    }
                );
                assert_eq!(
                    game.action(InputAction::Fire),
                    ActionState {
                        held: false,
                        pressed: true,
                        released: true
                    }
                );
                assert_eq!(game.mouse_delta, glam::vec2(6.0, -4.0));
            },
        )
        .textures_delta
        .clear();
        ctx.run_ui(frame(vec![]), |ui| {
            let game = ui.input(|input| settings.sample(input, true));
            assert_eq!(
                game.action(InputAction::MoveForward),
                ActionState {
                    held: true,
                    ..Default::default()
                }
            );
        })
        .textures_delta
        .clear();
        ctx.run_ui(frame(vec![key(false)]), |ui| {
            let game = ui.input(|input| settings.sample(input, true));
            assert_eq!(
                game.action(InputAction::MoveForward),
                ActionState {
                    released: true,
                    ..Default::default()
                }
            );
        })
        .textures_delta
        .clear();
    }
}
