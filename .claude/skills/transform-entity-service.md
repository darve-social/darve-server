# Transform Old Entity Service to New Architecture

This skill transforms old `*DbService` entity files (single-file pattern with mixed concerns) into the modern three-layer architecture (Entity + Repository Interface + Repository Implementation + Optional Service).

## Overview

**Purpose:** Migrate old entity files like `src/entities/community/community_entity.rs` that contain both entity structs and `*DbService` implementations into the clean layered architecture pattern.

## ⚠️ CRITICAL WARNING: DO NOT RENAME FIELDS

**THE #1 RULE OF ENTITY TRANSFORMATION:**

**NEVER rename struct fields, database field names, or query field names from the old code.**

This is the most common cause of transformation failures:
- ❌ Changing `created_at` to `r_created` breaks view models
- ❌ Renaming any field breaks existing queries and views
- ❌ Unit tests will pass, integration tests will fail mysteriously with 400 errors
- ✅ Always keep the EXACT same field names from old entity
- ✅ Preserve all field names in schema, queries, entities, and views

**Before starting transformation, read the old entity carefully and commit to memory all field names. These are sacred and must not change.**

See the detailed section "CRITICAL: Field Name Mismatch Between Schema and Views" (line 2004) for full explanation.

**Process:**
1. Analyze the old entity service file
2. Call `entity-sdb` skill to create base entity structure
3. Migrate database operations to repository implementation **KEEPING ALL FIELD NAMES IDENTICAL**
4. Create optional service layer for business logic
5. Replace all usages in codebase (including test files)
6. **Cleanup: DELETE old entity file completely** - only ONE entity file should remain
7. **ALWAYS run full `cargo test` (NOT `cargo test --lib`)** and fix issues iteratively
8. Final verification

**Result:** After completion, you should have:
- ✅ ONE entity file: `src/entities/{entity_name}.rs`
- ✅ ONE repository interface: `src/interfaces/repositories/{entity_name}_ifce.rs`
- ✅ ONE repository implementation: `src/database/repositories/{entity_name}_repo.rs`
- ❌ NO old entity file (deleted): `src/entities/{entity_name}/{entity_name}_entity.rs`

**⚠️ CRITICAL: Always run FULL test suite with `cargo test` (no flags). NEVER rely on just `cargo test --lib` for validation. Integration tests are required to catch serialization and field mismatch bugs that unit tests miss.**

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
    pub created_at: DateTime<Utc>,  // → KEEP AS created_at (DO NOT RENAME!)
    pub created_by: Thing,
}

// Example with Option<Thing> ID (common pattern)
pub struct SomeEntity {
    pub id: Option<Thing>,  // → Will become: id: Thing (unwrap Option!)
    pub name: String,
    pub owner: Thing,
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

**⚠️ CRITICAL: DO NOT RENAME FIELD NAMES!**

Map old entity fields to new entity pattern, **KEEPING THE EXACT SAME FIELD NAMES**:

```rust
// OLD                           NEW (TYPE CHANGES ONLY, NAMES STAY IDENTICAL!)
id: Thing                    →  id: String with deserialize_thing_id/serialize_string_id
id: Option<Thing>            →  id: Thing  // ✅ Unwrap Option, keep as Thing!
created_at: DateTime<Utc>    →  created_at: DateTime<Utc>  // ✅ NAME UNCHANGED!
created_by: Thing            →  created_by: String with serialize_to_user_thing
some_ref: Thing              →  some_ref: String with serialize_to_{table}_thing

// ❌ WRONG - DO NOT DO THIS:
created_at: DateTime<Utc>    →  r_created: DateTime<Utc>  // ❌ Field name changed!
id: Option<Thing>            →  id: String  // ❌ Wrong! Should stay as Thing, not String
```

**Rule:** Only change the Rust type, never the field name itself:
- `Thing` → `String` (with appropriate serde attributes)
- `Option<Thing>` → `Thing` (unwrap the Option, keep as Thing for ID fields)
- Never change field names! The new entity field names must match:
  - Old entity field names (for backwards compatibility)
  - View model field names (for deserialization)
  - Query SELECT field names (for database queries)
  - Schema DEFINE FIELD names (for schema definitions)

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
- **⚠️ CRITICAL: Fields with EXACT names from old entity (DO NOT RENAME!)**
- Foreign key relationships
- Enums (if any)
- Indexes and constraints

**IMPORTANT:** When extracting field information, copy the EXACT field names from the old entity. Do not "improve" or "standardize" field names. If the old entity has `created_at`, use `created_at`. If it has `r_created`, use `r_created`. Field name consistency is more important than naming conventions.

**Step 2.2: Call entity-sdb Skill**

**⚠️ WARNING:** When calling the entity-sdb skill, use the EXACT field names from the old entity!

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
// ⚠️ CRITICAL: Keep field name "created_at" EXACTLY THE SAME!
impl Repository<CommunityEntity> {
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
            DEFINE TABLE IF NOT EXISTS {COMMUNITY_TABLE_NAME} SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS created_by ON TABLE {COMMUNITY_TABLE_NAME} TYPE record<local_user>;
            DEFINE FIELD IF NOT EXISTS created_at ON TABLE {COMMUNITY_TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
            // ✅ Field name is "created_at" - same as old code!
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
- `{EntityName}DbService\\s*\\{`
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

**Step 5.5: Find and Catalog All Test Files**

This is a critical step often overlooked. Test files need updating just like production code.

**Action 1: Search for test files using the old entity service**

Run these searches to find all test files:

```bash
# Search for test modules
rg "{EntityName}DbService" --type rust -g "*test*.rs"
rg "{EntityName}DbService" --type rust -g "tests/"

# Search for #[cfg(test)] modules
rg "mod tests" -A 50 | rg "{EntityName}DbService"

# Search for test functions
rg "#\[test\]" -B 5 -A 20 | rg "{EntityName}DbService"
```

**Action 2: Create a test file checklist**

Document all test files that need updating:

```markdown
## Test Files to Update

### Unit Tests (in src/)
- [ ] src/entities/{entity_name}/tests.rs
- [ ] src/services/{related_service}/tests.rs
- [ ] ...

### Integration Tests (in tests/)
- [ ] tests/{entity_name}_tests.rs
- [ ] tests/integration/{entity_name}_integration.rs
- [ ] ...

### Test Utilities
- [ ] tests/common/mod.rs (test helpers)
- [ ] tests/fixtures/{entity_name}_fixtures.rs
- [ ] ...
```

**Action 3: Identify test helper functions**

Look for:
- Setup functions: `setup_test_db()`, `create_test_{entity}()`
- Fixture functions: `make_test_{entity}()`
- Assertion helpers: `assert_{entity}_equal()`
- Mock/fake implementations

These will need signature updates matching the new patterns.

### Phase 6: Cleanup Old Code

**⚠️ CRITICAL GOAL: After Phase 6, there should be EXACTLY ONE entity file, not two!**

**Step 6.1: Identify Files to Remove**

After transformation, you should have:
- ✅ **ONE** new entity file: `src/entities/{entity_name}.rs`
- ❌ **ZERO** old entity files: `src/entities/{entity_name}/{entity_name}_entity.rs` should be deleted

Check for duplicate entity files:
```bash
# Find all entity files for the entity
find src/entities -name "*{entity_name}*" -type f

# Example output showing TWO files (BAD):
# src/entities/task/task_request_entity.rs  ← OLD FILE, DELETE THIS
# src/entities/task_request.rs              ← NEW FILE, KEEP THIS
```

**Step 6.2: Verify All Helper Structs Migrated**

Before deleting the old entity file, ensure all helper structs and enums are in the new entity file:

```rust
// Check old file has these helper items:
// - TaskRequestCreate
// - TaskDonorForReward
// - TaskParticipantUserView
// - TaskParticipantForReward
// - TaskForReward
// - Enums: TaskRequestStatus, TaskRequestType, RewardType, DeliverableType

// Verify new file has ALL of them:
// src/entities/{entity_name}.rs should contain ALL helper structs and enums
```

**Step 6.3: Delete Old Entity File Completely**

Remove the entire old entity file:

```bash
# Example
rm src/entities/task/task_request_entity.rs
```

If the old entity was in a subdirectory that's now empty, remove the directory too:

```bash
# If src/entities/task/ is now empty, remove it
rmdir src/entities/task/
```

**Step 6.4: Update Module Registrations**

Remove old module registration from `src/entities/mod.rs`:

```rust
// OLD - Remove this
pub mod task {
    pub mod task_request_entity;
}

// NEW - Should only have this
pub mod task_request;
```

Or if using flat module structure:

```rust
// Remove old nested module
// pub mod task;  ← DELETE THIS LINE

// Keep new flat module
pub mod task_request;
```

**Step 6.5: Update All Imports Referencing Old Entity File**

Search and replace all imports:

```bash
# Find all imports of old entity
rg "use.*entities::task::task_request_entity" --type rust

# Replace with new import
# OLD: use crate::entities::task::task_request_entity::{TaskRequest, TABLE_NAME};
# NEW: use crate::entities::task_request::{TaskRequestEntity, ...};
```

**Step 6.6: Final Verification - Ensure Single Entity File**

Run this check to confirm only ONE entity file exists:

```bash
# Should show ONLY the new entity file
find src/entities -name "*{entity_name}*" -type f

# Example CORRECT output (only ONE file):
# src/entities/task_request.rs  ✅

# Example WRONG output (TWO files):
# src/entities/task/task_request_entity.rs  ❌
# src/entities/task_request.rs              ❌
```

**Step 6.7: Compile Check After Cleanup**

```bash
cargo check
```

Fix any remaining import errors pointing to the old entity file location.

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

**Step 7.2: Test Compilation Check**

Before running tests, ensure they compile:

```bash
cargo test --no-run
```

This compiles test code without running it, catching issues faster.

**⚠️ IMPORTANT: After fixing compilation, you MUST run `cargo test` (full suite), NOT `cargo test --lib`. See Step 7.4 for why this is critical.**

**Step 7.3: Pre-Test Checklist**

Before running `cargo test`, verify:

- [ ] All test imports updated from old entity service to new repository/service
- [ ] Test database initialization includes new repository migrations
- [ ] Test setup functions updated to use new repository pattern
- [ ] Test fixture/helper functions updated with new signatures
- [ ] Assertions updated for new entity types (Thing → String)
- [ ] Mock implementations updated if using dependency injection

**Step 7.4: Run Full Test Suite - MANDATORY TESTING STEPS**

**⚠️ CRITICAL: You MUST run the full test suite. Do NOT skip this or rely only on unit tests!**

**The ONLY acceptable test command after entity transformation:**

```bash
# THIS IS REQUIRED - Run FULL test suite including integration tests
cargo test

# Optional: Capture output for analysis
cargo test 2>&1 | tee test_output.txt
```

**❌ DO NOT DO THIS - Unit tests alone are insufficient:**

```bash
# ❌ WRONG - This will miss critical bugs!
cargo test --lib
```

**Why `cargo test --lib` is NOT enough:**

| Test Type | Command | What It Tests | Limitations |
|-----------|---------|---------------|-------------|
| Unit Tests | `cargo test --lib` | Code logic in isolation | Doesn't test actual HTTP endpoints, deserialization, or real database queries |
| Integration Tests | `cargo test` (no flags) | Full application stack including HTTP routes, serialization, database | Slower but catches real-world issues |

**Common Pitfall:** Unit tests may pass while integration tests fail due to:
- **Field name mismatches** between schema and view models (see below)
- Serialization/deserialization errors
- HTTP request/response handling issues
- Database constraint violations
- Missing error conversions

**❌ CRITICAL ERROR: Never assume the transformation is complete after only `cargo test --lib` passes!**

**✅ CORRECT: Entity transformation is ONLY complete when `cargo test` (full suite) passes with zero failures.**

**Step 7.5: Update Test Files - Comprehensive Patterns**

This section provides detailed patterns for updating different types of test code.

#### Pattern A: Basic Test Setup - Repository Instantiation

```rust
// OLD: Test setup with DbService
#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::community::community_entity::{Community, CommunityDbService};

    async fn setup() -> (Db, Ctx) {
        let db = test_db().await;
        let ctx = test_ctx();
        (db, ctx)
    }

    #[tokio::test]
    async fn test_get_community() {
        let (db, ctx) = setup().await;
        let service = CommunityDbService { db: &db, ctx: &ctx };

        let result = service.get_by_id("test_id").await;
        assert!(result.is_ok());
    }
}

// NEW: Test setup with Repository
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        entities::community::CommunityEntity,
        database::{
            repository_impl::Repository,
            repository_traits::RepositoryCore,
        },
    };

    async fn setup() -> Repository<CommunityEntity> {
        let db = test_db().await;
        Repository::<CommunityEntity>::new(
            Arc::new(db),
            "community".to_string()
        )
    }

    #[tokio::test]
    async fn test_get_community() {
        let repo = setup().await;

        let result = repo.item_by_id("test_id").await;
        assert!(result.is_ok());
    }
}
```

#### Pattern B: Tests Using CRUD Operations

```rust
// OLD: Generic get/create/delete
#[tokio::test]
async fn test_community_lifecycle() {
    let service = CommunityDbService { db: &db, ctx: &ctx };

    // Create
    let community = service.create(...).await.unwrap();

    // Get
    let fetched = service.get_by_id(&community.id.to_string()).await.unwrap();

    // Delete
    service.delete(&community.id.to_string()).await.unwrap();
}

// NEW: Using RepositoryCore methods
#[tokio::test]
async fn test_community_lifecycle() {
    let repo = setup_repo().await;

    // Create
    let community = repo.item_create(CommunityEntity { /* ... */ }).await.unwrap();

    // Get - note the entity type has String id now
    let fetched = repo.item_by_id(&community.id).await.unwrap();

    // Delete
    repo.item_delete(&community.id).await.unwrap();
}
```

#### Pattern C: Test Helper Functions - Signature Updates

```rust
// OLD: Helper function creating test entities
async fn create_test_community(
    db: &Db,
    ctx: &Ctx,
    user_id: &Thing,
) -> Community {
    let service = CommunityDbService { db, ctx };
    service.create_profile(
        Thing::from(("discussion", "test")),
        user_id.clone(),
    ).await.unwrap()
}

// NEW: Helper using repository directly
async fn create_test_community(
    repo: &Repository<CommunityEntity>,
    user_id: &Thing,
) -> CommunityEntity {
    repo.create_profile(
        Thing::from(("discussion", "test")),
        user_id.clone(),
    ).await.unwrap()
}
```

#### Pattern D: Test Fixtures - Type Updates

```rust
// OLD: Fixture with Thing types
fn make_test_community(id: Thing) -> Community {
    Community {
        id,
        created_by: Thing::from(("local_user", "test_user")),
        created_at: Utc::now(),
    }
}

// NEW: Fixture with String types
fn make_test_community(id: &str) -> CommunityEntity {
    CommunityEntity {
        id: id.to_string(),
        created_by: "test_user".to_string(),  // Serde will convert to Thing
        created_at: Utc::now(),  // ✅ Same field name as old entity
    }
}
```

#### Pattern E: Integration Tests with Multiple Entities

```rust
// OLD: Integration test with multiple services
#[tokio::test]
async fn test_community_with_discussions() {
    let (db, ctx) = setup().await;
    let comm_service = CommunityDbService { db: &db, ctx: &ctx };
    let disc_service = DiscussionDbService { db: &db, ctx: &ctx };

    let comm = comm_service.create(...).await.unwrap();
    let disc = disc_service.create(comm.id.clone(), ...).await.unwrap();

    assert_eq!(disc.belongs_to, comm.id);
}

// NEW: Integration test with multiple repositories
#[tokio::test]
async fn test_community_with_discussions() {
    let test_state = setup_test_state().await;

    let comm = test_state.db.community.item_create(...).await.unwrap();
    let disc = test_state.db.discussion.item_create(
        DiscussionEntity {
            belongs_to: comm.id.clone(),  // Now String, not Thing
            // ...
        }
    ).await.unwrap();

    assert_eq!(disc.belongs_to, comm.id);
}
```

#### Pattern F: Mock/Fake Repository Implementations

```rust
// OLD: No mocking, used actual DbService
// Tests were tightly coupled to database

// NEW: Mock repository for unit testing services
#[cfg(test)]
mod tests {
    use mockall::mock;

    mock! {
        CommunityRepo {}

        #[async_trait]
        impl RepositoryCore for CommunityRepo {
            async fn item_by_id(&self, id: &str) -> Result<CommunityEntity, surrealdb::Error>;
            // ... other RepositoryCore methods
        }

        #[async_trait]
        impl CommunityRepositoryInterface for CommunityRepo {
            fn get_profile_community_id(user_id: &Thing) -> Thing;
        }
    }

    #[tokio::test]
    async fn test_service_with_mock() {
        let mut mock_repo = MockCommunityRepo::new();
        mock_repo
            .expect_item_by_id()
            .returning(|_| Ok(make_test_community("test_id")));

        let ctx = test_ctx();
        let service = CommunityService::new(&mock_repo, &ctx);

        let result = service.get_by_id("test_id").await;
        assert!(result.is_ok());
    }
}
```

#### Pattern G: Assertion Updates for Type Changes

```rust
// OLD: Assertions with Thing comparisons
#[tokio::test]
async fn test_community_owner() {
    let service = CommunityDbService { db: &db, ctx: &ctx };
    let user_thing = Thing::from(("local_user", "user_123"));

    let comm = service.create_profile(..., user_thing.clone()).await.unwrap();

    assert_eq!(comm.created_by, user_thing);
    assert_eq!(comm.created_by.tb, "local_user");
    assert_eq!(comm.created_by.id.to_string(), "user_123");
}

// NEW: Assertions with String (Thing comparison removed)
#[tokio::test]
async fn test_community_owner() {
    let repo = setup_repo().await;
    let user_thing = Thing::from(("local_user", "user_123"));

    let comm = repo.create_profile(..., user_thing.clone()).await.unwrap();

    // created_by is now String, but serde deserializes from Thing
    assert_eq!(comm.created_by, "user_123");
    // Can't check .tb anymore, it's just a string
}
```

#### Pattern H: Test Database Initialization

```rust
// OLD: Test database setup
async fn setup_test_db() -> Db {
    let db = connect_test_db().await;

    // Manual schema creation
    let community_service = CommunityDbService { db: &db, ctx: &test_ctx() };
    community_service.mutate_db().await.unwrap();

    db
}

// NEW: Test database setup with migrations
async fn setup_test_db() -> Database {
    let db_client = connect_test_db().await;

    // Use Database struct which includes all repositories
    let database = Database::connect(test_config()).await;

    // Run all migrations including new repository
    database.run_migrations().await.unwrap();

    database
}
```

#### Pattern I: Tests with Static Methods

```rust
// OLD: Static method on DbService
#[test]
fn test_get_profile_community_id() {
    let user_thing = Thing::from(("local_user", "user_123"));
    let comm_id = CommunityDbService::get_profile_community_id(&user_thing);

    assert_eq!(comm_id.tb, "community");
    assert_eq!(comm_id.id.to_string(), "user_123");
}

// NEW: Static method on Repository Interface
#[test]
fn test_get_profile_community_id() {
    use crate::interfaces::repositories::community_ifce::CommunityRepositoryInterface;
    use crate::database::repository_impl::Repository;
    use crate::entities::community::CommunityEntity;

    let user_thing = Thing::from(("local_user", "user_123"));

    // Call via trait (can't use :: on trait, need concrete type)
    let comm_id = Repository::<CommunityEntity>::get_profile_community_id(&user_thing);

    assert_eq!(comm_id.tb, "community");
    assert_eq!(comm_id.id.to_string(), "user_123");
}
```

#### Pattern J: API Endpoint Tests with Entity IDs

**⚠️ CRITICAL: ID Format in API Endpoint Tests After Entity Transformation**

After transforming entities from Thing to String IDs, integration tests that call HTTP endpoints need special attention. The entity's `.id` field stores just the ID part (without table prefix), but API endpoints expect the full "table:id" format.

**Common Error:**
```
Error: "error into Thing" when calling /api/tasks/{task_id}/accept
Status: 400 Bad Request
```

This happens because:
1. `TaskRequestEntity.id` is a String containing just the ID part: `"01KCETG9RT7..."`
2. API endpoints parse the path parameter as a full Thing ID: `"task_request:01KCETG9RT7..."`
3. Service methods use `get_str_thing(task_id)?` which expects the full format
4. Tests using `task.id` directly fail because they're missing the table prefix

```rust
// ❌ WRONG: API endpoint test after entity transformation
#[tokio::test]
async fn test_accept_task() {
    let server = setup_test_server().await;

    // Create task via API
    let task = server
        .post("/api/posts/some_post/tasks")
        .json(&json!({
            "content": "Do something",
            "participants": vec!["user123"]
        }))
        .await
        .json::<TaskRequestEntity>();

    // ❌ FAILS: task.id is "01KCETG9RT7..." (no table prefix)
    server
        .post(&format!("/api/tasks/{}/accept", task.id))
        .await
        .assert_status_success();  // FAILS with "error into Thing"

    // ❌ ALSO FAILS: .as_str() doesn't add table prefix
    server
        .post(&format!("/api/tasks/{}/accept", task.id.as_str()))
        .await
        .assert_status_success();  // FAILS with "error into Thing"
}

// ✅ CORRECT: Add table prefix to entity ID for API calls
#[tokio::test]
async fn test_accept_task() {
    let server = setup_test_server().await;

    // Create task via API
    let task = server
        .post("/api/posts/some_post/tasks")
        .json(&json!({
            "content": "Do something",
            "participants": vec!["user123"]
        }))
        .await
        .json::<TaskRequestEntity>();

    // ✅ CORRECT: Format with table prefix
    server
        .post(&format!("/api/tasks/{}/accept", format!("task_request:{}", task.id)))
        .await
        .assert_status_success();

    // Alternative: Create a helper function
    fn task_api_id(task: &TaskRequestEntity) -> String {
        format!("task_request:{}", task.id)
    }

    server
        .post(&format!("/api/tasks/{}/deliver", task_api_id(&task)))
        .json(&json!({"post_id": "some_post"}))
        .await
        .assert_status_success();
}
```

**Pattern: Multiple API Calls with Same Entity**

```rust
// OLD: Thing-based ID worked everywhere
#[tokio::test]
async fn test_task_lifecycle() {
    let task = create_task_via_api().await;

    // All these worked with task.id (Thing)
    accept_task(&task.id).await;
    deliver_task(&task.id).await;
    get_task(&task.id).await;
}

// NEW: String-based ID needs table prefix for API calls
#[tokio::test]
async fn test_task_lifecycle() {
    let task = create_task_via_api().await;

    // Helper function to avoid repetition
    let task_id = format!("task_request:{}", task.id);

    // All API calls use the formatted ID
    accept_task(&task_id).await;
    deliver_task(&task_id).await;
    get_task(&task_id).await;
}
```

**When This Pattern Applies:**

| Test Type | Needs Table Prefix? | Reason |
|-----------|---------------------|--------|
| API endpoint tests (HTTP routes) | ✅ YES | Routes parse path params as full Thing IDs |
| Direct repository tests | ❌ NO | Repository methods accept just the ID part |
| Service layer tests | ❌ NO | Services call repositories which use ID part |
| Database query tests | Depends | If building Thing in query, use full format |

**Quick Checklist for API Endpoint Tests:**

After entity transformation, search for patterns like:
- `format!("/api/tasks/{}", task.id)` ← ❌ Needs table prefix
- `format!("/api/posts/{}", post.id)` ← ❌ Needs table prefix
- `&task.id.as_str()` in API paths ← ❌ Needs table prefix
- `&task.id` in API paths ← ❌ Needs table prefix

Fix by wrapping with:
```rust
format!("{table_name}:{}", entity.id)
```

**Common Endpoints Requiring Table Prefix:**

```rust
// Task endpoints
format!("/api/tasks/{}/accept", format!("task_request:{}", task.id))
format!("/api/tasks/{}/reject", format!("task_request:{}", task.id))
format!("/api/tasks/{}/deliver", format!("task_request:{}", task.id))
format!("/api/tasks/{}/donor", format!("task_request:{}", task.id))

// Post endpoints
format!("/api/posts/{}/like", format!("post:{}", post.id))
format!("/api/posts/{}/replies", format!("post:{}", post.id))

// Discussion endpoints
format!("/api/discussions/{}/posts", format!("discussion:{}", disc.id))
format!("/api/discussions/{}/tasks", format!("discussion:{}", disc.id))
```

**Step 7.6: Iterative Test Fixing Workflow**

This is the core of getting tests to pass. Follow this systematic approach:

#### Iteration 1: Initial Test Run and Categorization

**Action 1.1: Run tests and save output**
```bash
cargo test 2>&1 | tee test_output.txt
```

**Action 1.2: Analyze and categorize failures**

Create categories for the failures you see:

```markdown
## Test Failure Categories

### Category 1: Compilation Errors (Fix First)
- Missing imports: 15 tests
- Type mismatches: 8 tests
- Method not found: 12 tests
- Lifetime errors: 3 tests

### Category 2: Runtime Errors
- EntityNotFound: 5 tests
- SurrealDB errors: 7 tests
- Assertion failures: 4 tests
- "error into Thing" (API endpoint tests): See Pattern J

### Category 2a: API Endpoint ID Format Errors (Common After Transformation)
- "error into Thing" with 400 status: Tests using entity.id in API paths
- Fix: Add table prefix - format!("{table}:{}", entity.id)

### Category 3: Test Setup Issues
- Database not initialized: 10 tests
- Missing migrations: 6 tests
```

#### Iteration 2: Fix Compilation Errors (Category 1)

**Action 2.1: Fix missing imports**

Search for all import errors and fix systematically:

```bash
# Find all files with import errors
rg "unresolved import.*{EntityName}DbService" --type rust

# Replace in each file
# OLD import
use crate::entities::community::community_entity::{Community, CommunityDbService};

# NEW import
use crate::{
    entities::community::CommunityEntity,
    database::repository_traits::RepositoryCore,
};
```

**Action 2.2: Fix type mismatches**

Common type errors and fixes:

```rust
// Error: expected Thing, found String
// OLD
let comm_id: Thing = community.id;

// NEW
let comm_id: String = community.id;
// Or if you need a Thing:
let comm_id = Thing::from(("community", community.id.as_str()));

// Error: expected String, found Thing
// OLD
let thing = Thing::from(("community", "123"));
repo.item_by_id(thing);

// NEW
let thing = Thing::from(("community", "123"));
repo.item_by_id(&thing.id.to_string());
// Or just:
repo.item_by_id("123");
```

**Action 2.3: Fix "method not found" errors**

Replace old service methods with new repository methods:

```rust
// Error: method `get_by_id` not found
// OLD
service.get_by_id(id).await

// NEW
repo.item_by_id(id).await

// Error: method `get_view_by_id` not found
// OLD
service.get_view_by_id::<ProfileView>(id).await

// NEW
use crate::middleware::utils::db_utils::IdentIdName;
let thing = Thing::from(("community", id));
repo.item_view_by_ident::<ProfileView>(&IdentIdName::Id(thing)).await
```

**Action 2.4: Fix lifetime errors**

Common lifetime issues in test helpers:

```rust
// Error: lifetime mismatch in helper function
// OLD
async fn create_community(db: &Db, ctx: &Ctx) -> Community {
    let service = CommunityDbService { db, ctx };
    service.create(...).await.unwrap()
}

// NEW - remove ctx, use repo directly
async fn create_community(repo: &Repository<CommunityEntity>) -> CommunityEntity {
    repo.item_create(CommunityEntity { /* ... */ }).await.unwrap()
}
```

**Action 2.5: Compile tests again**

```bash
cargo test --no-run
```

Repeat until compilation succeeds.

#### Iteration 3: Fix Runtime Errors (Category 2)

**Action 3.0: Fix "error into Thing" API Endpoint Errors (MOST COMMON)**

**⚠️ This is the #1 runtime error after entity transformation!**

**Symptoms:**
- Test fails with `400 Bad Request` status
- Error message: `{"error":"error into Thing"}`
- Happens on API endpoint tests (POST/GET/DELETE to `/api/...`)
- Specifically when using entity IDs in URL paths

**Root Cause:**
Entity IDs are now Strings (just the ID part), but API endpoints expect full Thing format (`table:id`).

**Quick Fix:**
Search for all API endpoint calls in tests and add table prefix:

```bash
# Find all potential issues
rg 'format!\("/api/tasks/\{[^}]*\}"' tests/
rg 'format!\("/api/posts/\{[^}]*\}"' tests/
rg '\.post\(&format!\("/api/' tests/
```

**Fix Pattern:**
```rust
// ❌ BEFORE (causes "error into Thing")
server.post(&format!("/api/tasks/{}/accept", task.id)).await

// ✅ AFTER (works correctly)
server.post(&format!("/api/tasks/{}/accept", format!("task_request:{}", task.id))).await
```

**See Pattern J (above) for comprehensive examples and explanation.**

**Action 3.1: Fix EntityNotFound errors**

Often caused by:
- Test data not created
- Wrong table name
- ID format mismatch

```rust
// Common fix: ensure test data exists
#[tokio::test]
async fn test_get_community() {
    let repo = setup_repo().await;

    // OLD: assumed entity exists
    // let comm = repo.item_by_id("test_id").await.unwrap();

    // NEW: create entity first
    let created = repo.item_create(make_test_community("test_id")).await.unwrap();
    let fetched = repo.item_by_id(&created.id).await.unwrap();

    assert_eq!(created.id, fetched.id);
}
```

**Action 3.2: Fix SurrealDB errors**

Common database errors:

```rust
// Error: "Table not found"
// Cause: Migration not run
// Fix: Ensure mutate_db() is called in test setup

async fn setup_test_db() -> Database {
    let db = Database::connect(test_config()).await;

    // MUST run migrations
    db.run_migrations().await.unwrap();

    db
}

// Error: "Field constraint violation"
// Cause: Required field missing
// Fix: Provide all required fields in test data

fn make_test_community(id: &str) -> CommunityEntity {
    CommunityEntity {
        id: id.to_string(),
        created_by: "test_user".to_string(),  // Was missing
        created_at: Utc::now(),                // Was missing - same field name as old entity!
    }
}
```

**Action 3.3: Fix assertion failures**

Update assertions for new types:

```rust
// Error: assertion failed: `(left == right)` left: `"user_123"`, right: `Thing {...}`
// OLD
assert_eq!(community.created_by, Thing::from(("local_user", "user_123")));

// NEW
assert_eq!(community.created_by, "user_123");
```

#### Iteration 4: Fix Test Setup Issues (Category 3)

**Action 4.1: Fix database initialization**

Ensure all test setup functions use new Database struct:

```rust
// OLD: Per-test database setup
#[tokio::test]
async fn test_something() {
    let db = Surreal::new::<Mem>(()).await.unwrap();
    // ... manual setup
}

// NEW: Centralized test database setup
// In tests/common/mod.rs or similar
pub async fn setup_test_database() -> Database {
    let db = Database::connect(DbConfig {
        endpoint: "memory",
        namespace: "test",
        database: "test",
    }).await;

    db.run_migrations().await.unwrap();
    db
}

// In test
#[tokio::test]
async fn test_something() {
    let test_db = setup_test_database().await;
    let result = test_db.community.item_by_id("test").await;
    // ...
}
```

**Action 4.2: Fix missing migrations**

Add new repository migration to test setup:

```rust
// In src/database/client.rs
impl Database {
    pub async fn run_migrations(&self) -> Result<(), AppError> {
        // ... existing migrations

        // ADD THIS - new repository migration
        self.community.mutate_db().await?;

        Ok(())
    }
}
```

#### Iteration 5: Re-run and Verify

**Action 5.1: Run full test suite**

```bash
cargo test -- --nocapture
```

**Action 5.2: Track progress**

Keep track of passing vs failing tests:

```bash
# Get test count
cargo test 2>&1 | grep "test result"

# Example output:
# test result: FAILED. 45 passed; 12 failed; 0 ignored; 0 measured; 0 filtered out
```

Create a progress tracker:

```markdown
## Test Fix Progress

### Iteration 1
- Total: 57 tests
- Passing: 12
- Failing: 45

### Iteration 2
- Total: 57 tests
- Passing: 32
- Failing: 25

### Iteration 3
- Total: 57 tests
- Passing: 50
- Failing: 7

### Iteration 4
- Total: 57 tests
- Passing: 57
- Failing: 0 ✅
```

#### Iteration 6: Check for Warnings

After all tests pass, clean up warnings:

```bash
cargo test 2>&1 | grep "warning"
```

Common warnings to fix:
- Unused imports
- Unused variables
- Deprecated code paths
- Dead code

**Step 7.7: Test-Specific Debugging Techniques**

When individual tests fail, use these techniques:

#### Technique 1: Run Single Test with Full Output

```bash
# Run specific test
cargo test test_community_create -- --exact --nocapture

# Run all tests in a module
cargo test community_tests --nocapture

# Run tests matching pattern
cargo test community --nocapture
```

#### Technique 2: Use RUST_BACKTRACE for Detailed Errors

```bash
# Full backtrace
RUST_BACKTRACE=1 cargo test test_name -- --exact

# Full backtrace (more detail)
RUST_BACKTRACE=full cargo test test_name -- --exact
```

#### Technique 3: Run Tests Serially to Avoid Database Conflicts

```bash
# Run tests one at a time (prevents parallel DB access issues)
cargo test -- --test-threads=1
```

This is crucial if tests share database state or have race conditions.

#### Technique 4: Add Debug Output to Tests

```rust
#[tokio::test]
async fn test_community_create() {
    let repo = setup_repo().await;

    // Debug: print what we're creating
    let test_entity = make_test_community("test_123");
    println!("Creating entity: {:?}", test_entity);

    let result = repo.item_create(test_entity).await;

    // Debug: print result
    println!("Create result: {:?}", result);

    assert!(result.is_ok());
}
```

Run with `--nocapture` to see output.

#### Technique 5: Inspect Test Database State

```rust
#[tokio::test]
async fn test_community_lifecycle() {
    let test_db = setup_test_database().await;

    // Create entity
    let comm = test_db.community.item_create(...).await.unwrap();

    // Debug: Query raw database to see what's there
    let raw_query = test_db.client.query("SELECT * FROM community").await.unwrap();
    println!("Raw DB state: {:?}", raw_query);

    // Continue test...
}
```

#### Technique 6: Use Test Fixtures for Complex Data

```rust
// In tests/common/fixtures.rs
pub struct CommunityFixture {
    pub repo: Repository<CommunityEntity>,
    pub test_user: String,
}

impl CommunityFixture {
    pub async fn new(db: &Database) -> Self {
        let repo = &db.community;
        let test_user = "test_user_123".to_string();

        Self {
            repo: repo.clone(),
            test_user,
        }
    }

    pub async fn create_test_community(&self, id: &str) -> CommunityEntity {
        self.repo.item_create(CommunityEntity {
            id: id.to_string(),
            created_by: self.test_user.clone(),
            created_at: Utc::now(),  // ✅ Same field name as old entity
        }).await.unwrap()
    }
}

// In test
#[tokio::test]
async fn test_with_fixture() {
    let test_db = setup_test_database().await;
    let fixture = CommunityFixture::new(&test_db).await;

    let comm = fixture.create_test_community("test_123").await;
    // ...
}
```

#### Technique 7: Isolate Failing Tests

If many tests fail, comment out all but one:

```rust
#[cfg(test)]
mod tests {
    // #[tokio::test]
    // async fn test_1() { ... }

    #[tokio::test]  // Only run this one
    async fn test_2() { ... }

    // #[tokio::test]
    // async fn test_3() { ... }
}
```

Fix one test at a time, then uncomment others.

**Step 7.8: Cargo Test Command Reference**

Quick reference for common test scenarios:

**⚠️ REMEMBER: For entity transformation validation, you MUST use `cargo test` (no flags) to run the full suite!**

```bash
# === Compilation ===
# Compile tests without running
cargo test --no-run

# === Running Tests ===
# Run all tests (REQUIRED for entity transformation validation!)
cargo test

# Run all tests with output
cargo test -- --nocapture

# Run specific test by exact name
cargo test test_community_create -- --exact

# Run all tests matching pattern
cargo test community

# Run all tests in a module
cargo test community_tests::

# Run tests from specific file/integration test
cargo test --test community_integration

# === Debugging ===
# Show full backtrace
RUST_BACKTRACE=1 cargo test

# Show very detailed backtrace
RUST_BACKTRACE=full cargo test

# Run tests serially (no parallelism)
cargo test -- --test-threads=1

# Show output even for passing tests
cargo test -- --show-output

# === Filtering ===
# ⚠️ WARNING: These filtered test runs are for debugging ONLY, NOT for validation!
# Run only unit tests (in src/) - DO NOT use this for entity transformation validation!
cargo test --lib

# Run only integration tests (in tests/)
cargo test --tests

# Run only doc tests
cargo test --doc

# === Performance ===
# Run tests in release mode (faster, less debug info)
cargo test --release

# === Specific Crates ===
# Run tests for specific crate in workspace
cargo test -p darve-server

# === Continuous Testing ===
# Re-run tests on file changes (requires cargo-watch)
cargo watch -x test

# Re-run specific test on changes
cargo watch -x "test test_community_create -- --exact"

# === Coverage ===
# Generate test coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html

# === Common Combinations ===
# Debug single failing test
RUST_BACKTRACE=1 cargo test test_name -- --exact --nocapture --test-threads=1

# Quick check all tests compile
cargo test --no-run --quiet

# Run tests with verbose compiler output
cargo test --verbose

# Ignore test failures and continue
cargo test --no-fail-fast
```

### Phase 8: Final Verification

**Step 8.1: Code Review Checklist**

**File Structure:**
- [ ] **CRITICAL: Only ONE entity file exists** - `src/entities/{name}.rs` (not two!)
- [ ] Old entity file completely deleted (e.g., `src/entities/{name}/{name}_entity.rs`)
- [ ] Old subdirectory removed if empty (e.g., `src/entities/{name}/`)
- [ ] Entity struct in `src/entities/{name}.rs` with `EntityWithId` trait
- [ ] All helper structs migrated to new entity file (Create structs, view structs, enums)

**Repository Layer:**
- [ ] Repository interface in `src/interfaces/repositories/{name}_ifce.rs`
- [ ] Repository implementation in `src/database/repositories/{name}_repo.rs`
- [ ] All methods migrated from old service
- [ ] `mutate_db()` has complete schema

**Service Layer:**
- [ ] Service layer created if business logic exists

**Integration:**
- [ ] All modules registered in mod.rs files
- [ ] Old module registrations removed from mod.rs
- [ ] Database struct has repository field
- [ ] Repository initialized in `Database::connect()`
- [ ] `mutate_db()` called in `Database::run_migrations()`

**Code Updates:**
- [ ] All old usages replaced in production code
- [ ] All imports updated (no references to old entity file path)
- [ ] All test files updated
- [ ] All test helper functions updated
- [ ] All test fixtures updated
- [ ] Test database setup includes new migrations
- [ ] Old service code removed (DbService struct and impl)

**Validation:**
- [ ] `cargo check` passes
- [ ] `cargo test` passes with 0 failures
- [ ] `cargo test` runs with 0 warnings
- [ ] No deprecation warnings
- [ ] No unused imports
- [ ] No dead code warnings

**Final File Count Verification:**
- [ ] Run `find src/entities -name "*{entity_name}*" -type f` shows EXACTLY ONE file

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

**Step 8.4: Final Test Run**

One last comprehensive test run:

```bash
# Clean build
cargo clean

# Full rebuild and test (THIS IS MANDATORY - NO --lib FLAG!)
cargo test --release -- --nocapture

# Also run in dev mode to catch any differences
cargo test

# Check for any warnings
cargo test 2>&1 | grep -i "warning"
```

**✅ Transformation is ONLY complete when:**
- `cargo test` (full suite, no flags) passes with **0 failures**
- `cargo test` shows **0 warnings**
- All integration tests pass (not just unit tests)

**❌ Transformation is NOT complete if:**
- You only ran `cargo test --lib`
- Integration tests were skipped
- Any test failures remain

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

    // ⚠️ CRITICAL: Keep field name "created_at" - same as old entity!
    pub created_at: DateTime<Utc>,  // ✅ NOT r_created!
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
        // ⚠️ CRITICAL: Field name is "created_at" - same as old entity!
        let sql = format!("
            DEFINE TABLE IF NOT EXISTS {COMMUNITY_TABLE_NAME} SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS created_by ON TABLE {COMMUNITY_TABLE_NAME} TYPE record<local_user>;
            DEFINE FIELD IF NOT EXISTS created_at ON TABLE {COMMUNITY_TABLE_NAME} TYPE datetime DEFAULT time::now() VALUE $before OR time::now();
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

### Example: Entity with Option<Thing> ID

Some old entities use `Option<Thing>` for the ID field. When transforming, unwrap the Option and keep it as Thing.

**Input: Old Pattern**
```rust
// src/entities/wallet/wallet_entity.rs
#[derive(Debug, Serialize, Deserialize)]
pub struct Wallet {
    pub id: Option<Thing>,  // ← Note: Option<Thing>
    pub owner: Thing,
    pub balance: i64,
}
```

**Output: New Pattern**
```rust
// src/entities/wallet.rs
use surrealdb::sql::Thing;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WalletEntity {
    pub id: Thing,  // ✅ Unwrapped Option, kept as Thing (NOT String!)
    pub owner: Thing,
    pub balance: i64,
}

impl EntityWithId for WalletEntity {
    fn id_str(&self) -> Option<&str> {
        Some(self.id.id.to_raw().as_str())
    }
}
```

**Key Points:**
- `Option<Thing>` → `Thing` (unwrap the Option, keep as Thing)
- Do NOT convert to String for Option<Thing> fields
- The EntityWithId implementation uses `self.id.id.to_raw()` instead of `self.id.as_ref()`

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

## Critical Issues and Resolutions

### CRITICAL: Field Name Mismatch Between Schema and Views

**This is the most common and dangerous issue in entity transformations.**

#### The Problem

When transforming entities, the repository schema may use different field names than what view models expect. This causes **silent deserialization failures** that:

- ✅ Pass unit tests (because they don't hit real database/HTTP layer)
- ❌ Fail integration tests with generic "Internal error" or 400 Bad Request
- Are difficult to debug because error messages don't indicate the root cause

#### Real-World Example

**The Bug:**
```rust
// Repository schema - uses r_created
// src/database/repositories/task_request_repo.rs
let sql = format!("
    DEFINE FIELD IF NOT EXISTS r_created ON TABLE {TABLE_NAME} TYPE datetime...
");

// Query returns r_created
let query = format!("SELECT {fields}, r_created FROM {TABLE_NAME}...");

// BUT View model expects created_at!
// src/models/view/task.rs
pub struct TaskRequestView {
    pub created_at: DateTime<Utc>,  // Wrong! Database has r_created
    // ...
}
```

**What happens:**
1. Database stores data with field `r_created`
2. Query returns `r_created` in results
3. Serde tries to deserialize into `TaskRequestView.created_at`
4. Field not found → deserialization fails
5. HTTP endpoint returns 400 "Internal error"
6. Unit tests pass (they don't use views)
7. Integration tests fail mysteriously

#### The Solution

**Rule: Field names must match EXACTLY between:**
1. Database schema (`DEFINE FIELD ... ON TABLE`)
2. Query SELECT statements
3. Entity structs
4. View model structs

**Fix for the example:**
```rust
// Option A: Keep created_at everywhere (RECOMMENDED)
// Repository schema
DEFINE FIELD IF NOT EXISTS created_at ON TABLE {TABLE_NAME} TYPE datetime...

// Query
SELECT {fields}, created_at FROM {TABLE_NAME}...

// View model
pub struct TaskRequestView {
    pub created_at: DateTime<Utc>,  // ✅ Matches schema
}

// Option B: Use r_created everywhere (NOT recommended, breaks existing code)
```

#### How to Prevent This

**Step 1: Check field naming conventions**

Before transformation, verify the field naming pattern used in existing working entities:

```bash
# Check existing schemas
rg "DEFINE FIELD.*created.*ON TABLE" --type rust

# Check existing view models
rg "pub created_at:" --type rust -g "src/models/view/*.rs"
```

**Step 2: Maintain naming consistency**

When creating the new repository schema, use the SAME field names as:
- The view models that will consume the data
- Other similar entities in the codebase
- The old entity (if it worked before)

**Step 3: Verify in three places**

After writing repository implementation, verify field names match in:

```rust
// 1. Schema definition
DEFINE FIELD IF NOT EXISTS created_at ON TABLE task_request...
                           ^^^^^^^^^^

// 2. Query selection
SELECT {fields}, created_at FROM task_request...
                 ^^^^^^^^^^

// 3. View model
pub struct TaskRequestView {
    pub created_at: DateTime<Utc>,
        ^^^^^^^^^^
}
```

**Step 4: Test with integration tests**

Always run full `cargo test` (not just `cargo test --lib`):

```bash
# This will miss the issue ❌
cargo test --lib

# This will catch the issue ✅
cargo test
```

#### Debugging Field Name Mismatches

If you suspect a field name mismatch:

**Step 1: Check the error**
```rust
// Integration test failure typically shows:
// 400 Bad Request, "Internal error"
// OR JSON deserialization error
```

**Step 2: Compare schema vs model**
```bash
# Find schema definition
rg "DEFINE FIELD.*ON TABLE task_request" --type rust -A 5

# Find view model
rg "struct TaskRequestView" --type rust -A 20

# Look for mismatches
```

**Step 3: Check query field selection**
```bash
# Find SELECT queries
rg "SELECT.*FROM.*task_request" --type rust
```

**Step 4: Add debug output**
```rust
// In repository, before returning:
let raw_result = self.client.query(query).await?;
println!("Raw DB result: {:?}", raw_result);  // See actual field names

// In view model, add Debug
#[derive(Debug, Serialize, Deserialize)]  // Add Debug
pub struct TaskRequestView { ... }
```

#### Why Unit Tests Don't Catch This

Unit tests (`cargo test --lib`) typically:
- Test pure logic without database
- Use mock data with correct structure
- Don't go through HTTP serialization
- Don't use real database queries

Integration tests (`cargo test` without flags):
- Hit actual HTTP endpoints
- Use real database
- Perform actual serialization/deserialization
- Catch field name mismatches

**❌ CRITICAL MISTAKE: Relying on `cargo test --lib` for validation**

Many developers think:
- "Unit tests pass, so my code is correct" ❌
- "I'll just run `cargo test --lib` to save time" ❌
- "Integration tests are optional" ❌

**✅ CORRECT APPROACH: Always run full `cargo test`**

Entity transformation is ONLY validated when:
- You run `cargo test` (no flags)
- All integration tests pass
- Zero test failures

**Rule:** Never consider an entity transformation complete until `cargo test` (full suite) passes with zero failures!

### CRITICAL: SurrealDB Schema Syntax for Object Type Definitions

**This is a common syntax error that causes SurrealDB parse failures.**

#### The Problem

When defining SurrealDB schemas with object type definitions (like enums serialized as objects), using escaped double quotes (`\"`) causes parse errors. This breaks database migrations during `mutate_db()` calls.

#### Real-World Example

**The Bug:**
```rust
// ❌ WRONG - Escaped double quotes cause parse errors
let sql = format!("
    DEFINE FIELD IF NOT EXISTS deliverable_type ON TABLE {TABLE_NAME} TYPE {{ type: \\\"PublicPost\\\"}};
    DEFINE FIELD IF NOT EXISTS reward_type ON TABLE {TABLE_NAME} TYPE {{ type: \\\"OnDelivery\\\"}} | {{ type: \\\"VoteWinner\\\", voting_period_min: int }};
");
```

**Error:**
```
Parse error: Invalid token `\`
 --> [5:92]
  |
5 | ...E { type: \"}
  |              ^
```

**What happens:**
1. Repository `mutate_db()` runs during migrations or test setup
2. SurrealDB parser encounters `\"`
3. Parser fails with "Invalid token" error
4. Database table is not created
5. All database operations fail
6. Tests fail with cryptic errors like "Table not found"

#### The Solution

**Use single quotes for string literals in SurrealDB object type definitions:**

```rust
// ✅ CORRECT - Use single quotes for string literals
let sql = format!("
    DEFINE FIELD IF NOT EXISTS deliverable_type ON TABLE {TABLE_NAME} TYPE {{ type: 'PublicPost'}};
    DEFINE FIELD IF NOT EXISTS reward_type ON TABLE {TABLE_NAME} TYPE {{ type: 'OnDelivery'}} | {{ type: 'VoteWinner', voting_period_min: int }};
");
```

#### The Rule

**In SurrealDB schema definitions:**
- ❌ Never use `\\\"` (escaped double quotes) inside object type definitions
- ✅ Always use `'` (single quotes) for string literals in object types
- ✅ Double quotes `"` are fine for the outer format string, but objects inside need single quotes

**Examples:**

```rust
// ✅ Correct - object types with single quotes
DEFINE FIELD type ON TABLE task TYPE { type: 'Public' };
DEFINE FIELD status ON TABLE task TYPE { type: 'Init' } | { type: 'InProgress' };
DEFINE FIELD metadata ON TABLE task TYPE { type: 'PublicPost', created: datetime };

// ❌ Wrong - escaped double quotes
DEFINE FIELD type ON TABLE task TYPE { type: \"Public\" };  // Parse error!
DEFINE FIELD type ON TABLE task TYPE { type: \\\"Public\\\" };  // Parse error!

// ✅ Correct - regular string types (not objects) can use either
DEFINE FIELD name ON TABLE task TYPE string;
DEFINE FIELD currency ON TABLE task TYPE 'USD'|'EUR';  // Enum-like
```

#### How to Prevent This

**Step 1: Check existing repository schemas**

Before writing a new schema, check the pattern used in working repositories:

```bash
# Search for object type definitions
rg "TYPE \{\{.*type:" --type rust src/database/repositories/

# Look for the quote style used
```

**Step 2: Use single quotes for all object type string literals**

When writing schema definitions with object types:

```rust
impl Repository<MyEntity> {
    pub(in crate::database) async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
            DEFINE TABLE IF NOT EXISTS {TABLE_NAME} SCHEMAFULL;

            -- ✅ Single quotes for object type string values
            DEFINE FIELD IF NOT EXISTS my_enum ON TABLE {TABLE_NAME}
                TYPE {{ type: 'VariantA' }} | {{ type: 'VariantB' }};

            -- ✅ Single quotes even for complex objects
            DEFINE FIELD IF NOT EXISTS complex ON TABLE {TABLE_NAME}
                TYPE {{ type: 'TypeA', value: int }} | {{ type: 'TypeB', value: string }};
        ");

        self.client.query(sql).await?.check()?;
        Ok(())
    }
}
```

**Step 3: Test migrations immediately**

After writing a schema, test it immediately:

```bash
# This will catch parse errors right away
cargo test --lib
```

If you see "Parse error: Invalid token", check for escaped quotes in your schema.

#### Debugging Schema Parse Errors

If you encounter SurrealDB parse errors:

**Step 1: Check the error message**
```
Parse error: Invalid token `\`
 --> [5:92]
```

The line and column number point to the issue.

**Step 2: Find the schema definition**
```bash
# Find the repository with the error
rg "DEFINE FIELD.*TYPE.*\\\\" --type rust
```

**Step 3: Replace escaped quotes with single quotes**

Replace patterns like:
- `\\\"` → `'`
- `\"` → `'`
- Any escaped quotes in object type definitions

**Step 4: Verify the fix**
```bash
cargo test --no-run  # Compile check
cargo test  # Full test
```

### CRITICAL: Using ViewFieldSelector in Repository Methods

**This prevents deserialization failures when querying view models.**

#### The Problem

Repository methods that use `SELECT *` instead of explicitly selecting fields cause silent deserialization failures when view models expect joined data or specific field formatting. This leads to "Internal error" responses in integration tests.

#### Real-World Example

**The Bug:**
```rust
// ❌ WRONG - Using SELECT * for view queries
async fn get_by_posts<T: for<'de> serde::Deserialize<'de>>(
    &self,
    posts: Vec<Thing>,
    user: Thing,
) -> Result<Vec<T>, surrealdb::Error> {
    let query = format!("
        SELECT * FROM {TASK_REQUEST_TABLE_NAME}
        WHERE belongs_to IN $posts
    ");

    let mut res = self.client.query(query).bind(("posts", posts)).await?;
    Ok(res.take::<Vec<T>>(0)?)
}
```

**What happens:**
1. View model `TaskRequestView` expects fields like `participants` and `donors` from joins
2. Query returns raw database fields without joins
3. View model fields are missing from query result
4. Serde deserialization fails silently
5. HTTP endpoint returns 400 "Internal error"
6. Integration tests fail mysteriously

#### The Solution

**Add `ViewFieldSelector` constraint and use explicit field selection:**

```rust
// ✅ CORRECT - Use ViewFieldSelector and select specific fields
async fn get_by_posts<T: for<'de> serde::Deserialize<'de> + ViewFieldSelector>(
    &self,
    posts: Vec<Thing>,
    user: Thing,
) -> Result<Vec<T>, surrealdb::Error> {
    let fields = T::get_select_query_fields();  // Get fields from view model
    let query = format!("
        SELECT {fields} FROM {TASK_REQUEST_TABLE_NAME}
        WHERE belongs_to IN $posts
    ");

    let mut res = self.client.query(query).bind(("posts", posts)).await?;
    Ok(res.take::<Vec<T>>(0)?)
}
```

**And update the interface:**

```rust
// Interface - add ViewFieldSelector constraint
#[async_trait]
pub trait TaskRequestRepositoryInterface: RepositoryCore {
    async fn get_by_posts<T: for<'de> serde::Deserialize<'de> + ViewFieldSelector>(
        &self,
        posts: Vec<Thing>,
        user: Thing,
    ) -> Result<Vec<T>, surrealdb::Error>;
}
```

#### The Rule

**For repository methods that return view models:**

1. ❌ Never use `SELECT *` for methods that return views
2. ✅ Always add `ViewFieldSelector` constraint to generic type `T`
3. ✅ Always call `T::get_select_query_fields()` to get proper field selection
4. ✅ Use the fields in your `SELECT {fields}` query

**When to apply this:**

```rust
// ✅ Apply to methods returning view models
async fn get_by_posts<T: ViewFieldSelector>(...) -> Result<Vec<T>, ...>
async fn get_by_creator<T: ViewFieldSelector>(...) -> Result<Vec<T>, ...>
async fn get_by_user<T: ViewFieldSelector>(...) -> Result<Vec<T>, ...>

// ✅ Apply to methods returning custom views
async fn get_with_details<T: ViewFieldSelector>(...) -> Result<T, ...>

// ❌ Don't apply to methods returning entities (not views)
async fn get_entity(&self, id: &str) -> Result<MyEntity, ...>  // Entity, not view
```

#### How View Models Define Fields

View models implement `ViewFieldSelector` to specify their required fields and joins:

```rust
// Example from src/models/view/task.rs
impl ViewFieldSelector for TaskRequestView {
    fn get_select_query_fields() -> String {
        "id,
        due_at,
        created_at,
        delivery_period,
        acceptance_period,
        wallet_id,
        belongs_to,
        currency,
        status,
        type,
        request_txt,
        created_by.* as created_by,
        ->task_participant.{ user: out.*, status, timelines, result } as participants,
        ->task_donor.{id, user: out.*, amount: transaction.amount_out} as donors"
            .to_string()
    }
}
```

This ensures:
- All needed fields are selected
- Joins are performed (e.g., `->task_participant`)
- Related records are fetched (e.g., `user: out.*`)
- Complex nested data is structured correctly

#### How to Prevent This

**Step 1: Identify view-returning methods**

When migrating repository methods, identify which ones return view models:

```rust
// In old entity service
pub async fn get_tasks_by_post(&self, post_id: &str) -> Vec<TaskView> { ... }
//                                                          ^^^^^^^^ View model!
```

**Step 2: Add ViewFieldSelector constraint**

```rust
// In new repository interface
async fn get_by_posts<T: for<'de> Deserialize<'de> + ViewFieldSelector>(
    &self,
    posts: Vec<Thing>,
    user: Thing,
) -> Result<Vec<T>, surrealdb::Error>;
```

**Step 3: Use field selection in implementation**

```rust
// In repository implementation
async fn get_by_posts<T: for<'de> Deserialize<'de> + ViewFieldSelector>(
    &self,
    posts: Vec<Thing>,
    user: Thing,
) -> Result<Vec<T>, surrealdb::Error> {
    let fields = T::get_select_query_fields();  // ✅ Get fields from view

    let query = format!("
        SELECT {fields} FROM {TABLE_NAME}  -- ✅ Use {fields}, not *
        WHERE belongs_to IN $posts
    ");

    let mut res = self.client.query(query).bind(("posts", posts)).await?;
    Ok(res.take::<Vec<T>>(0)?)
}
```

**Step 4: Apply to all view-returning methods**

Common methods that need this pattern:
- `get_by_posts` - Gets tasks by post IDs
- `get_by_creator` - Gets tasks created by user
- `get_by_user` - Gets tasks for user participation
- `get_by_public_disc` - Gets tasks in public discussion
- `get_by_private_disc` - Gets tasks in private discussion
- Any method with generic `<T>` that returns view models

#### Debugging ViewFieldSelector Issues

**Symptoms:**
- Integration tests fail with "Internal error" 400 responses
- Unit tests pass
- Error occurs when fetching data via HTTP endpoints
- No clear error message about what's wrong

**How to debug:**

**Step 1: Check if method has ViewFieldSelector constraint**

```bash
# Find methods returning generic T
rg "async fn.*<T.*Deserialize" --type rust src/database/repositories/

# Check if they have ViewFieldSelector
rg "ViewFieldSelector" --type rust src/interfaces/repositories/
```

**Step 2: Check if query uses SELECT ***

```bash
# Find SELECT * queries in repositories
rg "SELECT \* FROM" --type rust src/database/repositories/
```

**Step 3: Add ViewFieldSelector and field selection**

For each method found:

1. Add `+ ViewFieldSelector` to the type constraint
2. Add `let fields = T::get_select_query_fields();`
3. Replace `SELECT *` with `SELECT {fields}`
4. Update the interface to match

**Step 4: Verify the fix**

```bash
# Full test suite to catch integration test issues
cargo test
```

The integration tests should now pass because:
- View models get all required fields
- Joins are performed correctly
- Deserialization succeeds with complete data

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

```rust
#[tokio::test]
async fn test_get_entity() {
    let repo = setup_repo().await;

    // Create entity first
    let created = repo.item_create(make_test_entity("test_id")).await.unwrap();

    // Then fetch it
    let fetched = repo.item_by_id(&created.id).await.unwrap();
    assert_eq!(created.id, fetched.id);
}
```

### Issue: "Database field not found"

**Cause:** Repository not registered in Database struct

**Fix:** Add field, initialization, and migration call in client.rs

```rust
// In src/database/client.rs
pub struct Database {
    pub community: Repository<CommunityEntity>,  // Add this
}

impl Database {
    pub async fn connect(config: DbConfig<'_>) -> Self {
        Self {
            community: Repository::<CommunityEntity>::new(
                client.clone(),
                COMMUNITY_TABLE_NAME.to_string(),
            ),
        }
    }

    pub async fn run_migrations(&self) -> Result<(), AppError> {
        self.community.mutate_db().await?;  // Add this
        Ok(())
    }
}
```

### Issue: "Table not found" in tests

**Cause:** Test database not running migrations

**Fix:** Ensure test setup calls `run_migrations()`

```rust
async fn setup_test_database() -> Database {
    let db = Database::connect(test_config()).await;

    // CRITICAL: Run migrations
    db.run_migrations().await.unwrap();

    db
}
```

### Issue: "Tests pass individually but fail together"

**Cause:** Shared database state, ID conflicts, or race conditions

**Fix:** Run tests serially or ensure unique IDs

```bash
# Run serially
cargo test -- --test-threads=1
```

Or use unique IDs per test:

```rust
#[tokio::test]
async fn test_community_1() {
    let repo = setup_repo().await;
    let comm = repo.item_create(make_test_community("unique_id_1")).await.unwrap();
    // ...
}

#[tokio::test]
async fn test_community_2() {
    let repo = setup_repo().await;
    let comm = repo.item_create(make_test_community("unique_id_2")).await.unwrap();
    // ...
}
```

### Issue: "Type mismatch Thing vs String in tests"

**Cause:** Old test code using Thing, new entity uses String

**Fix:** Update test assertions

```rust
// OLD
let user_thing = Thing::from(("local_user", "user_123"));
assert_eq!(entity.created_by, user_thing);

// NEW
assert_eq!(entity.created_by, "user_123");
```

### Issue: "Test helper functions don't compile"

**Cause:** Helper function signatures not updated

**Fix:** Update helper function parameters and return types

```rust
// OLD
async fn create_test_entity(db: &Db, ctx: &Ctx) -> OldEntity { ... }

// NEW
async fn create_test_entity(repo: &Repository<NewEntity>) -> NewEntity { ... }
```

### Issue: "Mock repository trait not satisfied"

**Cause:** Mock not implementing all required traits

**Fix:** Ensure mock implements both RepositoryCore and custom interface

```rust
mock! {
    MyRepo {}

    #[async_trait]
    impl RepositoryCore for MyRepo {
        // Implement all RepositoryCore methods
    }

    #[async_trait]
    impl MyRepositoryInterface for MyRepo {
        // Implement custom methods
    }
}
```

## Summary

This skill provides a complete, systematic approach to transforming old entity service files into the modern layered architecture. By following these phases:

1. **Analyze** the old file structure
2. **Generate** base entity files with entity-sdb skill
3. **Migrate** database operations to repository **WITHOUT RENAMING ANY FIELDS**
4. **Create** optional service layer for business logic
5. **Replace** all usages in the codebase (including tests)
6. **Cleanup** old code - **DELETE old entity file completely (only ONE entity file should remain!)**
7. **Validate** with **FULL `cargo test`** (NOT `cargo test --lib`) and fix issues iteratively
8. **Verify** final implementation with **FULL `cargo test`** again

**⚠️ CRITICAL: After transformation, there should be EXACTLY ONE entity file:**
- ✅ `src/entities/{entity_name}.rs` - The new entity file (KEEP THIS)
- ❌ `src/entities/{entity_name}/{entity_name}_entity.rs` - The old entity file (DELETE THIS)

Use `find src/entities -name "*{entity_name}*" -type f` to verify only one file exists.

## ⚠️ THE TWO MOST IMPORTANT RULES

### Rule #1: NEVER RENAME FIELDS

**NEVER RENAME STRUCT FIELDS, DATABASE FIELD NAMES, OR QUERY FIELD NAMES.**

This single rule prevents 90% of transformation bugs. When in doubt:
- ✅ Keep the exact field name from old entity
- ✅ Preserve field names in schema, queries, entities, views
- ❌ Do not "improve" or "standardize" field names
- ❌ Do not change `created_at` to `r_created` or vice versa

### Rule #2: ALWAYS RUN FULL TEST SUITE

**ALWAYS RUN `cargo test` (full suite), NEVER just `cargo test --lib`.**

This is not optional. Entity transformation is ONLY complete when:
- ✅ You run `cargo test` (no flags)
- ✅ All integration tests pass
- ✅ Zero test failures
- ✅ Zero warnings

Unit tests alone (`cargo test --lib`) will NOT catch:
- Field name mismatches between schema and views
- Serialization/deserialization errors
- HTTP endpoint issues
- Real database query problems

## The Key to Success

1. **Field name preservation** - Never rename fields from old entity
2. **Always run full test suite** - `cargo test` (NOT `cargo test --lib`)
3. **Complete cleanup** - Delete old entity file completely, leaving only ONE entity file
4. **Iterative test fixing workflow** in Phase 7:
   - Categorize failures systematically
   - Fix compilation errors first
   - Then runtime errors
   - Then test setup issues
   - Run `cargo test` repeatedly until all pass
   - Use debugging techniques for stubborn failures

**Final verification command:**
```bash
# This should show EXACTLY ONE file
find src/entities -name "*{entity_name}*" -type f
```

You can successfully modernize legacy entity code while maintaining functionality and passing all tests.
