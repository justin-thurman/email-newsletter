use email_newsletter::configuration::{get_configuration, DatabaseSettings};
use email_newsletter::email_client::EmailClient;
use email_newsletter::telemetry::{get_tracing_subscriber, init_subscriber};
use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;

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
}

// Spawns an app inside a future and returns the configured TestApp.
pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind a random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let mut configuration = get_configuration().expect("Failed to read configuration.");
    configuration.database.database_name = Uuid::new_v4().to_string();

    let connection_pool = configure_database(&configuration.database).await;

    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email address.");
    let email_client = EmailClient::new(
        configuration.email_client.base_url,
        sender_email,
        configuration.email_client.authorization_token,
        std::time::Duration::from_millis(100),
    );

    let server = email_newsletter::startup::run(listener, connection_pool.clone(), email_client)
        .expect("Failed to bind address");
    let _ = tokio::spawn(server);
    TestApp {
        address,
        connection_pool,
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
