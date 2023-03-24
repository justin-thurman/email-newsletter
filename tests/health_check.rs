use std::net::TcpListener;

use sqlx::PgPool;

use email_newsletter::configuration::get_configuration;

// A struct holding data needed to access a test version of our application
pub struct TestApp {
    pub address: String,
    pub connection_pool: PgPool,
}

// Spawns an app inside a future and returns the IP address that it's listening on.
async fn spawn_app() -> TestApp {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind a random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let configuration = get_configuration().expect("Failed to read configuration.");
    let connection_pool = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to postgres.");

    let server = email_newsletter::startup::run(listener, connection_pool.clone())
        .expect("Failed to bind address");
    let _ = tokio::spawn(server);
    TestApp {
        address,
        connection_pool,
    }
}

#[tokio::test]
async fn health_check_responds_200() {
    // arrange
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();

    // act
    let response = client
        .get(&format!("{}/health_check", &test_app.address))
        .send()
        .await
        .expect("Failed to execute request");

    // assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscribe_with_valid_form_data_returns_200() {
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    // act
    let response = client
        .post(&format!("{}/subscriptions", &test_app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request");

    // assert
    assert_eq!(200, response.status().as_u16());

    let saved_subscriber = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&test_app.connection_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved_subscriber.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved_subscriber.name, "le guin");
}

#[tokio::test]
async fn subscribe_with_missing_form_data_returns_400() {
    // arrange
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=le%20guin", "missing email"),
        ("email=ursula_le_guin%40gmail.com", "missing name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        // act
        let response = client
            .post(&format!("{}/subscriptions", &test_app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request");

        // assert
        assert_eq!(
            400,
            response.status().as_u16(),
            "API did not fail with 400 when payload was {}",
            error_message
        )
    }
}
