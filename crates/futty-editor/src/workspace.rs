use eframe::egui;
use egui_stack::{
    DockArea, DockState, NodeIndex, Style, SurfaceIndex, TabDestination, TabInsert, TabPath,
    TabViewer, widgets::tab_viewer::OnCloseResponse,
};
use serde::{Deserialize, Serialize};

use futty_runtime::{PhysicsEvent, Runtime};

use crate::{Action, BACKGROUND, Editor, FOCUS, PANEL, physics, script_console};

pub(crate) const STORAGE_KEY: &str = "futty.workspace.v2";
const LEGACY_KEY: &str = "futty.workspace.v1";

#[derive(Default)]
pub(crate) struct RunDiagnostics {
    logs: Vec<String>,
    events: Vec<PhysicsEvent>,
    bodies: usize,
}

impl RunDiagnostics {
    pub fn capture(runtime: &Runtime) -> Self {
        Self {
            logs: runtime.script_log().map(str::to_owned).collect(),
            events: runtime.physics_events().copied().collect(),
            bodies: runtime.physics_body_count(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum Panel {
    Scene,
    Game,
    Hierarchy,
    Inspector,
    Project,
    Console,
    Physics,
    ScriptEditor,
}

impl Panel {
    pub const ALL: [Self; 8] = [
        Self::Scene,
        Self::Game,
        Self::Hierarchy,
        Self::Inspector,
        Self::Project,
        Self::Console,
        Self::Physics,
        Self::ScriptEditor,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Scene => "Scene",
            Self::Game => "Game",
            Self::Hierarchy => "Hierarchy",
            Self::Inspector => "Inspector",
            Self::Project => "Project",
            Self::Console => "Console",
            Self::Physics => "Physics",
            Self::ScriptEditor => "Script Editor",
        }
    }

    fn neighbor(self) -> Self {
        match self {
            Self::Console | Self::Physics => Self::Project,
            Self::Project => Self::Console,
            Self::Scene => Self::Game,
            _ => Self::Scene,
        }
    }
}

pub(crate) fn default_layout() -> DockState<Panel> {
    let mut dock = DockState::new(vec![Panel::Scene, Panel::Game]);
    let tree = dock.main_surface_mut();
    let [workspace, _] = tree.split_right(NodeIndex::root(), 0.76, vec![Panel::Inspector]);
    let [top, _] = tree.split_below(workspace, 0.73, vec![Panel::Project]);
    tree.split_left(top, 0.21, vec![Panel::Hierarchy]);
    dock
}

pub(crate) fn load_layout(storage: Option<&dyn eframe::Storage>) -> DockState<Panel> {
    if let Some(storage) = storage {
        if let Some(dock) = eframe::get_value(storage, STORAGE_KEY) {
            return dock;
        }
        if let Some(mut dock) = eframe::get_value(storage, LEGACY_KEY) {
            remove_legacy_default_tabs(&mut dock);
            return dock;
        }
    }
    default_layout()
}

fn remove_legacy_default_tabs(dock: &mut DockState<Panel>) {
    let layout = default_layout();
    let Some(project) = layout.find_tab(&Panel::Project) else {
        return;
    };
    let counts_match = dock.iter_all_tabs().count() == 7;
    let core_matches = layout
        .iter_all_tabs()
        .all(|(path, panel)| dock.find_tab(panel) == Some(path));
    let diagnostics_match = [Panel::Console, Panel::Physics].into_iter().all(|panel| {
        dock.find_tab(&panel)
            .is_some_and(|path| path.node_path() == project.node_path())
    });
    if counts_match && core_matches && diagnostics_match {
        for panel in [Panel::Console, Panel::Physics] {
            if let Some(path) = dock.find_tab(&panel) {
                dock.remove_tab(path);
            }
        }
    }
}

pub(crate) fn panels_menu(ui: &mut egui::Ui, action: &mut Option<Action>) {
    for panel in Panel::ALL {
        if panel == Panel::Console {
            ui.separator();
        }
        crate::menu_item(ui, panel.title(), Action::ShowPanel(panel), true, action);
    }
}

pub(crate) fn panels_submenu(ui: &mut egui::Ui, action: &mut Option<Action>) {
    ui.menu_button("Panels", |ui| panels_menu(ui, action));
}

pub(crate) fn dock_panel(dock: &mut DockState<Panel>, panel: Panel) {
    let Some(source) = dock.find_tab(&panel).filter(|path| !path.surface.is_main()) else {
        return;
    };
    let node = dock
        .find_tab(&panel.neighbor())
        .filter(|path| path.surface.is_main())
        .or_else(|| {
            dock.iter_all_tabs()
                .map(|(path, _)| path)
                .find(|path| path.surface.is_main())
        });
    let target = node.map_or(TabDestination::EmptySurface(SurfaceIndex::main()), |path| {
        TabDestination::Node(path.node_path(), TabInsert::Append)
    });
    dock.move_tab(source, target);
    if let Some(path) = dock.find_tab(&panel) {
        let _ = dock.focus_tab(path);
    }
}

impl Editor {
    pub(crate) fn show_panel(&mut self, panel: Panel) {
        if self.dock.find_tab(&panel).is_none() {
            if let Some(path) = self.dock.find_tab(&panel.neighbor()) {
                self.dock[path.node_path()].append_tab(panel);
            } else {
                self.dock.push_to_first_leaf(panel);
            }
        }
        if let Some(path) = self.dock.find_tab(&panel) {
            let _ = self.dock.focus_tab(path);
        }
    }

    pub(crate) fn dock_workspace(
        &mut self,
        ui: &mut egui::Ui,
        action: &mut Option<Action>,
        modal: bool,
    ) {
        let mut dock = std::mem::replace(&mut self.dock, DockState::new(Vec::new()));
        let mut style = Style::from_egui(ui.style());
        style.separator.width = 3.0;
        style.separator.extra = 90.0;
        style.separator.color_idle = BACKGROUND;
        style.separator.color_dragged = FOCUS;
        style.tab_bar.bg_fill = BACKGROUND;
        style.tab_bar.height = 25.0;
        style.tab.tab_body.bg_fill = PANEL;
        // shortcut: floating panels live inside the app, use native viewports for cross-monitor docking.
        DockArea::new(&mut dock)
            .style(style)
            .draggable_tabs(!modal)
            .window_bounds(ui.available_rect_before_wrap())
            .show_leaf_collapse_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show_inside(
                ui,
                &mut Workspace {
                    editor: self,
                    action,
                    modal,
                },
            );
        self.dock = dock;
    }
}

struct Workspace<'a> {
    editor: &'a mut Editor,
    action: &'a mut Option<Action>,
    modal: bool,
}

impl TabViewer for Workspace<'_> {
    type Tab = Panel;

    fn id(&mut self, panel: &mut Panel) -> egui::Id {
        egui::Id::unique(("workspace", *panel))
    }

    fn title(&mut self, panel: &mut Panel) -> egui::WidgetText {
        panel.title().into()
    }

    fn scroll_bars(&self, _: &Panel) -> [bool; 2] {
        [false, false]
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, panel: &mut Panel, path: TabPath) {
        if !self.modal {
            panels_submenu(ui, self.action);
            ui.separator();
        }
        if !path.surface.is_main() {
            crate::menu_item(
                ui,
                "Dock in Main Window",
                Action::DockPanel(*panel),
                !self.modal,
                self.action,
            );
        }
    }

    fn on_close(&mut self, panel: &mut Panel) -> OnCloseResponse {
        if *panel == Panel::ScriptEditor {
            if self
                .editor
                .script_editor
                .as_ref()
                .is_some_and(crate::script_editor::ScriptEditor::dirty)
            {
                self.editor.report(
                    "Apply or discard the script draft before closing this tab.",
                    true,
                );
                return OnCloseResponse::Focus;
            }
            self.editor.script_editor = None;
        }
        OnCloseResponse::Close
    }

    fn ui(&mut self, ui: &mut egui::Ui, panel: &mut Panel) {
        match panel {
            Panel::Scene | Panel::Game => {
                self.editor
                    .scene_view(ui, self.modal, *panel == Panel::Game, self.action)
            }
            Panel::ScriptEditor => self.editor.show_script_panel(
                ui,
                !self.modal && self.editor.play.is_none(),
                self.action,
            ),
            Panel::Hierarchy | Panel::Inspector | Panel::Project => {
                ui.add_enabled_ui(!self.modal, |ui| match panel {
                    Panel::Hierarchy => self.editor.hierarchy(ui, self.action),
                    Panel::Inspector => self.editor.inspector(ui, self.action),
                    _ => self.editor.assets_panel(ui, self.action),
                });
            }
            Panel::Console => {
                if let Some(play) = &self.editor.play {
                    script_console(ui, play.runtime().script_log());
                } else {
                    script_console(ui, self.editor.diagnostics.logs.iter().map(String::as_str));
                }
            }
            Panel::Physics => {
                let objects = &self.editor.doc.scene.entities;
                if let Some(play) = &self.editor.play {
                    let runtime = play.runtime();
                    let objects = match play {
                        crate::Play::Loaded(world) => &world.scene.entities,
                        _ => objects,
                    };
                    physics::events(
                        ui,
                        runtime.physics_body_count(),
                        runtime.physics_events(),
                        objects,
                    );
                } else {
                    let last = &self.editor.diagnostics;
                    physics::events(ui, last.bodies, last.events.iter(), objects);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn legacy_layout() -> DockState<Panel> {
        let mut dock = default_layout();
        let project = dock.find_tab(&Panel::Project).unwrap().node_path();
        dock[project].append_tab(Panel::Console);
        dock[project].append_tab(Panel::Physics);
        dock
    }

    #[test]
    fn tab_drag_moves_it_into_another_group() {
        #[derive(Default)]
        struct Tabs(HashMap<Panel, egui::Pos2>);

        impl TabViewer for Tabs {
            type Tab = Panel;
            fn id(&mut self, tab: &mut Panel) -> egui::Id {
                egui::Id::unique(*tab)
            }
            fn title(&mut self, tab: &mut Panel) -> egui::WidgetText {
                tab.title().into()
            }
            fn ui(&mut self, _: &mut egui::Ui, _: &mut Panel) {}
            fn on_tab_button(&mut self, tab: &mut Panel, response: &egui::Response) {
                self.0.insert(*tab, response.rect.center());
            }
        }

        let ctx = egui::Context::default();
        let mut dock = legacy_layout();
        let mut tabs = Tabs::default();
        let frame = |dock: &mut DockState<Panel>, tabs: &mut Tabs, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| DockArea::new(dock).show_inside(ui, tabs),
            );
            output.textures_delta.clear();
        };
        frame(&mut dock, &mut tabs, vec![]);
        let from = tabs.0[&Panel::Console];
        let to = tabs.0[&Panel::Scene];
        let click = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut dock, &mut tabs, vec![egui::Event::PointerMoved(from)]);
        frame(&mut dock, &mut tabs, vec![click(from, true)]);
        frame(
            &mut dock,
            &mut tabs,
            vec![egui::Event::PointerMoved(from.lerp(to, 0.5))],
        );
        frame(&mut dock, &mut tabs, vec![egui::Event::PointerMoved(to)]);
        frame(&mut dock, &mut tabs, vec![click(to, false)]);
        let path = |panel| dock.find_tab(&panel).unwrap().node_path();
        assert_eq!(path(Panel::Console), path(Panel::Scene));
    }

    #[test]
    fn default_workspace_groups_unity_style_tabs_without_duplicates() {
        let dock = default_layout();
        let path = |panel| dock.find_tab(&panel).unwrap().node_path();
        assert_eq!(path(Panel::Scene), path(Panel::Game));
        assert!(dock.find_tab(&Panel::Console).is_none());
        assert!(dock.find_tab(&Panel::Physics).is_none());
        assert!(dock.find_tab(&Panel::ScriptEditor).is_none());
        assert_ne!(path(Panel::Hierarchy), path(Panel::Inspector));
        assert_eq!(dock.iter_all_tabs().count(), 5);
    }

    #[test]
    fn upgrades_default_tabs_without_changing_a_custom_dock() {
        let mut default = legacy_layout();
        remove_legacy_default_tabs(&mut default);
        assert_eq!(default.iter_all_tabs().count(), 5);
        assert!(default.find_tab(&Panel::Console).is_none());

        let mut custom = legacy_layout();
        let console = custom.find_tab(&Panel::Console).unwrap();
        let scene = custom.find_tab(&Panel::Scene).unwrap().node_path();
        custom.move_tab(console, TabDestination::Node(scene, TabInsert::Append));
        remove_legacy_default_tabs(&mut custom);
        assert_eq!(custom.iter_all_tabs().count(), 7);
        assert_eq!(custom.find_tab(&Panel::Console).unwrap().node_path(), scene);
    }

    #[test]
    fn layout_storage_preserves_a_floating_tab_and_active_groups() {
        #[derive(Default)]
        struct Memory(Option<String>);
        impl eframe::Storage for Memory {
            fn get_string(&self, _: &str) -> Option<String> {
                self.0.clone()
            }
            fn set_string(&mut self, _: &str, value: String) {
                self.0 = Some(value);
            }
            fn remove_string(&mut self, _: &str) {
                self.0 = None;
            }
            fn flush(&mut self) {}
        }

        let mut dock = legacy_layout();
        let console = dock.find_tab(&Panel::Console).unwrap();
        dock.detach_tab(
            console,
            egui::Rect::from_min_size(egui::pos2(200.0, 100.0), egui::vec2(400.0, 300.0)),
        );
        let game = dock.find_tab(&Panel::Game).unwrap();
        dock.focus_tab(game).unwrap();
        let mut storage = Memory::default();
        eframe::set_value(&mut storage, STORAGE_KEY, &dock);
        let mut restored: DockState<Panel> = eframe::get_value(&storage, STORAGE_KEY).unwrap();
        assert!(
            !restored
                .find_tab(&Panel::Console)
                .unwrap()
                .surface
                .is_main()
        );
        assert!(
            restored
                .find_tab(&Panel::Project)
                .unwrap()
                .surface
                .is_main()
        );
        assert_eq!(restored.find_tab(&Panel::Game), Some(game));
        dock_panel(&mut restored, Panel::Console);
        assert_eq!(
            restored.find_tab(&Panel::Console).unwrap().node_path(),
            restored.find_tab(&Panel::Project).unwrap().node_path(),
        );
    }
}
