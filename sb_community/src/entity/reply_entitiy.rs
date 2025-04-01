use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use sb_middleware::db;
use sb_middleware::utils::db_utils::{
    get_entity_list_view, get_entity_view, with_not_found_err, IdentIdName, Pagination, QryOrder,
    ViewFieldSelector,
};
use sb_middleware::{
    ctx::Ctx,
    error::{AppError, CtxError, CtxResult},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reply {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,
    pub discussion: Thing,
    pub belongs_to: Thing,
    pub created_by: Thing,
    pub title: String,
    pub content: String,
    // #[serde(skip_serializing)]
    pub r_created: Option<String>,
    // #[serde(skip_serializing)]
    pub r_updated: Option<String>,
}

pub struct ReplyDbService<'a> {
    pub db: &'a db::Db,
    pub ctx: &'a Ctx,
}

pub const TABLE_NAME: &str = "reply";
const TABLE_COL_DISCUSSION: &str = crate::entity::discussion_entitiy::TABLE_NAME;
const TABLE_COL_POST: &str = crate::entity::post_entitiy::TABLE_NAME;
const TABLE_COL_USER: &str = sb_user_auth::entity::local_user_entity::TABLE_NAME;

impl<'a> ReplyDbService<'a> {
    pub async fn mutate_db(&self) -> Result<(), AppError> {
        let sql = format!("
    DEFINE TABLE {TABLE_NAME} SCHEMAFULL;
    DEFINE FIELD {TABLE_COL_DISCUSSION} ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_DISCUSSION}>;
    DEFINE FIELD belongs_to ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_POST}>;
    DEFINE INDEX belongs_to_idx ON TABLE {TABLE_NAME} COLUMNS belongs_to;
    DEFINE FIELD created_by ON TABLE {TABLE_NAME} TYPE record<{TABLE_COL_USER}>;
    DEFINE FIELD title ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD content ON TABLE {TABLE_NAME} TYPE string ASSERT string::len(string::trim($value))>0;
    DEFINE FIELD r_created ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE $before OR time::now();
    DEFINE FIELD r_updated ON TABLE {TABLE_NAME} TYPE option<datetime> DEFAULT time::now() VALUE time::now();
    ");
        let mutation = self.db.query(sql).await?;

        mutation.check().expect("should mutate reply");

        Ok(())
    }

    pub async fn create(&self, record: Reply) -> CtxResult<Reply> {
        let res = self
            .db
            .create(TABLE_NAME)
            .content(record)
            .await
            .map_err(CtxError::from(self.ctx))
            .map(|v: Option<Reply>| v.unwrap());

        // let things: Vec<Domain> = self.db.select(TABLE_NAME).await.ok().unwrap();
        // dbg!(things);
        res
    }

    pub async fn get_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        ident_id_name: &IdentIdName,
    ) -> CtxResult<T> {
        let opt = get_entity_view::<T>(self.db, TABLE_NAME.to_string(), ident_id_name).await?;
        with_not_found_err(opt, self.ctx, ident_id_name.to_string().as_str())
    }

    pub async fn get_by_post_desc_view<T: for<'b> Deserialize<'b> + ViewFieldSelector>(
        &self,
        post_id: Thing,
        from: i32,
        count: i8,
    ) -> CtxResult<Vec<T>> {
        get_entity_list_view::<T>(
            self.db,
            TABLE_NAME.to_string(),
            &IdentIdName::ColumnIdent {
                column: "belongs_to".to_string(),
                val: post_id.to_raw(),
                rec: true,
            },
            Some(Pagination {
                order_by: Option::from("r_created".to_string()),
                order_dir: Some(QryOrder::DESC),
                count: count.clone(),
                start: from.clone(),
            }),
        )
        .await
    }
}
