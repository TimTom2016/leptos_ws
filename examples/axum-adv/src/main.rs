#[cfg(feature = "ssr")]
pub mod fileserv;
use std::sync::Arc;

#[cfg(feature = "ssr")]
use axum::{
    Router,
    routing::{get, post},
};
#[cfg(feature = "ssr")]
use axum::{
    extract::{FromRef, Request, State},
    response::IntoResponse,
};
#[cfg(feature = "ssr")]
pub use axum_adv_example::app;
use axum_adv_example::app::{App, Blocked};
#[cfg(feature = "ssr")]
use config::get_configuration;
#[cfg(feature = "ssr")]
use http::HeaderMap;
#[cfg(feature = "ssr")]
use leptos::*;
#[cfg(feature = "ssr")]
use leptos::{
    config::LeptosOptions,
    prelude::{provide_context, *},
};
#[cfg(feature = "ssr")]
use leptos_axum::{AxumRouteListing, handle_server_fns_with_context};
#[cfg(feature = "ssr")]
use leptos_axum::{LeptosRoutes, generate_route_list_with_exclusions_and_ssg_and_context};
use leptos_ws::WsSignals;

#[cfg(feature = "ssr")]
#[derive(Clone, FromRef)]
pub struct AppState {
    server_signals: WsSignals,
    routes: Option<Vec<AxumRouteListing>>,
    options: LeptosOptions,
    blocked: app::Blocked,
}

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    pub fn shell(options: LeptosOptions) -> impl IntoView {
        view! {
            <!DOCTYPE html>
            <html lang="en">
                <head>
                    <meta charset="utf-8"/>
                    <meta name="viewport" content="width=device-width, initial-scale=1"/>
                    <AutoReload options=options.clone()/>
                    <HydrationScripts options=options islands=true/>
                </head>
                <body>
                    <App/>
                </body>
            </html>
        }
    }

    async fn leptos_routes_handler(
        state: State<AppState>,
        req: Request,
    ) -> axum::response::Response {
        let state1 = state.0.clone();
        let options2 = state.clone().0.options.clone();
        let handler = leptos_axum::render_route_with_context(
            state.routes.clone().unwrap(),
            move || {
                provide_context(state1.options.clone());
                provide_context(state1.server_signals.clone());
                provide_context(state1.blocked.clone());
            },
            move || shell(options2.clone()),
        );
        handler(state, req).await.into_response()
    }
    async fn server_fn_handler(
        State(state): State<AppState>,
        _path: axum::extract::Path<String>,
        _headers: HeaderMap,
        _query: axum::extract::RawQuery,
        request: Request,
    ) -> impl IntoResponse {
        handle_server_fns_with_context(
            move || {
                provide_context(state.options.clone());
                provide_context(state.server_signals.clone());
                provide_context(state.blocked.clone());
            },
            request,
        )
        .await
    }

    let blocked_users = Blocked::new();
    simple_logger::init_with_level(log::Level::Debug).expect("couldn't initialize logging");
    let server_signals = WsSignals::new();
    let conf = get_configuration(None).unwrap();
    let leptos_options = conf.leptos_options;
    let mut state = AppState {
        options: leptos_options.clone(),
        routes: None,
        server_signals: server_signals.clone(),
        blocked: blocked_users.clone(),
    };
    let addr = leptos_options.site_addr;
    let state2 = state.clone();

    let (routes, _) = generate_route_list_with_exclusions_and_ssg_and_context(
        || view! { <App/> },
        None,
        move || {
            provide_context(state2.server_signals.clone());
            provide_context(state2.blocked.clone())
        },
    );
    state.routes = Some(routes.clone());
    let state2 = state.clone();

    let app = Router::new()
        .route("/api/{*fn_name}", post(server_fn_handler))
        .route("/api/{*fn_name}", get(server_fn_handler))
        .leptos_routes_with_handler(routes, get(leptos_routes_handler))
        .fallback(leptos_axum::file_and_error_handler_with_context::<
            AppState,
            _,
        >(
            move || {
                provide_context(state2.server_signals.clone());
                provide_context(state2.blocked.clone())
            },
            shell,
        ))
        .with_state(state);

    leptos::logging::log!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
