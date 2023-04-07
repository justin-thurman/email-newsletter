use email_newsletter::configuration::get_configuration;
use email_newsletter::startup::build;
use email_newsletter::telemetry;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let subscriber = telemetry::get_tracing_subscriber("email-newsletter", "info", std::io::stdout);
    telemetry::init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration.");

    let server = build(configuration).await?;
    server.await?;
    Ok(())
}
