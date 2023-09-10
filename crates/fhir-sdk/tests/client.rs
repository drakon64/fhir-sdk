#![cfg(all(feature = "r5", feature = "builders", feature = "client"))]
#![allow(clippy::expect_used, clippy::print_stdout)]

use std::{env, str::FromStr};

use eyre::Result;
use fhir_sdk::{
	client::{Client, DateSearch, ResourceWrite, SearchParameters, TokenSearch},
	r5::{
		codes::{EncounterStatus, SearchComparator},
		reference_to,
		resources::{BaseResource, Encounter, Patient, Resource, ResourceType},
		types::Reference,
	},
	Date,
};
use futures::TryStreamExt;

fn client() -> Result<Client> {
	let base_url =
		env::var("FHIR_SERVER").unwrap_or("http://localhost:8090/fhir/".to_owned()).parse()?;
	Ok(Client::new(base_url)?)
}

#[tokio::test]
async fn crud() -> Result<()> {
	let client = client()?;

	let mut patient = Patient::builder().active(false).build();
	let id = patient.create(&client).await?;
	let resource = client.read::<Patient>(&id).await?.expect("should find resource");
	assert_eq!(resource.active, patient.active);

	patient.active = Some(true);
	patient.update(false, &client).await?;
	patient.active = None;
	patient.update(true, &client).await?;
	let version_id =
		patient.meta.as_ref().and_then(|meta| meta.version_id.as_ref()).expect("get version ID");
	let resource =
		client.read_version::<Patient>(&id, version_id).await?.expect("should find resource");
	assert_eq!(resource.active, patient.active);

	patient.delete(&client).await?;
	let resource = client.read::<Patient>(&id).await?;
	assert_eq!(resource, None);

	Ok(())
}

#[tokio::test]
async fn read_referenced() -> Result<()> {
	let client = client()?;

	let mut patient = Patient::builder().build();
	patient.create(&client).await?;

	let reference = reference_to(&patient).expect("creating reference");
	let read = client.read_referenced(&reference).await?;
	assert_eq!(read.as_base_resource().id(), patient.id());

	Ok(())
}

#[tokio::test]
async fn search() -> Result<()> {
	let client = client()?;

	let date_str = "5123-05-05";
	let date = Date::from_str(date_str).expect("parse Date");

	let mut patient = Patient::builder().active(false).birth_date(date.clone()).build();
	let id = patient.create(&client).await?;

	let patients: Vec<Patient> = client
		.search(
			SearchParameters::empty()
				.and_raw("_id", id)
				.and(DateSearch {
					name: "birthdate",
					comparator: Some(SearchComparator::Eq),
					value: date_str,
				})
				.and(TokenSearch::Standard {
					name: "active",
					system: None,
					code: Some("false"),
					not: false,
				}),
		)
		.try_collect()
		.await?;
	assert_eq!(patients.len(), 1);
	assert_eq!(patients[0].active, Some(false));
	assert_eq!(patients[0].birth_date, Some(date));

	patient.delete(&client).await?;
	Ok(())
}

#[tokio::test]
async fn transaction() -> Result<()> {
	let client = client()?;

	let mut patient1 = Patient::builder().build();
	patient1.create(&client).await?;
	let mut patient2 = Patient::builder().build();
	patient2.create(&client).await?;
	let mut patient3 = Patient::builder().build();
	patient3.create(&client).await?;

	let mut transaction = client.transaction();
	transaction.delete(ResourceType::Patient, patient1.id.as_ref().expect("Patient.id"));
	transaction.read(ResourceType::Patient, patient1.id.as_ref().expect("Patient.id"));
	transaction.update(patient3, true)?;
	let patient_ref = transaction.create(Patient::builder().build());
	let _encounter_ref = transaction.create(
		Encounter::builder()
			.status(EncounterStatus::Planned)
			.subject(Reference::builder().reference(patient_ref.clone()).build())
			.build(),
	);

	let mut entries = transaction.send().await?.0.entry.into_iter().flatten();
	let _delete = entries.next().expect("DELETE response");
	let _read = entries.next().expect("GET response");
	let _update = entries.next().expect("PUT response");
	let _create_patient = entries.next().expect("POST Patient response");
	let create_encounter = entries.next().expect("POST Encounter response");
	assert!(entries.next().is_none());

	let encounter_ref = create_encounter
		.full_url
		.as_ref()
		.or(create_encounter.response.as_ref().and_then(|response| response.location.as_ref()))
		.expect("Encounter ID in response");
	let Resource::Encounter(encounter) = client
		.read_referenced(&Reference::builder().reference(encounter_ref.clone()).build())
		.await?
	else {
		panic!("Resource should be Encounter");
	};
	let subject_ref = encounter
		.subject
		.as_ref()
		.expect("Encounter.subject")
		.reference
		.as_ref()
		.expect("Encounter.subject.reference");
	println!("Subject reference is: {subject_ref}");
	assert_ne!(subject_ref, &patient_ref);

	Ok(())
}

#[tokio::test]
async fn paging() -> Result<()> {
	let client = client()?;

	let date = "5123-05-10";
	let n = 99;

	println!("Preparing..");
	let mut ids = Vec::new();
	// TODO: Use batch/transaction instead.
	for _ in 0..n {
		let mut patient = Patient::builder()
			.active(false)
			.birth_date(Date::from_str(date).expect("parse Date"))
			.build();
		let id = patient.create(&client).await?;
		ids.push(id);
	}

	println!("Starting search..");
	let patients: Vec<Patient> = client
		.search(SearchParameters::empty().and(DateSearch {
			name: "birthdate",
			comparator: Some(SearchComparator::Eq),
			value: date,
		}))
		.try_collect()
		.await?;
	assert_eq!(patients.len(), n);

	println!("Cleaning up..");
	// TODO: Use batch/transaction instead.
	for id in ids {
		client.delete(ResourceType::Patient, &id).await?;
	}
	Ok(())
}
