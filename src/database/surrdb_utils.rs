use askama::Template;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::method::Query;
use surrealdb::sql::Thing;

use crate::database::client::Db;
use crate::middleware::utils::db_utils::{
    IdentIdName, Pagination, QryBindingsVal, QryOrder, ViewFieldSelector,
};

pub static NO_SUCH_THING: Lazy<Thing> = Lazy::new(|| Thing::from(("none", "none")));

pub fn get_string_thing_surr(value: String) -> Result<Thing, surrealdb::Error> {
    get_str_thing_surr(value.as_str())
}

pub fn get_str_thing_surr(value: &str) -> Result<Thing, surrealdb::Error> {
    if value.is_empty() || !value.contains(":") {
        return Err(surrealdb::Error::Db(surrealdb::error::Db::IdInvalid {
            value: format!("{value} - can't create Thing without table part"),
        }));
    }
    Thing::try_from(value).map_err(|_| {
        surrealdb::Error::Db(surrealdb::error::Db::IdInvalid {
            value: value.to_string(),
        })
    })
}

pub fn get_str_id_thing(tb: &str, id: &str) -> Result<Thing, surrealdb::Error> {
    if id.is_empty() || id.contains(":") {
        return Err(surrealdb::Error::Db(surrealdb::error::Db::IdInvalid {
            value: format!("{}:{}", tb, id),
        }));
    }
    Thing::try_from((tb, id)).map_err(|_| {
        surrealdb::Error::Db(surrealdb::error::Db::IdInvalid {
            value: format!("{}:{}", tb, id),
        })
    })
}

// get id from Thing's string
pub fn get_thing_id(thing_str: &str) -> &str {
    match thing_str.find(":") {
        None => thing_str,
        Some(ind) => &thing_str[ind + 1..],
    }
}

#[derive(Template, Serialize, Deserialize, Debug)]
#[template(path = "nera2/default-content.html")]
pub struct RecordWithId {
    #[allow(dead_code)]
    pub id: Thing,
}

fn get_entity_query_str(
    ident: &IdentIdName,
    select_fields_or_id: Option<&str>,
    pagination: Option<Pagination>,
    table_name: &str,
) -> Result<QryBindingsVal<String>, surrealdb::Error> {
    let mut q_bindings: HashMap<String, String> = HashMap::new();

    let query_string = match ident {
        IdentIdName::Id(id) => {
            if id.to_raw().len() < 3 {
                // TODO create app db error
                return Err(surrealdb::Error::Db(surrealdb::error::Db::IdInvalid {
                    value: "id value too short".to_string(),
                }));
            }
            let fields = select_fields_or_id.unwrap_or("*");
            q_bindings.insert("id".to_string(), id.to_raw());

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

pub async fn get_entity<T: for<'a> Deserialize<'a>>(
    db: &Db,
    table_name: &str,
    ident: &IdentIdName,
) -> Result<Option<T>, surrealdb::Error> {
    let query_string = get_entity_query_str(ident, Some("*"), None, table_name)?;
    // println!("QRY={:#?}", query_string);
    get_query(db, query_string).await
}

pub async fn get_entities_by_id<T: for<'a> Deserialize<'a>>(
    db: &Db,
    ids: Vec<Thing>,
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
                (format!("id_{}", i_t.0), i_t.1.to_raw()),
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
    // let mut res = db.query(query_string);
    let mut res = qry_bindings
        .into_iter()
        .fold(db.query(query_string), |qry, qry_binding| {
            qry.bind(qry_binding.1)
        })
        .await?;

    let res = res.take::<Vec<T>>(0)?;
    Ok(res)
}

pub async fn get_entity_view<T: for<'a> Deserialize<'a> + ViewFieldSelector>(
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

pub async fn get_query<T: for<'a> Deserialize<'a>>(
    db: &Db,
    query_string: QryBindingsVal<String>,
) -> Result<Option<T>, surrealdb::Error> {
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
    table_name: &str,
    ident: &IdentIdName,
    pagination: Option<Pagination>,
) -> Result<Vec<T>, surrealdb::Error> {
    let query_string = get_entity_query_str(ident, Some("*"), pagination, table_name)?;

    get_list_qry(db, query_string).await
}

pub async fn get_entity_list_view<T: for<'a> Deserialize<'a> + ViewFieldSelector>(
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
    // debug query values
    // dbg!(&query_string);
    get_list_qry(db, query_string).await
}

pub async fn get_list_qry<T: for<'a> Deserialize<'a>>(
    db: &Db,
    query_string: QryBindingsVal<String>,
) -> Result<Vec<T>, surrealdb::Error> {
    if query_string.is_empty_qry() {
        return Ok(vec![]);
    }
    let qry = create_db_qry(db, query_string);
    let mut res = qry.await?;
    // dbg!(&res);
    let res = res.take::<Vec<T>>(0)?;
    Ok(res)
}

fn create_db_qry(
    db: &Db,
    query_string: QryBindingsVal<String>,
) -> Query<'_, surrealdb::engine::any::Any> {
    // let qry = db.query(query_string.0);
    // let qry = query_string.1.into_iter().fold(qry, |acc, name_value| {
    //     acc.bind(name_value)
    // });
    // qry
    query_string.into_query(db)
}

pub async fn exists_entity(
    db: &Db,
    table_name: &str,
    ident: &IdentIdName,
) -> Result<Thing, surrealdb::Error> {
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
                None => Err(surrealdb::Error::Db(surrealdb::error::Db::IdNotFound {
                    rid: ident.to_string(),
                })),
                Some(rec) => Ok(rec.id),
            }
        }
    }
}

pub async fn exists_by_thing(db: &Db, record_id: &Thing) -> Result<(), surrealdb::Error> {
    let qry = "RETURN record::exists(<record>$rec_id);";
    let mut res = db.query(qry).bind(("rec_id", record_id.to_raw())).await?;
    let res: Option<bool> = res.take(0)?;
    match res.unwrap_or(false) {
        true => Ok(()),
        false => Err(surrealdb::Error::Db(surrealdb::error::Db::IdNotFound {
            rid: record_id.to_raw(),
        })),
    }
}

pub async fn record_exist_all(
    db: &Db,
    record_ids: Vec<String>,
) -> Result<Vec<Thing>, surrealdb::Error> {
    if record_ids.is_empty() {
        return Ok(vec![]);
    }

    let things = record_ids
        .iter()
        .map(|rec_id| {
            Thing::try_from(rec_id.as_str()).map_err(|_| {
                surrealdb::Error::Db(surrealdb::error::Db::IdInvalid {
                    value: rec_id.to_string(),
                })
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let conditions = (0..things.len())
        .map(|i| format!("record::exists(<record>$rec_id_{i})"))
        .collect::<Vec<_>>()
        .join(" AND ");

    let query = {
        let query_str = format!("RETURN {conditions};");
        let mut query = db.query(query_str);

        for (i, val) in things.iter().enumerate() {
            query = query.bind((format!("rec_id_{i}"), val.clone()));
        }

        query
    };

    let mut res = query.await?;
    let exists: Option<bool> = res.take(0)?;

    if !exists.unwrap_or(false) {
        return Err(surrealdb::Error::Db(surrealdb::error::Db::IdNotFound {
            rid: "some id(s) not in db".to_string(),
        }));
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
    res.ok_or(
        surrealdb::error::Db::TbNotFound {
            name: format!("table {}", table_name),
        }
        .into(),
    )
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
