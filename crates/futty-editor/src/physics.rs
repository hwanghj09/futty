use eframe::egui;
use egui::RichText;
use futty_runtime::{Body, Motion, Object, Scene};
use glam::{Quat, Vec3};
use uuid::Uuid;

use crate::{FOCUS, MUTED, WARNING, vector_fields};

pub fn inspector(ui: &mut egui::Ui, object: &mut Object, [min, max]: [Vec3; 2]) {
    ui.add_space(10.0);
    ui.label(RichText::new("Physics").strong());
    ui.separator();
    let mut enabled = object.body.is_some();
    let response = ui.add_enabled(
        object.parent.is_none(),
        egui::Checkbox::new(&mut enabled, "Rigid body"),
    );
    if response.changed() {
        object.body = enabled.then(|| fitted_box(min, max));
    }
    if object.parent.is_some() {
        ui.label(RichText::new("Rigid bodies use scene-root transforms.").color(MUTED));
    }
    if let Some(body) = &mut object.body {
        egui::ComboBox::from_id_salt("body-motion")
            .selected_text(body.motion.label())
            .show_ui(ui, |ui| {
                for motion in [Motion::Dynamic, Motion::Fixed, Motion::Kinematic] {
                    ui.selectable_value(&mut body.motion, motion, motion.label());
                }
            });
        ui.checkbox(&mut body.sensor, "Trigger sensor")
            .on_hover_text("Reports objects entering and leaving this volume");
        if body.motion == Motion::Dynamic {
            scalar_field(ui, "Mass (kg)", &mut body.mass, 0.01..=1000.0);
            scalar_field(ui, "Gravity scale", &mut body.gravity_scale, 0.0..=10.0);
        }
        scalar_field(ui, "Friction", &mut body.friction, 0.0..=5.0);
        scalar_field(ui, "Restitution", &mut body.restitution, 0.0..=1.0);
        ui.horizontal(|ui| {
            ui.label("Box Collider");
            if ui.small_button("Fit to mesh").clicked() {
                let fitted = fitted_box(min, max);
                body.size = fitted.size;
                body.center = fitted.center;
            }
        });
        vector_fields(ui, "Local size", &mut body.size, 0.01..=20_000.0, 0.025);
        vector_fields(
            ui,
            "Local center",
            &mut body.center,
            -20_000.0..=20_000.0,
            0.025,
        );
    }
    if object
        .body
        .is_none_or(|body| body.motion == Motion::Kinematic)
    {
        ui.checkbox(&mut object.spin, "Rotate in Play");
    }
}

fn fitted_box(min: Vec3, max: Vec3) -> Body {
    Body {
        size: (max - min).max(Vec3::splat(0.01)),
        center: (min + max) * 0.5,
        ..Default::default()
    }
}

fn scalar_field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(value)
                .range(range)
                .speed(0.025)
                .fixed_decimals(2),
        );
    });
}

pub fn events<'a>(
    ui: &mut egui::Ui,
    bodies: usize,
    events: impl DoubleEndedIterator<Item = &'a futty_runtime::PhysicsEvent>,
    objects: &[Object],
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("60 Hz   ·   {bodies} bodies")).color(MUTED));
    });
    ui.separator();
    let name = |id: Uuid| {
        objects
            .iter()
            .find(|object| object.uuid == id)
            .map_or("Object", |object| object.name.as_str())
    };
    let mut events = events.rev().peekable();
    if events.peek().is_none() {
        ui.label(RichText::new("No collision or trigger events").color(MUTED));
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        for event in events {
            ui.horizontal(|ui| {
                ui.label(RichText::new(event.label()).color(if event.sensor {
                    WARNING
                } else {
                    FOCUS
                }));
                ui.label(format!("{}  /  {}", name(event.first), name(event.second)));
            });
        }
    });
}

pub fn playground() -> Scene {
    let mut scene = Scene::default();
    scene.name = "Physics Playground".into();
    let cube = |name: &str, position, scale, color, motion| {
        let mut object = Object::cube(name.into(), position);
        object.transform.scale = scale;
        object.color = color;
        object.spin = motion == Motion::Kinematic;
        object.body = Some(Body {
            motion,
            ..Default::default()
        });
        object
    };
    let drop = cube(
        "Drop cube",
        Vec3::new(-2.0, 3.5, 0.8),
        Vec3::ONE,
        [0.18, 0.36, 0.62],
        Motion::Dynamic,
    );
    let mut bounce = cube(
        "Bouncy cube",
        Vec3::new(0.3, 4.6, -0.6),
        Vec3::splat(0.72),
        [0.56, 0.28, 0.12],
        Motion::Dynamic,
    );
    bounce.transform.rotation = Quat::from_euler(glam::EulerRot::XYZ, 0.3, 0.2, 0.15);
    bounce.body.as_mut().unwrap().restitution = 0.75;
    let mut ramp = cube(
        "Fixed ramp",
        Vec3::new(0.2, 1.05, -0.7),
        Vec3::new(2.1, 0.2, 1.4),
        [0.28; 3],
        Motion::Fixed,
    );
    ramp.transform.rotation = Quat::from_rotation_z(-0.22);
    let mut trigger = cube(
        "Trigger gate",
        Vec3::new(-2.8, 1.5, 0.8),
        Vec3::new(0.12, 0.12, 1.2),
        [0.52, 0.31, 0.14],
        Motion::Fixed,
    );
    trigger.body = Some(Body {
        motion: Motion::Fixed,
        sensor: true,
        size: Vec3::new(13.0, 3.0, 1.15),
        center: Vec3::new(6.4, 0.0, 0.0),
        ..Default::default()
    });
    scene.entities = vec![
        drop,
        bounce,
        ramp,
        trigger,
        cube(
            "Rotating platform",
            Vec3::new(2.2, 0.55, 1.0),
            Vec3::new(2.4, 0.15, 0.5),
            [0.19, 0.34, 0.31],
            Motion::Kinematic,
        ),
        cube(
            "Ground",
            Vec3::new(0.0, -0.25, 0.0),
            Vec3::new(8.4, 0.5, 6.3),
            [0.25; 3],
            Motion::Fixed,
        ),
    ];
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use futty_runtime::Runtime;

    #[test]
    fn playground_demo_reaches_floor_after_trigger() {
        let scene = playground();
        let dropped = scene.entities[0].uuid;
        let gate = scene.entities[3].uuid;
        let mut runtime = Runtime::new(&scene).unwrap();
        for _ in 0..140 {
            runtime.tick(1.0 / 60.0);
        }
        assert!(runtime.physics_events().any(|event| {
            event.first == gate && event.second == dropped && event.sensor && !event.started
        }));
        let (_, cube, _) = runtime.extract().find(|&(id, _, _)| id == dropped).unwrap();
        assert!((cube.translation.y - 0.5).abs() < 0.025, "{cube:?}");
        assert_eq!(scene.entities[0].transform.translation.y, 3.5);
    }
}
