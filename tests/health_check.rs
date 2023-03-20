#[tokio::test]
async fn health_check_responds_200() {
    // arrange
    spawn_app();
    let client = reqwest::Client::new();

    // act
    let response = client
        .get("http://127.0.0.1:8000/health_check")
        .send()
        .await
        .expect("Failed to execute request");

    // assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

fn spawn_app() {
    let server = email_newsletter::run().expect("Failed to bind address");
    let _ = tokio::spawn(server);
}
