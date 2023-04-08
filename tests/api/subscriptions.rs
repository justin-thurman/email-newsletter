use crate::helpers::spawn_app;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn subscribe_with_valid_form_data_returns_200() {
    let test_app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&test_app.email_server)
        .await;

    // act
    let response = test_app.post_subscriptions(body.to_string()).await;

    // assert
    assert_eq!(200, response.status().as_u16());

    let saved_subscriber = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&test_app.connection_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved_subscriber.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved_subscriber.name, "le guin");
}

#[tokio::test]
async fn subscribe_with_missing_form_data_returns_400() {
    // arrange
    let test_app = spawn_app().await;
    let test_cases = vec![
        ("name=le%20guin", "missing email"),
        ("email=ursula_le_guin%40gmail.com", "missing name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        // act
        let response = test_app.post_subscriptions(invalid_body.to_string()).await;

        // assert
        assert_eq!(
            400,
            response.status().as_u16(),
            "API did not fail with 400 when payload was {}",
            error_message
        )
    }
}

#[tokio::test]
async fn subscribe_with_invalid_fields_returns_400() {
    // arrange
    let test_app = spawn_app().await;
    let test_cases = vec![
        ("name=&email=test%40email.com", "empty name"),
        ("name=test&email=", "empty email"),
        ("name=test&email=invalid-email", "invalid email"),
    ];

    for (invalid_body, error_message) in test_cases {
        // act
        let response = test_app.post_subscriptions(invalid_body.to_string()).await;

        // assert
        assert_eq!(
            400,
            response.status().as_u16(),
            "API did not fail with 400 when payload was {}",
            error_message
        )
    }
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data() {
    // arrange
    let app = spawn_app().await;
    let body = "name=test&email=test%40email.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // act
    app.post_subscriptions(body.to_string()).await;

    // mock asserts when dropped
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_with_a_link() {
    // arrange
    let app = spawn_app().await;
    let body = "name=test&email=test%40email.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    // act
    app.post_subscriptions(body.to_string()).await;

    // assert
    // get the first intercepted request
    let request = &app
        .email_server
        .received_requests()
        .await
        .expect("Failed to unwrap request")[0];
    // parse the body as JSON, starting from raw bytes
    let body: serde_json::Value =
        serde_json::from_slice(&request.body).expect("Failed to unwrap request body");

    // extract the link from one of the request fields
    let get_link = |s: &str| {
        let links: Vec<_> = linkify::LinkFinder::new()
            .links(s)
            .filter(|l| *l.kind() == linkify::LinkKind::Url)
            .collect();
        links[0].as_str().to_string()
    };

    let html_link = get_link(body["HtmlBody"].as_str().unwrap());
    let text_link = get_link(body["TextBody"].as_str().unwrap());

    assert_eq!(html_link, text_link)
}
