mod state;
mod transport;
mod workspace;

use dioxus::prelude::*;
use state::MemoryState;

fn main() {
    console_error_panic_hook::set_once();
    LaunchBuilder::web()
        .with_cfg(
            dioxus::web::Config::new()
                .history(std::rc::Rc::new(dioxus::history::MemoryHistory::default())),
        )
        .launch(App);
}

#[component]
fn App() -> Element {
    let state = use_signal(MemoryState::default);
    use_context_provider(|| state);
    use_future(move || async move {
        state::load(state).await;
    });
    rsx! {
        az_ui_components::UiStylesheets { relative_paths: true }
        workspace::MemoryWorkspace {}
    }
}
