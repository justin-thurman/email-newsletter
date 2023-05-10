use email_newsletter::configuration::get_configuration;
use email_newsletter::startup::Application;
use email_newsletter::telemetry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = telemetry::get_tracing_subscriber(
        "email-newsletter".into(),
        "info".into(),
        std::io::stdout,
    );
    telemetry::init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration.");

    let application = Application::build(configuration)
        .await
        .expect("Failed to build application");
    application.run_until_stopped().await?;
    Ok(())
}
