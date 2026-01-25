// SecureWipe API server binary
// Runs the Axum REST API for frontend-backend integration

use axum::Router;
use securewipe_core::api_router;

#[tokio::main]
async fn main() {
    use tower_http::cors::{CorsLayer, Any};
    let cors = CorsLayer::new()
        .allow_origin(["http://localhost:5173".parse().unwrap(), "http://127.0.0.1:5173".parse().unwrap()])
        .allow_methods(Any)
        .allow_headers(Any);
    let app = api_router().layer(cors);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("SecureWipe API server running at http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
