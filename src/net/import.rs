use crate::dbs::DB;
use crate::err::Error;
use crate::net::input::bytes_to_utf8;
use crate::net::output;
use axum::extract::DefaultBodyLimit;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Extension;
use axum::Router;
use axum::TypedHeader;
use bytes::Bytes;
use http_body::Body as HttpBody;
use surrealdb::dbs::Session;
use tower_http::limit::RequestBodyLimitLayer;

use super::headers::Accept;

const MAX: usize = 1024 * 1024 * 1024 * 4; // 4 GiB

pub(super) fn router<S, B>() -> Router<S, B>
where
	B: HttpBody + Send + 'static,
	B::Data: Send,
	B::Error: std::error::Error + Send + Sync + 'static,
	S: Clone + Send + Sync + 'static,
{
	Router::new()
		.route("/import", post(handler))
		.route_layer(DefaultBodyLimit::disable())
		.layer(RequestBodyLimitLayer::new(MAX))
}

async fn handler(
	Extension(session): Extension<Session>,
	maybe_output: Option<TypedHeader<Accept>>,
	sql: Bytes,
) -> Result<impl IntoResponse, impl IntoResponse> {
	// Check the permissions
	match session.au.is_db() {
		true => {
			// Get the datastore reference
			let db = DB.get().unwrap();
			// Convert the body to a byte slice
			let sql = bytes_to_utf8(&sql)?;
			// Execute the sql query in the database
			match db.execute(sql, &session, None).await {
				Ok(res) => match maybe_output.as_deref() {
					// Simple serialization
					Some(Accept::ApplicationJson) => Ok(output::json(&output::simplify(res))),
					Some(Accept::ApplicationCbor) => Ok(output::cbor(&output::simplify(res))),
					Some(Accept::ApplicationPack) => Ok(output::pack(&output::simplify(res))),
					// Internal serialization
					Some(Accept::Surrealdb) => Ok(output::full(&res)),
					// Return nothing
					Some(Accept::ApplicationOctetStream) => Ok(output::none()),
					// An incorrect content-type was requested
					_ => Err(Error::InvalidType),
				},
				// There was an error when executing the query
				Err(err) => Err(Error::from(err)),
			}
		}
		_ => Err(Error::InvalidAuth),
	}
}
