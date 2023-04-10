use crate::helpers::spawn_app;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn confirmations_without_token_are_rejected_with_400() {
    // arrange
    let app = spawn_app().await;

    // act
    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    // assert
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
    // arrange
    // make post to subscriptions
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.to_string()).await;
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request).await;

    // act
    // make get request to the confirm endpoint
    let response = reqwest::get(confirmation_links.html).await.unwrap();

    // assert
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn clicking_on_the_confirmation_link_confirms_a_subscriber() {
    // arrange
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.to_string()).await;
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request).await;

    // act
    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // assert
    let saved_subscirber = sqlx::query!("SELECT email, name, status FROM subscriptions",)
        .fetch_one(&app.connection_pool)
        .await
        .expect("Failed to fetch saved subscriber");
    assert_eq!(saved_subscirber.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved_subscirber.name, "le guin");
    assert_eq!(saved_subscirber.status, "confirmed");
}
