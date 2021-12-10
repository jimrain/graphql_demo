//! Default Compute@Edge template program.

mod graphql_schema;

use fastly::http::{HeaderValue, Method, StatusCode};
// use fastly::request::CacheOverride;
use fastly::{Body, Error, Request, Response};

use std::convert::TryFrom;

#[macro_use]
use juniper::{
    http::playground::playground_source, http::GraphQLRequest, DefaultScalarValue, EmptyMutation,
    FieldResult, GraphQLObject, RootNode,
};
use serde::Deserialize;
use std::str::FromStr;
use std::sync::Arc;

// mod graphql_schema;
// use crate::graphql_schema::{create_schema, Schema};

/// The name of a backend server associated with this service.
const BACKEND: &str = "GQL_Backend";
const API_BACKEND: &str = "API_Backend";
const GRAPHQL_API_URI: &str = "https://django.shastarain.com/pmvc/api/";

const LOGGING_ENDPOINT: &str = "GQL_Syslog";

/// Allows us to see which version of the app is running in our log messages.
const APP_VERSION_NUMBER: u8 = 17;

const TTL: u32 = 60;

// Juniper/GraphQL Plumping
// Wrap the backend API in a Juniper Context object
struct ApiBackend {}

impl juniper::Context for ApiBackend {}

impl ApiBackend {
    pub fn new() -> ApiBackend {
        ApiBackend {}
    }

    /// Get a user, given their ID.
    pub fn get_user(&self, id: i32) -> FieldResult<User> {
        let mut resp = Request::new(Method::GET, format!("{}users/{}", GRAPHQL_API_URI, id))
            .with_pass(true)
            .send(API_BACKEND)
            .unwrap();

        let status = resp.get_status().to_string();
        println!("{}", format!("Returning from get_user: Status =  {}", status));

        let user: User = serde_json::from_reader(resp.get_body_mut())?;
        Ok(user)
    }

    /// Get a product, given its ID.
    pub fn get_videos(&self) -> FieldResult<Vec<Video>> {
        let mut uri = format!("{}videos/", GRAPHQL_API_URI);

        let mut resp = Request::new(Method::GET, uri)
            .with_ttl(TTL)
            .send(API_BACKEND)
            .unwrap();

        let status = resp.get_status().to_string();
        println!("{}", format!("Returning from get_videos: Status =  {}", status));

        let videos: Vec<Video> = serde_json::from_reader(resp.get_body_mut())?;
        Ok(videos)
    }

        /// Get a video, given its ID.
    pub fn get_video(&self, id: Option<i32>) -> FieldResult<Video> {
        let mut uri = format!("{}videos/", GRAPHQL_API_URI);
        if let vid = Some(id) {
            // let iVid = vid.unwrap().unwrap()
            uri = format!("{}{}/", uri, vid.unwrap().unwrap());
        }

        let mut resp = Request::new(Method::GET,  uri)
            .with_ttl(TTL)
            .send(API_BACKEND)
            .unwrap();

        let status = resp.get_status().to_string();
        println!("{}", format!("Returning from get_video: Status =  {}", status));

        let video: Video = serde_json::from_reader(resp.get_body_mut())?;
        Ok(video)
    }
}


#[derive(Deserialize)]
struct User {
    id: i32,
    user_name: String,
    email: String,
}

#[juniper::object(
    Context = ApiBackend
)]
/// User object with shopping cart
impl User {
    /// Current contents of shopping cart
    /*
    fn cart(&self, context: &ApiBackend) -> FieldResult<Vec<Product>> {
        // Get full product object (cached) for all product IDs in cart
        Ok(self
            .cart
            .iter()
            .filter_map(|id| context.get_product(*id).ok())
            .collect())
    }
    */
    /// User name (first + last)
    fn name(&self) -> FieldResult<&String> {
        // Construct name from first_name/last_name keys
        // let name = format!("{} {}", self.first_name, self.last_name);
        Ok(&self.user_name)
    }

    /// Email address
    fn email(&self) -> FieldResult<&String> {
        Ok(&self.email)
    }

    /// User ID
    fn id(&self) -> FieldResult<&i32> {
        Ok(&self.id)
    }
}

#[derive(Deserialize, GraphQLObject)]
/// Product object
struct Video {
    /// Product ID
    id: i32,
    /// Name of the product
    title: String,
    /// Units product is sold in
    url: String,
}

struct Query;

#[juniper::object(
    Context = ApiBackend
)]
/// API root
impl Query {
    fn user(&self, id: i32, context: &ApiBackend) -> FieldResult<User> {
        context.get_user(id)
    }

    fn video(&self, id: i32, context: &ApiBackend) -> FieldResult<Video> {
        context.get_video(Some(id))
    }

    fn videos(&self, context: &ApiBackend) -> FieldResult<Vec<Video>> {
        context.get_videos()
    }

}

/// The entrypoint for your application.
///
/// This function is triggered when your service receives a client request. It could be used to
/// route based on the request properties (such as method or path), send the request to a backend,
/// make completely new requests, and/or generate synthetic responses.
///
/// If `main` returns an error a 500 error response will be delivered to the client.
#[fastly::main]
fn main(mut req: Request) -> Result<Response, Error> {
    // Dispatch the request based on the method and path.
    // Getting / returns GraphQL Playground, a graphical interactive in-browser GraphQL IDE.
    // The GraphQL API itself is at /graphql. All other paths return 404s.
    let resp = match (req.get_method(), req.get_path()) {

        (&Method::GET, "/") => Response::new()
                .with_body_bytes(playground_source("/graphql").as_bytes()),

        (&Method::POST, "/graphql") => {
            // Instantiate the GraphQL schema
            let root_node = Arc::new(RootNode::new(Query, EmptyMutation::<ApiBackend>::new()));

            // Add context to be used by the GraphQL resolver functions,
            // in this case a wrapper for a Fastly backend.
            let ctx = Arc::new(ApiBackend::new());

            // Deserialize the post body into a GraphQL request
            let graphql_request: GraphQLRequest<DefaultScalarValue> =
                serde_json::from_reader(req.get_body_mut()).unwrap();

            // Execute the request, serialize the response to JSON, and return it
            let res = graphql_request.execute(&root_node, &ctx);
            let output = serde_json::to_string(&res).unwrap();
            Response::new()
                .with_body_bytes(output.as_bytes())
        }


        _ => Response::from_status(StatusCode::NOT_FOUND)
                .with_body_text_plain("404 Not Found"),
    };

    resp.send_to_client();
    let new_resp = Response::new();
    Ok(new_resp)
}
