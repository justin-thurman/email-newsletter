use crate::helpers::spawn_app;

#[tokio::test]
async fn subscribe_with_valid_form_data_returns_200() {
    let test_app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

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
