use actix_web::cookie::Cookie;
use actix_web::http::header::ContentType;
use actix_web::{HttpRequest, HttpResponse};

pub async fn login_form(request: HttpRequest) -> HttpResponse {
    let error_html: String = match request.cookie("_flash") {
        None => "".to_string(),
        Some(cookie) => {
            format!("<p><i>{}</i></p>", cookie.value())
        }
    };
    let mut response = HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Login</title>
</head>
<body>
    {error_html}
    <form action="/login" method="post">
        <label>Username
            <input
                type="text"
                placeholder="Enter Username"
                name="username"
            >
        </label>
        <label>Password
            <input
                type="password"
                placeholder="Enter Password"
                name="password"
            >
        </label>
        <button type="submit">Login</button>
    </form>
</body>
</html>"#,
        ));
    // reset the cookie with a max age of 0 to immediately expire, clearing it after we have the error html
    response
        .add_removal_cookie(&Cookie::new("_flash", ""))
        .unwrap();
    response
}
