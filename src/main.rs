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
        PgPool::connect(configuration.database.connection_string().expose_secret())
            .await
            .expect("Failed to connect to postgres.");
    let address = format!("127.0.0.1:{}", configuration.application_port);
    println!("Running application on {}", address);
    let listener = TcpListener::bind(address)?;
    run(listener, connection_pool)?.await
}
