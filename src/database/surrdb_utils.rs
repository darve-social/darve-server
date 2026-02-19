use askama::Template;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::types::{RecordId, RecordIdKey, SurrealValue, Variables};

use crate::database::client::Db;
use crate::middleware::utils::db_utils::{
    IdentIdName, Pagination, QryBindingsVal, QryOrder, ViewFieldSelector,
};

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

pub fn get_str_id_thing(tb: &str, id: &str) -> Result<RecordId, surrealdb::Error> {
    if id.is_empty() || id.contains(":") {
        return Err(surrealdb::Error::validation(
            format!("{}:{}", tb, id),
            None,
        ));
    }
    Ok(RecordId::new(tb, id))
}

pub fn get_thing(value: &str) -> Result<RecordId, surrealdb::Error> {
    RecordId::parse_simple(value).map_err(|_| {
        surrealdb::Error::validation(value.to_string(), None)
    })
}

// get id from Thing's string
pub fn get_thing_id(thing_str: &str) -> &str {
    match thing_str.find(":") {
        None => thing_str,
        Some(ind) => &thing_str[ind + 1..],
    }
}

#[derive(Template, Serialize, Deserialize, Debug, SurrealValue)]
#[template(path = "nera2/default-content.html")]
pub struct RecordWithId {
    #[allow(dead_code)]
    pub id: RecordId,
}

fn get_entity_query_str(
    ident: &IdentIdName,
    select_fields_or_id: Option<&str>,
    pagination: Option<Pagination>,
    table_name: &str,
) -> Result<QryBindingsVal, surrealdb::Error> {
    let mut q_bindings: HashMap<String, String> = HashMap::new();

    let query_string = match ident {
        IdentIdName::Id(id) => {
            let raw = record_id_to_raw(id);
            if raw.len() < 3 {
                return Err(surrealdb::Error::validation(
                    "id value too short".to_string(),
                    None,
                ));
            }
            let fields = select_fields_or_id.unwrap_or("*");
            q_bindings.insert("id".to_string(), raw);

            format!("SELECT {fields} FROM <record>$id;")
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
            q_bindings.insert("_table".to_string(), table_name.to_string());
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
    table_name: &str,
    ident: &IdentIdName,
) -> Result<Option<T>, surrealdb::Error> {
    let query_string = get_entity_query_str(ident, Some("*"), None, table_name)?;
    get_query(db, query_string).await
}

pub async fn get_entities_by_id<T: for<'a> Deserialize<'a> + SurrealValue>(
    db: &Db,
    ids: Vec<RecordId>,
) -> Result<Vec<T>, surrealdb::Error> {
    if ids.len() < 1 {
        return Ok(vec![]);
    }
    let qry_bindings = ids
        .iter()
        .enumerate()
        .map(|i_t| {
            (
                format!("<record>$id_{}", i_t.0),
                (format!("id_{}", i_t.0), record_id_to_raw(i_t.1)),
            )
        })
        .collect::<Vec<(String, (String, String))>>();

    let query_string = format!(
        "SELECT * FROM {};",
        qry_bindings
            .iter()
            .map(|i_t| i_t.0.clone())
            .collect::<Vec<String>>()
            .join(",")
    );

    let mut vars = Variables::new();
    for (_placeholder, (key, val)) in &qry_bindings {
        vars.insert(key.clone(), val.clone());
    }

    let mut res = db.query(query_string).bind(vars).await?;

    let res = res.take::<Vec<T>>(0)?;
    Ok(res)
}

pub async fn get_entity_view<T: for<'a> Deserialize<'a> + SurrealValue + ViewFieldSelector>(
    db: &Db,
    table_name: &str,
    ident: &IdentIdName,
) -> Result<Option<T>, surrealdb::Error> {
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
) -> Result<Option<T>, surrealdb::Error> {
    let qry = create_db_qry(db, query_string);

    let mut res = qry.await?;
    let res = res.take::<Option<T>>(0)?;
    Ok(res)
}

pub async fn get_entity_list<T: for<'a> Deserialize<'a> + SurrealValue>(
    db: &Db,
    table_name: &str,
    ident: &IdentIdName,
    pagination: Option<Pagination>,
) -> Result<Vec<T>, surrealdb::Error> {
    let query_string = get_entity_query_str(ident, Some("*"), pagination, table_name)?;

    get_list_qry(db, query_string).await
}

pub async fn get_entity_list_view<T: for<'a> Deserialize<'a> + SurrealValue + ViewFieldSelector>(
    db: &Db,
    table_name: &str,
    ident: &IdentIdName,
    pagination: Option<Pagination>,
) -> Result<Vec<T>, surrealdb::Error> {
    let query_string = get_entity_query_str(
        ident,
        Some(T::get_select_query_fields().as_str()),
        pagination,
        table_name,
    )?;
    get_list_qry(db, query_string).await
}

pub async fn get_list_qry<T: for<'a> Deserialize<'a> + SurrealValue>(
    db: &Db,
    query_string: QryBindingsVal,
) -> Result<Vec<T>, surrealdb::Error> {
    if query_string.is_empty_qry() {
        return Ok(vec![]);
    }
    let qry = create_db_qry(db, query_string);
    let mut res = qry.await?;
    let res = res.take::<Vec<T>>(0)?;
    Ok(res)
}

fn create_db_qry(
    db: &Db,
    query_string: QryBindingsVal,
) -> surrealdb::method::Query<'_, surrealdb::engine::any::Any> {
    query_string.into_query(db)
}

pub async fn exists_entity(
    db: &Db,
    table_name: &str,
    ident: &IdentIdName,
) -> Result<RecordId, surrealdb::Error> {
    match ident {
        IdentIdName::Id(id) => {
            exists_by_thing(db, id).await?;
            Ok(id.clone())
        }
        _ => {
            let query_string = get_entity_query_str(ident, None, None, table_name)?;
            let qry = create_db_qry(db, query_string);

            let mut res = qry.await?;
            let res = res.take::<Option<RecordWithId>>(0)?;
            match res {
                None => Err(surrealdb::Error::not_found(ident.to_string(), None)),
                Some(rec) => Ok(rec.id),
            }
        }
    }
}

pub async fn exists_by_thing(db: &Db, record_id: &RecordId) -> Result<(), surrealdb::Error> {
    let qry = "RETURN record::exists(<record>$rec_id);";
    let mut res = db.query(qry).bind(("rec_id", record_id_to_raw(record_id))).await?;
    let res: Option<bool> = res.take(0)?;
    match res.unwrap_or(false) {
        true => Ok(()),
        false => Err(surrealdb::Error::not_found(record_id_to_raw(record_id), None)),
    }
}

pub async fn record_exist_all(
    db: &Db,
    record_ids: Vec<String>,
) -> Result<Vec<RecordId>, surrealdb::Error> {
    if record_ids.is_empty() {
        return Ok(vec![]);
    }

    let things = record_ids
        .iter()
        .map(|rec_id| {
            RecordId::parse_simple(rec_id.as_str()).map_err(|_| {
                surrealdb::Error::validation(rec_id.to_string(), None)
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
        return Err(surrealdb::Error::not_found(
            "some id(s) not in db".to_string(),
            None,
        ));
    }

    Ok(things)
}

pub async fn count_records(db: &Db, table_name: &str) -> Result<u64, surrealdb::Error> {
    let query = "(SELECT count() as count FROM ONLY $table_name GROUP ALL).count;";
    let mut res = db
        .query(query)
        .bind(("table_name", table_name.to_string()))
        .await?;
    let res: Option<u64> = res.take(0)?;
    res.ok_or(surrealdb::Error::not_found(
        format!("table {}", table_name),
        None,
    ))
}

#[cfg(test)]
mod tests {
    use crate::middleware::utils::db_utils::IdentIdName;

    #[tokio::test]
    async fn test_ident_qry() {
        let ident = IdentIdName::ColumnIdent {
            column: "col".to_string(),
            val: "vvv".to_string(),
            rec: false,
        };
        assert_eq!(ident.to_string(), "col=$col".to_string());

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
        assert_eq!(ident.to_string(), "col=$col AND column=$column".to_string());

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
            "col=<record>$col AND column=$column".to_string()
        );
    }
}
