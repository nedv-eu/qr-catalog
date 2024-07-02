use rusqlite::ffi::SQLITE_DESERIALIZE_FREEONCLOSE;
use sqlx::{sqlite::SqlitePool, Connection, Row};
use std::collections::HashSet;

#[derive(Clone)]
pub struct SqliteDbx {
    pub pool: SqlitePool,
}

impl SqliteDbx {
    pub async fn init_db(db_file: &str) -> Self {
        let db = Self {
            pool: SqlitePool::connect(db_file).await.unwrap(),
        };

        // let pool_conn = db.pool.clone();
        // let conn = pool_conn.get().unwrap();
        let mut conn = db.pool.acquire().await.unwrap();

        sqlx::query!(
            "CREATE TABLE IF NOT EXISTS category (
                 id INTEGER PRIMARY KEY,
                 name TEXT UNIQUE NOT NULL
             );",
        )
        .execute(&mut *conn)
        .await
        .unwrap();

        sqlx::query!(
            "CREATE TABLE IF NOT EXISTS item_category (
                item_id INTEGER,
                cat_id INTEGER,
                PRIMARY KEY ( item_id, cat_id),
                FOREIGN KEY(cat_id) REFERENCES category(id)
            );",
        )
        .execute(&mut *conn)
        .await
        .unwrap();

        db
    }

    pub async fn set_categories(&self, item_id: u32, cats: HashSet<String>) {
        let mut con = self.pool.acquire().await.unwrap();
        let mut trans = con.begin().await.unwrap();

        sqlx::query!(
            "DELETE FROM item_category 
            WHERE item_id = ?1;",
            item_id
        )
        .execute(&mut *trans)
        .await
        .unwrap();

        {
            let rows = sqlx::query!("SELECT category.name FROM category;")
                .fetch_all(&mut *trans)
                .await
                .unwrap();

            // let mut statement = trans
            //     .prepare("SELECT category.name FROM category;")
            //     .unwrap();
            // let mut rows = statement.query(()).unwrap();
            let mut rows = rows.iter();
            while let Some(row) = rows.next() {
                let cat: String = row.name.clone();
                if cats.contains(&cat) {
                    sqlx::query!(
                        "
                        INSERT INTO item_category (cat_id, item_id)
                        SELECT category.id, ?1 
                        FROM category 
                        WHERE category.name = ?2
                        ",
                        item_id,
                        cat
                    )
                    .execute(&mut *trans)
                    .await
                    .unwrap();
                }
            }
        }
        trans.commit().await.unwrap();
    }

    pub async fn add_category(&self, cat: &str) {
        let mut con = self.pool.acquire().await.unwrap();
        sqlx::query!("INSERT INTO category (name) VALUES (?1);", cat)
            .execute(&mut *con)
            .await
            .unwrap();
    }

    pub async fn get_item_categories(&self, item_id: u32) -> Vec<(bool, String)> {
        let mut con = self.pool.acquire().await.unwrap();

        let res = sqlx::query!(
            "SELECT
                category.name,
                EXISTS(
                    SELECT 1
                    FROM item_category
                    WHERE category.id = item_category.cat_id
                        AND item_category.item_id = ?1
                ) AS \"selected\"
            FROM category;",
            item_id
        )
        .fetch_all(&mut *con)
        .await
        .unwrap();

        res.iter()
            .map(|r| (r.selected.unwrap() != 0, r.name.clone()))
            .collect()

        // let mut statement = con
        //     .prepare(
        //         "
        // SELECT
        //     category.name,
        //     EXISTS(
        //         SELECT 1
        //         FROM item_category
        //         WHERE category.id = item_category.cat_id
        //             AND item_category.item_id = ?1
        //     )
        // FROM category
        // ;",
        //     )
        //     .unwrap();
        // let rows = statement
        //     .query_map((item_id,), |row| {
        //         Ok((
        //             row.get_unwrap::<usize, bool>(1),
        //             row.get_unwrap::<usize, String>(0),
        //         ))
        //     })
        //     .unwrap();
        // rows.map(|r| r.unwrap()).collect()
    }

    pub async fn get_all_categories(&self) -> Vec<String> {
        let mut con = self.pool.acquire().await.unwrap();
        let rows = sqlx::query!(
            "SELECT category.name
             FROM category;"
        )
        .fetch_all(&mut *con)
        .await
        .unwrap();
        rows.iter().map(|r| r.name.clone()).collect()
    }

    pub async fn has_no_category_assigned(&self, item_id: u32) -> bool {
        let mut con = self.pool.acquire().await.unwrap();

        let res = sqlx::query!(
            "SELECT (0 == (
                      SELECT count(cat_id)
                      FROM item_category
                      WHERE item_category.item_id = ?1
                  )) as \"no_category\";",
            item_id
        )
        .fetch_one(&mut *con)
        .await
        .unwrap();
        res.no_category.unwrap() != 0
    }

    pub async fn is_item_in_category(&self, item_id: u32, cat: &str) -> bool {
        let mut con = self.pool.acquire().await.unwrap();
        let res = sqlx::query!(
            "SELECT (?1 IN (
                      SELECT name
                      FROM item_category JOIN category
                      ON item_category.cat_id = category.id
                      WHERE item_category.item_id = ?2
                 )) as \"in_category\";",
            cat,
            item_id
        )
        .fetch_one(&mut *con)
        .await
        .unwrap();
        res.in_category.unwrap() != 0

        // con.query_row(
        //     "SELECT ?1 IN (
        //               SELECT name
        //               FROM item_category JOIN category
        //               ON item_category.cat_id = category.id
        //               WHERE item_category.item_id = ?2
        //           );",
        //     (cat, item_id),
        //     |row| row.get(0),
        // )
        // .unwrap()
    }
}
