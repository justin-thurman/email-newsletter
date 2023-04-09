use actix_web::{web, HttpResponse};

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

/// Handles confirming a subscriber using a subscription token; updates status to confirmed
#[tracing::instrument(name = "Confirm a pending subscriber", skip(_parameters))]
pub async fn confirm(_parameters: web::Query<Parameters>) -> HttpResponse {
    // using web::Query<Parameters> tells actix that the parameters are mandatory; this handler is only called if 
    // those query parameters extract; otherwise, returns a 400
    HttpResponse::Ok().finish()
}
