use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockBuilder, ResponseTemplate};

use crate::helpers::{assert_is_redirect_to, spawn_app, ConfirmationLinks, TestApp};

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    // arrange
    let app = spawn_app().await;
    app.default_login().await;
    create_unconfirmed_subscriber(&app).await;

    when_sending_an_email()
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    // act
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });
    let response = app.post_newsletter(&newsletter_request_body).await;

    // assert
    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_newsletter_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    // arrange
    let app = spawn_app().await;
    app.default_login().await;
    create_confirmed_subscriber(&app).await;

    when_sending_an_email()
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // act
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });
    let response = app.post_newsletter(&newsletter_request_body).await;

    // assert
    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_newsletter_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));
}

#[tokio::test]
async fn newsletter_delivery_is_idempotent() {
    // arrange
    let app = spawn_app().await;
    app.default_login().await;
    create_confirmed_subscriber(&app).await;

    when_sending_an_email()
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // act 1: first newsletter delivery
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });
    let response = app.post_newsletter(&newsletter_request_body).await;

    // assert
    assert_is_redirect_to(&response, "/admin/newsletters");
    let html_page = app.get_newsletter_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));

    // act 2: second newsletter delivery
    let response = app.post_newsletter(&newsletter_request_body).await;

    // assert
    assert_is_redirect_to(&response, "/admin/newsletters");
    let html_page = app.get_newsletter_html().await;
    assert!(html_page.contains("<p><i>The newsletter issue has been published!</i></p>"));
    // Upon drop, mock asserts that only a single call to the email server was made
}

#[tokio::test]
async fn concurrent_form_submission_is_handled_gracefully() {
    // arrange
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    app.default_login().await;

    when_sending_an_email()
        // setting a long delay to ensure that the second request arrives before first completes
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // act 1: submit two newsletter forms concurrently
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });
    let first_response = app.post_newsletter(&newsletter_request_body);
    let second_response = app.post_newsletter(&newsletter_request_body);
    let (first_response, second_response) = tokio::join!(first_response, second_response);

    // assert
    assert_eq!(first_response.status(), second_response.status());
    assert_eq!(
        first_response.text().await.unwrap(),
        second_response.text().await.unwrap()
    );

    // mock verifies on drop that we sent the newsletter once
}

#[tokio::test]
async fn newsletters_returns_400_for_invalid_data() {
    // arrange
    let app = spawn_app().await;
    app.default_login().await;
    let test_cases = vec![
        (
            serde_json::json!({
            "text_content": "Newsletter body as plain text",
            "html_content": "<p>Newsletter body as HTML</p>",
            "idempotency_key": uuid::Uuid::new_v4().to_string(),
            }),
            "missing title",
        ),
        (serde_json::json!({"title": "Valid"}), "missing content"),
    ];

    for (invalid_body, error_message) in test_cases {
        // act
        let response = app.post_newsletter(&invalid_body).await;

        // assert
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 when the payload was {}",
            error_message
        );
    }
}

#[tokio::test]
async fn must_be_logged_in_to_post_newsletter() {
    // arrange
    let app = spawn_app().await;

    // act
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content": "Newsletter body as plain text",
        "html_content": "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });
    let response = app.post_newsletter(&newsletter_request_body).await;

    // assert
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn must_be_logged_in_to_get_newsletter() {
    // arrange
    let app = spawn_app().await;

    // act
    let response = app.get_newsletter().await;

    // assert
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn transient_errors_do_not_cause_duplicate_deliveries_on_retries() {
    todo!()
}

/// Returns the mock builder used for mocking the email server
fn when_sending_an_email() -> MockBuilder {
    Mock::given(path("/email")).and(method("POST"))
}

/// Using the public API of app under test to create unconfirmed subscriber
async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=test&email=test%40email.com";

    // by using mount_as_scoped here, this mock is only active while the returned MockGuard is in scope
    // i.e., this mock stops working once we leave `create_unconfirmed_subscriber`;
    // this is important, as the mock in `newsletters_are_not_delivered_to_unconfirmed_subscribers` overlaps
    // with this mock
    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;
    app.post_subscriptions(body.to_string())
        .await
        .error_for_status()
        .unwrap();

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();
    app.get_confirmation_links(email_request).await
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_links = create_unconfirmed_subscriber(app).await;
    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}
