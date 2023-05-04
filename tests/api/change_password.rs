use crate::helpers::{assert_is_redirect_to, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn user_must_be_logged_in_to_see_change_password_form() {
    // arrange
    let app = spawn_app().await;

    // act
    let response = app.get_change_password().await;

    // assert
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn user_must_be_logged_in_to_change_password() {
    // arrange
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();

    // act
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": Uuid::new_v4().to_string(),
            "new_password": &new_password,
            "new_password_check": &new_password,
        }))
        .await;

    // assert
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn new_password_fields_must_match() {
    // arrange
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();
    let non_matching_confirm_password = Uuid::new_v4().to_string();

    // act 1: login
    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    }))
    .await;

    // act 2: try to change password
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &new_password,
            "new_password_check": &non_matching_confirm_password,
        }))
        .await;

    // assert
    assert_is_redirect_to(&response, "/admin/password");

    // act 3: follow the redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains(
        "<p><i>You entered two different new passwords - \
            the field values must match.</i></p>"
    ));
}

#[tokio::test]
async fn current_password_must_be_valid() {
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();
    let wrong_password = Uuid::new_v4().to_string();

    // act 1: login
    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    }))
    .await;

    // act 2: try to change password
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &wrong_password,
            "new_password": &new_password,
            "new_password_check": &new_password,
        }))
        .await;

    // assert
    assert_is_redirect_to(&response, "/admin/password");

    // act 3: follow the redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>The current password is incorrect.</i></p>"));
}

#[tokio::test]
async fn current_password_is_too_short() {
    let app = spawn_app().await;
    let short_password = "1".repeat(12);

    // act 1: login
    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    }))
    .await;

    // act 2: try to change password
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &short_password,
            "new_password_check": &short_password,
        }))
        .await;

    // assert
    assert_is_redirect_to(&response, "/admin/password");

    // act 3: follow the redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>Password must be at least 12 characters.</i></p>"));
}

#[tokio::test]
async fn current_password_is_too_long() {
    let app = spawn_app().await;
    let long_password = "1".repeat(129);

    // act 1: login
    app.post_login(&serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    }))
    .await;

    // act 2: try to change password
    let response = app
        .post_change_password(&serde_json::json!({
            "current_password": &app.test_user.password,
            "new_password": &long_password,
            "new_password_check": &long_password,
        }))
        .await;

    // assert
    assert_is_redirect_to(&response, "/admin/password");

    // act 3: follow the redirect
    let html_page = app.get_change_password_html().await;
    assert!(html_page.contains("<p><i>Password must be no more than 128 characters.</i></p>"));
}
