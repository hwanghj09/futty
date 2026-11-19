fn fields() {
    #{ speed: 2.4, movement_enabled: true }
}

fn on_start(entity) {
    this.elapsed = 0.0;
    log("Ready. Move actions steer the cube. Jump lifts it.");
}

fn on_update(entity, dt) {
    this.elapsed += dt;
    if !param("movement_enabled") { return; }
    let step = param("speed") * dt;
    if input_down("MoveForward") { translate_local(entity, 0, 0, -step); }
    if input_down("MoveBackward") { translate_local(entity, 0, 0, step); }
    if input_down("MoveLeft") { translate_local(entity, -step, 0, 0); }
    if input_down("MoveRight") { translate_local(entity, step, 0, 0); }
    if input_down("Jump") { translate(entity, 0, step, 0); }
}
