//! FHIR REST Client Implementation.

mod aliases;
mod auth;
mod builder;
mod error;
mod fhir;
mod misc;
mod request;
mod search;

use std::{marker::PhantomData, sync::Arc};

use misc::parse_major_fhir_version;
use reqwest::{header, StatusCode, Url};

use self::auth::AuthCallback;
pub use self::{
	aliases::*, auth::LoginManager, builder::ClientBuilder, error::Error, fhir::*,
	request::RequestSettings, search::SearchParameters,
};
use crate::version::{DefaultVersion, FhirR4B, FhirR5, FhirStu3, FhirVersion};

/// FHIR REST Client.
pub struct Client<Version = DefaultVersion>(Arc<ClientData>, PhantomData<Version>);

/// FHIR Rest Client data.
struct ClientData {
	/// The FHIR server's base URL.
	base_url: Url,
	/// HTTP request client.
	client: reqwest::Client,
	/// Request settings.
	request_settings: std::sync::Mutex<RequestSettings>,
	/// Authorization callback method, returning the authorization header value.
	auth_callback: tokio::sync::Mutex<Option<AuthCallback>>,

	/// Whether to error if the server responds with a different major FHIR
	/// version.
	error_on_version_mismatch: bool,
	/// Whether to error before we try to send a request to a different server
	/// than is configured in the base URL. Does not apply to manual requests.
	/// Mostly applies to search results and references to resources on other
	/// server.
	error_on_origin_mismatch: bool,
}

impl<V: FhirVersion> From<ClientData> for Client<V> {
	fn from(data: ClientData) -> Self {
		Self(Arc::new(data), PhantomData)
	}
}

impl<V: FhirVersion> Client<V> {
	/// Start building a new client with custom settings.
	#[must_use]
	pub fn builder() -> ClientBuilder<V> {
		ClientBuilder::default()
	}

	/// Create a new client with default settings.
	pub fn new(base_url: Url) -> Result<Self, Error> {
		Self::builder().base_url(base_url).build()
	}

	/// Get the configured base URL.
	#[must_use]
	pub fn base_url(&self) -> &Url {
		&self.0.base_url
	}

	/// Get the URL with the configured base URL and the given path segments.
	fn url(&self, segments: &[&str]) -> Url {
		let mut url = self.0.base_url.clone();
		#[allow(clippy::expect_used)] // We made sure of it in the constructor.
		url.path_segments_mut().expect("Base URL cannot be base").pop_if_empty().extend(segments);
		url
	}

	/// Get the request settings configured in this client.
	#[must_use]
	pub fn request_settings(&self) -> RequestSettings {
		#[allow(clippy::expect_used)] // only happens on panics, so we can panic again.
		self.0.request_settings.lock().expect("mutex poisened").clone()
	}

	/// Set the request settings for this client. Be warned that this can be
	/// subject to race conditions. Prefer to use
	/// [Self::patch_request_settings]. Basically same as
	/// `client.patch_request_settings(|_| new_settings)`.
	pub fn set_request_settings(&self, settings: RequestSettings) {
		tracing::debug!("Setting new request settings");
		#[allow(clippy::expect_used)] // only happens on panics, so we can panic again.
		let mut request_settings = self.0.request_settings.lock().expect("mutex poisened");
		*request_settings = settings;
	}

	/// Patch the request settings atomically. Blocks all requests until the
	/// change to request settings is finished.
	pub fn patch_request_settings<F>(&self, mutator: F)
	where
		F: FnOnce(RequestSettings) -> RequestSettings,
	{
		tracing::debug!("Patching request settings");
		#[allow(clippy::expect_used)] // only happens on panics, so we can panic again.
		let mut request_settings = self.0.request_settings.lock().expect("mutex poisened");
		let patched = mutator(request_settings.clone());
		*request_settings = patched;
	}

	/// Convert to a different version.
	fn convert_version<Version>(self) -> Client<Version> {
		Client(self.0, PhantomData)
	}

	/// Switch the client to STU3 mode.
	#[must_use]
	pub fn stu3(self) -> Client<FhirStu3> {
		self.convert_version()
	}

	/// Switch the client to R4B mode.
	#[must_use]
	pub fn r4b(self) -> Client<FhirR4B> {
		self.convert_version()
	}

	/// Switch the client to R5 mode.
	#[must_use]
	pub fn r5(self) -> Client<FhirR5> {
		self.convert_version()
	}

	/// Run a request using the internal request settings, calling the auth
	/// callback to retrieve a new Authorization header on `unauthtorized`
	/// responses.
	async fn run_request(
		&self,
		request: reqwest::RequestBuilder,
	) -> Result<reqwest::Response, Error> {
		// Check the URL origin if configured to ensure equality.
		if self.0.error_on_origin_mismatch {
			let request = request.try_clone().ok_or(Error::RequestNotClone)?.build()?;
			// Make sure we are not forwarded to any malicious server.
			if request.url().origin() != self.0.base_url.origin() {
				return Err(Error::DifferentOrigin(request.url().to_string()));
			}
		}

		// Try running the request
		let mut request_settings = self.request_settings();
		let mut response = request_settings
			.make_request(request.try_clone().ok_or(Error::RequestNotClone)?)
			.await?;

		// On authorization failure, retry after refreshing the authorization header.
		if response.status() == StatusCode::UNAUTHORIZED {
			if let Ok(mut auth_callback) = self.0.auth_callback.try_lock() {
				if let Some(auth_callback) = auth_callback.as_mut() {
					tracing::info!("Hit unauthorized response, calling auth_callback");
					let auth_value = auth_callback
						.authenticate(self.0.client.clone())
						.await
						.map_err(|err| Error::AuthCallback(format!("{err:#}")))?;
					self.patch_request_settings(move |settings| {
						settings.header(header::AUTHORIZATION, auth_value)
					});
				} else {
					// There is no auth callback, return without retrying.
					return Ok(response);
				}
			} else {
				// Auth callback was blocked, we assume there was a login in flight and update
				// our request settings after it is done.
				_ = self.0.auth_callback.lock().await;
			}
			// Retry request with new request settings.
			request_settings = self.request_settings();
			response = request_settings.make_request(request).await?;
		}

		// Test server FHIR version in response, if configured to do so.
		if self.0.error_on_version_mismatch {
			if let Some(version) = parse_major_fhir_version(response.headers())? {
				let expected = V::VERSION.split_once('.').map_or(V::VERSION, |(major, _)| major);
				if version != expected {
					return Err(Error::DifferentFhirVersion(version.to_owned()));
				}
			}
		}

		Ok(response)
	}

	/// Send a custom HTTP request anywhere you want, but using this client's
	/// internal HTTP machinery. The machinery includes automatic authentication
	/// if configured (`auth_callback`) and automatic retrying of requests on
	/// connection problems (as per `request_settings`).
	pub async fn send_custom_request<F>(&self, make_request: F) -> Result<reqwest::Response, Error>
	where
		F: FnOnce(&reqwest::Client) -> reqwest::RequestBuilder + Send,
	{
		let request = (make_request)(&self.0.client);
		self.run_request(request).await
	}
}

impl<V> Clone for Client<V> {
	fn clone(&self) -> Self {
		Self(self.0.clone(), self.1)
	}
}

impl std::fmt::Debug for ClientData {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let auth_callback = match self.auth_callback.try_lock() {
			Ok(inside) => {
				if inside.is_some() {
					"Some(<login_manager>)"
				} else {
					"None"
				}
			}
			Err(_) => "<blocked>",
		};

		f.debug_struct("ClientData")
			.field("base_url", &self.base_url)
			.field("client", &self.client)
			.field("request_settings", &self.request_settings)
			.field("auth_callback", &auth_callback)
			.finish()
	}
}

impl<V> std::fmt::Debug for Client<V> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("Client").field(&self.0).finish()
	}
}

#[cfg(test)]
mod tests;
