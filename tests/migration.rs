use readcrab::db::Db;
use rusqlite::Connection;

const TABLES: [&str; 6] = [
    "users",
    "series",
    "books",
    "progress",
    "reading_sessions",
    "goals",
];

fn dump(conn: &Connection) -> Vec<(String, Vec<String>)> {
    TABLES
        .iter()
        .map(|t| {
            let mut stmt = conn
                .prepare(&format!("SELECT * FROM {t} ORDER BY 1, 2"))
                .unwrap();
            let n = stmt.column_count();
            let rows = stmt
                .query_map([], |r| {
                    Ok((0..n)
                        .map(|i| format!("{:?}", r.get_ref(i).unwrap()))
                        .collect::<Vec<_>>()
                        .join("|"))
                })
                .unwrap()
                .map(|r| r.unwrap())
                .collect();
            (t.to_string(), rows)
        })
        .collect()
}

#[test]
fn v1_database_keeps_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db.sqlite");
    {
        let c = Connection::open(&path).unwrap();
        c.execute_batch(include_str!("../src/migrations/0001_init.sql"))
            .unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();
        c.execute_batch(
            "INSERT INTO users(id,username,password_hash,is_admin,created_at) VALUES(1,'admin','x',1,100);
             INSERT INTO series(id,rel_path,name,added_at) VALUES(1,'a','A',100);
             INSERT INTO books(id,series_id,rel_path,title,format,size,mtime,content_id,page_count,has_cover,sort_index,added_at)
               VALUES(1,1,'a/1.cbz','Bir','cbz',10,20,'c1',5,1,1,100),
                     (2,1,'a/2.epub','İki','epub',11,21,'c2',3,0,2,100);
             INSERT INTO progress(user_id,book_id,locator,percent,status,finished_date,updated_at)
               VALUES(1,1,'{\"ch\":2,\"p\":0.5}',0.5,'reading',NULL,200),
                     (1,2,'7',1.0,'finished','2024-03-05',300);
             INSERT INTO reading_sessions(id,user_id,book_id,client_id,started_at,ended_at,active_seconds,pages_read,start_percent,end_percent,local_date)
               VALUES(1,1,1,'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',100,200,90,4,0.0,0.3,'2024-03-04'),
                     (2,1,2,'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',300,400,120,6,0.3,1.0,'2024-03-05');
             INSERT INTO goals(user_id,year,target) VALUES(1,2024,12);",
        )
        .unwrap();
    }
    let before = dump(&Connection::open(&path).unwrap());

    drop(Db::open(&path).unwrap());
    let c = Connection::open(&path).unwrap();
    let v: i32 = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(v, 2);
    // Eski sütunlar birebir aynı: yeni sütunlar hariç karşılaştır.
    let after = dump(&c);
    for ((t, b), (_, a)) in before.iter().zip(after.iter()) {
        if *t == "books" {
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(b.iter()) {
                assert_eq!(x, &format!("{y}|Integer(0)|Integer(0)"));
            }
        } else {
            assert_eq!(a, b, "{t}");
        }
    }
    let n: i64 = c
        .query_row("SELECT COUNT(*) FROM bookmarks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);

    drop(Db::open(&path).unwrap());
    let c = Connection::open(&path).unwrap();
    assert_eq!(dump(&c), after);
}
