# Entity Creation Skill for SurrealDB

Create new entities for the darve-server project following the established three-layer architecture pattern.

## Architecture Overview

Every entity consists of three components:
1. **Entity struct** in `src/entities/`
2. **Repository interface trait** in `src/interfaces/repositories/`
3. **Repository implementation** in `src/database/repositories/`

## Instructions

When creating a new entity, follow these steps systematically:

### Step 1: Create the Entity Struct

**Location:** `src/entities/{entity_name}.rs`

**Required Components:**
- Entity struct with derives: `Debug, Serialize, Deserialize`
- ID field with SurrealDB serialization attributes
- Business logic fields
- Timestamp field `r_created: DateTime<Utc>`
- Implement `EntityWithId` trait
- Create associated enums if needed

**Template:**
```rust
use crate::database::repository_traits::EntityWithId;
use crate::utils::validate_utils::{
    deserialize_thing_id, serialize_string_id, serialize_to_{related}_thing,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct {EntityName}Entity {
    #[serde(deserialize_with = "deserialize_thing_id")]
    #[serde(serialize_with = "serialize_string_id")]
    pub id: String,

    // Add business fields here
    // For foreign key references use:
    // #[serde(deserialize_with = "deserialize_thing_id")]
    // #[serde(serialize_with = "serialize_to_{table}_thing")]
    // pub {field}: String,

    pub r_created: DateTime<Utc>,
}

impl EntityWithId for {EntityName}Entity {
    fn id_str(&self) -> Option<&str> {
        match self.id.is_empty() {
            true => None,
            false => Some(self.id.as_ref()),
        }
    }
}

// Optional: Create associated enums if needed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum {EntityName}Status {
    Active,
    Inactive,
}
```

### Step 2: Create the Repository Interface

**Location:** `src/interfaces/repositories/{entity_name}_ifce.rs`

**Required Components:**
- Async trait extending `RepositoryCore`
- Standard CRUD methods returning `Result<T, surrealdb::Error>`
- Custom query methods as needed

**Template:**
```rust
use crate::{
    database::repository_traits::RepositoryCore,
    entities::{entity_name}::{EntityName}Entity,
};
use async_trait::async_trait;

#[async_trait]
pub trait {EntityName}RepositoryInterface: RepositoryCore {
    // Get by ID (inherited from RepositoryCore, but can override)

    // Create
    async fn create(
        &self,
        // Add parameters
    ) -> Result<{EntityName}Entity, surrealdb::Error>;

    // Update
    async fn update(
        &self,
        id: &str,
        // Add update parameters
    ) -> Result<{EntityName}Entity, surrealdb::Error>;

    // Delete
    async fn delete(&self, id: &str) -> Result<(), surrealdb::Error>;

    // Custom queries
    async fn get_by_{custom_field}(
        &self,
        {field}: &str,
    ) -> Result<Vec<{EntityName}Entity>, surrealdb::Error>;
}
```

### Step 3: Create the Repository Implementation

**Location:** `src/database/repositories/{entity_name}_repo.rs`

**Required Components:**
- `mutate_db()` method for schema definition
- Implementation of interface trait
- Use SurrealDB query builder with bind()
- Use `Thing::from()` for record references

**Template:**
```rust
use crate::database::repository_impl::Repository;
use crate::database::repository_traits::RepositoryCore;
use crate::{
    entities::{entity_name}::{EntityName}Entity,
    interfaces::repositories::{entity_name}_ifce::{EntityName}RepositoryInterface,
    middleware::error::AppError,
};
use async_trait::async_trait;
use surrealdb::sql::Thing;

pub const {ENTITY_NAME}_TABLE_NAME: &str = "{entity_name}";

impl Repository<{EntityName}Entity> {
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
        DEFINE TABLE IF NOT EXISTS {{{ENTITY_NAME}_TABLE_NAME}} SCHEMAFULL;

        -- Define fields
        DEFINE FIELD IF NOT EXISTS {field1} ON TABLE {{{ENTITY_NAME}_TABLE_NAME}} TYPE {type};
        DEFINE FIELD IF NOT EXISTS {field2} ON TABLE {{{ENTITY_NAME}_TABLE_NAME}} TYPE {type} DEFAULT {default};

        -- For foreign keys use: TYPE record<{table_name}>
        DEFINE FIELD IF NOT EXISTS {fk_field} ON TABLE {{{ENTITY_NAME}_TABLE_NAME}} TYPE record<{related_table}>;

        -- Timestamp
        DEFINE FIELD IF NOT EXISTS r_created ON TABLE {{{ENTITY_NAME}_TABLE_NAME}} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();

        -- Define indexes
        DEFINE INDEX IF NOT EXISTS {unique_field}_idx ON TABLE {{{ENTITY_NAME}_TABLE_NAME}} COLUMNS {field} UNIQUE;
        ");

        let mutation = self.client.query(sql).await?;
        mutation.check().expect("should mutate {entity_name}");

        Ok(())
    }
}

#[async_trait]
impl {EntityName}RepositoryInterface for Repository<{EntityName}Entity> {
    // IMPORTANT: Prefer using RepositoryCore inherited methods inside function implementations where possible instead of custom queries.
    
    async fn create(
        &self,
        // Add parameters
    ) -> Result<{EntityName}Entity, surrealdb::Error> {
        let qry = format!("
            CREATE {{{ENTITY_NAME}_TABLE_NAME}} SET
                {field1}=$field1,
                {field2}=$field2;
        ");

        let mut res = self
            .client
            .query(qry)
            .bind(("{field1}", value1))
            .bind(("{field2}", value2))
            .await?;

        let data: {EntityName}Entity = res
            .take::<Option<{EntityName}Entity>>(0)?
            .expect("record created");

        Ok(data)
    }

    async fn update(
        &self,
        id: &str,
        // Add parameters
    ) -> Result<{EntityName}Entity, surrealdb::Error> {
        let thing = self.get_thing(id);
        let qry = "UPDATE $id SET {field}=$value;";

        let mut res = self
            .client
            .query(qry)
            .bind(("id", thing))
            .bind(("value", value))
            .await?;

        let data: {EntityName}Entity = res
            .take::<Option<{EntityName}Entity>>(0)?
            .expect("record updated");

        Ok(data)
    }

    async fn delete(&self, id: &str) -> Result<(), surrealdb::Error> {
        let thing = self.get_thing(id);
        let _: Option<{EntityName}Entity> = self
            .client
            .delete((thing.tb, thing.id.to_raw()))
            .await?;
        Ok(())
    }

    async fn get_by_{custom_field}(
        &self,
        {field}: &str,
    ) -> Result<Vec<{EntityName}Entity>, surrealdb::Error> {
        let qry = format!(
            "SELECT * FROM {{{ENTITY_NAME}_TABLE_NAME}} WHERE {field} = ${field};"
        );

        let mut res = self
            .client
            .query(qry)
            .bind(("{field}", {field}.to_string()))
            .await?;

        let data: Vec<{EntityName}Entity> = res.take(0)?;
        Ok(data)
    }
}
```

### Step 4: Register the Entity

1. **Add to `src/entities/mod.rs`:**
   ```rust
   pub mod {entity_name};
   ```

2. **Add to `src/interfaces/repositories/mod.rs`:**
   ```rust
   pub mod {entity_name}_ifce;
   ```

3. **Add to `src/database/repositories/mod.rs`:**
   ```rust
   pub mod {entity_name}_repo;
   ```

4. **Add to `src/database/client.rs` - Database struct** (import and add field):
   ```rust
   // At the top, add import
   use crate::database::repositories::{entity_name}_repo::{ENTITY_NAME}_TABLE_NAME;
   use crate::entities::{entity_name}::{EntityName}Entity;

   // In Database struct, add field
   pub struct Database {
       pub client: Arc<Surreal<Any>>,
       pub {entity_name}: Repository<{EntityName}Entity>,
       // ... other fields
   }
   ```

5. **Initialize in `Database::connect` method** in `src/database/client.rs`:
   ```rust
   Self {
       client: client.clone(),
       {entity_name}: Repository::<{EntityName}Entity>::new(
           client.clone(),
           {ENTITY_NAME}_TABLE_NAME.to_string(),
       ),
       // ... other repositories
   }
   ```

6. **Add to `run_migrations` method** in `src/database/client.rs`:
   ```rust
   pub async fn run_migrations(&self) -> Result<(), AppError> {
       self.{entity_name}.mutate_db().await?;
       // ... other migrations
       Ok(())
   }
   ```

## Important Rules

### SurrealDB Field Types
- `string` - For String fields
- `number` - For numeric fields (i32, u8, f64, etc.)
- `bool` - For boolean fields
- `datetime` - For DateTime<Utc> fields
- `record<{table}>` - For foreign key references
- `option<{type}>` - For Optional fields

### Foreign Key References
When referencing another table:
```rust
// In entity
#[serde(deserialize_with = "deserialize_thing_id")]
#[serde(serialize_with = "serialize_to_{table}_thing")]
pub {field}: String,

// In schema
DEFINE FIELD IF NOT EXISTS {field} ON TABLE {table} TYPE record<{referenced_table}>;

// In query
.bind(("{field}", Thing::from(({TABLE_NAME}, id))))
```

### Unique Constraints
```rust
DEFINE INDEX IF NOT EXISTS {field}_idx ON TABLE {table} COLUMNS {field} UNIQUE;

// For composite unique constraints
DEFINE INDEX IF NOT EXISTS {name}_idx ON TABLE {table} COLUMNS {field1}, {field2} UNIQUE;
```

### Transactions
For operations requiring atomicity:
```rust
let qry = format!("
    BEGIN TRANSACTION;
        DELETE FROM {table} WHERE condition;
        CREATE {table} SET field=$value;
    COMMIT TRANSACTION;
");
```

### Error Handling
- Use `surrealdb::Error` for repository methods
- Use `AppError` for `mutate_db()` methods
- Return `IdNotFound` error when entity doesn't exist:
  ```rust
  None => Err(surrealdb::Error::from(surrealdb::error::Db::IdNotFound {
      rid: format!("id={id}"),
  }))
  ```

### Query Result Extraction
```rust
// Single result
let data: Option<Entity> = res.take(0)?;

// Multiple results
let data: Vec<Entity> = res.take(0)?;

// With expectation
let data: Entity = res.take::<Option<Entity>>(0)?.expect("message");
```

## Common Patterns

### Increment Field
```rust
async fn increase_{field}(&self, id: &str) -> Result<(), surrealdb::Error> {
    let thing = self.get_thing(id);
    let res = self
        .client
        .query("UPDATE $id SET {field} += 1;")
        .bind(("id", thing))
        .await?;
    res.check()?;
    Ok(())
}
```

### Get or Create Pattern
```rust
let qry = format!("
    BEGIN TRANSACTION;
        DELETE FROM {table} WHERE user = $user_id AND use_for = $use_for;
        CREATE {table} SET user=$user_id, code=$code;
    COMMIT TRANSACTION;
");
```

### Conditional Updates
```rust
UPDATE {table} SET status = $status WHERE id = $id AND current_status = $old_status;
```

## Checklist

When creating a new entity, verify:
- [ ] Entity struct created in `src/entities/`
- [ ] EntityWithId trait implemented correctly
- [ ] Interface trait created in `src/interfaces/repositories/`
- [ ] Repository implementation created in `src/database/repositories/`
- [ ] `mutate_db()` method defines complete schema
- [ ] All fields have proper TYPE definitions
- [ ] Indexes created for unique constraints
- [ ] Foreign keys use `record<table>` type
- [ ] All modules registered in mod.rs files
- [ ] Entity imports added to `src/database/client.rs`
- [ ] Repository field added to Database struct in `src/database/client.rs`
- [ ] Repository initialized in `Database::connect()` method
- [ ] `mutate_db()` called in `Database::run_migrations()` method
- [ ] Timestamp field `r_created` included
- [ ] Error handling follows project patterns
- [ ] Query bindings used instead of string interpolation

## Example Usage

To create a new "task" entity, ask:
> "Create a new task entity with fields: title (string), description (string), status (enum: Pending/InProgress/Completed), assigned_to (user reference), and due_date (optional datetime)"

The skill will generate all three files following the patterns above.
