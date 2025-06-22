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
#[derive(Debug, Serialize, Deserialize)]
pub struct PasswordProfile {
    pub forceChangePasswordNextSignIn: bool,
    pub password: String,
}

// Struct for the Identity element
#[derive(Debug, Serialize, Deserialize)]
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
