#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate tera;
#[macro_use]
extern crate serde_derive;

use futures::{future, Future, Stream};
use hyper::{
    client::HttpConnector, rt, service::service_fn, Body, Client, Request,
    Response, Server, StatusCode, Method, header
};
use tera::{Context, Tera};
use std::{
  path::PathBuf,
  sync::{Arc, RwLock},
};
use uuid::Uuid;

lazy_static! {
    pub static ref TERA: Tera = compile_templates!("templates/**/*");
    pub static ref TODOS: Todos = Arc::new(RwLock::new(Vec::new()));
}

fn main() {
  pretty_env_logger::init();
  let addr: std::net::SocketAddr = "127.0.0.1:3000".parse().unwrap();
  rt::run(future::lazy(move || {
    // create a Client for all Services
    let client = Client::new();
    // define a service containing the router function
    let new_service = move || {
        // Move a clone of Client into the service_fn
        let client = client.clone();
        service_fn(move |req| router(req, &client))
    };
    // Define the server - this is what the future_lazy() we're building will resolve to
    let server = Server::bind(&addr)
        .serve(new_service)
        .map_err(|e| eprintln!("Server error: {}", e));
    println!("Listening on http://{}", addr);
    server
  }));
}

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<dyn Future<Item = Response<Body>, Error = GenericError> + Send>;

fn router(req: Request<Body>, _client: &Client<HttpConnector>) -> ResponseFuture {
  match (req.method(), req.uri().path()) {
    (&Method::GET, "/") | (&Method::GET, "index.html") => index(),
    (&Method::POST, "/done") => toggle_todo_handler(req),
    (&Method::POST, "/not-done") => toggle_todo_handler(req),
    (&Method::POST, "/delete") => remove_todo_handler(req),
    (&Method::POST, "/") => add_todo_handler(req),
    (&Method::GET, "/static/todo.css") => stylesheet(),
    (&Method::GET, path_str) => image(path_str),
    _ => four_oh_four(),
    }
}

static NOTFOUND: &[u8] = b"Oops! Not Found";

fn four_oh_four() -> ResponseFuture {
  let body = Body::from(NOTFOUND);
  Box::new(future::ok(
    Response::builder()
      .status(StatusCode::NOT_FOUND)
      .body(body)
      .unwrap(),
  ))
}

fn stylesheet() -> ResponseFuture {
  let body = Body::from(include_str!("resource/todo.css"));
  Box::new(future::ok(
    Response::builder()
      .status(StatusCode::OK)
      .header(header::CONTENT_TYPE, "text/css")
      .body(body)
      .unwrap(),
  ))
}

fn index() -> ResponseFuture {
  let mut ctx = Context::new();
  let todos = Arc::clone(&TODOS);
  let lock = todos.read().unwrap();
  ctx.insert("todos", &*lock);
  ctx.insert("todosLen", &(*lock).len());
  let body = Body::from(TERA.render("index.html", &ctx).unwrap().to_string());
  Box::new(future::ok(Response::new(body)))
}

#[derive(Debug, Serialize)]
pub struct Todo {
  done: bool,
  name: String,
  id: Uuid,
}

impl Todo {
  fn new(name: &str) -> Self {
    Self {
      done: false,
      name: String::from(name),
      id: Uuid::new_v4(),
    }
  }
}

type Todos = Arc<RwLock<Vec<Todo>>>;

fn add_todo(t: Todo) {
  let todos = Arc::clone(&TODOS);
  let mut lock = todos.write().unwrap();
  lock.push(t);
}

fn toggle_todo(id: Uuid) {
  let todos = Arc::clone(&TODOS);
  let mut lock = todos.write().unwrap();
  for todo in &mut *lock {
    if todo.id == id {
      todo.done = !todo.done;
    }
  }
}

fn remove_todo(id: Uuid) {
  let todos = Arc::clone(&TODOS);
  let mut lock = todos.write().unwrap();
  // find the index
  let mut idx = lock.len();
  for (i, todo) in lock.iter().enumerate() {
    if todo.id == id {
      idx = i;
    }
  }
  // remove that element if found
  if idx < lock.len() {
    lock.remove(idx);
  }
}

fn redirect_home() -> ResponseFuture {
  Box::new(future::ok(
    Response::builder()
      .status(StatusCode::SEE_OTHER)
      .header(header::LOCATION, "/")
      .body(Body::from(""))
      .unwrap(),
  ))
}

fn add_todo_handler(req: Request<Body>) -> ResponseFuture {
  Box::new(
    req.into_body()
      .concat2() // concatenate all the chunks in the body
      .from_err() // like try! for Result, but for Futures
      .and_then(|whole_body| {
        let str_body = String::from_utf8(whole_body.to_vec()).unwrap();
        let words: Vec<&str> = str_body.split('=').collect();
        add_todo(Todo::new(words[1]));
        redirect_home()
      }),
  )
}

fn toggle_todo_handler(req: Request<Body>) -> ResponseFuture {
  Box::new(
    req.into_body()
      .concat2() // concatenate all the chunks in the body
      .from_err() // like try! for Result, but for Futures
      .and_then(|whole_body| {
        let str_body = String::from_utf8(whole_body.to_vec()).unwrap();
        let words: Vec<&str> = str_body.split('=').collect();
        toggle_todo(Uuid::parse_str(words[1]).unwrap());
        redirect_home()
      }),
  )
}

fn remove_todo_handler(req: Request<Body>) -> ResponseFuture {
  Box::new(
    req.into_body()
      .concat2() // concatenate all the chunks in the body
      .from_err() // like try! for Result, but for Futures
      .and_then(|whole_body| {
        let str_body = String::from_utf8(whole_body.to_vec()).unwrap();
        let words: Vec<&str> = str_body.split('=').collect();
        remove_todo(Uuid::parse_str(words[1]).unwrap());
        redirect_home()
      }),
  )
}

fn image(path_str: &str) -> ResponseFuture {
    let path_buf = PathBuf::from(path_str);
    let file_name = path_buf.file_name().unwrap().to_str().unwrap();
    let ext = path_buf.extension().unwrap().to_str().unwrap();

    match ext {
        "svg" => {
            // build the response
            let body = {
                let xml = match file_name {
                    "check.svg" => include_str!("resource/check.svg"),
                    "plus.svg" => include_str!("resource/plus.svg"),
                    "trashcan.svg" => include_str!("resource/trashcan.svg"),
                    "x.svg" => include_str!("resource/x.svg"),
                    _ => "",
                };
                Body::from(xml)
            };
            Box::new(future::ok(
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "image/svg+xml")
                    .body(body)
                    .unwrap(),
            ))
        }
        _ => four_oh_four(),
    }
}
