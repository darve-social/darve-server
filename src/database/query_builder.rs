use surrealdb::engine::any::Any;
use surrealdb::method::Query;
use surrealdb::types::{SurrealValue, Variables};

use crate::database::client::Db;

pub struct SurrealQueryBuilder {
    pub sql: String,
    pub variables: Variables,
}

impl SurrealQueryBuilder {
    pub fn new(initial_sql: impl Into<String>) -> Self {
        Self {
            sql: initial_sql.into(),
            variables: Variables::new(),
        }
    }

    pub fn query(mut self, sql: impl Into<String>) -> Self {
        self.sql.push('\n');
        self.sql.push_str(&sql.into());
        self
    }

    pub fn bind_var(mut self, key: impl Into<String>, value: impl SurrealValue) -> Self {
        self.variables.insert(key.into(), value);
        self
    }

    pub fn into_db_query<'a>(self, db: &'a Db) -> Query<'a, Any> {
        db.query(self.sql).bind(self.variables)
    }
}
