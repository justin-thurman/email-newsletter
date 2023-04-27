use actix_session::{Session, SessionExt, SessionGetError, SessionInsertError};
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest};
use std::future::{ready, Ready};
use uuid::Uuid;

pub struct TypedSession(Session);

impl TypedSession {
    const USER_ID_KEY: &'static str = "user_id";

    pub fn renew(&self) {
        self.0.renew();
    }

    pub fn insert_user_id(&self, user_id: Uuid) -> Result<(), SessionInsertError> {
        self.0.insert(Self::USER_ID_KEY, user_id)
    }

    pub fn get_user_id(&self) -> Result<Option<Uuid>, SessionGetError> {
        self.0.get(Self::USER_ID_KEY)
    }
}

/// Allows us to use `TypedSession` as an actix_web extractor.
impl FromRequest for TypedSession {
    // this basically says we return the same error returned by
    // the implementation of FromRequest for Session
    type Error = <Session as FromRequest>::Error;
    /* Rust does not yet support async traits, but from_request expects a `Future`
    as a return type to allow for extractors that perform async operations, like HTTP calls.
    We don't really need a future because we're not performing any I/O here, so we wrap
    `TypedSession` into `Ready` to convert it into a future that resolves to the wrapped
    value the first time the executor polls it. */
    type Future = Ready<Result<TypedSession, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        ready(Ok(TypedSession(req.get_session())))
    }
}
