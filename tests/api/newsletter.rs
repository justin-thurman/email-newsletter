use crate::helpers::{spawn_app, ConfirmationLinks, TestApp};
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscirbers() {
    // arrange
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    // act
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Plaintext newsletter",
            "html": "<p>HTML newsletter</p>"
        }
    });
    let response = app.post_newsletter(newsletter_request_body).await;

    // assert
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscirbers() {
    // arrange
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // act
    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Plaintext newsletter",
            "html": "<p>HTML newsletter</p>"
        }
    });
    let response = app.post_newsletter(newsletter_request_body).await;

    // assert
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn newsletters_returns_400_for_invalid_data() {
    // arrange
    let app = spawn_app().await;
    let test_cases = vec![
        (
            serde_json::json!({
                "content": {
                    "text": "Valid",
                    "html": "<p>Valid</p>",
                }
            }),
            "missing title",
        ),
        (serde_json::json!({"title": "Valid"}), "missing content"),
    ];

    for (invalid_body, error_message) in test_cases {
        // act
        let response = app.post_newsletter(invalid_body).await;

        // assert
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 when the payload was {}",
            error_message
        );
    }
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

#[tokio::test]
async fn requests_missing_authorization_header_are_rejected() {
    // arrange
    let app = spawn_app().await;

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", app.address))
        .json(&serde_json::json!(
            {
                "title": "Newsletter title",
                "content": {
                    "text": "Plaintext",
                    "html": "Html",
                }
            }
        ))
        .send()
        .await
        .expect("Failed to execute request.");

    // assert
    assert_eq!(401, response.status().as_u16());
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-Authenticate"]
    );
}

#[tokio::test]
async fn non_existing_user_is_rejected() {
    // arrange
    let app = spawn_app().await;
    // random credentials
    let username = uuid::Uuid::new_v4().to_string();
    let password = uuid::Uuid::new_v4().to_string();

    // act
    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", &app.address))
        .basic_auth(username, Some(password))
        .json(&serde_json::json!({
            "title": "Title",
            "content": {
                "text": "Plaintext",
                "html": "HTML",
            }
        }))
        .send()
        .await
        .expect("Failed to execute request.");

    // assert
    assert_eq!(401, response.status().as_u16());
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-Authenticate"]
    )
}

#[tokio::test]
async fn invalid_password_is_rejected() {
    // arrange
    let app = spawn_app().await;
    let username = &app.test_user.username;
    // random password
    let password = uuid::Uuid::new_v4().to_string();
    assert_ne!(password, app.test_user.password);

    // act
    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", &app.address))
        .basic_auth(username, Some(password))
        .json(&serde_json::json!({
            "title": "Title",
            "content": {
                "text": "Plaintext",
                "html": "HTML",
            }
        }))
        .send()
        .await
        .expect("Failed to execute request.");

    // assert
    assert_eq!(401, response.status().as_u16());
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-Authenticate"]
    )
}
