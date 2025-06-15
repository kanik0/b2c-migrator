#![allow(non_snake_case)]

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

// Oggetto per rappresentare le identità
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestBody {
    // Campi obbligatori per la creazione di un utente
    pub accountEnabled: bool,
    pub displayName: String,
    pub mailNickname: String,
    pub userPrincipalName: String,
    #[serde(deserialize_with = "deserialize_password_profile")]
    pub passwordProfile: PasswordProfile,

    // Campi opzionali (basati sulle proprietà dell'oggetto user)
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

    // Altri campi (come job-related, date di creazione, ecc.) possono essere aggiunti se necessario.
    // I campi non gestiti esplicitamente verranno raccolti in custom_fields
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

// Custom deserializer per il campo passwordProfile. Qui ci aspettiamo una stringa JSON.
fn deserialize_password_profile<'de, D>(deserializer: D) -> Result<PasswordProfile, D::Error>
where
    D: Deserializer<'de>,
{
    // Deserializziamo il valore come stringa
    let s = String::deserialize(deserializer)?;
    serde_json::from_str(&s).map_err(serde::de::Error::custom)
}

pub fn deserialize_object_identities<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<Identity>>, D::Error>
where
    D: Deserializer<'de>,
{
    // Tenta di deserializzare la stringa come Option<String>
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) if !s.trim().is_empty() => {
            serde_json::from_str(&s).map(Some).map_err(D::Error::custom)
        }
        _ => Ok(None),
    }
}
