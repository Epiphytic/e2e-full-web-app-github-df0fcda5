mod auth;
mod db;
mod handlers;
mod models;

use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let public_key = std::fs::read("certs/public.pem")
        .expect("Missing certs/public.pem — run certs/generate-keys.sh");

    let conn =
        rusqlite::Connection::open("data.db").expect("Failed to open SQLite database");
    conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();

    let state = Arc::new(handlers::AppStateInner {
        db: Mutex::new(conn),
        public_key_pem: public_key,
    });

    // Public routes
    let public_routes = Router::new()
        .route("/login", get(handlers::login_page))
        .route("/login", post(handlers::login_submit))
        .route("/health", get(handlers::health))
        .route("/.well-known/jwks.json", get(auth::jwks_endpoint));

    // Protected routes
    let protected_routes = Router::new()
        .route("/", get(handlers::dashboard))
        .route("/tables/{table_name}", get(handlers::table_detail_handler))
        .route("/api/tables", get(handlers::list_tables_handler))
        .route("/api/tables", post(handlers::create_table_handler))
        .route(
            "/api/tables/{table_name}",
            delete(handlers::delete_table_handler),
        )
        .route(
            "/api/tables/{table_name}/columns",
            get(handlers::list_columns_handler),
        )
        .route(
            "/api/tables/{table_name}/columns",
            post(handlers::add_column_handler),
        )
        .route(
            "/api/tables/{table_name}/columns/{column_name}",
            delete(handlers::delete_column_handler),
        )
        .route("/logout", get(handlers::logout))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
