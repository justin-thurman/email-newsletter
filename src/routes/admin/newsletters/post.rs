use std::fmt::{Debug, Formatter};

use actix_web::body::BoxBody;
use actix_web::http::{header, StatusCode};
use actix_web::{web, HttpResponse, ResponseError};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::authentication::UserId;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::error_handling::error_chain_fmt;
use crate::idempotency::{save_response, try_processing, IdempotencyKey, NextAction};
use crate::routes::get_username;
use crate::routing_helpers::{e400, e500, see_other};

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl Debug for PublishError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        // by default, `error_response` invokes `status_code`, but since we have a bespoke `error_response`
        // implementation, we don't need `status_code`
        match self {
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PublishError::AuthError(_) => HttpResponse::build(StatusCode::UNAUTHORIZED)
                .append_header((header::WWW_AUTHENTICATE, r#"Basic realm="publish""#))
                .finish(),
        }
    }
}

#[tracing::instrument(
name = "Publish a newsletter issue",
skip(form, pool, email_client, user_id),
fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = *user_id.into_inner();
    let username = get_username(user_id, &pool)
        .await
        .map_err(PublishError::AuthError)?;
    tracing::Span::current().record("username", &tracing::field::display(username));
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));

    let FormData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;
    let transaction = match try_processing(&pool, &idempotency_key, user_id)
        .await
        .map_err(e500)?
    {
        NextAction::StartProcessing(transaction) => transaction,
        NextAction::ReturnSavedResponse(response) => {
            success_message().send();
            return Ok(response);
        }
    };

    let confirmed_subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;
    for subscriber in confirmed_subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &title,
                        &html_content,
                        &text_content,
                        // `with_context` is lazy, unlike `context`; used when the message has a runtime cost, as here
                        // where format allocates on the heap; note that must bring `anyhow::Context` trait into scope to use
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })
                    .map_err(e500)?;
            }
            Err(error) => {
                tracing::warn!(
                    // recording the error chain as a structured field on the log record
                    error.cause_chain = ?error,
                    "Skipping a confirmed subscriber. Their stored contact details are invalid."
                );
            }
        }
    }
    success_message().send();
    let response = see_other("/admin/newsletters");
    let response = save_response(transaction, &idempotency_key, user_id, response)
        .await
        .map_err(e500)?;
    Ok(response)
}

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been published!")
}

/// Gets all confirmed subscribers
#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?;
    let confirmed_subscribers = rows
        .into_iter()
        .map(|row| match SubscriberEmail::parse(row.email) {
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(error) => {
                tracing::warn!(
                    "A confirmed subscriber is using an invalid email address.\n{}.",
                    error
                );
                Err(anyhow::anyhow!(error))
            }
        })
        .collect();
    Ok(confirmed_subscribers)
}

/// Inserts a new newsletter issue
#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_issue_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO newsletter_issues (
            newsletter_issue_id,
            title,
            text_content,
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, now())
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content
    )
    .execute(transaction)
    .await?;
    Ok(newsletter_issue_id)
}

/// Inserts a newsletter delivery task into the queue table
#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_id,
            subscriber_email
        )
        SELECT $1, email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id
    )
    .execute(transaction)
    .await?;
    Ok(())
}
