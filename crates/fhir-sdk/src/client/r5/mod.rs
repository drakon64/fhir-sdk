//! FHIR R5 client implementation.

mod patch;
mod transaction;

use fhir_model::r5::{
	codes::SubscriptionPayloadContent,
	resources::{
		Bundle, Parameters, ParametersParameter, ParametersParameterValue, Patient, Resource,
		ResourceType, SubscriptionStatus,
	},
};
use reqwest::header;

use self::{
	patch::{PatchViaFhir, PatchViaJson},
	transaction::BatchTransaction,
};
use super::{Client, Error, FhirR5};
use crate::version::FhirVersion;

impl Client<FhirR5> {
	/// Operation `$everything` on `Encounter`, returning a Bundle with all
	/// resources for an `Encounter` record.
	pub async fn operation_encounter_everything(&self, id: &str) -> Result<Bundle, Error> {
		let url = self.url(&["Encounter", id, "$everything"]);
		let request = self.0.client.get(url).header(header::ACCEPT, FhirR5::MIME_TYPE);

		let response = self.run_request(request).await?;
		if response.status().is_success() {
			let resource: Bundle = response.json().await?;
			Ok(resource)
		} else {
			Err(Error::from_response::<FhirR5>(response).await)
		}
	}

	/// Operation `$everything` on `Patient`, returning a Bundle with all
	/// resources for an `Patient` record.
	pub async fn operation_patient_everything(&self, id: &str) -> Result<Bundle, Error> {
		let url = self.url(&["Patient", id, "$everything"]);
		let request = self.0.client.get(url).header(header::ACCEPT, FhirR5::MIME_TYPE);

		let response = self.run_request(request).await?;
		if response.status().is_success() {
			let resource: Bundle = response.json().await?;
			Ok(resource)
		} else {
			Err(Error::from_response::<FhirR5>(response).await)
		}
	}

	/// Operation `$match` on `Patient`, returning matches for Patient records
	/// based on a given incomplete Patient resource.
	pub async fn operation_patient_match(
		&self,
		patient: Patient,
		only_certain: bool,
		count: i32,
	) -> Result<Bundle, Error> {
		#[allow(clippy::unwrap_used)] // Will always succeed.
		let parameters = Parameters::builder()
			.parameter(vec![
				Some(
					ParametersParameter::builder()
						.name("resource".to_owned())
						.resource(Resource::from(patient))
						.build()
						.unwrap(),
				),
				Some(
					ParametersParameter::builder()
						.name("onlyCertainMatches".to_owned())
						.value(ParametersParameterValue::Boolean(only_certain))
						.build()
						.unwrap(),
				),
				Some(
					ParametersParameter::builder()
						.name("count".to_owned())
						.value(ParametersParameterValue::Integer(count))
						.build()
						.unwrap(),
				),
			])
			.build()
			.unwrap();

		let url = self.url(&["Patient", "$match"]);
		let request = self
			.0
			.client
			.post(url)
			.header(header::ACCEPT, FhirR5::MIME_TYPE)
			.header(header::CONTENT_TYPE, FhirR5::MIME_TYPE)
			.json(&parameters);

		let response = self.run_request(request).await?;
		if response.status().is_success() {
			let resource: Bundle = response.json().await?;
			Ok(resource)
		} else {
			Err(Error::from_response::<FhirR5>(response).await)
		}
	}

	/// Operation `$status` on `Subscription`, returning the
	/// `SubcriptionStatus`.
	pub async fn operation_subscription_status(
		&self,
		id: &str,
	) -> Result<SubscriptionStatus, Error> {
		let url = self.url(&["Subscription", id, "$status"]);
		let request = self.0.client.get(url.clone()).header(header::ACCEPT, FhirR5::MIME_TYPE);

		let response = self.run_request(request).await?;
		if response.status().is_success() {
			let bundle: Bundle = response.json().await?;
			let resource = bundle
				.0
				.entry
				.into_iter()
				.flatten()
				.filter_map(|entry| entry.resource)
				.find_map(|res| SubscriptionStatus::try_from(res).ok())
				.ok_or_else(|| Error::ResourceNotFound(url.to_string()))?;
			Ok(resource)
		} else {
			Err(Error::from_response::<FhirR5>(response).await)
		}
	}

	/// Operation `$events` on `Subscription`, returning the previous
	/// notifications that were triggered by a topic.
	pub async fn operation_subscription_events(
		&self,
		id: &str,
		events_since: Option<i64>,
		events_until: Option<i64>,
		content: Option<SubscriptionPayloadContent>,
	) -> Result<Bundle, Error> {
		let mut queries = Vec::new();
		if let Some(events_since) = events_since {
			queries.push(("eventsSinceNumber", events_since.to_string()));
		}
		if let Some(events_until) = events_until {
			queries.push(("eventsUntilNumber", events_until.to_string()));
		}
		if let Some(content) = content {
			queries.push(("content", content.to_string()));
		}

		let url = self.url(&["Subscription", id, "$events"]);
		let request =
			self.0.client.get(url).query(&queries).header(header::ACCEPT, FhirR5::MIME_TYPE);

		let response = self.run_request(request).await?;
		if response.status().is_success() {
			let bundle: Bundle = response.json().await?;
			Ok(bundle)
		} else {
			Err(Error::from_response::<FhirR5>(response).await)
		}
	}
}
