use std::fmt::{Debug, Display};
use tokio::task::JoinError;
use email_newsletter::configuration::get_configuration;
use email_newsletter::issue_delivery_worker::run_worker_until_stopped;
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

    let application = Application::build(configuration.clone())
        .await?;
    let application_task = tokio::spawn(application.run_until_stopped());
    let worker_task = tokio::spawn(run_worker_until_stopped(configuration));
    
    tokio::select! {
        output = application_task => report_exit("API", output),
        output = worker_task => report_exit("Background worker", output),
    };
        
    Ok(())
}

fn report_exit(
    task_name: &str,
    outcome: Result<Result<(), impl Debug + Display>, JoinError>
) {
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name)
        }
        Ok(Err(e)) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{} failed",
                task_name
            )
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{} task failed to complete",
                task_name
            )
        }
    }
}
