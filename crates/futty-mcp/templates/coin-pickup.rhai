fn on_start(entity) {
    this.collected = false;
}

fn on_message(entity, sender, name) {
    if this.collected || name != "pickup" {
        return;
    }
    this.collected = true;
    set_active(entity, false);
    send_event(sender, "coin");
}
