use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use wiremock::MockServer;

use email_newsletter::configuration::{get_configuration, DatabaseSettings};
use email_newsletter::startup::{get_connection_pool, Application};
use email_newsletter::telemetry::{get_tracing_subscriber, init_subscriber};

// ensure that the tracing stack is only initialized once
static TRACING: Lazy<()> = Lazy::new(|| {
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_tracing_subscriber("test", "debug", std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_tracing_subscriber("test", "debug", std::io::sink);
        init_subscriber(subscriber);
    }
});

// A struct holding data needed to access a test version of our application
pub struct TestApp {
    pub address: String,
    pub connection_pool: PgPool,
    // email_server stands in for Postmark's API
    pub email_server: MockServer,
    pub port: u16,
}

impl TestApp {
    /// Posts the provided body to the subscriptions endpoint
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request")
    }
}

// Spawns an app inside a future and returns the configured TestApp.
pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);
    let email_server = MockServer::start().await;

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration");
        // Use a difference database for each test case
        c.database.database_name = Uuid::new_v4().to_string();
        // Use a random OS port
        c.application.port = 0;
        // User the mock server's uri as email API
        c.email_client.base_url = email_server.uri();
        c
    };

    // Create and migrate the database
    configure_database(&configuration.database).await;

    // Launch the application as a background task
    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application");
    let port = application.port();
    let address = format!("http://127.0.0.1:{}", port);
    let _ = tokio::spawn(application.run_until_stopped());
    TestApp {
        address,
        connection_pool: get_connection_pool(&configuration.database),
        email_server,
        port,
    }
}

// Configures a test database, running all migrations, and then returning the connection pool handle
// needed to use the test database.
async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failed to connect to postgres.");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database");

    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connect to postgres.");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}
