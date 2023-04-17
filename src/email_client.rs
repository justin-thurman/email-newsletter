use reqwest::{Client, Url};
use secrecy::{ExposeSecret, Secret};

use crate::domain::SubscriberEmail;

pub struct EmailClient {
    sender: SubscriberEmail,
    http_client: Client,
    base_url: Url,
    authorization_token: Secret<String>,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        authorization_token: Secret<String>,
        timeout: std::time::Duration,
    ) -> Self {
        // more type-driven development: take a string, parse as a Url. Now we know, from this point forward,
        // that base_url is valid.
        let base_url = Url::parse(&base_url).expect("Failed to parse base_url");

        // building new http_client with a timeout; could also use per-request timeouts
        let http_client = Client::builder().timeout(timeout).build().unwrap();

        Self {
            http_client,
            base_url,
            sender,
            authorization_token,
        }
    }

    pub async fn send_email(
        &self,
        recipient: &SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = self
            .base_url
            .join("/email")
            .expect("Failed to join /email with base url");

        let request_body = SendEmailRequest {
            from: self.sender.as_ref(),
            to: recipient.as_ref(),
            subject,
            html_body: html_content,
            text_body: text_content,
        };

        self.http_client
            .post(url) // doesn't actually send request; that's what `send` method is for
            .header(
                "X-Postmark-Server-Token",
                self.authorization_token.expose_secret(),
            )
            .json(&request_body) // also sets appropriate content-type headers
            .send()
            .await?
            .error_for_status()?;
        /* Note that `send` only returns an error if sending the request failed, if a redirect loop
        was detected, or the redirect limit was exhausted. It does not return errors based on status codes,
        so we need to do that manually with `error_for_status`. */

        Ok(())
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct SendEmailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    html_body: &'a str,
    text_body: &'a str,
}

#[cfg(test)]
mod tests {
    use claims::{assert_err, assert_ok};
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker};
    use secrecy::Secret;
    use wiremock::matchers::{any, header, header_exists, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;

    struct SendEmailBodyMatcher;

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            // Try to parse the body as JSON
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                // check that the body contains mandatory fields
                body.get("From").is_some()
                    && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("HtmlBody").is_some()
                    && body.get("TextBody").is_some()
            } else {
                false
            }
        }
    }

    /// Generates a new email client for tests, using a random sender email and authorization token.
    fn email_client(base_url: String) -> EmailClient {
        EmailClient::new(
            base_url,
            email(),
            Secret::new(Faker.fake()),
            std::time::Duration::from_millis(100),
        )
    }

    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    fn content() -> String {
        Paragraph(1..10).fake()
    }

    fn subject() -> String {
        Sentence(1..2).fake()
    }

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        // Arrange
        let mock_server = MockServer::start().await; // spins up a server on random available port
        let email_client = email_client(mock_server.uri());

        // by default, MockServer returns 404 to all requests; we mount a mock to change this behavior
        Mock::given(header_exists("X-Postmark-Server-Token")) // match requests with that header
            .and(header("Content-Type", "application/json")) // and with this header, etc...
            .and(path("/email"))
            .and(method("POST"))
            .and(SendEmailBodyMatcher)
            .respond_with(ResponseTemplate::new(200)) // response with 200, no body
            .expect(1) // expect 1 incoming request; panics if not met by time mock server goes out of scope
            .mount(&mock_server) // mount the mock to the server
            .await;

        let subscriber_email = email();
        let subject = subject();
        let content = content();

        // Act
        let _ = email_client
            .send_email(&subscriber_email, &subject, &content, &content)
            .await;

        // Assert handled by Mock...expect(1)
    }

    #[tokio::test]
    async fn send_email_succeeds_if_server_returns_200() {
        // arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject = subject();
        let content = content();

        // matching any request here, as this test is about the behavior of our EmailClient, given a 200 response
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        // act
        let result = email_client
            .send_email(&subscriber_email, &subject, &content, &content)
            .await;

        // assert
        assert_ok!(result);
    }

    #[tokio::test]
    async fn send_email_fails_if_server_returns_500() {
        // arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject = subject();
        let content = content();

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        // act
        let result = email_client
            .send_email(&subscriber_email, &subject, &content, &content)
            .await;

        // assert
        assert_err!(result);
    }

    #[tokio::test]
    async fn send_email_times_out_if_server_takes_too_long() {
        // arrange
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject = subject();
        let content = content();

        let response = ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(11));
        Mock::given(any())
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;

        // act
        let result = email_client
            .send_email(&subscriber_email, &subject, &content, &content)
            .await;

        // assert
        assert_err!(result);
    }
}
