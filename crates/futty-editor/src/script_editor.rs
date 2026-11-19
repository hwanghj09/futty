use eframe::egui;
use egui::{Key, RichText};
use futty_core::{Asset, AssetKind};
use futty_runtime::{MAX_SCRIPT_BYTES, Object, ScriptFields, ScriptValue, inspect_script};
use uuid::Uuid;

use crate::{Action, Editor, FOCUS, MUTED, WARNING, workspace::Panel};

const STARTER: &str = include_str!("../../../examples/script-playground/Move.rhai");

pub(crate) struct ScriptEditor {
    pub asset: Uuid,
    name: String,
    source: String,
    baseline: (String, String),
    check: Option<Result<usize, String>>,
}

impl ScriptEditor {
    fn new(asset: Uuid, name: String, source: String) -> Self {
        Self {
            baseline: (name.clone(), source.clone()),
            asset,
            name,
            source,
            check: None,
        }
    }

    pub(crate) fn dirty(&self) -> bool {
        self.name != self.baseline.0 || self.source != self.baseline.1
    }
}

pub(crate) type SchemaCache =
    std::collections::HashMap<Uuid, (String, Result<ScriptFields, String>)>;

impl Editor {
    pub(crate) fn create_script(&mut self) {
        if !self.apply_script_draft() {
            return;
        }
        let before = self.doc.scene.clone();
        let id = Uuid::new_v4();
        let name = (1..)
            .map(|index| {
                if index == 1 {
                    "NewBehaviour".to_owned()
                } else {
                    format!("NewBehaviour {index}")
                }
            })
            .find(|candidate| {
                !self
                    .doc
                    .scene
                    .assets
                    .iter()
                    .any(|asset| asset.kind == AssetKind::Script && asset.name == *candidate)
            })
            .expect("there is an unused script name");
        self.doc.scene.assets.push(Asset {
            id,
            name: name.clone(),
            kind: AssetKind::Script,
        });
        self.doc.scene.script_sources.insert(id, STARTER.into());
        for object in self
            .doc
            .scene
            .entities
            .iter_mut()
            .filter(|object| self.doc.selection.ids().contains(&object.uuid))
        {
            attach(object, Some(id));
        }
        self.doc.record_frame(before, false);
        self.script_inspector = true;
        self.edit_script_in_code(id);
    }

    pub(crate) fn edit_script_in_code(&mut self, id: Uuid) {
        self.sync_external_scripts();
        if !self.apply_script_draft() {
            return;
        }
        self.refresh_scripts();
        let Some(asset) = self
            .doc
            .scene
            .assets
            .iter()
            .find(|asset| asset.id == id && asset.kind == AssetKind::Script)
        else {
            return;
        };
        let Some(source) = script_source(&self.doc.scene, &self.asset_bytes, id).map(str::to_owned)
        else {
            self.report(
                format!("Missing or invalid source for {}", asset.name),
                true,
            );
            return;
        };
        match self.open_external_script(id, &asset.name.clone(), &source) {
            Ok(()) => {
                if self
                    .script_editor
                    .as_ref()
                    .is_some_and(|editor| editor.asset == id)
                {
                    self.script_editor = None;
                }
                self.report("Opened in VS Code. Save there to apply changes.", false);
            }
            Err(error) => {
                self.open_script(id);
                self.report(
                    format!("VS Code could not open. Using the futty editor. {error}"),
                    true,
                );
            }
        }
        self.script_inspector = true;
    }

    pub(crate) fn apply_external_source(&mut self, id: Uuid, source: String) {
        if script_source(&self.doc.scene, &self.asset_bytes, id) == Some(&source) {
            return;
        }
        let before = self.doc.scene.clone();
        let schema = inspect_script(&source);
        let error = schema.is_err();
        self.script_schemas.insert(id, (source.clone(), schema));
        self.doc.scene.script_sources.insert(id, source);
        self.doc.record_frame(before, false);
        self.report(
            if error {
                "VS Code save applied. See the Script Inspector for errors."
            } else {
                "Script updated from VS Code."
            },
            error,
        );
    }

    pub(crate) fn open_script(&mut self, id: Uuid) {
        self.sync_external_scripts();
        if self
            .script_editor
            .as_ref()
            .is_some_and(|editor| editor.asset == id)
        {
            self.show_panel(Panel::ScriptEditor);
            return;
        }
        if !self.apply_script_draft() {
            return;
        }
        self.refresh_scripts();
        let Some(asset) = self
            .doc
            .scene
            .assets
            .iter()
            .find(|asset| asset.id == id && asset.kind == AssetKind::Script)
        else {
            return;
        };
        let Some(source) = script_source(&self.doc.scene, &self.asset_bytes, id) else {
            self.report(
                format!("Missing or invalid source for {}", asset.name),
                true,
            );
            return;
        };
        self.script_editor = Some(ScriptEditor::new(id, asset.name.clone(), source.to_owned()));
        self.show_panel(Panel::ScriptEditor);
        if self.external_scripts.remove(&id).is_some() {
            self.report(
                "Editing in futty. Reopen Edit code to resume VS Code syncing.",
                false,
            );
        }
        self.script_inspector = true;
    }

    pub(crate) fn apply_script_draft(&mut self) -> bool {
        let Some(editor) = &mut self.script_editor else {
            return true;
        };
        if !editor.dirty() {
            return true;
        }
        let name = editor.name.trim();
        if name.is_empty() || editor.source.len() > MAX_SCRIPT_BYTES {
            editor.check = Some(Err(
                "Enter a script name and keep source under 64 KiB.".into()
            ));
            return false;
        }
        let before = self.doc.scene.clone();
        let Some(asset) = self
            .doc
            .scene
            .assets
            .iter_mut()
            .find(|asset| asset.id == editor.asset)
        else {
            editor.check = Some(Err(
                "This script is no longer in the scene. Undo the removal to apply the draft."
                    .into(),
            ));
            return false;
        };
        asset.name = name.to_owned();
        self.doc
            .scene
            .script_sources
            .insert(asset.id, editor.source.clone());
        editor.name = name.to_owned();
        editor.baseline = (editor.name.clone(), editor.source.clone());
        let schema = inspect_script(&editor.source);
        editor.check = Some(
            schema
                .as_ref()
                .map(|fields| fields.len())
                .map_err(Clone::clone),
        );
        let has_errors = schema.is_err();
        self.script_schemas
            .insert(asset.id, (editor.source.clone(), schema));
        self.doc.record_frame(before, false);
        self.report(
            if has_errors {
                "Code applied with script errors"
            } else {
                "Script code applied"
            },
            has_errors,
        );
        true
    }

    pub(crate) fn script_fields(&mut self) -> Option<(Uuid, Result<ScriptFields, String>)> {
        let id = self
            .doc
            .scene
            .entities
            .iter()
            .find(|object| Some(object.uuid) == self.doc.selection.primary())?
            .script?;
        let Some(source) = script_source(&self.doc.scene, &self.asset_bytes, id) else {
            return Some((id, Err("Script source is missing.".into())));
        };
        let cached = self
            .script_schemas
            .entry(id)
            .or_insert_with(|| (source.into(), inspect_script(source)));
        if cached.0 != source {
            *cached = (source.into(), inspect_script(source));
        }
        Some((id, cached.1.clone()))
    }

    pub(crate) fn show_script_panel(
        &mut self,
        ui: &mut egui::Ui,
        enabled: bool,
        action: &mut Option<Action>,
    ) {
        if let Some(editor) = &mut self.script_editor
            && !editor.dirty()
        {
            if let Some(asset) = self
                .doc
                .scene
                .assets
                .iter()
                .find(|asset| asset.id == editor.asset)
                && let Some(source) = script_source(&self.doc.scene, &self.asset_bytes, asset.id)
            {
                if asset.name != editor.name || source != editor.source {
                    *editor = ScriptEditor::new(asset.id, asset.name.clone(), source.into());
                }
            } else {
                self.script_editor = None;
            }
        }
        let Some(editor) = &mut self.script_editor else {
            ui.label(RichText::new("Choose Edit in futty on a script in Project.").color(MUTED));
            return;
        };
        let mut apply = false;
        let mut close = false;
        let mut discard = false;
        ui.add_enabled_ui(enabled, |ui| {
            ui.horizontal(|ui| {
                ui.label("Script name");
                ui.add(
                    egui::TextEdit::singleline(&mut editor.name)
                        .char_limit(64)
                        .desired_width((ui.available_width() - 110.0).clamp(40.0, 220.0)),
                );
                ui.label(RichText::new("Rhai").color(MUTED));
            });
            ui.separator();
            egui::ScrollArea::both()
                .max_height((ui.available_height() - 85.0).max(50.0))
                .show(ui, |ui| {
                    if ui
                        .add(
                            egui::TextEdit::multiline(&mut editor.source)
                                .id_source("futty-rhai-source")
                                .code_editor()
                                .desired_rows(18)
                                .desired_width(ui.available_width())
                                .char_limit(MAX_SCRIPT_BYTES),
                        )
                        .changed()
                    {
                        editor.check = None;
                    }
                });
            if let Some(report) = &editor.check {
                match report {
                    Ok(count) => {
                        ui.label(
                            RichText::new(format!("Code checked · {count} Inspector fields"))
                                .color(FOCUS),
                        );
                    }
                    Err(error) => {
                        ui.label(RichText::new(error).color(WARNING));
                    }
                }
            }
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                if ui.button("Check code").clicked() {
                    editor.check = Some(inspect_script(&editor.source).map(|fields| fields.len()));
                }
                apply = ui
                    .add_enabled(editor.dirty(), egui::Button::new("Apply code"))
                    .clicked();
                close = ui
                    .button(if editor.dirty() {
                        "Apply & clear"
                    } else {
                        "Clear"
                    })
                    .clicked();
                if editor.dirty() {
                    discard = ui.button("Discard draft").clicked();
                }
            });
        });
        if discard {
            self.script_editor = None;
        } else if apply || close {
            *action = Some(Action::ApplyScriptDraft { clear: close });
        }
        if enabled && ui.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, Key::S)) {
            *action = Some(Action::Save);
        }
    }

    pub(crate) fn show_script_inspector(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let schema = self.script_fields();
        let Some(object) = self
            .doc
            .scene
            .entities
            .iter_mut()
            .find(|object| Some(object.uuid) == self.doc.selection.primary())
        else {
            ui.label(RichText::new("Select an object to attach a script.").color(MUTED));
            return;
        };
        let assets = &self.doc.scene.assets;
        egui::ScrollArea::vertical()
            .id_salt("script-inspector")
            .show(ui, |ui| {
                ui.push_id(object.uuid, |ui| {
                    ui.add_enabled_ui(self.play.is_none(), |ui| {
                        ui.label(RichText::new(&object.name).strong());
                        let mut id = object.script;
                        let name = assets
                            .iter()
                            .find(|asset| Some(asset.id) == id)
                            .map_or("None", |asset| asset.name.as_str());
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("object-script")
                                .selected_text(name)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut id, None, "None");
                                    for asset in assets
                                        .iter()
                                        .filter(|asset| asset.kind == AssetKind::Script)
                                    {
                                        ui.selectable_value(&mut id, Some(asset.id), &asset.name);
                                    }
                                });
                            if let Some(id) = id {
                                if ui
                                    .button("Edit code")
                                    .on_hover_text(
                                        "Open in VS Code. Save the file to apply changes.",
                                    )
                                    .clicked()
                                {
                                    *action = Some(Action::EditScript(id));
                                }
                            } else if ui.button("New script").clicked() {
                                *action = Some(Action::NewScript);
                            }
                        });
                        attach(object, id);
                        if let Some((id, schema)) = &schema
                            && Some(*id) == object.script
                        {
                            match schema {
                                Ok(fields) => fields_ui(ui, object, fields),
                                Err(error) => {
                                    ui.label(RichText::new(error).color(WARNING));
                                }
                            }
                        }
                    });
                });
            });
    }
}

pub(crate) fn script_source<'a>(
    scene: &'a futty_runtime::Scene,
    bytes: &'a std::collections::HashMap<Uuid, Vec<u8>>,
    id: Uuid,
) -> Option<&'a str> {
    scene
        .script_sources
        .get(&id)
        .map(String::as_str)
        .or_else(|| futty_assets::script_source(bytes.get(&id)?).ok())
}

pub(crate) fn attach(object: &mut Object, script: Option<Uuid>) {
    if object.script != script {
        object.script = script;
        object.script_fields.clear();
        if script.is_some() {
            object.spin = false;
        }
    }
}

fn fields_ui(ui: &mut egui::Ui, object: &mut Object, defaults: &ScriptFields) {
    for (name, default) in defaults {
        let mut value = object
            .script_fields
            .get(name)
            .filter(|value| value.same_type(default))
            .unwrap_or(default)
            .clone();
        ui.push_id(name, |ui| {
            ui.horizontal(|ui| {
                ui.label(name);
                let changed = match &mut value {
                    ScriptValue::Number(number) => {
                        ui.add(egui::DragValue::new(number).speed(0.05)).changed()
                    }
                    ScriptValue::Bool(flag) => ui.checkbox(flag, "").changed(),
                    ScriptValue::Text(text) => ui
                        .add(
                            egui::TextEdit::singleline(text)
                                .char_limit(256)
                                .desired_width(90.0),
                        )
                        .changed(),
                };
                if changed {
                    if value == *default {
                        object.script_fields.remove(name);
                    } else {
                        object.script_fields.insert(name.clone(), value);
                    }
                }
                if ui.small_button("Reset").clicked() {
                    object.script_fields.remove(name);
                }
            });
        });
    }
}
