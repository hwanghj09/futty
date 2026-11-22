fn on_start(entity) {
    this.powered = false;
    this.finished = false;
    set_hud("power", 0);
    set_hud("objective", "Find the power cell");
}

fn on_update(entity, dt) {
    if this.finished {
        return;
    }
    if !this.powered {
        for cell in find_nearby(entity, "power_cell", 0.75) {
            set_active(cell, false);
            this.powered = true;
            set_hud("power", 1);
            set_hud("objective", "Power on. Go to exit.");
            break;
        }
    }
    if !find_nearby(entity, "maze_goal", 0.8).is_empty() {
        if this.powered {
            this.finished = true;
            set_hud("objective", "Maze escaped");
            finish_game("success");
        } else {
            set_hud("objective", "Exit locked. Find cell.");
        }
    }
}
