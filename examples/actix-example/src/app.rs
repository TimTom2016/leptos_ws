use leptos::*;
use leptos_sse::create_sse_signal;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Count {
    pub value: i32,
}

#[component]
pub fn App() -> impl IntoView {
    // Provide sse connection
    leptos_sse::provide_sse("http://localhost:3000/sse").unwrap();

    // Create server signal
    let count = create_sse_signal::<Count>("counter");

    view! {
        <h1>"Count: " {move || count.get().value.to_string()}</h1>
    }
}
