use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

use crate::auth::Claims;
use crate::models::*;
use crate::db;

pub type AppState = Arc<AppStateInner>;

pub struct AppStateInner {
    pub db: Mutex<Connection>,
    pub public_key_pem: Vec<u8>,
}

// --- Template structs ---

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    user: String,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate {
    message: String,
}

#[derive(Template)]
#[template(path = "table_list.html")]
struct TableListTemplate {
    tables: Vec<String>,
}

#[derive(Template)]
#[template(path = "table_detail.html")]
struct TableDetailTemplate {
    table_name: String,
    user: String,
    columns: Vec<ColumnDef>,
    rows: Vec<Vec<String>>,
}

#[derive(Template)]
#[template(path = "column_list.html")]
struct ColumnListTemplate {
    table_name: String,
    columns: Vec<ColumnDef>,
}

// --- Auth and core page handlers ---

pub async fn login_page() -> impl IntoResponse {
    Html(LoginTemplate { error: None }.render().unwrap())
}

pub async fn login_submit(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Response {
    match crate::auth::validate_token(&form.token, &state.public_key_pem) {
        Ok(_claims) => {
            let cookie = format!("token={}; HttpOnly; Path=/; SameSite=Strict", form.token);
            (
                StatusCode::SEE_OTHER,
                [("Location", "/"), ("Set-Cookie", &*cookie)],
            )
                .into_response()
        }
        Err(e) => Html(
            LoginTemplate {
                error: Some(format!("Invalid token: {}", e)),
            }
            .render()
            .unwrap(),
        )
        .into_response(),
    }
}

pub async fn dashboard(claims: Claims) -> impl IntoResponse {
    Html(DashboardTemplate { user: claims.sub }.render().unwrap())
}

pub async fn logout() -> impl IntoResponse {
    (
        StatusCode::SEE_OTHER,
        [
            ("Location", "/login"),
            ("Set-Cookie", "token=; HttpOnly; Path=/; Max-Age=0"),
        ],
    )
}

pub async fn health() -> impl IntoResponse {
    "OK"
}

// --- DB Editor API handlers (return HTML fragments for htmx) ---

pub async fn list_tables_handler(State(state): State<AppState>) -> impl IntoResponse {
    let state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().unwrap();
        db::list_tables(&conn)
    })
    .await
    .unwrap();
    match result {
        Ok(tables) => Html(TableListTemplate { tables }.render().unwrap()).into_response(),
        Err(e) => Html(
            ErrorTemplate {
                message: e.to_string(),
            }
            .render()
            .unwrap(),
        )
        .into_response(),
    }
}

pub async fn create_table_handler(
    State(state): State<AppState>,
    Form(form): Form<CreateTableForm>,
) -> impl IntoResponse {
    let state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().unwrap();
        let columns = vec![ColumnDef {
            name: form.col_name_1,
            col_type: form.col_type_1,
        }];
        db::create_table(&conn, &form.table_name, &columns)?;
        db::list_tables(&conn)
    })
    .await
    .unwrap();
    match result {
        Ok(tables) => Html(TableListTemplate { tables }.render().unwrap()).into_response(),
        Err(e) => Html(
            ErrorTemplate {
                message: e.to_string(),
            }
            .render()
            .unwrap(),
        )
        .into_response(),
    }
}

pub async fn delete_table_handler(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
) -> impl IntoResponse {
    let state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().unwrap();
        db::drop_table(&conn, &table_name)?;
        db::list_tables(&conn)
    })
    .await
    .unwrap();
    match result {
        Ok(tables) => Html(TableListTemplate { tables }.render().unwrap()).into_response(),
        Err(e) => Html(
            ErrorTemplate {
                message: e.to_string(),
            }
            .render()
            .unwrap(),
        )
        .into_response(),
    }
}

pub async fn table_detail_handler(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    claims: Claims,
) -> impl IntoResponse {
    let user = claims.sub.clone();
    let state = state.clone();
    let (table_name, columns, rows) = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().unwrap();
        let columns = db::list_columns(&conn, &table_name).unwrap_or_default();
        let rows = db::get_table_rows(&conn, &table_name, 100, 0).unwrap_or_default();
        (table_name, columns, rows)
    })
    .await
    .unwrap();
    Html(
        TableDetailTemplate {
            table_name,
            user,
            columns,
            rows,
        }
        .render()
        .unwrap(),
    )
}

pub async fn list_columns_handler(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
) -> impl IntoResponse {
    let state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().unwrap();
        db::list_columns(&conn, &table_name).map(|columns| (table_name, columns))
    })
    .await
    .unwrap();
    match result {
        Ok((table_name, columns)) => {
            Html(ColumnListTemplate { table_name, columns }.render().unwrap()).into_response()
        }
        Err(e) => Html(
            ErrorTemplate {
                message: e.to_string(),
            }
            .render()
            .unwrap(),
        )
        .into_response(),
    }
}

pub async fn add_column_handler(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    Form(form): Form<AddColumnRequest>,
) -> impl IntoResponse {
    let state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().unwrap();
        let col = ColumnDef {
            name: form.column_name,
            col_type: form.column_type,
        };
        db::add_column(&conn, &table_name, &col)?;
        db::list_columns(&conn, &table_name).map(|columns| (table_name, columns))
    })
    .await
    .unwrap();
    match result {
        Ok((table_name, columns)) => {
            Html(ColumnListTemplate { table_name, columns }.render().unwrap()).into_response()
        }
        Err(e) => Html(
            ErrorTemplate {
                message: e.to_string(),
            }
            .render()
            .unwrap(),
        )
        .into_response(),
    }
}

pub async fn delete_column_handler(
    State(state): State<AppState>,
    Path((table_name, column_name)): Path<(String, String)>,
) -> impl IntoResponse {
    let state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = state.db.lock().unwrap();
        db::drop_column(&conn, &table_name, &column_name)?;
        db::list_columns(&conn, &table_name).map(|columns| (table_name, columns))
    })
    .await
    .unwrap();
    match result {
        Ok((table_name, columns)) => {
            Html(ColumnListTemplate { table_name, columns }.render().unwrap()).into_response()
        }
        Err(e) => Html(
            ErrorTemplate {
                message: e.to_string(),
            }
            .render()
            .unwrap(),
        )
        .into_response(),
    }
}
