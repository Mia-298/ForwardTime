wit_bindgen::generate!({
    path: "wit",
    world: "psys-world-v4",
    generate_all,
});

use exports::astrobox::psys_plugin_v4::{event, lifecycle};

mod model;
mod state;
mod sync;
mod ui;

struct ForwardTimerPlugin;

impl event::Guest for ForwardTimerPlugin {
    async fn on_event(event_type: event::EventType, event_payload: String) -> String {
        match event_type {
            event::EventType::InterconnectMessage => sync::handle_message(&event_payload).await,
            event::EventType::DeviceAction => sync::refresh_connection(false).await,
            _ => {}
        }
        String::new()
    }

    async fn on_ui_event(
        event_id: String,
        event_type: event::UiEvent,
        event_payload: String,
    ) -> String {
        ui::handle_event(event_type, &event_id, &event_payload).await;
        String::new()
    }

    async fn on_ui_render(element_id: String) {
        state::with_state(|state| state.root_element_id = Some(element_id));
        ui::rerender();
        sync::refresh_connection(false).await;
    }

    async fn on_card_render(_card_id: String) {}
}

impl lifecycle::Guest for ForwardTimerPlugin {
    async fn on_load() {
        state::load();
        sync::refresh_connection(true).await;
    }
}

export!(ForwardTimerPlugin);
