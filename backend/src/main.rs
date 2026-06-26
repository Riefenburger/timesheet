use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    // A Router maps URL paths to handler functions.
    // This says: a GET request to "/" runs root_handler.
    let app = Router::new().route("/", get(root_handler));

    // Bind a TCP port, then hand the router to axum::serve
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    println!("Backend listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

// A handler is an async function returing something axum can turn into
// a response. A string becomes a 200 OK with that text as the body.
async fn root_handler() -> &'static str {
    "Hello form the timesheet backend!"
}