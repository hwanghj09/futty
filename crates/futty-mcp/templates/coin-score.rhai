fn on_start(entity) {
    this.coins = 0;
    set_hud("coins", this.coins);
    set_hud("objective", "Collect all three coins");
}

fn on_update(entity, dt) {
    if this.coins == 3 {
        return;
    }
    for coin in find_nearby(entity, "coin", 0.95) {
        send_event(coin, "pickup");
    }
}

fn on_message(entity, sender, name) {
    if name == "coin" {
        this.coins += 1;
        set_hud("coins", this.coins);
        if this.coins == 3 {
            set_hud("objective", "All coins collected");
            finish_game("success");
        }
    }
}
