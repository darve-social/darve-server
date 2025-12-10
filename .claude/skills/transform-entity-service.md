# Transform Old Entity Service to New Architecture

This skill transforms old `*DbService` entity files (single-file pattern with mixed concerns) into the modern three-layer architecture (Entity + Repository Interface + Repository Implementation + Optional Service).

## Overview

**Purpose:** Migrate old entity files like `src/entities/community/community_entity.rs` that contain both entity structs and `*DbService` implementations into the clean layered architecture pattern.

**Process:**
1. Analyze the old entity service file
2. Call `entity-sdb` skill to create base entity structure
3. Migrate database operations to repository implementation
4. Create optional service layer for business logic
5. Replace all usages in codebase
6. Validate with `cargo test` and fix issues

## When to Use This Skill

Use this skill when you encounter:
- Entity files with `*DbService` structs (e.g., `CommunityDbService`, `LocalUserDbService`)
- Mixed concerns: entity definitions + database operations + business logic in one file
- Direct database access patterns using `db: &'a Db, ctx: &'a Ctx`
- Need to modernize to repository pattern

## Repository Pattern Overview

### Understanding Repository Traits

The codebase uses three main repository traits that provide different levels of functionality:

#### 1. RepositoryCore (Base Trait)

**What it provides:**
- Basic CRUD operations: `item_by_id`, `item_by_ident`, `item_create`, `item_delete`
- List operations: `list_by_ids`, `list_by_ident`
- Existence checks: `item_id_exists`, `item_ident_exists`, `items_exist_all`
- View queries: `item_view_by_ident`, `list_view_by_ident`
- Count operations: `count_records`
- Utility: `get_thing(id)` - converts string ID to Thing

**When to use:**
- All custom repository interfaces should extend `RepositoryCore`
- Automatically available on `Repository<E>` for any entity E
- Provides 90% of common database operations

**Example:**
```rust
#[async_trait]
pub trait CommunityRepositoryInterface: RepositoryCore {
    // Custom methods here
    async fn create_profile(...) -> Result<CommunityEntity, surrealdb::Error>;
}

// Usage - inherited methods
let entity = state.db.community.item_by_id("some_id").await?;
let list = state.db.community.list_by_ident(&ident, pagination).await?;
```

#### 2. RepositoryEntityId (Automatic Trait)

**What it provides:**
- `update_entity(entity)` - Updates entity using ID from the entity itself
- `create_update_entity(entity)` - Upserts entity (create or update) using ID from entity

**When to use:**
- Automatically implemented for `Repository<E>` where `E: EntityWithId`
- No need to extend explicitly in your interface
- Useful when you have a complete entity object with ID and want to save it
- Alternative to custom `update(id, fields...)` methods

**Key difference:**
```rust
// Custom update (more common pattern)
async fn update(&self, id: &str, title: &str) -> Result<Entity, surrealdb::Error> {
    // UPDATE with specific fields
}

// RepositoryEntityId update (when you have full entity)
let mut entity = repo.item_by_id("id").await?;
entity.title = "new title".to_string();
let updated = repo.update_entity(entity).await?;  // Uses entity.id_str()

// Upsert pattern
let entity = Entity { id: "custom_id".to_string(), /* ... */ };
let result = repo.create_update_entity(entity).await?;  // Creates or updates
```

**When NOT to use:**
- Don't expose these in your custom interface (they're already there via RepositoryEntityId)
- Don't use if you only want to update specific fields (use custom update method instead)

#### 3. RepositoryEntityView (Separate Pattern)

**What it provides:**
- `get_entity_view(ident)` - Get single view entity
- `list_view(ident, pagination)` - Get list of view entities

**When to use:**
- For complex view-only repositories separate from main entity repository
- Uses `RepositoryView<ViewEntity>` struct (different from `Repository<E>`)
- Currently not widely used in codebase

**Pattern:**
```rust
// Instead of Repository<E>, use RepositoryView<ViewE>
let view_repo = RepositoryView::<ProfileView>::new(client, "local_user".to_string());
let profile = view_repo.get_entity_view(&ident).await?;
```

**Note:** In practice, `RepositoryCore::item_view_by_ident<T>` is more commonly used for views:
```rust
// More common pattern - use generic view on regular repository
let view: ProfileView = state.db.community
    .item_view_by_ident(&IdentIdName::Id(thing))
    .await?;
```

### Repository Implementation Patterns

The codebase uses two distinct repository patterns:

#### Pattern A: Generic Repository<E> (Preferred for Standard Entities)

**Use when:**
- Standard entity table (not a RELATION table)
- Want base CRUD operations from RepositoryCore
- Entity implements EntityWithId trait

**Structure:**
```rust
// Entity with EntityWithId
#[derive(Debug, Serialize, Deserialize)]
pub struct CommunityEntity {
    pub id: String,  // With serde attributes
    // ... fields
}
impl EntityWithId for CommunityEntity { ... }

// Interface extends RepositoryCore
#[async_trait]
pub trait CommunityRepositoryInterface: RepositoryCore {
    // Only custom methods here
    async fn custom_query(...) -> Result<...>;
}

// Implementation on Repository<E>
#[async_trait]
impl CommunityRepositoryInterface for Repository<CommunityEntity> {
    async fn custom_query(...) -> Result<...> { ... }
}

// Registration in Database struct
pub struct Database {
    pub community: Repository<CommunityEntity>,  // Generic Repository
}
```

**Benefits:**
- Get all RepositoryCore methods for free
- Get RepositoryEntityId methods for free
- Consistent API across all entities
- Less boilerplate code

#### Pattern B: Custom Repository Struct (For Special Cases)

**Use when:**
- RELATION tables (many-to-many edges)
- Complex query patterns that don't fit standard CRUD
- Need full control over implementation
- Don't need standard CRUD operations

**Structure:**
```rust
// Custom repository struct
#[derive(Debug)]
pub struct AccessRepository {
    client: Arc<Db>,
}

impl AccessRepository {
    pub fn new(client: Arc<Db>) -> Self { ... }
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> { ... }
}

// Custom interface (may or may not extend RepositoryCore)
#[async_trait]
pub trait AccessRepositoryInterface {
    // Fully custom methods
    async fn add(...) -> AppResult<()>;
    async fn remove(...) -> AppResult<()>;
}

#[async_trait]
impl AccessRepositoryInterface for AccessRepository {
    // Custom implementations
}

// Registration in Database struct
pub struct Database {
    pub access: AccessRepository,  // Custom struct, not Repository<E>
}
```

**When to use:**
```rust
// RELATION tables
DEFINE TABLE access TYPE RELATION IN local_user OUT community ...

// Complex many-to-many operations
RELATE $users->access->$entities SET role=$role;
DELETE $user->access WHERE out IN $entities;
```

### Decision Tree: Which Pattern to Use?

```
Is this a RELATION table (edge table)?
├─ YES → Use Pattern B (Custom Repository Struct)
│   └─ Don't extend RepositoryCore
│   └─ Implement fully custom interface
│
└─ NO → Is this a standard entity table?
    └─ YES → Use Pattern A (Repository<E>)
        ├─ Create Entity with EntityWithId
        ├─ Create Interface extending RepositoryCore
        ├─ Implement custom methods only
        └─ Get base CRUD + update_entity for free
```

### Method Classification for Transformation

When migrating from old `*DbService`, classify each method:

| Method Type | Action | Example |
|-------------|--------|---------|
| `get()`, `get_by_id()` | **Don't reimplement** - Use `RepositoryCore::item_by_id` | `repo.item_by_id(id).await?` |
| `get_view()`, `get_view_by_id()` | **Don't reimplement** - Use `RepositoryCore::item_view_by_ident<T>` | `repo.item_view_by_ident::<View>(&ident).await?` |
| `update()` with specific fields | **Add to interface** - Custom update method | `async fn update(&self, id: &str, title: &str)` |
| `update()` with full entity | **Don't add** - Use `RepositoryEntityId::update_entity` | `repo.update_entity(entity).await?` |
| Custom queries | **Add to interface** - Business-specific method | `async fn find_by_status(&self, status: &str)` |
| Complex transactions | **Add to interface** - Business logic in repository | `async fn create_with_relation(...)` |
| Static helpers | **Add as associated function** - No `&self` | `fn get_profile_id(user_id: &Thing) -> Thing` |

## Skill Workflow

### Phase 1: Analysis and Planning

**Step 1.1: Analyze the Old Entity File**

Read and analyze the old entity file to extract:

```rust
// Example: src/entities/community/community_entity.rs

// 1. ENTITY STRUCT (will become *Entity)
pub struct Community {
    pub id: Thing,
    pub created_at: DateTime<Utc>,  // → r_created
    pub created_by: Thing,
}

// 2. SERVICE STRUCT (will be removed/replaced)
pub struct CommunityDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

// 3. TABLE NAME (will move to repository)
pub const TABLE_NAME: &str = "community";

// 4. METHODS (classify each)
impl<'a> CommunityDbService<'a> {
    pub fn get_table_name() -> &'static str { ... }           // → Keep as const
    pub async fn mutate_db(&self) -> Result<(), AppError> { ... }  // → Repository mutate_db
    pub async fn get(&self, ident: IdentIdName) -> CtxResult<Community> { ... }  // → Use RepositoryCore
    pub async fn get_by_id(&self, id: &str) -> CtxResult<Community> { ... }  // → Use RepositoryCore::item_by_id
    pub async fn create_profile(&self, ...) -> CtxResult<Community> { ... }  // → Custom repository method
    pub fn get_profile_community_id(user_id: &Thing) -> Thing { ... }  // → Static helper, keep in repository
}
```

**Classification Rules:**

| Old Method | New Location | Reasoning |
|------------|--------------|-----------|
| `mutate_db()` | Repository `mutate_db()` | Schema migrations stay with repository |
| `get()`, `get_by_id()` | Use `RepositoryCore` inherited methods | Generic CRUD |
| `get_view()`, `get_view_by_id()` | Use `RepositoryCore` view methods | Generic view queries |
| Custom queries | Repository Interface + Impl | Business-specific queries |
| Complex business logic | Service layer | Multi-step operations, validation, authorization |
| Static helpers | Repository associated functions | Stateless utilities |
| `get_table_name()` | Just use `TABLE_NAME` const | No need for method |

**Step 1.2: Identify Field Transformations**

Map old entity fields to new entity pattern:

```rust
// OLD                           NEW
id: Thing                    →  id: String with deserialize_thing_id/serialize_string_id
created_at: DateTime<Utc>    →  r_created: DateTime<Utc>
created_by: Thing            →  created_by: String with serialize_to_user_thing
some_ref: Thing              →  some_ref: String with serialize_to_{table}_thing
```

**Step 1.3: Determine Service Layer Necessity**

Create service layer if ANY of these apply:
- ✅ Authorization/access control logic
- ✅ Multi-repository coordination (joins, transactions across entities)
- ✅ Complex validation beyond field validation
- ✅ Business rules that aren't pure DB operations
- ✅ Error context enrichment
- ❌ Simple CRUD operations (use repository directly)
- ❌ Single-entity queries (use repository directly)

### Phase 2: Create Base Entity Structure

**Step 2.1: Prepare Entity Information**

Extract from analysis:
- Entity name (e.g., "community")
- Table name (e.g., "community")
- Fields with types
- Foreign key relationships
- Enums (if any)
- Indexes and constraints

**Step 2.2: Call entity-sdb Skill**

Construct a prompt for the entity-sdb skill with all extracted information:

```markdown
Create a new {entity_name} entity with the following:

**Fields:**
- {field1}: {type} ({constraints})
- {field2}: {type} (foreign key to {table})
- ...

**Enums:**
- {EnumName}: {variant1}, {variant2}, ...

**Constraints:**
- Unique index on {field}
- Composite unique on {field1}, {field2}

**Foreign Keys:**
- {field} references {table}
```

This will create:
- `src/entities/{entity_name}.rs`
- `src/interfaces/repositories/{entity_name}_ifce.rs`
- `src/database/repositories/{entity_name}_repo.rs`
- Registration in mod.rs files
- Registration in `src/database/client.rs`

### Phase 3: Migrate Database Operations

**Step 3.1: Migrate mutate_db() Schema**

The `entity-sdb` skill creates a basic schema, but the old `mutate_db()` may have additional fields or constraints.

**Action:** Compare and merge:
1. Read old `mutate_db()` from `*DbService`
2. Read new `mutate_db()` from repository implementation
3. Merge any missing fields, indexes, or constraints
4. Update the repository implementation

```rust
// OLD: src/entities/community/community_entity.rs
pub async fn mutate_db(&self) -> Result<(), AppError> {
    let sql = format!("
        DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
        DEFINE FIELD IF NOT EXISTS created_by ON TABLE {TABLE_NAME} TYPE record<local_user>;
        DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
    ");
    let mutation = self.db.query(sql).await?;
    mutation.check().expect("should mutate domain");
    Ok(())
}

// NEW: src/database/repositories/community_repo.rs
impl Repository<CommunityEntity> {
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
            DEFINE TABLE IF NOT EXISTS {COMMUNITY_TABLE_NAME} SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS created_by ON TABLE {COMMUNITY_TABLE_NAME} TYPE record<local_user>;
            DEFINE FIELD IF NOT EXISTS r_created ON TABLE {COMMUNITY_TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
        ");
        let mutation = self.client.query(sql).await?;
        mutation.check().expect("should mutate community");
        Ok(())
    }
}
```

**Step 3.2: Migrate Custom Methods to Repository**

For each custom method in old `*DbService`:

1. **Add to Repository Interface** (`src/interfaces/repositories/{entity_name}_ifce.rs`):
```rust
#[async_trait]
pub trait CommunityRepositoryInterface: RepositoryCore {
    async fn create_profile(
        &self,
        disc_id: Thing,
        user_id: Thing,
    ) -> Result<CommunityEntity, surrealdb::Error>;

    fn get_profile_community_id(user_id: &Thing) -> Thing;
}
```

2. **Implement in Repository** (`src/database/repositories/{entity_name}_repo.rs`):
```rust
#[async_trait]
impl CommunityRepositoryInterface for Repository<CommunityEntity> {
    async fn create_profile(
        &self,
        disc_id: Thing,
        user_id: Thing,
    ) -> Result<CommunityEntity, surrealdb::Error> {
        // Port logic from old method
        // Remove ctx dependencies
        // Change return type from CtxResult to Result<T, surrealdb::Error>
        let community_id = Self::get_profile_community_id(&user_id);

        let qry = "
            BEGIN TRANSACTION;
                CREATE $disc SET belongs_to=$community, created_by=$user, type=$type;
                RETURN CREATE $community SET created_by=$user;
            COMMIT TRANSACTION;
        ";

        let mut result = self
            .client
            .query(qry)
            .bind(("user", user_id))
            .bind(("disc", disc_id))
            .bind(("type", "Public"))
            .bind(("community", community_id.clone()))
            .await?;

        let comm: Option<CommunityEntity> = result.take(1)?;
        comm.ok_or(surrealdb::Error::from(surrealdb::error::Db::IdNotFound {
            rid: community_id.to_raw(),
        }))
    }

    fn get_profile_community_id(user_id: &Thing) -> Thing {
        Thing::from((COMMUNITY_TABLE_NAME, user_id.id.to_raw()))
    }
}
```

**Key Transformations:**
- Remove `&self.db` → use `&self.client`
- Remove all `ctx` usage (move to service if needed)
- Change `CtxResult<T>` → `Result<T, surrealdb::Error>`
- Change `AppError` → `surrealdb::Error`
- Update entity type: `Community` → `CommunityEntity`

**Step 3.3: Handle Generic CRUD Methods**

Don't reimplement methods available in `RepositoryCore`:

| Old Method | Use Instead |
|------------|-------------|
| `get(ident)` | `item_by_ident(ident)` |
| `get_by_id(id)` | `item_by_id(id)` |
| `get_view(ident)` | `item_view_by_ident::<T>(ident)` |
| `get_view_by_id(id)` | `item_view_by_ident::<T>(&IdentIdName::Id(thing))` |
| `delete(id)` | `item_delete(id)` |
| `create(entity)` | `item_create(entity)` |

### Phase 4: Create Service Layer (If Needed)

**Step 4.1: Identify Business Logic**

Review old methods for:
- Authorization checks (e.g., `if !can_edit(&user) { return Err(Forbidden) }`)
- Validation beyond field validation
- Multi-step operations
- Error context enrichment
- Ctx-dependent operations

**Step 4.2: Create Service File**

Create `src/services/{entity_name}_service.rs`:

```rust
use crate::{
    entities::{entity_name}::{EntityName}Entity,
    interfaces::repositories::{entity_name}_ifce::{EntityName}RepositoryInterface,
    middleware::{
        ctx::Ctx,
        error::{AppError, AppResult, CtxResult},
    },
};
use std::sync::Arc;

pub struct {EntityName}Service<'a, R>
where
    R: {EntityName}RepositoryInterface + Send + Sync,
{
    repository: &'a R,
    ctx: &'a Ctx,
}

impl<'a, R> {EntityName}Service<'a, R>
where
    R: {EntityName}RepositoryInterface + Send + Sync,
{
    pub fn new(repository: &'a R, ctx: &'a Ctx) -> Self {
        Self { repository, ctx }
    }

    // Wrapper methods with error conversion
    pub async fn get_by_id(&self, id: &str) -> AppResult<{EntityName}Entity> {
        self.repository
            .item_by_id(id)
            .await
            .map_err(|e| AppError::SurrealDb { source: e.to_string() })
    }

    // Business logic methods
    pub async fn create_with_validation(&self, data: CreateData) -> CtxResult<{EntityName}Entity> {
        // Validation
        data.validate()?;

        // Authorization
        // ...

        // Call repository
        self.repository.create(...).await
            .map_err(|e| self.ctx.to_ctx_error(AppError::SurrealDb { source: e.to_string() }))
    }
}
```

**Step 4.3: Register Service Module**

Add to `src/services/mod.rs`:
```rust
pub mod {entity_name}_service;
```

### Phase 5: Replace Usages in Codebase

**Step 5.1: Find All Usages**

Search for:
- `use.*{EntityName}DbService`
- `{EntityName}DbService\s*\{`
- `{EntityName}DbService::`

**Step 5.2: Replace Instantiation Patterns**

**Pattern A: Direct Repository Access (No Business Logic)**

```rust
// OLD
let community_repository = CommunityDbService {
    db: &ctx_state.db.client,
    ctx: &ctx,
};
let comm = community_repository.get_by_id(&data.community_id).await?;

// NEW
use crate::database::repository_traits::RepositoryCore;

let comm = ctx_state.db.community
    .item_by_id(&data.community_id)
    .await
    .map_err(|e| ctx.to_ctx_error(AppError::SurrealDb { source: e.to_string() }))?;
```

**Pattern B: Service Access (With Business Logic)**

```rust
// OLD
let community_repository = CommunityDbService {
    db: &ctx_state.db.client,
    ctx: &ctx,
};

// NEW
use crate::services::{entity_name}_service::{EntityName}Service;

let community_service = {EntityName}Service::new(&ctx_state.db.community, &ctx);
let comm = community_service.get_by_id(&data.community_id).await?;
```

**Pattern C: Static Method Calls**

```rust
// OLD
let community_id = CommunityDbService::get_profile_community_id(&user.id);

// NEW
use crate::interfaces::repositories::community_ifce::CommunityRepositoryInterface;

let community_id = CommunityRepositoryInterface::get_profile_community_id(&user.id);
```

**Step 5.3: Update Struct Fields in Services**

If old entity service is used in another service struct:

```rust
// OLD: src/services/discussion_service.rs
pub struct DiscussionService<'a> {
    community_repository: CommunityDbService<'a>,
}

// NEW
use crate::database::repository_impl::Repository;
use crate::entities::community::CommunityEntity;
use crate::interfaces::repositories::community_ifce::CommunityRepositoryInterface;

pub struct DiscussionService<'a, C>
where
    C: CommunityRepositoryInterface + Send + Sync,
{
    community_repository: &'a C,
}

impl<'a, C> DiscussionService<'a, C>
where
    C: CommunityRepositoryInterface + Send + Sync,
{
    pub fn new(
        state: &'a CtxState,
        ctx: &'a Ctx,
        // ... other params
    ) -> Self {
        Self {
            community_repository: &state.db.community,
            // ... other fields
        }
    }
}
```

**Step 5.4: Update Imports**

Replace imports throughout:

```rust
// OLD
use crate::entities::community::community_entity::{Community, CommunityDbService, TABLE_NAME};

// NEW
use crate::entities::community::CommunityEntity;
use crate::interfaces::repositories::community_ifce::CommunityRepositoryInterface;
use crate::database::repositories::community_repo::COMMUNITY_TABLE_NAME;
// Or if using service:
use crate::services::community_service::CommunityService;
```

### Phase 6: Cleanup Old Code

**Step 6.1: Remove Old Service Struct**

From old entity file (e.g., `src/entities/community/community_entity.rs`):
- Remove `pub struct {Entity}DbService<'a> { ... }`
- Remove `impl<'a> {Entity}DbService<'a> { ... }`
- Remove `pub const TABLE_NAME: &str = ...;`

**Step 6.2: Update or Remove Old Entity Struct**

Option A: Remove old entity entirely (if not used elsewhere)
Option B: Keep with deprecation warning temporarily
Option C: Create type alias for compatibility

```rust
// Option C: Compatibility alias
#[deprecated(note = "Use CommunityEntity instead")]
pub type Community = CommunityEntity;
```

**Step 6.3: Clean Up Imports in Old File**

Remove unused imports from the old entity file.

### Phase 7: Validation and Testing

**Step 7.1: Compile Check**

```bash
cargo check
```

Fix any compilation errors:
- Missing imports
- Type mismatches
- Method signature changes
- Lifetime issues

**Step 7.2: Run Tests**

```bash
cargo test
```

**Step 7.3: Analyze Test Failures**

Common failure patterns:

| Error | Cause | Fix |
|-------|-------|-----|
| "no field `{entity}` on `Database`" | Repository not registered | Add to Database struct in client.rs |
| "trait bound not satisfied" | Missing trait implementation | Ensure repository implements interface |
| "method not found" | Wrong method name | Use RepositoryCore methods or add custom |
| "mismatched types Thing vs String" | Entity field type mismatch | Update serde attributes |
| "cannot borrow as mutable" | Lifetime issues | Adjust service/repository lifetimes |

**Step 7.4: Fix Database Initialization**

If tests fail with "repository not found":

```rust
// src/database/client.rs - ensure all added:

// 1. Import
use crate::entities::community::CommunityEntity;
use crate::database::repositories::community_repo::COMMUNITY_TABLE_NAME;

// 2. Field in struct
pub struct Database {
    pub community: Repository<CommunityEntity>,
    // ...
}

// 3. Initialize
impl Database {
    pub async fn connect(config: DbConfig<'_>) -> Self {
        Self {
            community: Repository::<CommunityEntity>::new(
                client.clone(),
                COMMUNITY_TABLE_NAME.to_string(),
            ),
            // ...
        }
    }

    pub async fn run_migrations(&self) -> Result<(), AppError> {
        self.community.mutate_db().await?;
        // ...
    }
}
```

**Step 7.5: Fix Tests That Use Old Pattern**

Update test files:

```rust
// OLD: tests/community_tests.rs
let comm_service = CommunityDbService {
    db: &test_state.db.client,
    ctx: &ctx,
};

// NEW
use crate::database::repository_traits::RepositoryCore;
let comm = test_state.db.community.item_by_id("test_id").await.unwrap();
```

**Step 7.6: Iterative Testing**

Run tests repeatedly, fixing issues until all pass:

```bash
cargo test -- --nocapture
```

For each failure:
1. Identify the specific test and error
2. Determine root cause (missing method, wrong type, etc.)
3. Apply appropriate fix from the patterns above
4. Re-run tests
5. Repeat until all tests pass

**Step 7.7: Integration Testing**

After all unit tests pass, test the actual application:

```bash
# Start server
cargo run

# Test API endpoints manually or with integration tests
```

### Phase 8: Final Verification

**Step 8.1: Code Review Checklist**

- [ ] Entity struct in `src/entities/{name}.rs` with `EntityWithId` trait
- [ ] Repository interface in `src/interfaces/repositories/{name}_ifce.rs`
- [ ] Repository implementation in `src/database/repositories/{name}_repo.rs`
- [ ] All methods migrated from old service
- [ ] `mutate_db()` has complete schema
- [ ] Service layer created if business logic exists
- [ ] All modules registered in mod.rs files
- [ ] Database struct has repository field
- [ ] Repository initialized in `Database::connect()`
- [ ] `mutate_db()` called in `Database::run_migrations()`
- [ ] All old usages replaced
- [ ] Old service code removed
- [ ] All imports updated
- [ ] `cargo check` passes
- [ ] `cargo test` passes
- [ ] No deprecation warnings
- [ ] No unused imports

**Step 8.2: Documentation**

Add doc comments to key items:

```rust
/// Repository interface for Community entities.
///
/// Provides database operations for communities, including profile creation.
#[async_trait]
pub trait CommunityRepositoryInterface: RepositoryCore {
    /// Creates a profile community for a user.
    async fn create_profile(...) -> Result<CommunityEntity, surrealdb::Error>;
}
```

**Step 8.3: Performance Check**

Ensure no performance regressions:
- Query patterns are similar
- Transactions preserved
- Indexes still in place

## Example: Complete Transformation

### Input: Old Pattern

```rust
// src/entities/community/community_entity.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::database::client::Db;
use crate::middleware::ctx::Ctx;
use crate::middleware::error::{AppError, CtxResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct Community {
    pub id: Thing,
    pub created_at: DateTime<Utc>,
    pub created_by: Thing,
}

pub struct CommunityDbService<'a> {
    pub db: &'a Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "community";

impl<'a> CommunityDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
            DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS created_by ON TABLE {TABLE_NAME} TYPE record<local_user>;
            DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TABLE_NAME} TYPE datetime DEFAULT time::now();
        ");
        let mutation = self.db.query(sql).await?;
        mutation.check().expect("should mutate community");
        Ok(())
    }

    pub async fn get_by_id(&self, id: &str) -> CtxResult<Community> {
        let thing = Thing::from((TABLE_NAME, id));
        let opt: Option<Community> = self.db.select((thing.tb, thing.id.to_raw())).await?;
        opt.ok_or(self.ctx.to_ctx_error(AppError::EntityFailIdNotFound { ident: id.to_string() }))
    }

    pub fn get_profile_community_id(user_id: &Thing) -> Thing {
        Thing::from((TABLE_NAME, user_id.id.to_raw()))
    }
}
```

### Output: New Pattern

**File 1: `src/entities/community.rs`**
```rust
use crate::database::repository_traits::EntityWithId;
use crate::utils::validate_utils::{
    deserialize_thing_id, serialize_string_id, serialize_to_user_thing,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CommunityEntity {
    #[serde(deserialize_with = "deserialize_thing_id")]
    #[serde(serialize_with = "serialize_string_id")]
    pub id: String,

    #[serde(deserialize_with = "deserialize_thing_id")]
    #[serde(serialize_with = "serialize_to_user_thing")]
    pub created_by: String,

    pub r_created: DateTime<Utc>,
}

impl EntityWithId for CommunityEntity {
    fn id_str(&self) -> Option<&str> {
        match self.id.is_empty() {
            true => None,
            false => Some(self.id.as_ref()),
        }
    }
}
```

**File 2: `src/interfaces/repositories/community_ifce.rs`**
```rust
use crate::{
    database::repository_traits::RepositoryCore,
    entities::community::CommunityEntity,
};
use async_trait::async_trait;
use surrealdb::sql::Thing;

#[async_trait]
pub trait CommunityRepositoryInterface: RepositoryCore {
    fn get_profile_community_id(user_id: &Thing) -> Thing;
}
```

**File 3: `src/database/repositories/community_repo.rs`**
```rust
use crate::database::repository_impl::Repository;
use crate::{
    entities::community::CommunityEntity,
    interfaces::repositories::community_ifce::CommunityRepositoryInterface,
    middleware::error::AppError,
};
use async_trait::async_trait;
use surrealdb::sql::Thing;

pub const COMMUNITY_TABLE_NAME: &str = "community";

impl Repository<CommunityEntity> {
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
            DEFINE TABLE IF NOT EXISTS {COMMUNITY_TABLE_NAME} SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS created_by ON TABLE {COMMUNITY_TABLE_NAME} TYPE record<local_user>;
            DEFINE FIELD IF NOT EXISTS r_created ON TABLE {COMMUNITY_TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
        ");
        let mutation = self.client.query(sql).await?;
        mutation.check().expect("should mutate community");
        Ok(())
    }
}

#[async_trait]
impl CommunityRepositoryInterface for Repository<CommunityEntity> {
    fn get_profile_community_id(user_id: &Thing) -> Thing {
        Thing::from((COMMUNITY_TABLE_NAME, user_id.id.to_raw()))
    }
}
```

**Usage replacement:**
```rust
// OLD
let community_repo = CommunityDbService {
    db: &state.db.client,
    ctx: &ctx,
};
let comm = community_repo.get_by_id(&id).await?;

// NEW
use crate::database::repository_traits::RepositoryCore;

let comm = state.db.community
    .item_by_id(&id)
    .await
    .map_err(|e| ctx.to_ctx_error(AppError::SurrealDb { source: e.to_string() }))?;
```

## Common Patterns and Solutions

### Pattern: Update Operations

#### Using RepositoryEntityId (When You Have Full Entity)
```rust
// Pattern: Fetch, modify, save
pub async fn update_profile(&self, user_id: &str, new_data: ProfileUpdate) -> AppResult<User> {
    // Get full entity
    let mut user = self.repository.item_by_id(user_id).await
        .map_err(|e| AppError::SurrealDb { source: e.to_string() })?;

    // Modify fields
    user.full_name = new_data.full_name;
    user.bio = new_data.bio;

    // Save using RepositoryEntityId::update_entity
    let updated = self.repository.update_entity(user).await
        .map_err(|e| AppError::SurrealDb { source: e.to_string() })?;

    Ok(updated)
}
```

#### Using Custom Update Method (When Updating Specific Fields)
```rust
// In repository interface
#[async_trait]
pub trait UserRepositoryInterface: RepositoryCore {
    async fn update_profile(&self, id: &str, name: &str, bio: Option<String>)
        -> Result<UserEntity, surrealdb::Error>;
}

// In repository implementation
async fn update_profile(&self, id: &str, name: &str, bio: Option<String>)
    -> Result<UserEntity, surrealdb::Error> {
    let thing = self.get_thing(id);
    let qry = "UPDATE $id SET full_name=$name, bio=$bio;";

    let mut res = self.client
        .query(qry)
        .bind(("id", thing))
        .bind(("name", name))
        .bind(("bio", bio))
        .await?;

    let data: UserEntity = res.take::<Option<UserEntity>>(0)?.expect("record updated");
    Ok(data)
}
```

#### Upsert Pattern (Create or Update)
```rust
// Useful when you want specific IDs (like user profiles)
pub async fn ensure_profile(&self, user_id: &str) -> AppResult<Profile> {
    let profile = Profile {
        id: format!("profile:{}", user_id),  // Deterministic ID
        user_id: user_id.to_string(),
        created_at: Utc::now(),
        // ... other fields with defaults
    };

    // Will create if doesn't exist, update if exists
    let result = self.repository.create_update_entity(profile).await
        .map_err(|e| AppError::SurrealDb { source: e.to_string() })?;

    Ok(result)
}
```

### Pattern: Error Conversion

```rust
// Repository level (return surrealdb::Error)
async fn create(...) -> Result<Entity, surrealdb::Error> { ... }

// Service level (convert to AppError)
pub async fn create(...) -> AppResult<Entity> {
    self.repository.create(...)
        .await
        .map_err(|e| AppError::SurrealDb { source: e.to_string() })
}

// Route level with Ctx (convert to CtxResult)
async fn handler(...) -> CtxResult<Json<Entity>> {
    let entity = service.create(...).await
        .map_err(|e| ctx.to_ctx_error(e))?;
    Ok(Json(entity))
}
```

### Pattern: View Queries

```rust
// OLD
pub async fn get_view_by_id<T>(&self, id: &str) -> CtxResult<T>
where T: ViewFieldSelector + for<'de> Deserialize<'de> { ... }

// NEW - Use RepositoryCore::item_view_by_ident
use crate::middleware::utils::db_utils::IdentIdName;
use crate::database::repository_traits::RepositoryCore;

let thing = Thing::from(("community", id));
let view: ProfileView = state.db.community
    .item_view_by_ident(&IdentIdName::Id(thing))
    .await?
    .ok_or(AppError::EntityFailIdNotFound { ident: id.to_string() })?;
```

### Pattern: Transactions

```rust
// Keep transactions in repository, use bind parameters
async fn create_with_relation(...) -> Result<Entity, surrealdb::Error> {
    let qry = "
        BEGIN TRANSACTION;
            CREATE $table1 SET field=$val1;
            CREATE $table2 SET field=$val2;
        COMMIT TRANSACTION;
    ";

    let mut res = self.client
        .query(qry)
        .bind(("table1", thing1))
        .bind(("val1", value1))
        .bind(("table2", thing2))
        .bind(("val2", value2))
        .await?;

    let entity: Entity = res.take::<Option<Entity>>(0)?.expect("record created");
    Ok(entity)
}
```

## Troubleshooting

### Issue: "Type annotations needed"

**Cause:** Generic method without explicit type parameter

**Fix:**
```rust
// Add turbofish operator
let entity: EntityType = repo.item_by_id(id).await?;
// Or
let entity = repo.item_by_id::<EntityType>(id).await?;
```

### Issue: "Method not found in RepositoryCore"

**Cause:** Custom method not added to interface

**Fix:** Add method to repository interface trait and implement it

### Issue: "Lifetime errors in service struct"

**Cause:** Incorrect lifetime annotations

**Fix:**
```rust
pub struct MyService<'a, R>
where
    R: MyRepositoryInterface + Send + Sync,
{
    repository: &'a R,  // Reference lifetime
    ctx: &'a Ctx,       // Same lifetime
}
```

### Issue: "Tests fail with EntityFailIdNotFound"

**Cause:** Test data not created or wrong IDs

**Fix:** Ensure test fixtures create entities before querying

### Issue: "Database field not found"

**Cause:** Repository not registered in Database struct

**Fix:** Add field, initialization, and migration call in client.rs

## Summary

This skill provides a complete, systematic approach to transforming old entity service files into the modern layered architecture. By following these phases:

1. **Analyze** the old file structure
2. **Generate** base entity files with entity-sdb skill
3. **Migrate** database operations to repository
4. **Create** optional service layer for business logic
5. **Replace** all usages in the codebase
6. **Validate** with cargo test
7. **Fix** any issues iteratively

You can successfully modernize legacy entity code while maintaining functionality and passing all tests.
