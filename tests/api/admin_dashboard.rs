use crate::helpers::{assert_is_redirect_to, spawn_app};

#[tokio::test]
async fn user_must_be_logged_in_to_access_admin_dashboard() {
    // arrange
    let app = spawn_app().await;

    // act
    let response = app.get_admin_dashboard().await;

    // assert
    assert_is_redirect_to(&response, "/login");
}
