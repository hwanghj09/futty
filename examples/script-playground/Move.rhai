fn fields() {
    #{ speed: 2.4, movement_enabled: true }
}

fn on_start(entity) {
    this.elapsed = 0.0;
    log("Ready. W A S D move the cube. Space lifts it.");
}

fn on_update(entity, dt) {
    this.elapsed += dt;
    if !param("movement_enabled") { return; }
    let step = param("speed") * dt;
    if key_down("W") || input_down("MoveForward") { translate_local(entity, 0, 0, -step); }
    if key_down("S") || input_down("MoveBackward") { translate_local(entity, 0, 0, step); }
    if key_down("A") || input_down("MoveLeft") { translate_local(entity, -step, 0, 0); }
    if key_down("D") || input_down("MoveRight") { translate_local(entity, step, 0, 0); }
    if key_down("Space") { translate(entity, 0, step, 0); }
}
