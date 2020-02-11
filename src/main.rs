#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate tera;

use futures::{future, Future, Stream};
use hyper::{
    client::HttpConnector, rt, service::service_fn, Body, Client, Request,
    Response, Server, StatusCode, Method
};
use tera::{Context, Tera};

lazy_static! {
    pub static ref TERA: Tera = compile_templates!("templates/**/*");
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

fn index() -> ResponseFuture {
    let mut ctx = Context::new();
    let body = Body::from(TERA.render("index.html", &ctx).unwrap().to_string());
    Box::new(future::ok(Response::new(body)))
}
