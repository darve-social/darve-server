use askama::Template;
use core::fmt;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use surrealdb::engine::any::Any as SurDb;
use surrealdb::method::Query;
use surrealdb::types::{RecordId, RecordIdKey, SurrealValue, Variables};

use crate::database::client::Db;
use crate::middleware::ctx::Ctx;
use crate::middleware::error::{AppError, AppResult, CtxError, CtxResult};

pub static NO_SUCH_THING: Lazy<RecordId> =
    Lazy::new(|| RecordId::new("none", "none"));

pub fn record_id_key_to_string(key: &RecordIdKey) -> String {
    match key {
        RecordIdKey::String(s) => s.clone(),
        RecordIdKey::Number(n) => n.to_string(),
        RecordIdKey::Uuid(u) => u.to_string(),
        RecordIdKey::Array(a) => format!("{:?}", a),
        RecordIdKey::Object(o) => format!("{:?}", o),
        RecordIdKey::Range(_) => String::new(),
    }
}

pub fn record_id_to_raw(id: &RecordId) -> String {
    format!("{}:{}", id.table.as_str(), record_id_key_to_string(&id.key))
}

// TODO -move db specific things to /database-
#[derive(Template, Serialize, Deserialize, Debug, SurrealValue)]
#[template(path = "nera2/default-content.html")]
pub struct RecordWithId {
    #[allow(dead_code)]
    pub id: RecordId,
}

impl ViewFieldSelector for RecordWithId {
    fn get_select_query_fields() -> String {
        "id".to_string()
    }
}

pub enum IdentIdName {
    Id(RecordId),
    Ids(Vec<RecordId>),
    ColumnIdent {
        column: String,
        val: String,
        rec: bool,
    },
    ColumnIdentAnd(Vec<IdentIdName>),
}

impl IdentIdName {
    pub fn get_bindings_map(&self) -> HashMap<String, String> {
        let mut bindings: HashMap<String, String> = HashMap::new();
        match self {
            IdentIdName::Id(id) => {
                bindings.insert("id".to_string(), record_id_to_raw(id));
                bindings
            }
            IdentIdName::Ids(ids) => {
                ids.into_iter().enumerate().for_each(|i_id| {
                    bindings.insert(format!("id_{}", i_id.0), record_id_to_raw(i_id.1));
                });
                bindings
            }
            IdentIdName::ColumnIdent { val, column, .. } => {
                bindings.insert(format!("{}", column), val.clone());
                bindings
            }
            IdentIdName::ColumnIdentAnd(and_filters) => {
                and_filters.iter().fold(bindings, |mut acc, iin| {
                    acc.extend(iin.get_bindings_map());
                    acc
                })
            }
        }
    }
}

// TODO -move db specific things to /database-
impl Display for IdentIdName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            IdentIdName::Id(_) => f.write_str("<record>$id"),
            IdentIdName::Ids(ids) => {
                let ids_qry = ids
                    .iter()
                    .enumerate()
                    .map(|i_thg| format!("<record>$id_{}", i_thg.0))
                    .collect::<Vec<String>>()
                    .join(",");
                f.write_str(ids_qry.as_str())
            }
            IdentIdName::ColumnIdent { column, rec, .. } => {
                let prefix = if *rec { "<record>" } else { "" };
                f.write_str(format!("{column}={prefix}${column}").as_str())
            }
            IdentIdName::ColumnIdentAnd(add_filters) => f.write_str(
                add_filters
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(" AND ")
                    .as_str(),
            ),
        }
    }
}

impl From<IdentIdName> for String {
    fn from(value: IdentIdName) -> Self {
        format!("{value}")
    }
}

impl From<UsernameIdent> for IdentIdName {
    fn from(value: UsernameIdent) -> Self {
        IdentIdName::ColumnIdent {
            column: "username".to_string(),
            val: value.0,
            rec: false,
        }
    }
}

#[derive(Debug)]
pub struct QryBindingsVal(String, Variables);

impl QryBindingsVal {
    pub fn new(qry: String, bindings: HashMap<String, String>) -> Self {
        let mut vars = Variables::new();
        for (k, v) in bindings {
            vars.insert(k, v);
        }
        QryBindingsVal(qry, vars)
    }
    pub fn with_vars(qry: String, vars: Variables) -> Self {
        QryBindingsVal(qry, vars)
    }
    pub fn get_query_string(&self) -> String {
        self.0.clone()
    }
    pub fn into_query(self, db: &Db) -> Query<'_, SurDb> {
        db.query(self.0).bind(self.1)
    }
    pub fn is_empty_qry(&self) -> bool {
        self.0.len() < 1
    }
}

pub struct UsernameIdent(pub String);

// pub struct NameIdent(pub String);

pub struct Pagination {
    pub order_by: Option<String>,
    pub order_dir: Option<QryOrder>,
    pub count: u16,
    pub start: u32,
}

pub struct CursorPagination {
    pub order_by: Option<String>,
    pub order_dir: QryOrder,
    pub count: u16,
    pub cursor: Option<RecordId>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum QryOrder {
    DESC,
    ASC,
}

impl fmt::Display for QryOrder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            QryOrder::DESC => write!(f, "DESC"),
            QryOrder::ASC => write!(f, "ASC"),
        }
    }
}

pub trait ViewFieldSelector {
    // select query fields to fill the View object
    fn get_select_query_fields() -> String;
}

pub trait ViewRelateField {
    fn get_fields() -> String;
}

// TODO -move db specific things to /database- (remove queries after we replace with new services and interfaces and they are not used in old dbservices)
pub fn get_entity_query_str(
    ident: &IdentIdName,
    select_fields_or_id: Option<&str>,
    pagination: Option<Pagination>,
    table_name: String,
) -> Result<QryBindingsVal, AppError> {
    let mut q_bindings: HashMap<String, String> = HashMap::new();

    let query_string = match ident {
        IdentIdName::Id(id) => {
            if record_id_to_raw(id).len() < 3 {
                return Err(AppError::Generic {
                    description: "IdentIdName::Id() value too short".to_string(),
                });
            }
            let fields = select_fields_or_id.unwrap_or("*");
            let mut vars = Variables::new();
            vars.insert("id".to_string(), id.clone());
            return Ok(QryBindingsVal::with_vars(
                format!("SELECT {fields} FROM $id;"),
                vars,
            ));
        }

        IdentIdName::Ids(ids) => {
            if ids.len() < 1 {
                return Ok(QryBindingsVal::new(String::new(), HashMap::new()));
            }

            q_bindings.extend(ident.get_bindings_map());
            let fields = select_fields_or_id.unwrap_or("*");

            format!("SELECT {fields} FROM {};", ident.to_string())
        }

        _ => {
            let pagination_q = match pagination {
                None => "".to_string(),
                Some(pag) => {
                    let order_by = pag.order_by;
                    let mut pag_q = match order_by.clone() {
                        None => "".to_string(),
                        Some(order_by_f) => {
                            let order_by = format!(" ORDER BY {order_by_f} ");
                            match pag.order_dir {
                                None => format!(" {order_by} {} ", QryOrder::DESC.to_string()),
                                Some(direction) => {
                                    format!(" {order_by} {} ", direction.to_string())
                                }
                            }
                        }
                    };

                    let count = if pag.count <= 0 { 20 } else { pag.count };
                    q_bindings.insert("_limit_val".to_string(), count.to_string());
                    pag_q = format!(" {pag_q} LIMIT BY type::int($_limit_val) ");

                    let start = if pag.start <= 0 { 0 } else { pag.start };
                    if start > 0 && order_by.is_none() {
                        println!(
                            "WARNING - query for table {table_name} has START AT but no ORDER BY"
                        );
                    }
                    q_bindings.insert("_start_val".to_string(), start.to_string());
                    format!(" {pag_q} START AT type::int($_start_val) ")
                }
            };

            let fields = select_fields_or_id.unwrap_or("id");
            q_bindings.extend(ident.get_bindings_map());
            // TODO move table name to IdentIdName::ColumnIdent prop since it's used only here
            q_bindings.insert("_table".to_string(), table_name);
            format!(
                "SELECT {fields} FROM type::table($_table) WHERE {} {pagination_q};",
                ident.to_string()
            )
        }
    };
    Ok(QryBindingsVal::new(query_string, q_bindings))
}

pub async fn get_entity<T: for<'a> Deserialize<'a> + SurrealValue>(
    db: &Db,
    table_name: String,
    ident: &IdentIdName,
) -> CtxResult<Option<T>> {
    let query_string = get_entity_query_str(ident, Some("*"), None, table_name)?;
    // println!("QRY={:#?}", query_string);
    get_query(db, query_string).await
}

pub async fn get_entities_by_id<T: for<'a> Deserialize<'a> + SurrealValue>(
    db: &Db,
    ids: Vec<RecordId>,
) -> CtxResult<Vec<T>> {
    if ids.len() < 1 {
        return Ok(vec![]);
    }
    let query_string = format!(
        "SELECT * FROM {};",
        (0..ids.len())
            .map(|i| format!("$id_{}", i))
            .collect::<Vec<String>>()
            .join(",")
    );

    let mut vars = Variables::new();
    for (i, id) in ids.iter().enumerate() {
        vars.insert(format!("id_{}", i), id.clone());
    }

    let mut res = db.query(query_string).bind(vars).await?;

    let res = res.take::<Vec<T>>(0)?;
    Ok(res)
}

pub async fn get_entity_view<T: for<'a> Deserialize<'a> + SurrealValue + ViewFieldSelector>(
    db: &Db,
    table_name: String,
    ident: &IdentIdName,
) -> CtxResult<Option<T>> {
    let query_string = get_entity_query_str(
        ident,
        Some(T::get_select_query_fields().as_str()),
        None,
        table_name,
    )?;
    get_query(db, query_string).await
}

pub async fn get_query<T: for<'a> Deserialize<'a> + SurrealValue>(
    db: &Db,
    query_string: QryBindingsVal,
) -> Result<Option<T>, CtxError> {
    let qry = create_db_qry(db, query_string);

    let mut res = qry.await?;

    let res = res.take::<Option<T>>(0)?;
    Ok(res)
}

pub async fn get_entity_list<T: for<'a> Deserialize<'a> + SurrealValue>(
    db: &Db,
    table_name: String,
    ident: &IdentIdName,
    pagination: Option<Pagination>,
) -> CtxResult<Vec<T>> {
    let query_string = get_entity_query_str(ident, Some("*"), pagination, table_name)?;

    get_list_qry(db, query_string).await
}

pub async fn get_entity_list_view<T: for<'a> Deserialize<'a> + SurrealValue + ViewFieldSelector>(
    db: &Db,
    table_name: String,
    ident: &IdentIdName,
    pagination: Option<Pagination>,
) -> CtxResult<Vec<T>> {
    let query_string = get_entity_query_str(
        ident,
        Some(T::get_select_query_fields().as_str()),
        pagination,
        table_name,
    )?;
    // debug query values
    // dbg!(&query_string);
    get_list_qry(db, query_string).await
}

pub async fn get_list_qry<T: for<'a> Deserialize<'a> + SurrealValue>(
    db: &Db,
    query_string: QryBindingsVal,
) -> CtxResult<Vec<T>> {
    if query_string.is_empty_qry() {
        return Ok(vec![]);
    }
    let qry = create_db_qry(db, query_string);
    let mut res = qry.await?;
    let res = res.take::<Vec<T>>(0)?;
    Ok(res)
}

pub fn create_db_qry(
    db: &Db,
    query_string: QryBindingsVal,
) -> Query<'_, surrealdb::engine::any::Any> {
    query_string.into_query(db)
}

pub async fn exists_entity(
    db: &Db,
    table_name: String,
    ident: &IdentIdName,
) -> CtxResult<Option<RecordId>> {
    match ident {
        IdentIdName::Id(id) => {
            record_exists(db, id).await?;
            Ok(Some(id.clone()))
        }
        _ => {
            let query_string = get_entity_query_str(ident, None, None, table_name)?;
            let qry = create_db_qry(db, query_string);

            let mut res = qry.await?;
            let res = res.take::<Option<RecordWithId>>(0)?;
            match res {
                None => Ok(None),
                Some(rec) => Ok(Some(rec.id)),
            }
        }
    }
}

pub async fn record_exists(db: &Db, record_id: &RecordId) -> AppResult<()> {
    let qry = "RETURN record::exists(<record>$rec_id);";
    let mut res = db.query(qry).bind(("rec_id", record_id_to_raw(record_id))).await?;
    let res: Option<bool> = res.take(0)?;
    match res.unwrap_or(false) {
        true => Ok(()),
        false => Err(AppError::EntityFailIdNotFound {
            ident: record_id_to_raw(record_id),
        }),
    }
}

pub async fn record_exist_all(db: &Db, record_ids: Vec<String>) -> AppResult<Vec<RecordId>> {
    if record_ids.is_empty() {
        return Ok(vec![]);
    }

    let things = record_ids
        .iter()
        .map(|rec_id| {
            RecordId::parse_simple(rec_id).map_err(|_| AppError::Generic {
                description: format!("Invalid record id = {}", rec_id),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let conditions = (0..things.len())
        .map(|i| format!("record::exists(<record>$rec_id_{i})"))
        .collect::<Vec<_>>()
        .join(" AND ");

    let query = {
        let query_str = format!("RETURN {conditions};");
        let mut vars = Variables::new();

        for (i, val) in things.iter().enumerate() {
            vars.insert(format!("rec_id_{i}"), val.clone());
        }

        db.query(query_str).bind(vars)
    };

    let mut res = query.await?;
    let exists: Option<bool> = res.take(0)?;

    if !exists.unwrap_or(false) {
        return Err(AppError::EntityFailIdNotFound {
            ident: "Not all ids exist".to_string(),
        });
    }

    Ok(things)
}

pub fn with_not_found_err<T>(opt: Option<T>, ctx: &Ctx, ident: &str) -> CtxResult<T> {
    match opt {
        None => Err(ctx.to_ctx_error(AppError::EntityFailIdNotFound {
            ident: ident.to_string(),
        })),
        Some(res) => Ok(res),
    }
}
