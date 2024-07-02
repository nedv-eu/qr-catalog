use std::collections::{BTreeMap, HashMap, HashSet};

use actix_multipart::Multipart;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use futures_util::stream::StreamExt;

use askama::Template; // bring trait in scope
use mime_guess::from_path;
use rust_embed::RustEmbed;
use serde::Deserialize;
use tokio::fs;

mod db;
// mod xdb;

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

const DATA_DIR: &str = "data";

const TIMESTAMP_FORMAT: &str = "%Y-%m-%d_%H:%M:%S";

#[derive(Template)] // this will generate the code...
#[template(path = "item.html")] // using the template in this path, relative
                                // to the `templates` dir in the crate root
struct ItemTemplate<'a> {
    // the name of the struct can be anything
    item_id: &'a str, // the field name should match the variable name in your template
    content_imgs: &'a Vec<(String, String)>,
    package_imgs: &'a Vec<(String, String)>,
    location_imgs: &'a Vec<(String, String)>,
    removed_imgs: &'a Vec<(String, String)>,
    categories: &'a Vec<(bool, String)>,
    location_links: &'a Vec<(String, String)>,
}
#[derive(Template)] // this will generate the code...
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    // the name of the struct can be anything
    entries: &'a Vec<IndexItemEntry>,
    categories: &'a BTreeMap<String, bool>,
}

#[derive(Debug)]
struct IndexItemEntry {
    img_link: String,
    item_link: String,
    filename: String,
}

fn handle_embedded_file(path: &str) -> HttpResponse {
    match Assets::get(path) {
        Some(content) => HttpResponse::Ok()
            .content_type(from_path(path).first_or_octet_stream().as_ref())
            .body(content.data.into_owned()),
        None => HttpResponse::NotFound().body("404 Not Found"),
    }
}

async fn create_and_get_thumbnail(path: String) -> String {
    // tokio::task::spawn_blocking(move || {
    web::block(move || {
        let img_path = std::path::Path::new(&path);
        let filename = img_path.file_name().unwrap().to_string_lossy().to_string();
        if filename.starts_with("mini_") {
            return path.to_string();
        }
        let path = img_path.parent().unwrap().to_string_lossy().to_string();
        let img_mini_path = path + "/mini_" + &filename.clone();
        if std::fs::metadata(&img_mini_path).is_err() {
            print!("{:?}", &img_path);
            let img = image::open(img_path).unwrap();
            let img = img.resize(1024, 1024, image::imageops::Lanczos3);
            //let img = img.thumbnail(640, 640);
            img.save(&img_mini_path)
                .expect("Cannot save thumbnail image");
        }
        img_mini_path
    })
    .await
    .unwrap()
}

const NO_CATEGORY_TEXT: &str = "{no category}";

async fn render_index(
    db: web::Data<db::SqliteDb>,
    form_opt: Option<web::Form<HashMap<String, String>>>,
) -> String {
    let db_clone = db.clone();
    let (categories, enabled_categories) = {
        let mut categories_list: BTreeMap<String, bool> = BTreeMap::new();
        let mut enabled_cats: Vec<String> = Vec::new();
        let mut rows = db_clone.get_all_categories().await;
        rows.push(NO_CATEGORY_TEXT.into());

        for cat in rows.iter() {
            let enabled = if let Some(form) = &form_opt {
                let form_cat = form.get_key_value(cat);
                if let Some((_cat, val)) = form_cat {
                    enabled_cats.push(cat.to_string());
                    val == "on"
                } else {
                    false
                }
            } else {
                false
            };
            categories_list.insert(cat.to_string(), enabled);
        }
        (categories_list, enabled_cats)
    };

    let mut entries = Vec::new();
    let mut data_dir = fs::read_dir(DATA_DIR)
        .await
        .expect("Cannot open data folder");

    while let Some(item) = data_dir
        .next_entry()
        .await
        .expect("Cannot read data folder")
    {
        if !item.file_type().await.unwrap().is_dir() {
            continue;
        }
        let Ok(item_id): Result<u32, std::num::ParseIntError> = item.file_name().to_string_lossy().parse() else { continue };
        if !enabled_categories.is_empty() {
            let mut show = false;
            for cat in enabled_categories.iter() {
                let db_clone = db.clone();
                let cat_clone = cat.clone();
                let present: bool = {
                    if cat_clone == NO_CATEGORY_TEXT {
                        db_clone.has_no_category_assigned(item_id as u32).await
                    } else {
                        db_clone
                            .is_item_in_category(item_id as u32, &cat_clone)
                            .await
                    }
                };
                if present {
                    show = true;
                    break;
                }
            }
            if !show {
                continue;
            }
        }
        let mut item_dir = fs::read_dir(item.path())
            .await
            .expect("Cannot open item folder");
        while let Some(entry) = item_dir
            .next_entry()
            .await
            .expect("Cannot read item folder")
        {
            let img_name = entry.file_name().to_string_lossy().to_string();
            if img_name.starts_with("content") {
                let img_mini_path =
                    create_and_get_thumbnail(entry.path().to_string_lossy().to_string()).await;
                let index_entry = IndexItemEntry {
                    filename: entry.file_name().to_string_lossy().to_string(),
                    img_link: img_mini_path,
                    item_link: item.file_name().to_string_lossy().to_string(),
                };
                entries.push(index_entry)
            }
        }
    }

    entries.sort_by_key(|item| item.filename.clone());
    entries.reverse();
    let index_template = IndexTemplate {
        entries: &entries,
        categories: &categories,
    };
    index_template.render().unwrap()
}

#[get("/new_item")]
async fn new_item(_db: web::Data<db::SqliteDb>) -> impl Responder {
    let mut data_dir = fs::read_dir(DATA_DIR)
        .await
        .expect("Cannot open data folder");
    let mut items = std::collections::HashSet::new();

    while let Some(item) = data_dir
        .next_entry()
        .await
        .expect("Cannot read data folder")
    {
        if !item.file_type().await.unwrap().is_dir() {
            continue;
        }
        let Ok(item_id): Result<u32, std::num::ParseIntError> = item.file_name().to_string_lossy().parse() else { continue };
        // let item_id: usize = item.file_name().to_string_lossy().parse().unwrap();
        items.insert(item_id);
    }
    for i in (800..1000u32).rev() {
        if !items.contains(&i) {
            return web::Redirect::to(format!("./item/{}/", i)).see_other();
        }
    }
    web::Redirect::to("./no-more-manual-items-available").see_other()
}

#[post("/goto")]
async fn goto_item(
    _db: web::Data<db::SqliteDb>,
    form: web::Form<HashMap<String, String>>,
) -> impl Responder {
    let item_id = form.get("goto_item").unwrap();
    web::Redirect::to(format!("./item/{}/", item_id)).see_other()
}

#[post("/")]
async fn index_category_filter(
    db: web::Data<db::SqliteDb>,
    form: web::Form<HashMap<String, String>>,
) -> impl Responder {
    HttpResponse::Ok().body(render_index(db.clone(), Some(form)).await)
}

#[get("/")]
async fn index(db: web::Data<db::SqliteDb>) -> impl Responder {
    HttpResponse::Ok().body(render_index(db.clone(), None).await)
}

async fn render_catalog_item(item_id: u32, db: web::Data<db::SqliteDb>) -> String {
    let path = DATA_DIR.to_string() + "/" + &item_id.to_string();
    let item_dir = fs::read_dir(path.clone()).await;

    let mut content_list: Vec<(String, String)> = Vec::new();
    let mut package_list: Vec<(String, String)> = Vec::new();
    let mut location_list: Vec<(String, String)> = Vec::new();
    let mut removed_list: Vec<(String, String)> = Vec::new();
    let mut location_links: Vec<(String, String)> = Vec::new();

    if let Ok(mut dir) = item_dir {
        while let Some(entry) = dir.next_entry().await.expect("Cannot read item folder") {
            //println!("Item {}: {:?}", item_id, entry.path());
            //item_list.push("/".to_string() + &entry.path().to_str().unwrap().to_string());
            let filename = entry.file_name().to_str().unwrap().to_string();
            let filepath = entry.path().to_str().unwrap().to_string();
            let thumbnail = if filename.ends_with(".jpg") {
                create_and_get_thumbnail(filepath).await
            } else {
                "".to_string()
            };
            if filename.starts_with("content") {
                content_list.push((filename, format!("../../{}", thumbnail)));
            } else if filename.starts_with("package") {
                package_list.push((filename, format!("../../{}", thumbnail)));
            } else if filename.starts_with("location_link") {
                let link = std::fs::read_to_string(entry.path()).unwrap();
                location_links.push((filename, link));
            } else if filename.starts_with("location") {
                location_list.push((filename, format!("../../{}", thumbnail)));
            } else if filename.starts_with("removed") {
                removed_list.push((filename, format!("../../{}", thumbnail)));
            }
        }
    };

    content_list.sort();
    content_list.reverse();
    package_list.sort();
    package_list.reverse();
    location_list.sort();
    location_list.reverse();
    removed_list.sort();
    removed_list.reverse();
    location_links.sort();
    location_links.reverse();

    let categories_list = db.get_item_categories(item_id).await;

    let html = ItemTemplate {
        item_id: &item_id.to_string(),
        content_imgs: &content_list,
        package_imgs: &package_list,
        location_imgs: &location_list,
        removed_imgs: &removed_list,
        categories: &categories_list,
        location_links: &location_links,
    };
    html.render().unwrap()
}

#[get("/item/{item_id}")]
async fn get_catalog_item_forward(
    _db: web::Data<db::SqliteDb>,
    url_id: web::Path<u32>,
) -> impl Responder {
    let item_id = url_id.into_inner();
    println!("redirected");
    web::Redirect::to(format!("./{item_id}/")).see_other()
}

#[get("/item/{item_id}/")]
async fn get_catalog_item(db: web::Data<db::SqliteDb>, url_id: web::Path<u32>) -> impl Responder {
    let item_id = url_id.into_inner();
    HttpResponse::Ok().body(render_catalog_item(item_id, db.clone()).await)
}

#[post("/item/{item_id}/new_category")]
async fn new_category(
    db: web::Data<db::SqliteDb>,
    form: web::Form<HashMap<String, String>>,
    url_id: web::Path<u32>,
) -> impl Responder {
    let _item_id = url_id.into_inner();
    db.add_category(form.get("new_category").unwrap()).await;
    web::Redirect::to("./").see_other()
}

#[post("/item/{item_id}/categories")]
async fn catalog_item_categories(
    db: web::Data<db::SqliteDb>,
    form: web::Form<HashMap<String, String>>,
    url_id: web::Path<u32>,
) -> impl Responder {
    let item_id = url_id.into_inner();
    let cats = HashSet::from_iter(form.iter().filter_map(|(cat, val)| {
        if val == "on" {
            Some(cat.clone())
        } else {
            None
        }
    }));
    db.set_categories(item_id, cats).await;
    web::Redirect::to("./").see_other()
}

#[post("/item/{item_id}/upload")]
async fn post_catalog_item(
    _db: web::Data<db::SqliteDb>,
    mut payload: Multipart,
    url_id: web::Path<u32>,
) -> impl Responder {
    let item_id = url_id.into_inner();
    while let Some(field) = payload.next().await {
        // A multipart/form-data stream has to contain `content_disposition`
        let mut field = match field {
            Ok(f) => f,
            Err(e) => {
                println!("Multipart field error {:?}", e);
                continue;
            }
        };

        let field_name = field.name().to_string();

        let mut content: Vec<u8> = Vec::new();
        while let Some(chunk) = field.next().await {
            content.append(&mut chunk.unwrap().to_vec());
        }

        let prefix = if field_name == "content_img" {
            "content_"
        } else if field_name == "package_img" {
            "package_"
        } else if field_name == "location_img" {
            "location_"
        } else {
            continue;
        };

        if content.is_empty() {
            continue;
        }

        let datetime = chrono::Local::now();
        //let ts = datetime.format("%Y-%m-%d_%H:%M:%S.jpg");
        let ts = datetime.format(TIMESTAMP_FORMAT).to_string() + ".jpg";
        let path =
            DATA_DIR.to_string() + "/" + &item_id.to_string() + "/" + prefix + &ts.to_string();
        match fs::write(&path, &content).await {
            Ok(_) => {}
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => {
                    let dir_path = DATA_DIR.to_string() + "/" + &item_id.to_string();
                    fs::create_dir(&dir_path)
                        .await
                        .expect("Cannot create item directory");
                    fs::write(&path, &content)
                        .await
                        .expect("Cannot write item file");
                }
                _ => panic!("Cannot write item file {}", err),
            },
        }
    }
    web::Redirect::to("./").see_other()
}

#[post("/item/{item_id}/link_location")]
async fn link_location(
    _db: web::Data<db::SqliteDb>,
    form: web::Form<HashMap<String, String>>,
    url_id: web::Path<u32>,
) -> impl Responder {
    let item_id = url_id.into_inner();

    let (prefix, link_id) = if let Some(link_id) = form.get("content_link") {
        ("content_link_", link_id.clone())
    } else if let Some(link_id) = form.get("package_link") {
        ("package_link_", link_id.clone())
    } else if let Some(link_id) = form.get("location_link") {
        ("location_link_", link_id.clone())
    } else {
        ("unknown", "0".to_string())
    };

    let datetime = chrono::Local::now();
    //let ts = datetime.format("%Y-%m-%d_%H:%M:%S.jpg");
    let ts = datetime.format(TIMESTAMP_FORMAT).to_string() + ".link";
    let path = DATA_DIR.to_string() + "/" + &item_id.to_string() + "/" + prefix + &ts.to_string();
    match fs::write(&path, &link_id).await {
        Ok(_) => {}
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => {
                let dir_path = DATA_DIR.to_string() + "/" + &item_id.to_string();
                fs::create_dir(&dir_path)
                    .await
                    .expect("Cannot create item directory");
                fs::write(&path, &link_id)
                    .await
                    .expect("Cannot write item file");
            }
            _ => panic!("Cannot write item file {}", err),
        },
    }
    web::Redirect::to("./").see_other()
}

#[derive(Deserialize)]
struct RemoveCatalogItemImgForm {
    img_to_remove: String,
}
#[post("/item/{item_id}/remove")]
async fn remove_catalog_item_img(
    _db: web::Data<db::SqliteDb>,
    form: web::Form<RemoveCatalogItemImgForm>,
    url_id: web::Path<u32>,
) -> impl Responder {
    let item_id = url_id.into_inner();
    //println!("Removing {}", form.img_to_remove);
    if !form.img_to_remove.starts_with("removed_") {
        let datetime = chrono::Local::now();
        let ts = datetime.format(TIMESTAMP_FORMAT).to_string();

        let item_path = DATA_DIR.to_string() + "/" + &item_id.to_string() + "/";
        let _ign = fs::rename(
            item_path.clone() + &form.img_to_remove,
            item_path.clone() + "removed_" + &ts + "_" + &form.img_to_remove,
        )
        .await;
    }
    web::Redirect::to("./").see_other()
}

#[derive(Deserialize)]
struct RemoveCatalogItemLinkForm {
    link_to_remove: String,
}
#[post("/item/{item_id}/remove_link")]
async fn remove_catalog_item_link(
    _db: web::Data<db::SqliteDb>,
    form: web::Form<RemoveCatalogItemLinkForm>,
    url_id: web::Path<u32>,
) -> impl Responder {
    let item_id = url_id.into_inner();
    //println!("Removing {}", form.img_to_remove);
    if !form.link_to_remove.starts_with("removed_") {
        let datetime = chrono::Local::now();
        let ts = datetime.format(TIMESTAMP_FORMAT).to_string();

        let item_path = DATA_DIR.to_string() + "/" + &item_id.to_string() + "/";
        let _ign = fs::rename(
            item_path.clone() + &form.link_to_remove,
            item_path.clone() + "removed_" + &ts + "_" + &form.link_to_remove,
        )
        .await;
    }

    //let item_id = url_id.into_inner();
    //println!("Removing {}", &form.link_to_remove);
    //tokio::fs::remove_file(form.link_to_remove.clone()).await.unwrap();
    web::Redirect::to("./").see_other()
}

#[derive(Deserialize)]
struct RestoreCatalogItemImgForm {
    img_to_restore: String,
}
#[post("/item/{item_id}/restore")]
async fn restore_catalog_item_img(
    _db: web::Data<db::SqliteDb>,
    form: web::Form<RestoreCatalogItemImgForm>,
    url_id: web::Path<u32>,
) -> impl Responder {
    let item_id = url_id.into_inner();
    //println!("Removing {}", form.img_to_remove);
    if form.img_to_restore.starts_with("removed_") {
        //let datetime = chrono::Local::now();
        //let ts = datetime.format(TIMESTAMP_FORMAT).to_string();

        let item_path = DATA_DIR.to_string() + "/" + &item_id.to_string() + "/";
        let _ign = fs::rename(
            item_path.clone() + &form.img_to_restore,
            item_path.clone() + &form.img_to_restore[28..],
        )
        .await;
    }
    //HttpResponse::Ok().body(render_catalog_item(item_id, db.clone()).await)
    web::Redirect::to("./").see_other()
}

#[actix_web::get("/static/{_:.*}")]
async fn static_files(path: web::Path<String>) -> impl Responder {
    handle_embedded_file(path.as_str())
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "debug");
    std::env::set_var("RUST_BACKTRACE", "1");
    env_logger::init();

    let db = db::SqliteDb::init_db("./data/cat.db").await;
    HttpServer::new(move || {
        App::new()
            .app_data(actix_web::web::Data::new(db.clone()))
            .service(index)
            .service(index_category_filter)
            .service(get_catalog_item)
            .service(get_catalog_item_forward)
            .service(post_catalog_item)
            .service(remove_catalog_item_img)
            .service(restore_catalog_item_img)
            .service(catalog_item_categories)
            .service(new_category)
            .service(new_item)
            .service(goto_item)
            .service(link_location)
            .service(remove_catalog_item_link)
            .service(actix_files::Files::new("/data", "./data"))
            .service(static_files)
            .route("/hey", web::get().to(manual_hello))
    })
    //.bind(("127.0.0.1", 8080))?
    .bind(("0.0.0.0", 8081))?
    .run()
    .await
}
