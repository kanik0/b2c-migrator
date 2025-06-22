#![allow(non_snake_case)]

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

// Object to represent identities
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestBody {
    // Mandatory fields for creating a user
    pub displayName: String,
    #[serde(deserialize_with = "deserialize_password_profile")]
    pub passwordProfile: PasswordProfile,
    #[serde(deserialize_with = "deserialize_identities")]
    pub identities: Vec<Identity>,

    // Optional fields (based on user object properties) and extension attributes
    #[serde(flatten)]
    pub custom_fields: HashMap<String, serde_json::Value>,
}

// Struct for the Password Profile element
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PasswordProfile {
    pub forceChangePasswordNextSignIn: bool,
    pub password: String,
}

// Struct for the Identity element
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Identity {
    pub signInType: String,
    pub issuer: String,
    pub issuerAssignedId: String,
}

// Custom deserializer for the passwordProfile field. We expect a JSON string here.
fn deserialize_password_profile<'de, D>(deserializer: D) -> Result<PasswordProfile, D::Error>
where
    D: Deserializer<'de>,
{
    // Deserialize the value as a string
    let s = String::deserialize(deserializer)?;
    serde_json::from_str(&s).map_err(serde::de::Error::custom)
}

// Custom deserializer for the identities field. We expect a JSON string here.
pub fn deserialize_identities<'de, D>(deserializer: D) -> Result<Vec<Identity>, D::Error>
where
    D: Deserializer<'de>,
{
    // Attempt to deserialize the string as Option<String>
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) if !s.trim().is_empty() => serde_json::from_str(&s).map_err(D::Error::custom),
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::IntoDeserializer; // Keep this for general use
                                     // ValueError might not be needed if we switch helpers to use serde_json::Error

    // Helper for deserialize_password_profile tests
    fn deserialize_pp_helper(json_str: &str) -> Result<PasswordProfile, serde_json::Error> {
        let value = serde_json::Value::String(json_str.to_string());
        deserialize_password_profile(value.into_deserializer())
    }

    // Helper for deserialize_identities tests
    fn deserialize_ids_helper(
        json_value: serde_json::Value, // Now accepts a serde_json::Value
    ) -> Result<Vec<Identity>, serde_json::Error> {
        // Error type changed
        deserialize_identities(json_value.into_deserializer())
    }

    #[test]
    fn test_deserialize_password_profile_valid() {
        let json = r#"{"forceChangePasswordNextSignIn": true, "password": "testPassword123"}"#;
        let result = deserialize_pp_helper(json).unwrap();
        assert!(result.forceChangePasswordNextSignIn);
        assert_eq!(result.password, "testPassword123");
    }

    #[test]
    fn test_deserialize_password_profile_invalid_json() {
        let json = r#"{"forceChangePasswordNextSignIn": true, "password": "testPassword123""#; // Missing closing brace
        assert!(deserialize_pp_helper(json).is_err());
    }

    #[test]
    fn test_deserialize_password_profile_extra_fields() {
        let json =
            r#"{"forceChangePasswordNextSignIn": false, "password": "pw", "extra": "field"}"#;
        let result = deserialize_pp_helper(json).unwrap(); // serde by default ignores extra fields
        assert!(!result.forceChangePasswordNextSignIn);
        assert_eq!(result.password, "pw");
    }

    #[test]
    fn test_deserialize_password_profile_missing_fields() {
        let json = r#"{"password": "pw"}"#; // Missing forceChangePasswordNextSignIn
        assert!(deserialize_pp_helper(json).is_err());
    }

    #[test]
    fn test_deserialize_identities_valid_single() {
        let json_str = r#"[{"signInType": "emailAddress", "issuer": "test.com", "issuerAssignedId": "user1@test.com"}]"#;
        let result =
            deserialize_ids_helper(serde_json::Value::String(json_str.to_string())).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].signInType, "emailAddress");
        assert_eq!(result[0].issuer, "test.com");
        assert_eq!(result[0].issuerAssignedId, "user1@test.com");
    }

    #[test]
    fn test_deserialize_identities_valid_multiple() {
        let json_str = r#"[{"signInType": "emailAddress", "issuer": "test.com", "issuerAssignedId": "user1@test.com"}, {"signInType": "userName", "issuer": "test.com", "issuerAssignedId": "user2"}]"#;
        let result =
            deserialize_ids_helper(serde_json::Value::String(json_str.to_string())).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].signInType, "userName");
    }

    #[test]
    fn test_deserialize_identities_empty_array() {
        let json_str = r#"[]"#;
        let result =
            deserialize_ids_helper(serde_json::Value::String(json_str.to_string())).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_deserialize_identities_invalid_json() {
        let json_str = r#"[{"signInType": "emailAddress""#; // Malformed
        assert!(deserialize_ids_helper(serde_json::Value::String(json_str.to_string())).is_err());
    }

    #[test]
    fn test_deserialize_identities_missing_fields_in_object() {
        let json_str = r#"[{"issuer": "test.com"}]"#; // Missing signInType and issuerAssignedId
        assert!(deserialize_ids_helper(serde_json::Value::String(json_str.to_string())).is_err());
    }

    #[test]
    fn test_deserialize_identities_null_input() {
        // Changed from None to Null JSON value
        let result = deserialize_ids_helper(serde_json::Value::Null).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_deserialize_identities_empty_string_input() {
        // This represents an actual empty string value for the field, not a missing field
        let result = deserialize_ids_helper(serde_json::Value::String("".to_string())).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_deserialize_identities_whitespace_string_input() {
        // Similar to empty string
        let result = deserialize_ids_helper(serde_json::Value::String("   ".to_string())).unwrap();
        assert!(result.is_empty());
    }

    // Test RequestBody deserialization with valid custom deserializers
    #[derive(Debug, Deserialize, PartialEq)]
    struct TestOuterBody {
        displayName: String,
        #[serde(deserialize_with = "deserialize_password_profile")]
        passwordProfile: PasswordProfile,
        #[serde(deserialize_with = "deserialize_identities")]
        identities: Vec<Identity>,
    }

    #[test]
    fn test_request_body_deserialization_integration() {
        let data = r#"
        {
            "displayName": "Test User",
            "passwordProfile": "{\"forceChangePasswordNextSignIn\": true, \"password\": \"Pass123!\"}",
            "identities": "[{\"signInType\": \"emailAddress\", \"issuer\": \"example.com\", \"issuerAssignedId\": \"test@example.com\"}]"
        }
        "#;
        let body: TestOuterBody = serde_json::from_str(data).unwrap();
        assert_eq!(body.displayName, "Test User");
        assert!(body.passwordProfile.forceChangePasswordNextSignIn);
        assert_eq!(body.passwordProfile.password, "Pass123!");
        assert_eq!(body.identities.len(), 1);
        assert_eq!(body.identities[0].issuerAssignedId, "test@example.com");
    }
}
