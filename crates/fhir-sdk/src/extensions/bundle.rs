//! Generalized functionality of `Bundle`s.

use std::{
	fmt::{Debug, Display},
	str::FromStr,
};

use fhir_model::for_all_versions;
use serde::{de::DeserializeOwned, Serialize};

use super::{GenericResource, SearchEntryModeExt};

/// Additional/generalized functionality on `Bundle`s.
#[allow(dead_code)] // Future functionality.
pub trait BundleExt {
	/// Entry type.
	type Entry: BundleEntryExt
		+ Serialize
		+ DeserializeOwned
		+ Debug
		+ Clone
		+ PartialEq
		+ Unpin
		+ Send
		+ Sync;

	/// See [Bundle::next_page_url].
	fn next_page_url(&self) -> Option<&String>;
	/// Iterate over entries.
	fn entries(&self) -> impl Iterator<Item = &Self::Entry>;
	/// Iterate over owned entries, consuming this Bundle.
	fn into_entries(self) -> impl Iterator<Item = Self::Entry>;

	/// Create a new `Bundle` of type batch.
	fn make_batch(entries: Vec<Option<Self::Entry>>) -> Self;
	/// Create a new `Bundle` of type transaction.
	fn make_transaction(entries: Vec<Option<Self::Entry>>) -> Self;
}

/// Implement `BundleExt` for all `Bundle` versions.
macro_rules! impl_bundle_ext {
	($version:ident) => {
		mod $version {
			use fhir_model::$version::{
				codes::BundleType,
				resources::{Bundle, BundleEntry},
			};

			use super::*;

			impl BundleExt for Bundle {
				type Entry = BundleEntry;

				fn next_page_url(&self) -> Option<&String> {
					Bundle::next_page_url(self)
				}

				fn entries(&self) -> impl Iterator<Item = &Self::Entry> {
					self.0.entry.iter().flatten()
				}

				fn into_entries(self) -> impl Iterator<Item = Self::Entry> {
					self.0.entry.into_iter().flatten()
				}

				fn make_batch(entries: Vec<Option<Self::Entry>>) -> Self {
					#[allow(clippy::unwrap_used)] // Will always succeed.
					Self::builder().r#type(BundleType::Batch).entry(entries).build().unwrap()
				}

				fn make_transaction(entries: Vec<Option<Self::Entry>>) -> Self {
					#[allow(clippy::unwrap_used)] // Will always succeed.
					Self::builder().r#type(BundleType::Transaction).entry(entries).build().unwrap()
				}
			}
		}
	};
}
mod bundle_ext {
	//! Module to avoid conflicts.
	use super::*;
	for_all_versions!(impl_bundle_ext);
}

/// Additional/generalized functionality on `BundleEntry`s.
#[allow(dead_code)] // Future functionality.
pub trait BundleEntryExt {
	/// Generic resource enum for this version.
	type Resource: GenericResource
		+ Serialize
		+ DeserializeOwned
		+ Debug
		+ Clone
		+ PartialEq
		+ Unpin
		+ Send
		+ Sync;
	/// `BundleEntryRequest` type.
	type Request: BundleEntryRequestExt
		+ Serialize
		+ DeserializeOwned
		+ Debug
		+ Clone
		+ PartialEq
		+ Unpin
		+ Send
		+ Sync;
	/// Search entry mode.
	type SearchEntryMode: SearchEntryModeExt
		+ Serialize
		+ DeserializeOwned
		+ Debug
		+ FromStr
		+ AsRef<str>
		+ Display
		+ Clone
		+ Copy
		+ PartialEq
		+ Eq
		+ Unpin
		+ Send
		+ Sync;

	/// Get the search.mode field.
	fn search_mode(&self) -> Option<&Self::SearchEntryMode>;
	/// Get the full URL field.
	fn full_url(&self) -> Option<&String>;
	/// Get the inner resource.
	fn resource(&self) -> Option<&Self::Resource>;

	/// Consume the entry and turn it into its relevant parts.
	fn into_parts(self) -> (Option<String>, Option<Self::Resource>);

	/// Create a new empty `BundleEntry`.
	fn empty() -> Self;
	/// Use the current entry and return it with the full_url set to the value.
	fn with_full_url(self, full_url: String) -> Self;
	/// Use the current entry and return it with the request set to the value.
	fn with_request(self, request: Self::Request) -> Self;
	/// Use the current entry and return it with the resource set to the value.
	fn with_resource(self, resource: Self::Resource) -> Self;
}

/// Implement `BundleEntryExt` for all `BundleEntry` versions.
macro_rules! impl_bundle_entry_ext {
	($version:ident) => {
		mod $version {
			use fhir_model::$version::{
				codes::SearchEntryMode,
				resources::{BundleEntry, BundleEntryRequest, Resource},
			};

			use super::*;

			impl BundleEntryExt for BundleEntry {
				type Resource = Resource;
				type Request = BundleEntryRequest;
				type SearchEntryMode = SearchEntryMode;

				fn search_mode(&self) -> Option<&Self::SearchEntryMode> {
					self.search.as_ref().and_then(|search| search.mode.as_ref())
				}

				fn full_url(&self) -> Option<&String> {
					self.full_url.as_ref()
				}

				fn resource(&self) -> Option<&Self::Resource> {
					self.resource.as_ref()
				}

				fn into_parts(self) -> (Option<String>, Option<Self::Resource>) {
					(self.full_url, self.resource)
				}

				fn empty() -> Self {
					#[allow(clippy::unwrap_used)] // Will always succeed.
					Self::builder().build().unwrap()
				}

				fn with_full_url(mut self, full_url: String) -> Self {
					self.full_url = Some(full_url);
					self
				}

				fn with_request(mut self, request: Self::Request) -> Self {
					self.request = Some(request);
					self
				}

				fn with_resource(mut self, resource: Self::Resource) -> Self {
					self.resource = Some(resource);
					self
				}
			}
		}
	};
}
mod bundle_entry_ext {
	//! Module to avoid conflicts.
	use super::*;
	for_all_versions!(impl_bundle_entry_ext);
}

/// Additional/generalized functionality on `BundleEntryRequest`s.
pub trait BundleEntryRequestExt {
	/// Create new `BundleEntryRequest` with only url.
	fn make(url: String) -> Self;
	/// Use the current request and return it with the if_match set to the
	/// value.
	fn with_if_match(self, if_match: String) -> Self;
	/// Use the current request and return it with the method set to POST.
	fn with_method_post(self) -> Self;
	/// Use the current request and return it with the method set to PUT.
	fn with_method_put(self) -> Self;
	/// Use the current request and return it with the method set to DELETE.
	fn with_method_delete(self) -> Self;
	/// Use the current request and return it with the method set to GET.
	fn with_method_get(self) -> Self;
}

/// Implement `BundleEntryRequestExt` for all `BundleEntry` versions.
macro_rules! impl_bundle_entry_request_ext {
	($version:ident) => {
		mod $version {
			use fhir_model::$version::{codes::HTTPVerb, resources::BundleEntryRequest};

			use super::*;

			impl BundleEntryRequestExt for BundleEntryRequest {
				fn make(url: String) -> Self {
					#[allow(clippy::unwrap_used)] // Will always succeed.
					Self::builder().url(url).build().unwrap()
				}

				fn with_if_match(mut self, if_match: String) -> Self {
					self.if_match = Some(if_match);
					self
				}

				fn with_method_post(mut self) -> Self {
					self.method = HTTPVerb::Post;
					self
				}

				fn with_method_put(mut self) -> Self {
					self.method = HTTPVerb::Put;
					self
				}

				fn with_method_delete(mut self) -> Self {
					self.method = HTTPVerb::Delete;
					self
				}

				fn with_method_get(mut self) -> Self {
					self.method = HTTPVerb::Get;
					self
				}
			}
		}
	};
}
mod bundle_entry_request_ext {
	//! Module to avoid conflicts.
	use super::*;
	for_all_versions!(impl_bundle_entry_request_ext);
}
