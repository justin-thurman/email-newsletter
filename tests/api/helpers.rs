use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version};
use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use wiremock::MockServer;

use email_newsletter::configuration::{get_configuration, DatabaseSettings};
use email_newsletter::email_client::EmailClient;
use email_newsletter::issue_delivery_worker::{try_execute_task, ExecutionOutcome};
use email_newsletter::startup::{get_connection_pool, Application};
use email_newsletter::telemetry::{get_tracing_subscriber, init_subscriber};

// ensure that the tracing stack is only initialized once
static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber =
            get_tracing_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber =
            get_tracing_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

/// User info to use in tests
pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }

    async fn store(&self, pool: &PgPool) {
        let salt = SaltString::generate(&mut rand::thread_rng());
        let password_hash = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(15000, 2, 1, None).unwrap(),
        )
        .hash_password(self.password.as_bytes(), &salt)
        .unwrap()
        .to_string();
        sqlx::query!(
            "INSERT INTO users (user_id, username, password_hash) VALUES ($1, $2, $3)",
            self.user_id,
            self.username,
            password_hash
        )
        .execute(pool)
        .await
        .expect("Failed to store test user");
    }
}

/// Confirmation links embedded in request bodies to the email API.
pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url,
}

/// A struct holding data needed to access a test version of our application
pub struct TestApp {
    pub address: String,
    pub connection_pool: PgPool,
    // email_server stands in for Postmark's API
    pub email_server: MockServer,
    pub port: u16,
    pub test_user: TestUser,
    pub api_client: reqwest::Client,
    pub email_client: EmailClient,
}

impl TestApp {
    pub async fn dispatch_all_pending_emails(&self) {
        loop {
            if let ExecutionOutcome::EmptyQueue =
                try_execute_task(&self.connection_pool, &self.email_client)
                    .await
                    .unwrap()
            {
                break;
            }
        }
    }

    /// Gets the logout endpoint
    pub async fn post_logout(&self) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/admin/logout", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    /// Returns the change password get response
    pub async fn get_change_password(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/password", &self.address))
            .send()
            .await
            .expect("Failed to execute request")
    }

    /// Returns the rendered HTML string from a GET request to /admin/password
    pub async fn get_change_password_html(&self) -> String {
        self.get_change_password().await.text().await.unwrap()
    }

    /// Posts to the change password endpoint
    pub async fn post_change_password<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/admin/password", self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    /// Returns the rendered HTML string from a GET request to the /login endpoint
    pub async fn get_login_html(&self) -> String {
        self.api_client
            .get(&format!("{}/login", &self.address))
            .send()
            .await
            .expect("Failed to execute request")
            .text()
            .await
            .unwrap()
    }

    /// Posts a request to the login endpoint
    pub async fn post_login<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/login", &self.address))
            // the `form` method makes sure the body is URL-encoded and the
            // `Content-Type` header is set appropriately
            .form(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    /// Logs in with default credentials
    pub async fn default_login(&self) -> reqwest::Response {
        let login_body = serde_json::json!({
            "username": self.test_user.username,
            "password": self.test_user.password,
        });
        self.post_login(&login_body).await
    }

    /// Gets the admin dashboard endpoint
    pub async fn get_admin_dashboard(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/dashboard", &self.address))
            .send()
            .await
            .expect("Failed to execute request")
    }

    /// Gets the HTML of the admin dashboard endpoint
    pub async fn get_admin_dashboard_html(&self) -> String {
        self.get_admin_dashboard().await.text().await.unwrap()
    }

    /// Posts the provided body to the subscriptions endpoint
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    /// Posts the provided body to the newsletters endpoint
    pub async fn post_newsletter(&self, body: &serde_json::Value) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/admin/newsletters", self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    /// Get newsletter endpoint
    pub async fn get_newsletter(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/newsletters", self.address))
            .send()
            .await
            .expect("Failed to execute request")
    }

    /// Get newsletter html content
    pub async fn get_newsletter_html(&self) -> String {
        self.get_newsletter().await.text().await.unwrap()
    }

    /// Extracts confirmation links from mocked email API requests
    pub async fn get_confirmation_links(
        &self,
        email_request: &wiremock::Request,
    ) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        // extract the link from one of the request fields
        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let confirmation_link = links[0].as_str().to_string();
            let mut confirmation_link = reqwest::Url::parse(&confirmation_link).unwrap();
            // make sure the confirmation link points to our address, so we don't accidentally call live servers
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            // manually update the confirmation link to use the correct port; only necessary for testing purposes
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html = get_link(body["HtmlBody"].as_str().unwrap());
        let plain_text = get_link(body["TextBody"].as_str().unwrap());

        ConfirmationLinks { html, plain_text }
    }
}

/// Spawns an app inside a future and returns the configured TestApp.
pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);
    let email_server = MockServer::start().await;

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration");
        // Use a difference database for each test case
        c.database.database_name = Uuid::new_v4().to_string();
        // Use a random OS port
        c.application.port = 0;
        // User the mock server's uri as email API
        c.email_client.base_url = email_server.uri();
        c
    };

    // Create and migrate the database
    configure_database(&configuration.database).await;

    // Launch the application as a background task
    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application");
    let port = application.port();
    let address = format!("http://127.0.0.1:{}", port);
    tokio::spawn(application.run_until_stopped());

    // create a request client that stores cookies and store it in test app
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    let test_app = TestApp {
        address,
        connection_pool: get_connection_pool(&configuration.database),
        email_server,
        port,
        test_user: TestUser::generate(),
        api_client: client,
        email_client: configuration.email_client.client(),
    };
    test_app.test_user.store(&test_app.connection_pool).await;
    test_app
}

// Configures a test database, running all migrations, and then returning the connection pool handle
// needed to use the test database.
async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failed to connect to postgres.");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database");

    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connect to postgres.");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}

/// Asserts that a given redirect is to the provided location
pub fn assert_is_redirect_to(response: &reqwest::Response, location: &str) {
    assert_eq!(response.status().as_u16(), 303);
    assert_eq!(response.headers().get("Location").unwrap(), location);
}
