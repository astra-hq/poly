pub mod apple_calendar;
pub mod apple_provider;
pub mod commands;
pub mod domain;
pub mod eligibility;
pub mod fake_provider;
#[cfg(target_os = "macos")]
pub(crate) mod macos_eventkit;
#[cfg(target_os = "macos")]
pub(crate) mod macos_values;
pub mod privacy;
pub mod provider;
pub mod recording_metadata;
pub mod scheduler;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_domain;
pub mod types;
