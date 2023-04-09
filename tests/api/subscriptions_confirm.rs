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
    let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

    // extract the link from one of the request fields
    let get_link = |s: &str| {
        let links: Vec<_> = linkify::LinkFinder::new()
            .links(s)
            .filter(|l| *l.kind() == linkify::LinkKind::Url)
            .collect();
        assert_eq!(links.len(), 1);
        links[0].as_str().to_string()
    };

    let confirmation_link = &get_link(body["HtmlBody"].as_str().unwrap());
    let mut confirmation_link = reqwest::Url::parse(confirmation_link).unwrap();
    // make sure the confirmation link points to our address, so we don't accidentally call live servers
    assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
    // manually update the confirmation link to use the correct port; only necessary for testing purposes
    confirmation_link.set_port(Some(app.port)).unwrap();

    // act
    // make get request to the confirm endpoint
    let response = reqwest::get(confirmation_link).await.unwrap();

    // assert
    assert_eq!(response.status().as_u16(), 200);
}
