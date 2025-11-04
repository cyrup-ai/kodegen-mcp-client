//! Validation utilities for response deserialization
//!
//! Provides custom serde deserializers that validate data during deserialization,
//! ensuring invalid API responses fail fast with clear error messages.

use serde::{Deserialize, Deserializer};

/// Deserialize a non-empty string
///
/// Validates that deserialized string is not empty.
///
/// # Example
/// ```rust
/// #[derive(Deserialize)]
/// struct Response {
///     #[serde(deserialize_with = "deserialize_non_empty_string")]
///     session_id: String,
/// }
/// ```
///
/// # Errors
/// Returns error if string is empty
pub fn deserialize_non_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        return Err(serde::de::Error::custom("string cannot be empty"));
    }
    Ok(s)
}

/// Deserialize a positive i64 (> 0)
///
/// Validates that deserialized number is strictly positive.
///
/// # Example
/// ```rust
/// #[derive(Deserialize)]
/// struct Response {
///     #[serde(deserialize_with = "deserialize_positive_i64")]
///     pid: i64,
/// }
/// ```
///
/// # Errors
/// Returns error if number is <= 0
pub fn deserialize_positive_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let n = i64::deserialize(deserializer)?;
    if n <= 0 {
        return Err(serde::de::Error::custom(format!(
            "expected positive number, got {}",
            n
        )));
    }
    Ok(n)
}

/// Deserialize a positive u64 (> 0)
///
/// Validates that deserialized number is strictly positive (not zero).
///
/// # Example
/// ```rust
/// #[derive(Deserialize)]
/// struct GitHubUser {
///     #[serde(deserialize_with = "deserialize_positive_u64")]
///     id: u64,
/// }
/// ```
///
/// # Errors
/// Returns error if number is 0
pub fn deserialize_positive_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let n = u64::deserialize(deserializer)?;
    if n == 0 {
        return Err(serde::de::Error::custom("expected positive non-zero number"));
    }
    Ok(n)
}

/// Deserialize a vector of non-empty strings
///
/// Validates that each string in the vector is non-empty.
///
/// # Errors
/// Returns error if any string in the vector is empty
pub fn deserialize_vec_non_empty_strings<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let strings = Vec::<String>::deserialize(deserializer)?;
    for (idx, s) in strings.iter().enumerate() {
        if s.is_empty() {
            return Err(serde::de::Error::custom(format!(
                "string at index {} cannot be empty",
                idx
            )));
        }
    }
    Ok(strings)
}

/// Trait for types that can perform post-deserialization validation
///
/// Use this for complex invariants that can't be checked during deserialization,
/// such as cross-field validation or array length checks.
pub trait Validate {
    /// Validate the data structure's invariants
    ///
    /// # Errors
    /// Returns error message if validation fails
    fn validate(&self) -> Result<(), String>;
}

/// Helper to format count mismatch errors consistently
#[inline]
pub fn count_mismatch_error(field_name: &str, count: usize, actual: usize) -> String {
    format!(
        "{} field value ({}) does not match actual length ({})",
        field_name, count, actual
    )
}
