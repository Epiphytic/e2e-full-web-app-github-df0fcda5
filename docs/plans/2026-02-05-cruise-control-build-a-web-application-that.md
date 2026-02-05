# SQLite Database Editor Web Application - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Rust web application with htmx frontend that allows authenticated users to view and edit an SQLite database (add/remove tables, modify columns).

**Architecture:** Axum-based REST/HTML server serving htmx-powered templates via Askama. SQLite database managed through rusqlite (synchronous, simpler for direct DDL operations). JWT authentication with a local CA — private key signs tokens, public key served at `.well-known/jwks.json`. Playwright E2E tests validate all user flows. GitHub Actions for CI/CD with Super-Linter and dependency review.

**Tech Stack:**
- Backend: Rust + Axum + Tower
- Database: rusqlite (synchronous — better for DDL operations like CREATE/ALTER/DROP TABLE)
- Templates: Askama (compile-time, Jinja2-like syntax)
- Frontend: htmx (vendored), minimal CSS
- Auth: jsonwebtoken crate + local RSA keypair
- Testing: Playwright (TypeScript)
- CI: GitHub Actions + Super-Linter + dependency-review-action

---

## Overview

The application is a browser-based SQLite database editor. Users authenticate via JWT tokens signed by a local CA. Once authenticated, they can:

1. View all tables in the database
2. Create new tables with specified columns
3. Drop existing tables
4. Add columns to existing tables
5. Remove columns from existing tables (via table rebuild since SQLite doesn't support DROP COLUMN before 3.35.0 — we'll use ALTER TABLE DROP COLUMN where available, with fallback)
6. Browse table data

The frontend uses htmx for partial page updates — form submissions and table operations return HTML fragments that htmx swaps into the DOM without full page reloads.

### Directory Structure

```
.
├── .github/
│   └── workflows/
│       ├── ci.yml              # Super-Linter + dependency-review
│       └── e2e.yml             # Playwright E2E tests
├── .gitignore
├── Cargo.toml
├── Cargo.lock
├── certs/
│   ├── generate-keys.sh        # Script to generate RSA keypair
│   └── .gitkeep
├── src/
│   ├── main.rs                 # Axum server setup, router
│   ├── auth.rs                 # JWT validation middleware, JWKS endpoint
│   ├── db.rs                   # SQLite connection pool, schema operations
│   ├── handlers.rs             # Route handlers (tables, columns, data)
│   └── models.rs               # Request/response types
├── templates/
│   ├── base.html               # Base layout with htmx script
│   ├── login.html              # Login page
│   ├── dashboard.html          # Main dashboard showing tables
│   ├── table_list.html         # Partial: table list for htmx swap
│   ├── table_detail.html       # Table detail with columns and data
│   ├── column_list.html        # Partial: column list for htmx swap
│   └── error.html              # Error display partial
├── static/
│   └── htmx.min.js            # Vendored htmx 2.0
├── tests/
│   ├── package.json            # Playwright dependencies
│   ├── playwright.config.ts    # Playwright configuration
│   └── e2e/
│       ├── auth.spec.ts        # JWT login tests
│       ├── tables.spec.ts      # Table CRUD tests
│       └── columns.spec.ts     # Column modification tests
└── docs/
    └── plans/
        └── (this file)
```

## Risk Areas

1. **SQLite DDL limitations**: SQLite has limited ALTER TABLE support. DROP COLUMN was added in 3.35.0 (2021). We need to verify the rusqlite-bundled SQLite version supports it, or implement a table-rebuild fallback.
2. **Concurrent SQLite access**: SQLite has a single-writer model. Using rusqlite synchronously with Axum async means we need a connection pool or `tokio::task::spawn_blocking` to avoid blocking the async runtime.
3. **JWT key management in CI**: The local CA keypair must be generated during CI for E2E tests. Keys must never be committed to the repo.
4. **htmx partial rendering**: Each htmx-triggered route must return an HTML fragment (not a full page). Getting the template boundaries right requires careful design.
5. **Playwright with Rust server**: E2E tests need to start the Rust server, wait for it to be ready, then run tests. This requires a test fixture or setup script.
6. **Super-Linter configuration**: Super-Linter runs many linters by default. We need to configure it to only run relevant linters (Rust/clippy, TypeScript/ESLint, HTML, YAML, Markdown) to avoid noise.

---

## Tasks

### Task CRUISE-001: Project Scaffolding and .gitignore

**Files:**
- Create: `.gitignore`
- Create: `Cargo.toml`
- Create: `src/main.rs` (minimal hello world)

**Step 1: Create comprehensive .gitignore**

```gitignore
# Rust build artifacts
/target/
**/*.rs.bk
*.pdb

# Dependencies
Cargo.lock is committed for binaries, but listed here as reminder
# Cargo.lock  # DO commit for binary projects

# Keys and credentials
certs/*.pem
certs/*.key
certs/*.pub
certs/*.crt
*.p12
*.pfx
*.jks

# Environment files
.env
.env.*
!.env.example

# SQLite databases
*.db
*.sqlite
*.sqlite3
*.db-journal
*.db-wal
*.db-shm

# Node.js (Playwright tests)
node_modules/
tests/node_modules/
tests/test-results/
tests/playwright-report/
tests/blob-report/

# Editor/IDE files
.idea/
.vscode/
*.swp
*.swo
*~
.project
.classpath
.settings/
*.sublime-project
*.sublime-workspace
.vim/
*.code-workspace

# OS files
.DS_Store
.DS_Store?
._*
.Spotlight-V100
.Trashes
Thumbs.db
ehthumbs.db
Desktop.ini
$RECYCLE.BIN/
*.lnk

# Log files
*.log
logs/
npm-debug.log*
yarn-debug.log*
yarn-error.log*

# Temporary files
*.tmp
*.temp
*.bak
*.orig
/tmp/

# Build artifacts
dist/
build/
out/

# Fork-join directories
.fork-join/

# Test output
test-results/
playwright-report/
```

**Step 2: Create Cargo.toml with dependencies**

```toml
[package]
name = "sqlite-editor"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = { version = "0.8", features = ["macros"] }
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["fs", "cors"] }
rusqlite = { version = "0.32", features = ["bundled"] }
askama = "0.12"
askama_axum = "0.4"
jsonwebtoken = "9"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
base64 = "0.22"
ring = "0.17"
tracing = "0.1"
tracing-subscriber = "0.3"
```

**Step 3: Create minimal main.rs**

```rust
use axum::{routing::get, Router};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    tracing_subscriber::init();

    let app = Router::new().route("/", get(|| async { "SQLite Editor" }));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: Compiles with no errors.

**Step 5: Commit**

```bash
git add .gitignore Cargo.toml Cargo.lock src/main.rs
git commit -m "chore: scaffold project with Cargo.toml and .gitignore"
```

---

### Task CRUISE-002: JWT Authentication Infrastructure

**Files:**
- Create: `certs/generate-keys.sh`
- Create: `certs/.gitkeep`
- Create: `src/auth.rs`
- Modify: `src/main.rs`

**Step 1: Write the key generation script**

```bash
#!/usr/bin/env bash
set -euo pipefail

CERT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Generate RSA private key (2048-bit)
openssl genrsa -out "$CERT_DIR/private.pem" 2048

# Extract public key
openssl rsa -in "$CERT_DIR/private.pem" -pubout -out "$CERT_DIR/public.pem"

echo "Keys generated in $CERT_DIR"
echo "  Private: $CERT_DIR/private.pem"
echo "  Public:  $CERT_DIR/public.pem"
```

**Step 2: Write failing test for JWT validation**

Add to `src/auth.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_valid_token() {
        // Generate a test keypair, create a token, validate it
        let private_key = std::fs::read("certs/private.pem")
            .expect("Run certs/generate-keys.sh first");
        let public_key = std::fs::read("certs/public.pem").unwrap();

        let claims = Claims {
            sub: "testuser".to_string(),
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
        };

        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
            &claims,
            &jsonwebtoken::EncodingKey::from_rsa_pem(&private_key).unwrap(),
        ).unwrap();

        let decoded = validate_token(&token, &public_key).unwrap();
        assert_eq!(decoded.sub, "testuser");
    }

    #[test]
    fn test_reject_expired_token() {
        let private_key = std::fs::read("certs/private.pem").unwrap();
        let public_key = std::fs::read("certs/public.pem").unwrap();

        let claims = Claims {
            sub: "testuser".to_string(),
            exp: 1000, // expired long ago
        };

        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
            &claims,
            &jsonwebtoken::EncodingKey::from_rsa_pem(&private_key).unwrap(),
        ).unwrap();

        assert!(validate_token(&token, &public_key).is_err());
    }
}
```

**Step 3: Run test to verify it fails**

Run: `cargo test --lib auth`
Expected: FAIL — `validate_token` not defined.

**Step 4: Implement auth.rs**

```rust
use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

#[derive(Serialize)]
pub struct Jwks {
    pub keys: Vec<JwkKey>,
}

#[derive(Serialize)]
pub struct JwkKey {
    pub kty: String,
    pub alg: String,
    pub r#use: String,
    pub n: String,
    pub e: String,
    pub kid: String,
}

pub fn validate_token(token: &str, public_key_pem: &[u8]) -> Result<Claims, jsonwebtoken::errors::Error> {
    let decoding_key = DecodingKey::from_rsa_pem(public_key_pem)?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;
    let token_data = decode::<Claims>(token, &decoding_key, &validation)?;
    Ok(token_data.claims)
}

pub async fn jwks_endpoint(State(public_key_pem): State<Vec<u8>>) -> impl IntoResponse {
    // Parse public key and extract modulus/exponent for JWKS
    let rsa_key = jsonwebtoken::DecodingKey::from_rsa_pem(&public_key_pem).unwrap();
    // Serve the public key in JWKS format
    // Implementation: parse PEM, extract n and e, base64url encode
    Json(serde_json::json!({
        "keys": [{
            "kty": "RSA",
            "alg": "RS256",
            "use": "sig",
            "kid": "sqlite-editor-key-1",
            // n and e extracted from public key
        }]
    }))
}

pub async fn auth_middleware<B>(
    State(public_key_pem): State<Vec<u8>>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    // Check for token in cookie or Authorization header
    let token = req.headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    // Also check cookie
    let token = token.or_else(|| {
        req.headers()
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .and_then(|cookies| {
                cookies.split(';')
                    .find_map(|c| {
                        let c = c.trim();
                        c.strip_prefix("token=")
                    })
            })
    });

    match token {
        Some(token) => {
            match validate_token(token, &public_key_pem) {
                Ok(claims) => {
                    req.extensions_mut().insert(claims);
                    Ok(next.run(req).await)
                }
                Err(_) => Err(StatusCode::UNAUTHORIZED),
            }
        }
        None => Err(StatusCode::UNAUTHORIZED),
    }
}
```

**Step 5: Run tests to verify they pass**

Run: `chmod +x certs/generate-keys.sh && ./certs/generate-keys.sh && cargo test --lib auth`
Expected: PASS

**Step 6: Commit**

```bash
git add certs/generate-keys.sh certs/.gitkeep src/auth.rs src/main.rs
git commit -m "feat: add JWT authentication with local CA and JWKS endpoint"
```

---

### Task CRUISE-003: SQLite Database Operations

**Files:**
- Create: `src/db.rs`
- Create: `src/models.rs`

**Step 1: Write failing tests for database operations**

```rust
// src/db.rs
#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn
    }

    #[test]
    fn test_list_tables_empty() {
        let conn = setup_db();
        let tables = list_tables(&conn).unwrap();
        assert!(tables.is_empty());
    }

    #[test]
    fn test_create_table() {
        let conn = setup_db();
        create_table(&conn, "users", &[
            ColumnDef { name: "id".into(), col_type: "INTEGER".into() },
            ColumnDef { name: "name".into(), col_type: "TEXT".into() },
        ]).unwrap();
        let tables = list_tables(&conn).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0], "users");
    }

    #[test]
    fn test_drop_table() {
        let conn = setup_db();
        create_table(&conn, "users", &[
            ColumnDef { name: "id".into(), col_type: "INTEGER".into() },
        ]).unwrap();
        drop_table(&conn, "users").unwrap();
        let tables = list_tables(&conn).unwrap();
        assert!(tables.is_empty());
    }

    #[test]
    fn test_list_columns() {
        let conn = setup_db();
        create_table(&conn, "users", &[
            ColumnDef { name: "id".into(), col_type: "INTEGER".into() },
            ColumnDef { name: "name".into(), col_type: "TEXT".into() },
        ]).unwrap();
        let cols = list_columns(&conn, "users").unwrap();
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0].name, "id");
        assert_eq!(cols[1].name, "name");
    }

    #[test]
    fn test_add_column() {
        let conn = setup_db();
        create_table(&conn, "users", &[
            ColumnDef { name: "id".into(), col_type: "INTEGER".into() },
        ]).unwrap();
        add_column(&conn, "users", &ColumnDef { name: "email".into(), col_type: "TEXT".into() }).unwrap();
        let cols = list_columns(&conn, "users").unwrap();
        assert_eq!(cols.len(), 2);
    }

    #[test]
    fn test_drop_column() {
        let conn = setup_db();
        create_table(&conn, "users", &[
            ColumnDef { name: "id".into(), col_type: "INTEGER".into() },
            ColumnDef { name: "name".into(), col_type: "TEXT".into() },
        ]).unwrap();
        drop_column(&conn, "users", "name").unwrap();
        let cols = list_columns(&conn, "users").unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].name, "id");
    }

    #[test]
    fn test_sql_injection_prevention_table_name() {
        let conn = setup_db();
        let result = create_table(&conn, "users; DROP TABLE users;--", &[
            ColumnDef { name: "id".into(), col_type: "INTEGER".into() },
        ]);
        assert!(result.is_err());
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib db`
Expected: FAIL — functions not defined.

**Step 3: Implement models.rs**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub col_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub row_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct CreateTableRequest {
    pub table_name: String,
    pub columns: Vec<ColumnDef>,
}

#[derive(Debug, Deserialize)]
pub struct AddColumnRequest {
    pub column_name: String,
    pub column_type: String,
}
```

**Step 4: Implement db.rs**

```rust
use crate::models::ColumnDef;
use rusqlite::{Connection, Result, params};
use std::sync::{Arc, Mutex};

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(path: &str) -> Result<DbPool> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    Ok(Arc::new(Mutex::new(conn)))
}

fn validate_identifier(name: &str) -> Result<(), String> {
    // Only allow alphanumeric and underscore, must start with letter or underscore
    if name.is_empty() {
        return Err("Identifier cannot be empty".into());
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(format!("Invalid identifier: {}", name));
    }
    if name.chars().next().unwrap().is_numeric() {
        return Err("Identifier cannot start with a number".into());
    }
    Ok(())
}

pub fn list_tables(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    )?;
    let tables = stmt.query_map([], |row| row.get(0))?
        .collect::<Result<Vec<String>>>()?;
    Ok(tables)
}

pub fn create_table(conn: &Connection, name: &str, columns: &[ColumnDef]) -> Result<()> {
    validate_identifier(name).map_err(|e| rusqlite::Error::InvalidParameterName(e))?;
    if columns.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName("At least one column required".into()));
    }
    for col in columns {
        validate_identifier(&col.name).map_err(|e| rusqlite::Error::InvalidParameterName(e))?;
        validate_identifier(&col.col_type).map_err(|e| rusqlite::Error::InvalidParameterName(e))?;
    }

    let cols: Vec<String> = columns.iter()
        .map(|c| format!("{} {}", c.name, c.col_type))
        .collect();
    let sql = format!("CREATE TABLE \"{}\" ({})", name, cols.join(", "));
    conn.execute(&sql, [])?;
    Ok(())
}

pub fn drop_table(conn: &Connection, name: &str) -> Result<()> {
    validate_identifier(name).map_err(|e| rusqlite::Error::InvalidParameterName(e))?;
    let sql = format!("DROP TABLE IF EXISTS \"{}\"", name);
    conn.execute(&sql, [])?;
    Ok(())
}

pub fn list_columns(conn: &Connection, table_name: &str) -> Result<Vec<ColumnDef>> {
    validate_identifier(table_name).map_err(|e| rusqlite::Error::InvalidParameterName(e))?;
    let sql = format!("PRAGMA table_info(\"{}\")", table_name);
    let mut stmt = conn.prepare(&sql)?;
    let columns = stmt.query_map([], |row| {
        Ok(ColumnDef {
            name: row.get(1)?,
            col_type: row.get(2)?,
        })
    })?.collect::<Result<Vec<_>>>()?;
    Ok(columns)
}

pub fn add_column(conn: &Connection, table_name: &str, column: &ColumnDef) -> Result<()> {
    validate_identifier(table_name).map_err(|e| rusqlite::Error::InvalidParameterName(e))?;
    validate_identifier(&column.name).map_err(|e| rusqlite::Error::InvalidParameterName(e))?;
    validate_identifier(&column.col_type).map_err(|e| rusqlite::Error::InvalidParameterName(e))?;
    let sql = format!(
        "ALTER TABLE \"{}\" ADD COLUMN \"{}\" {}",
        table_name, column.name, column.col_type
    );
    conn.execute(&sql, [])?;
    Ok(())
}

pub fn drop_column(conn: &Connection, table_name: &str, column_name: &str) -> Result<()> {
    validate_identifier(table_name).map_err(|e| rusqlite::Error::InvalidParameterName(e))?;
    validate_identifier(column_name).map_err(|e| rusqlite::Error::InvalidParameterName(e))?;
    let sql = format!(
        "ALTER TABLE \"{}\" DROP COLUMN \"{}\"",
        table_name, column_name
    );
    conn.execute(&sql, [])?;
    Ok(())
}

pub fn get_table_rows(conn: &Connection, table_name: &str, limit: usize, offset: usize) -> Result<Vec<Vec<String>>> {
    validate_identifier(table_name).map_err(|e| rusqlite::Error::InvalidParameterName(e))?;
    let sql = format!("SELECT * FROM \"{}\" LIMIT {} OFFSET {}", table_name, limit, offset);
    let mut stmt = conn.prepare(&sql)?;
    let col_count = stmt.column_count();
    let rows = stmt.query_map([], |row| {
        let mut values = Vec::new();
        for i in 0..col_count {
            let val: String = row.get::<_, rusqlite::types::Value>(i)
                .map(|v| format!("{:?}", v))
                .unwrap_or_default();
            values.push(val);
        }
        Ok(values)
    })?.collect::<Result<Vec<_>>>()?;
    Ok(rows)
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test --lib db`
Expected: PASS

**Step 6: Commit**

```bash
git add src/db.rs src/models.rs
git commit -m "feat: add SQLite database operations with SQL injection prevention"
```

---

### Task CRUISE-004: Askama Templates and Static Assets

**Files:**
- Create: `templates/base.html`
- Create: `templates/login.html`
- Create: `templates/dashboard.html`
- Create: `templates/table_list.html`
- Create: `templates/table_detail.html`
- Create: `templates/column_list.html`
- Create: `templates/error.html`
- Create: `static/htmx.min.js` (vendor htmx 2.0)

**Step 1: Vendor htmx**

Run: `mkdir -p static && curl -o static/htmx.min.js https://unpkg.com/htmx.org@2.0.4/dist/htmx.min.js`

**Step 2: Create base template**

`templates/base.html`:
```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}SQLite Editor{% endblock %}</title>
    <script src="/static/htmx.min.js"></script>
    <style>
        body { font-family: system-ui, sans-serif; max-width: 960px; margin: 0 auto; padding: 1rem; }
        table { border-collapse: collapse; width: 100%; margin: 1rem 0; }
        th, td { border: 1px solid #ddd; padding: 0.5rem; text-align: left; }
        th { background: #f5f5f5; }
        button, input[type="submit"] { padding: 0.5rem 1rem; cursor: pointer; }
        .error { color: red; padding: 0.5rem; background: #fee; border: 1px solid #fcc; margin: 0.5rem 0; }
        form { margin: 1rem 0; }
        input, select { padding: 0.4rem; margin: 0.2rem; }
        .danger { background: #dc3545; color: white; border: none; }
        nav { display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #ddd; padding-bottom: 0.5rem; margin-bottom: 1rem; }
    </style>
</head>
<body>
    {% block content %}{% endblock %}
</body>
</html>
```

**Step 3: Create login template**

`templates/login.html`:
```html
{% extends "base.html" %}
{% block title %}Login - SQLite Editor{% endblock %}
{% block content %}
<h1>SQLite Editor</h1>
<form method="POST" action="/login">
    <label for="token">JWT Token:</label><br>
    <textarea id="token" name="token" rows="4" cols="60" placeholder="Paste your JWT token here"></textarea><br>
    <button type="submit">Login</button>
</form>
{% if let Some(err) = error %}
<div class="error">{{ err }}</div>
{% endif %}
{% endblock %}
```

**Step 4: Create dashboard template**

`templates/dashboard.html`:
```html
{% extends "base.html" %}
{% block title %}Dashboard - SQLite Editor{% endblock %}
{% block content %}
<nav>
    <h1>SQLite Editor</h1>
    <span>Logged in as {{ user }} | <a href="/logout">Logout</a></span>
</nav>

<h2>Tables</h2>
<div id="table-list"
     hx-get="/api/tables"
     hx-trigger="load"
     hx-swap="innerHTML">
    Loading tables...
</div>

<h3>Create New Table</h3>
<form hx-post="/api/tables"
      hx-target="#table-list"
      hx-swap="innerHTML"
      hx-on::after-request="this.reset()">
    <input type="text" name="table_name" placeholder="Table name" required>
    <input type="text" name="col_name_1" placeholder="Column name" required>
    <select name="col_type_1">
        <option value="TEXT">TEXT</option>
        <option value="INTEGER">INTEGER</option>
        <option value="REAL">REAL</option>
        <option value="BLOB">BLOB</option>
    </select>
    <button type="submit">Create Table</button>
</form>
{% endblock %}
```

**Step 5: Create partial templates (table_list, table_detail, column_list, error)**

`templates/table_list.html`:
```html
{% if tables.is_empty() %}
<p>No tables yet. Create one above.</p>
{% else %}
<ul>
{% for table in tables %}
    <li>
        <a href="/tables/{{ table }}">{{ table }}</a>
        <button class="danger"
                hx-delete="/api/tables/{{ table }}"
                hx-target="#table-list"
                hx-swap="innerHTML"
                hx-confirm="Drop table '{{ table }}'? This cannot be undone.">
            Drop
        </button>
    </li>
{% endfor %}
</ul>
{% endif %}
```

`templates/table_detail.html`:
```html
{% extends "base.html" %}
{% block title %}{{ table_name }} - SQLite Editor{% endblock %}
{% block content %}
<nav>
    <h1><a href="/">SQLite Editor</a> / {{ table_name }}</h1>
    <span>Logged in as {{ user }} | <a href="/logout">Logout</a></span>
</nav>

<h2>Columns</h2>
<div id="column-list"
     hx-get="/api/tables/{{ table_name }}/columns"
     hx-trigger="load"
     hx-swap="innerHTML">
    Loading columns...
</div>

<h3>Add Column</h3>
<form hx-post="/api/tables/{{ table_name }}/columns"
      hx-target="#column-list"
      hx-swap="innerHTML"
      hx-on::after-request="this.reset()">
    <input type="text" name="column_name" placeholder="Column name" required>
    <select name="column_type">
        <option value="TEXT">TEXT</option>
        <option value="INTEGER">INTEGER</option>
        <option value="REAL">REAL</option>
        <option value="BLOB">BLOB</option>
    </select>
    <button type="submit">Add Column</button>
</form>

<h2>Data</h2>
<div id="table-data">
{% if rows.is_empty() %}
<p>No data in this table.</p>
{% else %}
<table>
    <thead>
        <tr>
        {% for col in columns %}
            <th>{{ col.name }} ({{ col.col_type }})</th>
        {% endfor %}
        </tr>
    </thead>
    <tbody>
    {% for row in rows %}
        <tr>
        {% for cell in row %}
            <td>{{ cell }}</td>
        {% endfor %}
        </tr>
    {% endfor %}
    </tbody>
</table>
{% endif %}
</div>
{% endblock %}
```

`templates/column_list.html`:
```html
{% if columns.is_empty() %}
<p>No columns.</p>
{% else %}
<table>
    <thead><tr><th>Name</th><th>Type</th><th>Actions</th></tr></thead>
    <tbody>
    {% for col in columns %}
    <tr>
        <td>{{ col.name }}</td>
        <td>{{ col.col_type }}</td>
        <td>
            <button class="danger"
                    hx-delete="/api/tables/{{ table_name }}/columns/{{ col.name }}"
                    hx-target="#column-list"
                    hx-swap="innerHTML"
                    hx-confirm="Drop column '{{ col.name }}'?">
                Remove
            </button>
        </td>
    </tr>
    {% endfor %}
    </tbody>
</table>
{% endif %}
```

`templates/error.html`:
```html
<div class="error">{{ message }}</div>
```

**Step 6: Commit**

```bash
git add templates/ static/
git commit -m "feat: add Askama templates and vendored htmx"
```

---

### Task CRUISE-005: Base Server Setup and Auth Routes

**Files:**
- Create: `src/handlers.rs` (AppState, auth-related handlers, health endpoint)
- Modify: `src/main.rs` (server config, public routes, auth middleware, static files)

**Step 1: Write failing test for health endpoint**

```rust
// In src/handlers.rs
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test::TestServer; // axum-test crate for integration tests

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = create_router();
        let server = TestServer::new(app).unwrap();
        let response = server.get("/health").await;
        response.assert_status_ok();
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib handlers`
Expected: FAIL

**Step 3: Implement AppState and auth/core handlers in handlers.rs**

```rust
use axum::{
    extract::{State, Form},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use askama::Template;
use crate::{auth::Claims, models::*};
use std::sync::{Arc, Mutex};
use rusqlite::Connection;

pub type AppState = Arc<AppStateInner>;

pub struct AppStateInner {
    pub db: Mutex<Connection>,
    pub public_key_pem: Vec<u8>,
    pub private_key_pem: Option<Vec<u8>>, // Only for dev/test
}

// Template structs (auth-related)
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

// --- Auth and core page handlers ---

pub async fn login_page() -> impl IntoResponse {
    Html(LoginTemplate { error: None }.render().unwrap())
}

pub async fn login_submit(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Response {
    match crate::auth::validate_token(&form.token, &state.public_key_pem) {
        Ok(claims) => {
            // Set cookie and redirect to dashboard
            let cookie = format!("token={}; HttpOnly; Path=/; SameSite=Strict", form.token);
            (
                StatusCode::SEE_OTHER,
                [
                    ("Location", "/"),
                    ("Set-Cookie", &cookie),
                ],
            ).into_response()
        }
        Err(e) => {
            Html(LoginTemplate { error: Some(format!("Invalid token: {}", e)) }.render().unwrap()).into_response()
        }
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
```

**Step 4: Implement main.rs with server config and auth routes**

```rust
mod auth;
mod db;
mod handlers;
mod models;

use axum::{
    routing::{get, post},
    middleware,
    Router,
};
use handlers::AppState;
use std::sync::{Arc, Mutex};
use std::net::SocketAddr;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let public_key = std::fs::read("certs/public.pem")
        .expect("Missing certs/public.pem — run certs/generate-keys.sh");

    let conn = rusqlite::Connection::open("data.db")
        .expect("Failed to open SQLite database");
    conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();

    let state = Arc::new(handlers::AppStateInner {
        db: Mutex::new(conn),
        public_key_pem: public_key,
        private_key_pem: None,
    });

    // Public routes
    let public_routes = Router::new()
        .route("/login", get(handlers::login_page))
        .route("/login", post(handlers::login_submit))
        .route("/health", get(handlers::health))
        .route("/.well-known/jwks.json", get(auth::jwks_endpoint));

    // Protected routes (auth-only for now; DB editor routes added in CRUISE-005b)
    let protected_routes = Router::new()
        .route("/", get(handlers::dashboard))
        .route("/logout", get(handlers::logout))
        .layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let port: u16 = std::env::var("PORT").ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

**Step 5: Verify it compiles**

Run: `cargo check`
Expected: Compiles.

**Step 6: Run all unit tests**

Run: `cargo test`
Expected: All tests pass.

**Step 7: Commit**

```bash
git add src/handlers.rs src/main.rs
git commit -m "feat: add base server setup with auth routes and health endpoint"
```

---

### Task CRUISE-005b: DB Editor Route Handlers

**Files:**
- Modify: `src/handlers.rs` (add DB editor template structs and route handlers)
- Modify: `src/main.rs` (wire up table and column CRUD routes in protected group)

**Step 1: Write failing test for table list endpoint**

```rust
// Add to src/handlers.rs tests module
#[tokio::test]
async fn test_list_tables_requires_auth() {
    let app = create_router();
    let server = TestServer::new(app).unwrap();
    let response = server.get("/api/tables").await;
    response.assert_status(StatusCode::UNAUTHORIZED);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib handlers`
Expected: FAIL

**Step 3: Add DB editor template structs and handlers to handlers.rs**

```rust
// Add these template structs to handlers.rs

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

// --- DB Editor API handlers (return HTML fragments for htmx) ---

pub async fn list_tables(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.db.lock().unwrap();
    match db::list_tables(&conn) {
        Ok(tables) => Html(TableListTemplate { tables }.render().unwrap()).into_response(),
        Err(e) => Html(ErrorTemplate { message: e.to_string() }.render().unwrap()).into_response(),
    }
}

pub async fn create_table(
    State(state): State<AppState>,
    Form(form): Form<CreateTableForm>,
) -> impl IntoResponse {
    let conn = state.db.lock().unwrap();
    let columns = vec![ColumnDef { name: form.col_name_1, col_type: form.col_type_1 }];
    if let Err(e) = db::create_table(&conn, &form.table_name, &columns) {
        return Html(ErrorTemplate { message: e.to_string() }.render().unwrap()).into_response();
    }
    match db::list_tables(&conn) {
        Ok(tables) => Html(TableListTemplate { tables }.render().unwrap()).into_response(),
        Err(e) => Html(ErrorTemplate { message: e.to_string() }.render().unwrap()).into_response(),
    }
}

pub async fn delete_table(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
) -> impl IntoResponse {
    let conn = state.db.lock().unwrap();
    if let Err(e) = db::drop_table(&conn, &table_name) {
        return Html(ErrorTemplate { message: e.to_string() }.render().unwrap()).into_response();
    }
    match db::list_tables(&conn) {
        Ok(tables) => Html(TableListTemplate { tables }.render().unwrap()).into_response(),
        Err(e) => Html(ErrorTemplate { message: e.to_string() }.render().unwrap()).into_response(),
    }
}

pub async fn table_detail(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    claims: Claims,
) -> impl IntoResponse {
    let conn = state.db.lock().unwrap();
    let columns = db::list_columns(&conn, &table_name).unwrap_or_default();
    let rows = db::get_table_rows(&conn, &table_name, 100, 0).unwrap_or_default();
    Html(TableDetailTemplate {
        table_name,
        user: claims.sub,
        columns,
        rows,
    }.render().unwrap())
}

pub async fn list_columns(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
) -> impl IntoResponse {
    let conn = state.db.lock().unwrap();
    match db::list_columns(&conn, &table_name) {
        Ok(columns) => Html(ColumnListTemplate { table_name, columns }.render().unwrap()).into_response(),
        Err(e) => Html(ErrorTemplate { message: e.to_string() }.render().unwrap()).into_response(),
    }
}

pub async fn add_column(
    State(state): State<AppState>,
    Path(table_name): Path<String>,
    Form(form): Form<AddColumnRequest>,
) -> impl IntoResponse {
    let conn = state.db.lock().unwrap();
    let col = ColumnDef { name: form.column_name, col_type: form.column_type };
    if let Err(e) = db::add_column(&conn, &table_name, &col) {
        return Html(ErrorTemplate { message: e.to_string() }.render().unwrap()).into_response();
    }
    match db::list_columns(&conn, &table_name) {
        Ok(columns) => Html(ColumnListTemplate { table_name, columns }.render().unwrap()).into_response(),
        Err(e) => Html(ErrorTemplate { message: e.to_string() }.render().unwrap()).into_response(),
    }
}

pub async fn delete_column(
    State(state): State<AppState>,
    Path((table_name, column_name)): Path<(String, String)>,
) -> impl IntoResponse {
    let conn = state.db.lock().unwrap();
    if let Err(e) = db::drop_column(&conn, &table_name, &column_name) {
        return Html(ErrorTemplate { message: e.to_string() }.render().unwrap()).into_response();
    }
    match db::list_columns(&conn, &table_name) {
        Ok(columns) => Html(ColumnListTemplate { table_name, columns }.render().unwrap()).into_response(),
        Err(e) => Html(ErrorTemplate { message: e.to_string() }.render().unwrap()).into_response(),
    }
}
```

**Step 4: Wire up DB editor routes in main.rs**

Add the following routes to the `protected_routes` group in `main.rs`:

```rust
    // Protected routes (now including DB editor)
    let protected_routes = Router::new()
        .route("/", get(handlers::dashboard))
        .route("/tables/{table_name}", get(handlers::table_detail))
        .route("/api/tables", get(handlers::list_tables))
        .route("/api/tables", post(handlers::create_table))
        .route("/api/tables/{table_name}", delete(handlers::delete_table))
        .route("/api/tables/{table_name}/columns", get(handlers::list_columns))
        .route("/api/tables/{table_name}/columns", post(handlers::add_column))
        .route("/api/tables/{table_name}/columns/{column_name}", delete(handlers::delete_column))
        .route("/logout", get(handlers::logout))
        .layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware));
```

**Step 5: Verify it compiles**

Run: `cargo check`
Expected: Compiles.

**Step 6: Run all unit tests**

Run: `cargo test`
Expected: All tests pass.

**Step 7: Commit**

```bash
git add src/handlers.rs src/main.rs
git commit -m "feat: add DB editor route handlers for table and column CRUD"
```

---

### Task CRUISE-006: Playwright E2E Test Setup

**Files:**
- Create: `tests/package.json`
- Create: `tests/playwright.config.ts`
- Create: `tests/e2e/helpers.ts` (JWT token generation helper)
- Create: `tests/tsconfig.json`

**Step 1: Initialize Playwright project**

```bash
cd tests
npm init -y
npm install -D @playwright/test typescript
npx playwright install chromium
```

**Step 2: Create playwright.config.ts**

```typescript
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false, // SQLite single-writer
  retries: 1,
  reporter: [
    ['html', { outputFolder: 'playwright-report' }],
    ['json', { outputFile: 'test-results/results.json' }],
  ],
  use: {
    baseURL: 'http://localhost:3000',
    trace: 'on-first-retry',
  },
  webServer: {
    command: 'cd .. && cargo run',
    url: 'http://localhost:3000/health',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000, // Rust compile can be slow
  },
});
```

**Step 3: Create test helper for JWT generation**

`tests/e2e/helpers.ts`:
```typescript
import * as crypto from 'crypto';
import * as fs from 'fs';
import * as path from 'path';

export function generateToken(sub: string, expiresInSeconds: number = 60): string {
  const privateKeyPath = path.resolve(__dirname, '../../certs/private.pem');
  const privateKey = fs.readFileSync(privateKeyPath, 'utf-8');

  const header = Buffer.from(JSON.stringify({ alg: 'RS256', typ: 'JWT' })).toString('base64url');
  const now = Math.floor(Date.now() / 1000);
  const payload = Buffer.from(JSON.stringify({
    sub,
    exp: now + expiresInSeconds,
    iat: now,
  })).toString('base64url');

  const signature = crypto.sign('RSA-SHA256', Buffer.from(`${header}.${payload}`), privateKey);
  return `${header}.${payload}.${signature.toString('base64url')}`;
}

export function generateExpiredToken(sub: string): string {
  return generateToken(sub, -60); // Already expired
}
```

**Step 4: Commit**

```bash
git add tests/
git commit -m "chore: set up Playwright E2E test infrastructure"
```

---

### Task CRUISE-007: E2E Tests — Authentication

**Files:**
- Create: `tests/e2e/auth.spec.ts`

**Step 1: Write auth E2E tests**

```typescript
import { test, expect } from '@playwright/test';
import { generateToken, generateExpiredToken } from './helpers';

test.describe('Authentication', () => {
  test('shows login page when not authenticated', async ({ page }) => {
    await page.goto('/');
    // Should redirect to login
    await expect(page).toHaveURL(/\/login/);
    await expect(page.locator('h1')).toContainText('SQLite Editor');
    await expect(page.locator('textarea[name="token"]')).toBeVisible();
  });

  test('login with valid short-lived JWT token', async ({ page }) => {
    const token = generateToken('testuser', 120); // 2-minute token
    await page.goto('/login');
    await page.locator('textarea[name="token"]').fill(token);
    await page.locator('button[type="submit"]').click();
    // Should redirect to dashboard
    await expect(page).toHaveURL('/');
    await expect(page.locator('text=Logged in as testuser')).toBeVisible();
  });

  test('login with expired token shows error', async ({ page }) => {
    const token = generateExpiredToken('testuser');
    await page.goto('/login');
    await page.locator('textarea[name="token"]').fill(token);
    await page.locator('button[type="submit"]').click();
    await expect(page.locator('.error')).toContainText('Invalid token');
  });

  test('logout clears session', async ({ page }) => {
    const token = generateToken('testuser');
    await page.goto('/login');
    await page.locator('textarea[name="token"]').fill(token);
    await page.locator('button[type="submit"]').click();
    await expect(page).toHaveURL('/');

    await page.locator('a:text("Logout")').click();
    await expect(page).toHaveURL(/\/login/);
  });

  test('.well-known/jwks.json endpoint returns public key', async ({ request }) => {
    const response = await request.get('/.well-known/jwks.json');
    expect(response.ok()).toBeTruthy();
    const jwks = await response.json();
    expect(jwks.keys).toHaveLength(1);
    expect(jwks.keys[0].kty).toBe('RSA');
    expect(jwks.keys[0].alg).toBe('RS256');
  });
});
```

**Step 2: Run tests (expect them to pass once server is built)**

Run: `cd tests && npx playwright test e2e/auth.spec.ts`
Expected: All 5 tests pass.

**Step 3: Commit**

```bash
git add tests/e2e/auth.spec.ts
git commit -m "test: add E2E authentication tests with short-lived JWT"
```

---

### Task CRUISE-008: E2E Tests — Table Operations

**Files:**
- Create: `tests/e2e/tables.spec.ts`

**Step 1: Write table E2E tests**

```typescript
import { test, expect } from '@playwright/test';
import { generateToken } from './helpers';

test.describe('Table Operations', () => {
  test.beforeEach(async ({ page }) => {
    const token = generateToken('testuser');
    await page.goto('/login');
    await page.locator('textarea[name="token"]').fill(token);
    await page.locator('button[type="submit"]').click();
    await expect(page).toHaveURL('/');
  });

  test('shows empty state when no tables exist', async ({ page }) => {
    await expect(page.locator('#table-list')).toContainText('No tables yet');
  });

  test('create a new table', async ({ page }) => {
    await page.locator('input[name="table_name"]').fill('users');
    await page.locator('input[name="col_name_1"]').fill('id');
    await page.locator('select[name="col_type_1"]').selectOption('INTEGER');
    await page.locator('button:text("Create Table")').click();

    // htmx will swap the table list
    await expect(page.locator('#table-list')).toContainText('users');
  });

  test('drop a table', async ({ page }) => {
    // First create a table
    await page.locator('input[name="table_name"]').fill('temp_table');
    await page.locator('input[name="col_name_1"]').fill('id');
    await page.locator('select[name="col_type_1"]').selectOption('INTEGER');
    await page.locator('button:text("Create Table")').click();
    await expect(page.locator('#table-list')).toContainText('temp_table');

    // Accept the confirmation dialog
    page.on('dialog', dialog => dialog.accept());

    // Drop the table
    await page.locator('button:text("Drop")').click();
    await expect(page.locator('#table-list')).not.toContainText('temp_table');
  });

  test('navigate to table detail', async ({ page }) => {
    // Create a table first
    await page.locator('input[name="table_name"]').fill('products');
    await page.locator('input[name="col_name_1"]').fill('id');
    await page.locator('button:text("Create Table")').click();
    await expect(page.locator('#table-list')).toContainText('products');

    // Click on the table name
    await page.locator('a:text("products")').click();
    await expect(page).toHaveURL(/\/tables\/products/);
    await expect(page.locator('h1')).toContainText('products');
  });
});
```

**Step 2: Run tests**

Run: `cd tests && npx playwright test e2e/tables.spec.ts`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add tests/e2e/tables.spec.ts
git commit -m "test: add E2E tests for table creation and deletion"
```

---

### Task CRUISE-009: E2E Tests — Column Operations

**Files:**
- Create: `tests/e2e/columns.spec.ts`

**Step 1: Write column E2E tests**

```typescript
import { test, expect } from '@playwright/test';
import { generateToken } from './helpers';

test.describe('Column Operations', () => {
  test.beforeEach(async ({ page }) => {
    const token = generateToken('testuser');
    await page.goto('/login');
    await page.locator('textarea[name="token"]').fill(token);
    await page.locator('button[type="submit"]').click();
    await expect(page).toHaveURL('/');

    // Create a test table
    await page.locator('input[name="table_name"]').fill('test_columns');
    await page.locator('input[name="col_name_1"]').fill('id');
    await page.locator('select[name="col_type_1"]').selectOption('INTEGER');
    await page.locator('button:text("Create Table")').click();
    await expect(page.locator('#table-list')).toContainText('test_columns');

    // Navigate to table detail
    await page.locator('a:text("test_columns")').click();
    await expect(page).toHaveURL(/\/tables\/test_columns/);
  });

  test('shows existing columns', async ({ page }) => {
    await expect(page.locator('#column-list')).toContainText('id');
    await expect(page.locator('#column-list')).toContainText('INTEGER');
  });

  test('add a new column', async ({ page }) => {
    await page.locator('input[name="column_name"]').fill('email');
    await page.locator('select[name="column_type"]').selectOption('TEXT');
    await page.locator('button:text("Add Column")').click();

    await expect(page.locator('#column-list')).toContainText('email');
    await expect(page.locator('#column-list')).toContainText('TEXT');
  });

  test('remove a column', async ({ page }) => {
    // First add a column to remove
    await page.locator('input[name="column_name"]').fill('temp_col');
    await page.locator('select[name="column_type"]').selectOption('TEXT');
    await page.locator('button:text("Add Column")').click();
    await expect(page.locator('#column-list')).toContainText('temp_col');

    // Accept the confirmation dialog
    page.on('dialog', dialog => dialog.accept());

    // Remove the column — click the Remove button in the row containing 'temp_col'
    const row = page.locator('tr', { has: page.locator('text=temp_col') });
    await row.locator('button:text("Remove")').click();

    await expect(page.locator('#column-list')).not.toContainText('temp_col');
  });

  test('add multiple columns of different types', async ({ page }) => {
    // Add TEXT column
    await page.locator('input[name="column_name"]').fill('name');
    await page.locator('select[name="column_type"]').selectOption('TEXT');
    await page.locator('button:text("Add Column")').click();
    await expect(page.locator('#column-list')).toContainText('name');

    // Add REAL column
    await page.locator('input[name="column_name"]').fill('price');
    await page.locator('select[name="column_type"]').selectOption('REAL');
    await page.locator('button:text("Add Column")').click();
    await expect(page.locator('#column-list')).toContainText('price');
    await expect(page.locator('#column-list')).toContainText('REAL');
  });
});
```

**Step 2: Run tests**

Run: `cd tests && npx playwright test e2e/columns.spec.ts`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add tests/e2e/columns.spec.ts
git commit -m "test: add E2E tests for column add and remove operations"
```

---

### Task CRUISE-010: GitHub Actions CI/CD Workflows

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/e2e.yml`

**Step 1: Create CI workflow (lint + dependency review)**

`.github/workflows/ci.yml`:
```yaml
name: CI

on:
  pull_request:
    branches: [main]

permissions:
  contents: read
  pull-requests: write

jobs:
  lint:
    name: Super-Linter
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: read
      statuses: write
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Run Super-Linter
        uses: super-linter/super-linter@v7
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          DEFAULT_BRANCH: main
          VALIDATE_ALL_CODEBASE: false
          VALIDATE_RUST_2021: true
          VALIDATE_TYPESCRIPT_ES: true
          VALIDATE_YAML: true
          VALIDATE_MARKDOWN: true
          VALIDATE_HTML: true
          FILTER_REGEX_EXCLUDE: "(static/htmx\\.min\\.js|tests/node_modules/.*)"

  dependency-review:
    name: Dependency Review
    runs-on: ubuntu-latest
    if: github.event_name == 'pull_request'
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Dependency Review
        uses: actions/dependency-review-action@v4
```

**Step 2: Create E2E workflow**

`.github/workflows/e2e.yml`:
```yaml
name: E2E Tests

on:
  pull_request:
    branches: [main]

permissions:
  contents: write
  pull-requests: write

jobs:
  e2e:
    name: Playwright E2E Tests
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache Rust dependencies
        uses: Swatinem/rust-cache@v2

      - name: Build Rust server
        run: cargo build --release

      - name: Generate test keys
        run: |
          chmod +x certs/generate-keys.sh
          ./certs/generate-keys.sh

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Install Playwright dependencies
        working-directory: tests
        run: |
          npm ci
          npx playwright install --with-deps chromium

      - name: Run E2E tests
        working-directory: tests
        run: npx playwright test
        env:
          CI: true

      - name: Upload test results
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: playwright-report
          path: tests/playwright-report/
          retention-days: 30

      - name: Commit test results
        if: always()
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          mkdir -p test-results
          cp tests/test-results/results.json test-results/ 2>/dev/null || true
          git add test-results/
          git diff --cached --quiet || git commit -m "ci: update E2E test results"
          git push || echo "Push failed — may need PR permissions"
```

**Step 3: Commit**

```bash
git add .github/
git commit -m "ci: add GitHub Actions for linting, dependency review, and E2E tests"
```

---

### Task CRUISE-011: Integration Testing and Final Polish

**Files:**
- Modify: `Cargo.toml` (add dev dependencies if needed)
- Run: Full test suite (unit + E2E)
- Verify: All CI workflows are valid YAML

**Step 1: Run cargo clippy for linting**

Run: `cargo clippy -- -W warnings`
Expected: No warnings. Fix any issues.

**Step 2: Run all unit tests**

Run: `cargo test`
Expected: All pass.

**Step 3: Run E2E tests locally**

```bash
./certs/generate-keys.sh
cd tests && npm install && npx playwright install chromium
npx playwright test
```
Expected: All pass.

**Step 4: Validate GitHub Actions YAML**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); yaml.safe_load(open('.github/workflows/e2e.yml')); print('Valid')"`
Expected: "Valid"

**Step 5: Final commit**

```bash
git add -A
git commit -m "chore: final polish and integration verification"
```

---

## Task Dependency Graph

```
CRUISE-001 (Scaffolding + .gitignore)
    ├── CRUISE-002 (JWT Auth)
    │       └── CRUISE-005 (Base Server + Auth) ──→ CRUISE-005b (DB Editor Routes)
    ├── CRUISE-003 (SQLite Operations)                         │
    │       └── CRUISE-005b (DB Editor Routes)                 │
    └── CRUISE-004 (Templates + htmx)                          │
            └── CRUISE-005 (Base Server + Auth)                │
                                                               ▼
                                            CRUISE-005b ──→ CRUISE-006 (Playwright Setup)
                                                               ├── CRUISE-007 (Auth E2E)
                                                               ├── CRUISE-008 (Table E2E)
                                                               └── CRUISE-009 (Column E2E)
                                            CRUISE-010 (CI/CD) ← independent
                                            CRUISE-011 (Integration) ← depends on all
```

---

## Spawn Instance Configuration

```json
{
  "title": "SQLite Database Editor Web Application",
  "overview": "Rust + Axum web application with htmx frontend for editing SQLite databases. JWT authentication with local CA, Askama templates, Playwright E2E tests, GitHub Actions CI/CD with Super-Linter and dependency review.",
  "spawn_instances": [
    {
      "id": "SPAWN-001",
      "name": "Project Setup and .gitignore",
      "use_spawn_team": false,
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 180",
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "task_ids": ["CRUISE-001"]
    },
    {
      "id": "SPAWN-002",
      "name": "Backend Core (Auth + DB + Models)",
      "use_spawn_team": true,
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "task_ids": ["CRUISE-002", "CRUISE-003"]
    },
    {
      "id": "SPAWN-003",
      "name": "Frontend Templates and Static Assets",
      "use_spawn_team": false,
      "cli_params": "claude --model haiku --allowedTools Read,Write,Edit,Bash --timeout 180",
      "permissions": ["Read", "Write", "Edit", "Bash"],
      "task_ids": ["CRUISE-004"]
    },
    {
      "id": "SPAWN-004",
      "name": "Base Server Setup and Auth Routes",
      "use_spawn_team": true,
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "task_ids": ["CRUISE-005"]
    },
    {
      "id": "SPAWN-004b",
      "name": "DB Editor Route Handlers",
      "use_spawn_team": true,
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "task_ids": ["CRUISE-005b"]
    },
    {
      "id": "SPAWN-005",
      "name": "Playwright E2E Test Suite",
      "use_spawn_team": true,
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "task_ids": ["CRUISE-006", "CRUISE-007", "CRUISE-008", "CRUISE-009"]
    },
    {
      "id": "SPAWN-006",
      "name": "CI/CD GitHub Actions",
      "use_spawn_team": false,
      "cli_params": "claude --model haiku --allowedTools Read,Write,Edit --timeout 180",
      "permissions": ["Read", "Write", "Edit"],
      "task_ids": ["CRUISE-010"]
    },
    {
      "id": "SPAWN-007",
      "name": "Final Integration and Verification",
      "use_spawn_team": true,
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "task_ids": ["CRUISE-011"]
    }
  ],
  "tasks": [
    {
      "id": "CRUISE-001",
      "subject": "Project Scaffolding and .gitignore",
      "description": "Create comprehensive .gitignore covering Rust build artifacts, keys/credentials, SQLite databases, Node.js/Playwright, editor/IDE files, OS files (macOS/Windows/Linux), .env files, log files, .fork-join directories, and temp files. Create Cargo.toml with all dependencies (axum, tokio, tower, tower-http, rusqlite, askama, askama_axum, jsonwebtoken, serde, serde_json, base64, ring, tracing, tracing-subscriber). Create minimal main.rs that compiles.",
      "blocked_by": [],
      "complexity": "low",
      "acceptance_criteria": [
        ".gitignore covers all required categories: Rust, keys, SQLite, Node, editors, OS files, .env, logs, .fork-join",
        "Cargo.toml has all required dependencies",
        "cargo check passes",
        "No keys, credentials, or sensitive files are tracked"
      ],
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 180",
      "spawn_instance": "SPAWN-001"
    },
    {
      "id": "CRUISE-002",
      "subject": "JWT Authentication Infrastructure",
      "description": "Create RSA keypair generation script (certs/generate-keys.sh). Implement JWT token validation using jsonwebtoken crate with RS256. Create auth middleware for Axum that checks Authorization header and cookies. Implement .well-known/jwks.json endpoint serving the public key in JWKS format. Write unit tests for valid token, expired token, and invalid token scenarios.",
      "blocked_by": ["CRUISE-001"],
      "complexity": "high",
      "acceptance_criteria": [
        "generate-keys.sh creates RSA 2048-bit keypair",
        "JWT validation accepts valid RS256 tokens",
        "JWT validation rejects expired tokens",
        "Auth middleware extracts claims from Bearer token and cookies",
        ".well-known/jwks.json returns valid JWKS with RSA public key",
        "Unit tests pass for all auth scenarios",
        "Private keys are never committed to git"
      ],
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "spawn_instance": "SPAWN-002"
    },
    {
      "id": "CRUISE-003",
      "subject": "SQLite Database Operations",
      "description": "Implement SQLite operations: list_tables, create_table, drop_table, list_columns, add_column, drop_column, get_table_rows. Use rusqlite with in-memory connection for tests. Implement SQL injection prevention via identifier validation (only alphanumeric and underscore allowed). Use Arc<Mutex<Connection>> for thread-safe access from async Axum handlers.",
      "blocked_by": ["CRUISE-001"],
      "complexity": "medium",
      "acceptance_criteria": [
        "list_tables returns empty vec for fresh database",
        "create_table creates table with specified columns",
        "drop_table removes table",
        "list_columns returns correct column names and types",
        "add_column adds column to existing table",
        "drop_column removes column from existing table",
        "SQL injection attempts are rejected",
        "All unit tests pass"
      ],
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "spawn_instance": "SPAWN-002"
    },
    {
      "id": "CRUISE-004",
      "subject": "Askama Templates and Static Assets",
      "description": "Create Askama HTML templates: base.html (layout with htmx script), login.html (JWT token input form), dashboard.html (table list with create form), table_list.html (htmx partial), table_detail.html (columns and data view), column_list.html (htmx partial), error.html (error display). Vendor htmx 2.0 minified JS into static/htmx.min.js.",
      "blocked_by": ["CRUISE-001"],
      "complexity": "medium",
      "acceptance_criteria": [
        "base.html includes htmx script and basic styling",
        "login.html has JWT token textarea and submit button",
        "dashboard.html uses htmx for loading/creating/deleting tables",
        "table_detail.html shows columns with add/remove and data rows",
        "All partials return HTML fragments (not full pages) for htmx swaps",
        "htmx.min.js is vendored locally (no CDN dependency)",
        "Templates use hx-confirm for destructive operations"
      ],
      "permissions": ["Read", "Write", "Edit", "Bash"],
      "cli_params": "claude --model haiku --allowedTools Read,Write,Edit,Bash --timeout 180",
      "spawn_instance": "SPAWN-003"
    },
    {
      "id": "CRUISE-005",
      "subject": "Base Server Setup and Auth Routes",
      "description": "Create AppState struct with Mutex<Connection> and public key. Implement core handlers: login_page, login_submit (JWT validation + cookie), dashboard, logout, health. Set up main.rs with server config, public routes (login, health, JWKS), protected route group with auth middleware, and static file serving via tower-http ServeDir. This establishes the core infrastructure that DB editor routes build upon.",
      "blocked_by": ["CRUISE-002", "CRUISE-004"],
      "complexity": "medium",
      "acceptance_criteria": [
        "GET /login shows login form",
        "POST /login validates JWT and sets cookie",
        "GET / shows dashboard (requires auth)",
        "GET /health returns 200 OK",
        "GET /.well-known/jwks.json returns JWKS",
        "GET /logout clears cookie and redirects to login",
        "Static files served at /static/",
        "Auth middleware rejects unauthenticated requests to protected routes",
        "cargo check passes with no errors"
      ],
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "spawn_instance": "SPAWN-004"
    },
    {
      "id": "CRUISE-005b",
      "subject": "DB Editor Route Handlers",
      "description": "Add DB editor template structs (TableListTemplate, TableDetailTemplate, ColumnListTemplate) and implement htmx route handlers for table and column CRUD operations: list_tables, create_table, delete_table, table_detail, list_columns, add_column, delete_column. Wire up all DB editor routes in the protected route group in main.rs. All handlers return HTML fragments for htmx swaps.",
      "blocked_by": ["CRUISE-005", "CRUISE-003"],
      "complexity": "medium",
      "acceptance_criteria": [
        "GET /api/tables returns table list HTML partial",
        "POST /api/tables creates table and returns updated list",
        "DELETE /api/tables/:name drops table",
        "GET /tables/:name shows table detail page",
        "GET /api/tables/:name/columns returns column list HTML partial",
        "POST /api/tables/:name/columns adds column",
        "DELETE /api/tables/:name/columns/:col removes column",
        "All handlers return HTML fragments (not full pages) for htmx",
        "cargo check passes with no errors"
      ],
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "spawn_instance": "SPAWN-004b"
    },
    {
      "id": "CRUISE-006",
      "subject": "Playwright E2E Test Setup",
      "description": "Initialize Playwright project in tests/ directory. Create playwright.config.ts with webServer config pointing to cargo run. Create test helper that generates JWT tokens using the local private key (crypto.sign with RSA-SHA256). Install Playwright and chromium browser.",
      "blocked_by": ["CRUISE-005b"],
      "complexity": "medium",
      "acceptance_criteria": [
        "tests/package.json has @playwright/test dependency",
        "playwright.config.ts configures webServer to start Rust server",
        "helpers.ts generates valid RS256 JWT tokens using local private key",
        "helpers.ts can generate expired tokens for negative tests",
        "npx playwright test runs without config errors"
      ],
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "spawn_instance": "SPAWN-005"
    },
    {
      "id": "CRUISE-007",
      "subject": "E2E Tests — Authentication",
      "description": "Write Playwright E2E tests for authentication flows: login page display, login with valid short-lived JWT, login with expired token shows error, logout clears session, .well-known/jwks.json endpoint returns valid JWKS.",
      "blocked_by": ["CRUISE-006"],
      "complexity": "medium",
      "acceptance_criteria": [
        "Test: unauthenticated user sees login page",
        "Test: valid JWT token logs user in and shows dashboard",
        "Test: expired token shows error message",
        "Test: logout redirects to login and clears cookie",
        "Test: JWKS endpoint returns RSA public key",
        "All auth E2E tests pass"
      ],
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "spawn_instance": "SPAWN-005"
    },
    {
      "id": "CRUISE-008",
      "subject": "E2E Tests — Table Operations",
      "description": "Write Playwright E2E tests for table CRUD: empty state display, create table, drop table with confirmation, navigate to table detail page.",
      "blocked_by": ["CRUISE-006"],
      "complexity": "medium",
      "acceptance_criteria": [
        "Test: empty state shows 'No tables yet' message",
        "Test: creating a table shows it in the list",
        "Test: dropping a table removes it from the list",
        "Test: clicking table name navigates to detail page",
        "All table E2E tests pass"
      ],
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "spawn_instance": "SPAWN-005"
    },
    {
      "id": "CRUISE-009",
      "subject": "E2E Tests — Column Operations",
      "description": "Write Playwright E2E tests for column modifications: view existing columns, add new column, remove column with confirmation, add multiple columns of different types.",
      "blocked_by": ["CRUISE-006"],
      "complexity": "medium",
      "acceptance_criteria": [
        "Test: existing columns are displayed with names and types",
        "Test: adding a column shows it in the column list",
        "Test: removing a column removes it from the list",
        "Test: multiple columns of different types can be added",
        "All column E2E tests pass"
      ],
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "spawn_instance": "SPAWN-005"
    },
    {
      "id": "CRUISE-010",
      "subject": "GitHub Actions CI/CD Workflows",
      "description": "Create .github/workflows/ci.yml with Super-Linter (github/super-linter@v7) configured for Rust, TypeScript, YAML, Markdown, HTML — excluding vendored htmx. Add dependency-review-action (actions/dependency-review-action@v4) for PR dependency scanning. Create .github/workflows/e2e.yml that builds Rust, generates test keys, installs Playwright, runs E2E tests, uploads report artifact, and commits test results to the repo. Both workflows trigger on PRs to main.",
      "blocked_by": [],
      "complexity": "medium",
      "acceptance_criteria": [
        "ci.yml runs Super-Linter with correct language filters",
        "ci.yml runs dependency-review-action on PRs",
        "e2e.yml builds Rust project with caching",
        "e2e.yml generates RSA keys for testing",
        "e2e.yml runs Playwright tests",
        "e2e.yml uploads test report as artifact",
        "e2e.yml commits test results to the repo",
        "Both workflows trigger on pull_request to main",
        "YAML is valid"
      ],
      "permissions": ["Read", "Write", "Edit"],
      "cli_params": "claude --model haiku --allowedTools Read,Write,Edit --timeout 180",
      "spawn_instance": "SPAWN-006"
    },
    {
      "id": "CRUISE-011",
      "subject": "Integration Testing and Final Polish",
      "description": "Run full test suite: cargo clippy, cargo test, Playwright E2E tests. Validate GitHub Actions YAML. Fix any issues found. Ensure the application builds cleanly, all tests pass, and CI configuration is correct.",
      "blocked_by": ["CRUISE-001", "CRUISE-002", "CRUISE-003", "CRUISE-004", "CRUISE-005", "CRUISE-005b", "CRUISE-006", "CRUISE-007", "CRUISE-008", "CRUISE-009", "CRUISE-010"],
      "complexity": "medium",
      "acceptance_criteria": [
        "cargo clippy passes with no warnings",
        "cargo test passes all unit tests",
        "Playwright E2E tests all pass",
        "GitHub Actions YAML is valid",
        "No secrets or keys are committed",
        ".gitignore covers all required patterns"
      ],
      "permissions": ["Read", "Write", "Edit", "Bash", "Glob", "Grep"],
      "cli_params": "claude --model sonnet --allowedTools Read,Write,Edit,Bash,Glob,Grep --timeout 600",
      "spawn_instance": "SPAWN-007"
    }
  ],
  "risks": [
    "SQLite DROP COLUMN requires SQLite 3.35.0+ — rusqlite bundled version must be checked; if too old, need table rebuild fallback",
    "Concurrent SQLite access through Mutex<Connection> may become a bottleneck — acceptable for this use case but would need connection pooling for production",
    "JWT private key must never be committed to git — generate-keys.sh creates keys in certs/ which is gitignored, but CI must generate fresh keys",
    "Playwright webServer config with cargo run has slow startup due to Rust compilation — CI should use pre-built binary (cargo build --release then run binary directly)",
    "htmx partial rendering requires careful template boundaries — full pages vs fragments must be clearly separated in Askama templates",
    "Super-Linter may have false positives on vendored htmx.min.js — needs FILTER_REGEX_EXCLUDE pattern",
    "Form-based table/column creation uses simple single-column form — may need extension for multi-column table creation (currently creates one column at a time)",
    "rusqlite InvalidParameterName error variant may not exist — may need custom error type for validation errors",
    "Axum middleware signature may differ between axum versions — code is written for axum 0.8, verify API compatibility",
    "Committing test results from CI may cause race conditions if multiple PRs push simultaneously — use unique branch or artifact-only approach as fallback"
  ]
}
```
