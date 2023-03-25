use std::net::TcpListener;

use sqlx::PgPool;
use tracing::subscriber::set_global_default;

use email_newsletter::configuration::get_configuration;
use email_newsletter::startup::run;

mod telemetry;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let subscriber = telemetry::get_tracing_subscriber();
    set_global_default(subscriber).expect("Failed to set subscriber");
    let configuration = get_configuration().expect("Failed to read configuration.");
    let connection_pool = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to postgres.");
    let address = format!("127.0.0.1:{}", configuration.application_port);
    let listener = TcpListener::bind(address)?;
    run(listener, connection_pool)?.await
}
