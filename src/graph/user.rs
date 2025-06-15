#![allow(non_snake_case)]

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

// Object to represent identities
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestBody {
    // Mandatory fields for creating a user
    pub accountEnabled: bool,
    pub displayName: String,
    pub mailNickname: String,
    pub userPrincipalName: String,
    #[serde(deserialize_with = "deserialize_password_profile")]
    pub passwordProfile: PasswordProfile,

    // Optional fields (based on user object properties)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub businessPhones: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub department: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub givenName: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobTitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mobilePhone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub officeLocation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postalCode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferredLanguage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streetAddress: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usageLocation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passwordPolicies: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onPremisesDistinguishedName: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onPremisesDomainName: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onPremisesImmutableId: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onPremisesSamAccountName: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onPremisesSecurityIdentifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onPremisesSyncEnabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onPremisesUserPrincipalName: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otherMails: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userType: Option<String>,
    #[serde(deserialize_with = "deserialize_object_identities")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identities: Option<Vec<Identity>>,

    // Other fields (like job-related, creation dates, etc.) can be added if necessary.
    // Fields not explicitly handled will be collected in custom_fields
    #[serde(flatten)]
    pub custom_fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PasswordProfile {
    pub forceChangePasswordNextSignIn: bool,
    pub password: String,
}

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

pub fn deserialize_object_identities<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<Identity>>, D::Error>
where
    D: Deserializer<'de>,
{
    // Attempt to deserialize the string as Option<String>
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) if !s.trim().is_empty() => {
            serde_json::from_str(&s).map(Some).map_err(D::Error::custom)
        }
        _ => Ok(None),
    }
}
