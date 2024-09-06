#[macro_use] extern crate rocket;

use rocket::form::Form;
use rocket::response::Redirect;
use rocket::fs::{FileServer, relative};
use rocket::serde::{json::Json, Serialize, Deserialize};
use std::sync::Mutex;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};

#[derive(FromForm)]
struct Input {
    key: String,
    value: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdateRequest {
    value: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Database {
    store: Mutex<HashMap<String, String>>,
    file_path: String,
}

impl Database {
    fn new(file_path: &str) -> Self {
        let mut db = Database {
            store: Mutex::new(HashMap::new()),
            file_path: file_path.to_string(),
        };

        // Load data from file if it exists
        if let Err(e) = db.load() {
            eprintln!("Failed to load data from file: {}", e);
        }

        db
    }

    fn insert(&self, key: String, value: String) {
        let mut store = self.store.lock().unwrap();
        store.insert(key, value);

        // Save data to file asynchronously
        let file_path = self.file_path.clone();
        let store_clone = store.clone();
        rocket::tokio::spawn(async move {
            if let Err(e) = save_to_file(&file_path, &store_clone).await {
                eprintln!("Failed to save data to file: {}", e);
            }
        });
    }

    fn update(&self, key: String, value: String) {
        let mut store = self.store.lock().unwrap();
        if store.contains_key(&key) {
            store.insert(key, value);

            // Save data to file asynchronously
            let file_path = self.file_path.clone();
            let store_clone = store.clone();
            rocket::tokio::spawn(async move {
                if let Err(e) = save_to_file(&file_path, &store_clone).await {
                    eprintln!("Failed to save data to file: {}", e);
                }
            });
        }
    }

    fn get_all(&self) -> HashMap<String, String> {
        let store = self.store.lock().unwrap();
        store.clone()
    }

    fn load(&mut self) -> io::Result<()> {
        let mut file = fs::File::open(&self.file_path)?;
        let mut data = String::new();
        file.read_to_string(&mut data)?;

        let store: HashMap<String, String> = serde_json::from_str(&data)?;
        *self.store.lock().unwrap() = store;

        Ok(())
    }
}

async fn save_to_file(file_path: &str, store: &HashMap<String, String>) -> io::Result<()> {
    let data = serde_json::to_string(store)?;
    let mut file = fs::File::create(file_path)?;
    file.write_all(data.as_bytes())
}

#[post("/submit", data = "<input>")]
fn submit(input: Form<Input>, db: &rocket::State<Database>) -> Redirect {
    db.insert(input.key.clone(), input.value.clone());
    Redirect::to("/")
}

#[put("/update/<key>", format = "json", data = "<update_request>")]
async fn update_data(key: String, update_request: Json<UpdateRequest>, db: &rocket::State<Database>) -> &'static str {
    db.update(key, update_request.value.clone());
    "Updated"
}

#[get("/")]
async fn index() -> Result<rocket::fs::NamedFile, std::io::Error> {
    rocket::fs::NamedFile::open("static/index.html").await
}

#[get("/data")]
async fn get_data(db: &rocket::State<Database>) -> Json<HashMap<String, String>> {
    let data = db.get_all();
    Json(data)
}

#[get("/data/<key>")]
async fn get_data_by_key(key: String, db: &rocket::State<Database>) -> Json<HashMap<String, String>> {
    let store = db.get_all();
    let value = store.get(&key).unwrap_or(&"Not found".to_string()).clone();
    Json(HashMap::from([(key, value)]))
}

#[delete("/data/<key>")]
async fn delete_data(key: String, db: &rocket::State<Database>) -> &'static str {
    let mut store = db.store.lock().unwrap();
    store.remove(&key);

    // Save data to file asynchronously
    let file_path = db.file_path.clone();
    let store_clone = store.clone();
    rocket::tokio::spawn(async move {
        if let Err(e) = save_to_file(&file_path, &store_clone).await {
            eprintln!("Failed to save data to file: {}", e);
        }
    });

    "Deleted"
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", FileServer::from(relative!("static")))
        .mount("/", routes![index, submit, get_data, get_data_by_key, delete_data, update_data])
        .manage(Database::new("data.json")) // Initialize the database with a file path
}
