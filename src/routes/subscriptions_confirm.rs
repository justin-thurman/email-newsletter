use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

/// Handles confirming a subscriber using a subscription token; updates status to confirmed
#[tracing::instrument(name = "Confirm a pending subscriber", skip(parameters))]
pub async fn confirm(
    parameters: web::Query<Parameters>,
    connection_pool: web::Data<PgPool>,
) -> HttpResponse {
    // using web::Query<Parameters> tells actix that the parameters are mandatory; this handler is only called if
    // those query parameters extract; otherwise, returns a 400
    /* My first implementation, before reading the book, was the use a single UPDAte query.
    I see the benefit of the book's approach, of course, but I wanted to get a little in
    the weeds of the SQL. Leaving this here for posterity, since this ia learning project after all.
    sqlx::query!(
        "UPDATE subscriptions
         SET status = 'confirmed'
         FROM subscription_tokens
         WHERE subscriptions.id = subscription_tokens.subscriber_id
         AND subscription_tokens.subscription_token = $1",
        parameters.subscription_token
    )
     */
    Ok(())
}

#[tracing::instrument(
    name = "Mark subscriber as confirmed",
    skip(subscriber_id, connection_pool)
)]
pub async fn confirm_subscriber(
    subscriber_id: Uuid,
    connection_pool: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE subscriptions SET status = 'confirmed' WHERE id = $1
    "#,
        subscriber_id
    )
    .execute(connection_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}

#[tracing::instrument(
    name = "Get subscriber_id from token",
    skip(subscription_token, connection_pool)
)]
pub async fn get_subscriber_id_from_token(
    subscription_token: &str,
    connection_pool: &PgPool,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        "SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1",
        subscription_token,
    )
    .fetch_optional(connection_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(result.map(|r| r.subscriber_id))
}
