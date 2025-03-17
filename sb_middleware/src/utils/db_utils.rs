use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use strum::Display;
use surrealdb::engine::local::Db as SurDb;
use surrealdb::method::Query;
use surrealdb::sql::Thing;
use tower::ServiceExt;

use crate::ctx::Ctx;
use crate::db::Db;
use crate::error::{AppError, AppResult, CtxError, CtxResult};

pub static NO_SUCH_THING: Lazy<Thing> = Lazy::new(|| Thing::from(("none", "none")));

#[derive(Debug, Deserialize)]
pub struct RecordWithId {
    #[allow(dead_code)]
    pub id: Thing,
}

pub enum IdentIdName {
    Id(Thing),
    Ids(Vec<Thing>),
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
                bindings.insert("id".to_string(), id.to_raw());
                bindings
            }
            IdentIdName::Ids(ids) => {
                ids.into_iter().enumerate()
                    .for_each(|i_id|{
                        bindings.insert(format!("id_{}",i_id.0), i_id.1.to_raw());
                    } );
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

impl Display for IdentIdName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            IdentIdName::Id(_) => f.write_str("<record>$id"),
            IdentIdName::Ids(ids) => {
                let ids_qry = ids.iter().enumerate()
                    .map(|i_thg| format!("<record>$id_{}", i_thg.0))
                    .collect::<Vec<String>>()
                    .join(",");
                f.write_str(ids_qry.as_str())
            },
            IdentIdName::ColumnIdent { column, rec, .. } => {
                let prefix = if *rec { "<record>" } else { "" };
                f.write_str(format!("{column}={prefix}${column}").as_str())
            }
            IdentIdName::ColumnIdentAnd(andFilters) => f.write_str(
                andFilters
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

type SerializableQryValsHash<T: Serialize + 'static + Clone> = HashMap<String, T>;

#[derive(Debug)]
pub struct QryBindingsVal<T: Serialize + 'static + Clone>(String, SerializableQryValsHash<T>);

impl<T: Serialize + 'static + Clone> QryBindingsVal<T> {
    pub fn new(qry: String, bindings: HashMap<String, T>) -> Self {
        QryBindingsVal(qry, bindings)
    }
    pub fn get_query_string(&self) -> String {
        self.0.clone()
    }
    pub fn get_bindings(&self) -> HashMap<String, T> {
        self.1.clone()
    }
    pub fn into_query(self, db: &Db) -> Query<SurDb> {
        self.1
            .into_iter()
            .fold(db.query(self.0), |qry, n_val| qry.bind(n_val))
    }
    pub fn is_empty_qry(&self) -> bool {
        self.0.len()<1
    }
}

pub struct UsernameIdent(pub String);

// pub struct NameIdent(pub String);

pub struct Pagination {
    pub order_by: Option<String>,
    pub order_dir: Option<QryOrder>,
    pub count: i8,
    pub start: i32,
}

#[derive(Display)]
pub enum QryOrder {
    DESC,
    ASC,
}

pub trait ViewFieldSelector {
    // select query fields to fill the View object
    fn get_select_query_fields(ident: &IdentIdName) -> String;
}

fn get_entity_query_str(
    ident: &IdentIdName,
    select_fields_or_id: Option<&str>,
    pagination: Option<Pagination>,
    table_name: String,
) -> Result<QryBindingsVal<String>, AppError> {
    let mut q_bindings: HashMap<String, String> = HashMap::new();

    let query_string = match ident.clone() {

        IdentIdName::Id(id) => {
            if id.to_raw().len() < 3 {
                return Err(AppError::Generic {
                    description: "IdentIdName::Id() value too short".to_string(),
                });
            }
            let fields = select_fields_or_id.unwrap_or("*");
            q_bindings.insert("id".to_string(), id.to_raw());

            format!("SELECT {fields} FROM <record>$id;")
        }

        IdentIdName::Ids(ids)=>{
            if ids.len()<1 {
                return Ok(QryBindingsVal::new(String::new(), HashMap::new()));
            }

            q_bindings.extend(ident.get_bindings_map());
            let fields = select_fields_or_id.unwrap_or("*");

             format!(
                "SELECT {fields} FROM {};",
                ident.to_string()
             )

        }

        _ => {
            let pagination_q = match pagination {
                None => "".to_string(),
                Some(pag) => {
                    let mut pag_q = match pag.order_by {
                        None => "".to_string(),
                        Some(order_by_f) => format!(" ORDER BY {order_by_f} "),
                    };
                    let mut pag_q = match pag.order_dir {
                        None => format!(" {pag_q} {} ", QryOrder::DESC.to_string()),
                        Some(direction) => format!(" {pag_q} {} ", direction.to_string()),
                    };

                    let count = if pag.count <= 0 { 20 } else { pag.count };
                    q_bindings.insert("_count_val".to_string(), count.to_string());
                    pag_q = format!(" {pag_q} LIMIT type::int($_count_val) ");

                    let start = if pag.start < 0 { 0 } else { pag.start };
                    q_bindings.insert("_start_val".to_string(), start.to_string());
                    format!(" {pag_q} START type::int($_start_val) ")
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
    Ok(QryBindingsVal(query_string, q_bindings))
}

pub async fn get_entity<T: for<'a> Deserialize<'a>>(
    db: &Db,
    table_name: String,
    ident: &IdentIdName,
) -> CtxResult<Option<T>> {
    let query_string = get_entity_query_str(ident, Some("*"), None, table_name)?;
    get_query(db, query_string).await
}

pub async fn get_entities_by_id<T: for<'a> Deserialize<'a>>(
    db: &Db,
    ids: Vec<Thing>,
) -> CtxResult<Vec<T>> {
    if ids.len() < 1 {
        return Ok(vec![]);
    }
    let qry_bindings = ids.iter()
        .enumerate()
        .map(|i_t| (format!("<record>$id_{}", i_t.0), (format!("id_{}", i_t.0), i_t.1.to_raw())))
        .collect::<Vec<(String, (String, String))>>();

    let query_string = format!(
        "SELECT * FROM {};",
        qry_bindings.iter()
            .map(|i_t| i_t.0.clone())
            .collect::<Vec<String>>()
            .join(",")
    );
    // let mut res = db.query(query_string);
    let mut res = qry_bindings.into_iter().fold(db.query(query_string), |qry, qry_binding| {
        qry.bind(qry_binding.1)
    })
        .await?;

    let res = res.take::<Vec<T>>(0)?;
    Ok(res)
}

pub async fn get_entity_view<T: for<'a> Deserialize<'a> + ViewFieldSelector>(
    db: &Db,
    table_name: String,
    ident: &IdentIdName,
) -> CtxResult<Option<T>> {
    let query_string = get_entity_query_str(
        ident,
        Some(T::get_select_query_fields(ident).as_str()),
        None,
        table_name,
    )?;
    // println!("QQQ={}", query_string);
    get_query(db, query_string).await
}

async fn get_query<T: for<'a> Deserialize<'a>>(
    db: &Db,
    query_string: QryBindingsVal<String>,
) -> Result<Option<T>, CtxError> {
    let qry = create_db_qry(db, query_string);

    let mut res = qry.await?;
    // if table_name.eq("reply"){
    // println!("Q={}", query_string);
    // dbg!(&res);
    // }
    let res = res.take::<Option<T>>(0)?;
    Ok(res)
}

pub async fn get_entity_list<T: for<'a> Deserialize<'a>>(
    db: &Db,
    table_name: String,
    ident: &IdentIdName,
    pagination: Option<Pagination>,
) -> CtxResult<Vec<T>> {
    let query_string = get_entity_query_str(ident, Some("*"), pagination, table_name)?;

    get_list_qry(db, query_string).await
}

pub async fn get_entity_list_view<T: for<'a> Deserialize<'a> + ViewFieldSelector>(
    db: &Db,
    table_name: String,
    ident: &IdentIdName,
    pagination: Option<Pagination>,
) -> CtxResult<Vec<T>> {
    let query_string = get_entity_query_str(
        ident,
        Some(T::get_select_query_fields(ident).as_str()),
        pagination,
        table_name,
    )?;
    // println!("QQQ={:#?}", &query_string);
    get_list_qry(db, query_string).await
}

pub async fn get_list_qry<T: for<'a> Deserialize<'a>>(
    db: &Db,
    query_string: QryBindingsVal<String>,
) -> CtxResult<Vec<T>> {
    if query_string.is_empty_qry() {
        return Ok(vec![]);
    }
    let qry = create_db_qry(db, query_string);
    let mut res = qry.await?;
    // dbg!(&res);
    let res = res.take::<Vec<T>>(0)?;
    Ok(res)
}

fn create_db_qry(db: &Db, query_string: QryBindingsVal<String>) -> Query<surrealdb::engine::local::Db> {
    // let qry = db.query(query_string.0);
    // let qry = query_string.1.into_iter().fold(qry, |acc, name_value| {
    //     acc.bind(name_value)
    // });
    // qry
    query_string.into_query(db)
}

pub async fn exists_entity(
    db: &Db,
    table_name: String,
    ident: &IdentIdName,
) -> CtxResult<Option<Thing>> {
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

pub async fn record_exists(db: &Db, record_id: &Thing) -> AppResult<()> {
    let qry = format!("RETURN record::exists(r\"{}\");", record_id.to_raw());
    let mut res = db.query(qry).await?;
    let res: Option<bool> = res.take(0)?;
    match res.unwrap_or(false) {
        true => Ok(()),
        false => Err(AppError::EntityFailIdNotFound {
            ident: record_id.to_raw(),
        }),
    }
}

pub fn with_not_found_err<T>(opt: Option<T>, ctx: &Ctx, ident: &str) -> CtxResult<T> {
    match opt {
        None => Err(ctx.to_ctx_error(AppError::EntityFailIdNotFound {
            ident: ident.to_string(),
        })),
        Some(res) => Ok(res),
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::db_utils::IdentIdName;

    #[tokio::test]
    async fn test_ident_qry() {
        let ident = IdentIdName::ColumnIdent {
            column: "col".to_string(),
            val: "vvv".to_string(),
            rec: false,
        };
        assert_eq!(ident.to_string(), "col='vvv'".to_string());

        let ident = IdentIdName::ColumnIdentAnd(vec![
            IdentIdName::ColumnIdent {
                column: "col".to_string(),
                val: "vvv".to_string(),
                rec: false,
            },
            IdentIdName::ColumnIdent {
                column: "column".to_string(),
                val: "ooooo".to_string(),
                rec: false,
            },
        ]);
        assert_eq!(
            ident.to_string(),
            "col='vvv' AND column='ooooo'".to_string()
        );

        let ident = IdentIdName::ColumnIdentAnd(vec![
            IdentIdName::ColumnIdent {
                column: "col".to_string(),
                val: "vvv:56".to_string(),
                rec: true,
            },
            IdentIdName::ColumnIdent {
                column: "column".to_string(),
                val: "ooooo".to_string(),
                rec: false,
            },
        ]);
        assert_eq!(
            ident.to_string(),
            "col=vvv:56 AND column='ooooo'".to_string()
        );
    }
}
