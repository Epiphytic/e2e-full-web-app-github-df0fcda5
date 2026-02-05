use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub col_type: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTableForm {
    pub table_name: String,
    pub col_name_1: String,
    pub col_type_1: String,
}

#[derive(Debug, Deserialize)]
pub struct AddColumnRequest {
    pub column_name: String,
    pub column_type: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub token: String,
}
