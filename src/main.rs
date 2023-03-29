use secrecy::ExposeSecret;
use std::net::TcpListener;

use sqlx::PgPool;

use crate::telemetry::init_subscriber;
use email_newsletter::configuration::get_configuration;
use email_newsletter::startup::run;

mod telemetry;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let subscriber = telemetry::get_tracing_subscriber("email-newsletter", "info", std::io::stdout);
    init_subscriber(subscriber);
    let configuration = get_configuration().expect("Failed to read configuration.");
    let connection_pool =
        PgPool::connect_lazy(configuration.database.connection_string().expose_secret())
            .expect("Failed to create Postgres connection pool");
    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );
    println!("Running application on {}", address);
    let listener = TcpListener::bind(address)?;
    run(listener, connection_pool)?.await
}
