use serde::{Deserialize, Serialize};

/// A device product in the catalog (brand, model, specs).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Product {
    pub id: i64,
    pub brand: String,
    pub model: String,
    pub name: Option<String>,
    pub cpu: Option<String>,
    pub gpu: Option<String>,
    pub link: Option<String>,
    pub coverage: Option<i64>,
}

/// Request body for creating a new product.
#[derive(Debug, Deserialize)]
pub struct CreateProductRequest {
    pub brand: String,
    pub model: String,
    pub name: Option<String>,
    pub cpu: Option<String>,
    pub gpu: Option<String>,
    pub link: Option<String>,
    pub coverage: Option<i64>,
}

/// Request body for updating an existing product.
#[derive(Debug, Deserialize)]
pub struct UpdateProductRequest {
    pub brand: Option<String>,
    pub model: Option<String>,
    pub name: Option<String>,
    pub cpu: Option<String>,
    pub gpu: Option<String>,
    pub link: Option<String>,
    pub coverage: Option<i64>,
}
