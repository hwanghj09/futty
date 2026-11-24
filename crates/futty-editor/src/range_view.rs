use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2, pos2, vec2};
use futty_runtime::{GameInput, InputAction, MAGAZINE, ROUND_SECONDS, RangeInput, RangeStatus};

use crate::input::{InputSettings, MouseMode};

const INK: Color32 = Color32::from_rgb(12, 22, 29);
const MINT: Color32 = Color32::from_rgb(108, 239, 206);
const AMBER: Color32 = Color32::from_rgb(255, 164, 94);
const PAPER: Color32 = Color32::from_rgb(239, 246, 244);

#[derive(Default)]
pub(crate) struct ViewportControls {
    grabbed: bool,
    fresh: bool,
}

impl ViewportControls {
    pub(crate) fn cursor_free(&self) -> bool {
        !self.grabbed
    }

    pub(crate) fn release(&mut self, ctx: &egui::Context) {
        if std::mem::take(&mut self.grabbed) {
            self.fresh = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::CursorGrab(egui::CursorGrab::None));
            ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(true));
        }
    }

    pub(crate) fn input(
        &mut self,
        ctx: &egui::Context,
        game: Option<Rect>,
        mode: MouseMode,
        settings: &InputSettings,
        hud_blocks_mouse: bool,
        player_two: bool,
    ) -> (f32, GameInput, GameInput) {
        if self.grabbed
            && (mode == MouseMode::Drag
                || game.is_none()
                || ctx.input(|input| !input.focused || input.key_pressed(egui::Key::Escape)))
        {
            self.release(ctx);
        }
        let active = mode == MouseMode::Drag || self.grabbed;
        let fresh = self.grabbed && std::mem::take(&mut self.fresh);
        let editing_text = ctx.text_edit_focused();
        ctx.input(|input| {
            let mouse = self.grabbed
                || game.is_some_and(|rect| {
                    input
                        .pointer
                        .hover_pos()
                        .is_some_and(|pos| rect.contains(pos))
                });
            let mut keys = GameInput::default();
            let mut second = GameInput::default();
            if game.is_some()
                && active
                && input.focused
                && (mode == MouseMode::FirstPerson || !input.modifiers.command)
                && !editing_text
                && (mouse || !input.pointer.secondary_down())
            {
                keys = settings.sample(input, mouse && !fresh && !hud_blocks_mouse, player_two);
                if mode == MouseMode::FirstPerson {
                    keys.sprint = input.modifiers.ctrl;
                    keys.sneak = input.modifiers.shift;
                }
                if player_two {
                    second = InputSettings::sample_player_two(input);
                }
            }
            if self.grabbed && mode == MouseMode::Range {
                let fire = keys.action(InputAction::Fire);
                keys.range = RangeInput {
                    active: true,
                    look: keys.mouse_delta,
                    fire: !fresh && (fire.held || fire.pressed),
                    reload: keys.action(InputAction::Reload).pressed,
                    retry: keys.action(InputAction::Retry).pressed,
                };
            }
            let dt = if mode == MouseMode::FirstPerson && !active {
                0.0
            } else {
                input.stable_dt
            };
            (dt, keys, second)
        })
    }

    fn capture(&mut self, ctx: &egui::Context, grab: egui::CursorGrab) {
        self.grabbed = true;
        self.fresh = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::CursorGrab(grab));
        ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(false));
    }

    pub(crate) fn paint_first_person(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        allow_capture: bool,
    ) {
        if self.grabbed {
            return;
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                allow_capture,
                "Capture the mouse and play",
            )
        });
        if allow_capture && response.clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CursorPosition(
                    response.rect.center(),
                ));
            let grab = if cfg!(windows) {
                egui::CursorGrab::Locked
            } else {
                egui::CursorGrab::Confined
            };
            self.capture(ui.ctx(), grab);
            return;
        }
        let rect = response.rect;
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, Color32::from_black_alpha(112));
        painter.text(
            rect.center() - vec2(0.0, 13.0),
            Align2::CENTER_CENTER,
            "CLICK TO PLAY",
            FontId::monospace(24.0),
            PAPER,
        );
        painter.text(
            rect.center() + vec2(0.0, 19.0),
            Align2::CENTER_CENTER,
            "Esc releases the mouse",
            FontId::monospace(14.0),
            PAPER,
        );
    }

    pub(crate) fn paint(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        status: RangeStatus,
        settings: &InputSettings,
        show_default_hud: bool,
        allow_capture: bool,
    ) {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, true, "Activate Pulse Range")
        });
        if !self.grabbed && allow_capture && response.clicked() {
            self.capture(ui.ctx(), egui::CursorGrab::Confined);
        }

        let rect = response.rect;
        let painter = ui.painter().with_clip_rect(rect);
        let padding = if rect.width() < 480.0 { 8.0 } else { 17.0 };
        let reload = settings.label(InputAction::Reload);
        if show_default_hud {
            let score = Rect::from_min_size(rect.min + Vec2::splat(padding), vec2(122.0, 54.0));
            plate(
                &painter,
                score,
                MINT,
                "SCORE",
                &format!("{:04}", status.points),
            );
            if status.streak > 1 {
                painter.text(
                    score.left_bottom() + vec2(0.0, 8.0),
                    Align2::LEFT_TOP,
                    format!("{} HITS IN A ROW", status.streak),
                    FontId::monospace(12.0),
                    MINT,
                );
            }
            let clock = Rect::from_min_size(
                pos2(rect.right() - padding - 108.0, rect.top() + padding),
                vec2(108.0, 54.0),
            );
            let time_color = if status.remaining < 10.0 {
                AMBER
            } else {
                PAPER
            };
            plate(
                &painter,
                clock,
                time_color,
                "TIME LEFT",
                &format!("00:{:02}", status.remaining.ceil() as u8),
            );
            let meter = Rect::from_min_size(
                clock.left_bottom(),
                vec2(clock.width() * (status.remaining / ROUND_SECONDS), 3.0),
            );
            painter.rect_filled(meter, 0.0, time_color);

            let ammo = Rect::from_min_size(
                rect.right_bottom() - vec2(padding + 122.0, padding + 56.0),
                vec2(122.0, 56.0),
            );
            let ammo_color = if status.ammo == 0 || status.reloading {
                AMBER
            } else {
                MINT
            };
            let empty_label = format!("{reload} RELOAD");
            let ammo_label = if status.reloading {
                "RECHARGING"
            } else if status.ammo == 0 {
                &empty_label
            } else {
                "PULSE CELLS"
            };
            plate(
                &painter,
                ammo,
                ammo_color,
                ammo_label,
                &format!("{:02} / {MAGAZINE:02}", status.ammo),
            );
        }
        if self.grabbed && status.remaining > 0.0 {
            crosshair(&painter, rect.center(), status.hit_flash);
            if rect.width() > 505.0 {
                painter.text(
                    rect.left_bottom() + vec2(padding, -padding),
                    Align2::LEFT_BOTTOM,
                    format!(
                        "{} MOVE    {reload} RELOAD    ESC PAUSE",
                        movement_keys(settings)
                    ),
                    FontId::monospace(11.0),
                    PAPER,
                );
            }
        } else {
            self.paint_card(&painter, rect, status, settings);
        }
    }

    fn paint_card(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        status: RangeStatus,
        settings: &InputSettings,
    ) {
        let finished = status.remaining <= 0.0;
        let fresh = status.remaining >= ROUND_SECONDS;
        let card = Rect::from_center_size(
            rect.center(),
            vec2((rect.width() - 16.0).clamp(0.0, 420.0), 158.0),
        );
        painter.rect_filled(card, 10.0, INK.gamma_multiply(0.94));
        painter.line_segment(
            [
                card.left_top() + vec2(12.0, 1.0),
                card.right_top() - vec2(12.0, -1.0),
            ],
            Stroke::new(2.0, MINT),
        );
        let title = if finished {
            "ROUND COMPLETE"
        } else if fresh {
            "PULSE RANGE"
        } else {
            "PAUSED"
        };
        let detail = if finished {
            let accuracy = (status.hits * 100).checked_div(status.shots).unwrap_or(0);
            format!(
                "{} POINTS   /   {} HITS   /   {}% ACC",
                status.points, status.hits, accuracy
            )
        } else {
            format!(
                "{} MOVE  /  {} FIRE  /  F10 CONTROLS",
                movement_keys(settings),
                settings.label(InputAction::Fire)
            )
        };
        let retry = settings.label(InputAction::Retry);
        let prompt = if finished && self.grabbed {
            format!("{retry}  PLAY AGAIN")
        } else if finished {
            format!("CLICK, THEN {retry} TO RETRY")
        } else if fresh {
            "CLICK TO START".into()
        } else {
            "CLICK TO RESUME".into()
        };
        let font_size = if card.width() >= 345.0 { 31.0 } else { 22.0 };
        for (y, text, font, color) in [
            (38.0, title, FontId::proportional(font_size), PAPER),
            (77.0, detail.as_str(), FontId::monospace(11.0), PAPER),
            (117.0, prompt.as_str(), FontId::monospace(15.0), MINT),
        ] {
            painter.text(
                pos2(card.center().x, card.top() + y),
                Align2::CENTER_CENTER,
                text,
                font,
                color,
            );
        }
    }
}

fn movement_keys(settings: &InputSettings) -> String {
    [
        InputAction::MoveForward,
        InputAction::MoveLeft,
        InputAction::MoveBackward,
        InputAction::MoveRight,
    ]
    .map(|action| settings.label(action))
    .join("/")
}

fn plate(painter: &egui::Painter, rect: Rect, accent: Color32, label: &str, value: &str) {
    painter.rect_filled(rect, 6.0, INK.gamma_multiply(0.91));
    let x = rect.left() + 11.0;
    painter.text(
        pos2(x, rect.top() + 7.0),
        Align2::LEFT_TOP,
        label,
        FontId::monospace(11.0),
        accent,
    );
    painter.text(
        pos2(x, rect.top() + 22.0),
        Align2::LEFT_TOP,
        value,
        FontId::monospace(21.0),
        PAPER,
    );
}

fn crosshair(painter: &egui::Painter, center: Pos2, hit: bool) {
    let axes = if hit {
        [vec2(0.71, 0.71), vec2(-0.71, 0.71)]
    } else {
        [Vec2::X, Vec2::Y]
    };
    for axis in axes.into_iter().flat_map(|axis| [axis, -axis]) {
        let line = [
            center + axis * 5.0,
            center + axis * if hit { 13.0 } else { 10.0 },
        ];
        painter.line_segment(line, Stroke::new(3.0, INK));
        painter.line_segment(line, Stroke::new(1.25, if hit { MINT } else { PAPER }));
    }
    painter.circle_filled(center, 1.25, if hit { MINT } else { PAPER });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_person_keeps_raw_look_and_hotbar_events_until_escape() {
        let ctx = egui::Context::default();
        let mut viewport = ViewportControls::default();
        let settings = InputSettings::default();
        let game = Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 360.0)));
        viewport.capture(&ctx, egui::CursorGrab::Locked);
        let key = |key, pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let sample = |ui: &mut egui::Ui, viewport: &mut ViewportControls| {
            viewport.input(
                ui.ctx(),
                game,
                MouseMode::FirstPerson,
                &settings,
                false,
                false,
            )
        };
        let mut first = ctx.run_ui(egui::RawInput::default(), |ui| {
            let (_, keys, _) = sample(ui, &mut viewport);
            assert_eq!(keys.mouse_delta, glam::Vec2::ZERO);
        });
        first.textures_delta.clear();
        let mut moving = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::MouseMoved(vec2(14.0, -7.0)),
                    key(egui::Key::Num3, true),
                    key(egui::Key::Num3, false),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Line,
                        delta: vec2(0.0, -1.0),
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Unknown,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::ModifiersChanged(egui::Modifiers {
                        ctrl: true,
                        command: true,
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            },
            |ui| {
                let (dt, keys, _) = sample(ui, &mut viewport);
                assert!(dt > 0.0 && keys.sprint);
                assert_eq!(keys.mouse_delta, glam::vec2(14.0, -7.0));
                assert_eq!(keys.pressed_keys, ["3"]);
                assert_eq!(keys.wheel_y, -1.0);
            },
        );
        moving.textures_delta.clear();
        let mut released = ctx.run_ui(
            egui::RawInput {
                events: vec![key(egui::Key::Escape, true)],
                ..Default::default()
            },
            |ui| {
                let (dt, _, _) = sample(ui, &mut viewport);
                assert_eq!(dt, 0.0);
                assert!(viewport.cursor_free());
            },
        );
        released.textures_delta.clear();
    }
}
