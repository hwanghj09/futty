fn fields() {
    #{ fall_loss: false, sprite_walk: false }
}

fn on_start(entity) {
    this.coins = 0;
    this.finished = false;
    set_hud("coins", this.coins);
    if param("fall_loss") {
        set_hud("objective", "Jump the gap. Collect 3 coins.");
    } else {
        set_hud("objective", "Collect all three coins");
    }
}

fn on_update(entity, dt) {
    if this.finished {
        return;
    }
    if param("fall_loss") && get_position(entity).y < -2.5 {
        this.finished = true;
        set_hud("objective", "You fell. Restart to try again.");
        finish_game("failure");
        return;
    }
    if param("sprite_walk") {
        let moving = input_down("MoveLeft") || input_down("MoveRight");
        play_sprite(entity, 0, if moving { 4 } else { 1 }, 8, moving);
    }
    for coin in find_nearby(entity, "coin", 0.95) {
        send_event(coin, "pickup");
    }
}

fn on_message(entity, sender, name) {
    if name == "coin" && !this.finished {
        this.coins += 1;
        set_hud("coins", this.coins);
        if this.coins == 3 {
            this.finished = true;
            set_hud("objective", "All coins collected");
            finish_game("success");
        }
    }
}
