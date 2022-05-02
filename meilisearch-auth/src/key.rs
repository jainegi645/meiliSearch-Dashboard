use crate::action::Action;
use crate::error::{AuthControllerError, Result};
use crate::store::{KeyId, KEY_ID_LENGTH};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{from_value, Value};
use time::format_description::well_known::Rfc3339;
use time::macros::{format_description, time};
use time::{Date, OffsetDateTime, PrimitiveDateTime};

#[derive(Debug, Deserialize, Serialize)]
pub struct Key {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub id: KeyId,
    pub actions: Vec<Action>,
    pub indexes: Vec<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl Key {
    pub fn create_from_value(value: Value) -> Result<Self> {
        let description = match value.get("description") {
            Some(Value::Null) => None,
            Some(des) => Some(
                from_value(des.clone())
                    .map_err(|_| AuthControllerError::InvalidApiKeyDescription(des.clone()))?,
            ),
            None => None,
        };

        let id = generate_id();

        let actions = value
            .get("actions")
            .map(|act| {
                from_value(act.clone())
                    .map_err(|_| AuthControllerError::InvalidApiKeyActions(act.clone()))
            })
            .ok_or(AuthControllerError::MissingParameter("actions"))??;

        let indexes = value
            .get("indexes")
            .map(|ind| {
                from_value(ind.clone())
                    .map_err(|_| AuthControllerError::InvalidApiKeyIndexes(ind.clone()))
            })
            .ok_or(AuthControllerError::MissingParameter("indexes"))??;

        let expires_at = value
            .get("expiresAt")
            .map(parse_expiration_date)
            .ok_or(AuthControllerError::MissingParameter("expiresAt"))??;

        let created_at = OffsetDateTime::now_utc();
        let updated_at = created_at;

        Ok(Self {
            description,
            id,
            actions,
            indexes,
            expires_at,
            created_at,
            updated_at,
        })
    }

    pub fn update_from_value(&mut self, value: Value) -> Result<()> {
        if let Some(des) = value.get("description") {
            let des = from_value(des.clone())
                .map_err(|_| AuthControllerError::InvalidApiKeyDescription(des.clone()));
            self.description = des?;
        }

        if let Some(act) = value.get("actions") {
            let act = from_value(act.clone())
                .map_err(|_| AuthControllerError::InvalidApiKeyActions(act.clone()));
            self.actions = act?;
        }

        if let Some(ind) = value.get("indexes") {
            let ind = from_value(ind.clone())
                .map_err(|_| AuthControllerError::InvalidApiKeyIndexes(ind.clone()));
            self.indexes = ind?;
        }

        if let Some(exp) = value.get("expiresAt") {
            self.expires_at = parse_expiration_date(exp)?;
        }

        self.updated_at = OffsetDateTime::now_utc();

        Ok(())
    }

    pub(crate) fn default_admin() -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            description: Some("Default Admin API Key (Use it for all other operations. Caution! Do not use it on a public frontend)".to_string()),
            id: generate_id(),
            actions: vec![Action::All],
            indexes: vec!["*".to_string()],
            expires_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub(crate) fn default_search() -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            description: Some(
                "Default Search API Key (Use it to search from the frontend)".to_string(),
            ),
            id: generate_id(),
            actions: vec![Action::Search],
            indexes: vec!["*".to_string()],
            expires_at: None,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Generate a printable key of 64 characters using thread_rng.
fn generate_id() -> [u8; KEY_ID_LENGTH] {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

    let mut rng = rand::thread_rng();
    let mut bytes = [0; KEY_ID_LENGTH];
    for byte in bytes.iter_mut() {
        *byte = CHARSET[rng.gen_range(0..CHARSET.len())];
    }

    bytes
}

fn parse_expiration_date(value: &Value) -> Result<Option<OffsetDateTime>> {
    match value {
        Value::String(string) => OffsetDateTime::parse(string, &Rfc3339)
            .or_else(|_| {
                PrimitiveDateTime::parse(
                    string,
                    format_description!(
                        "[year repr:full base:calendar]-[month repr:numerical]-[day]T[hour]:[minute]:[second]"
                    ),
                ).map(|datetime| datetime.assume_utc())
            })
            .or_else(|_| {
                PrimitiveDateTime::parse(
                    string,
                    format_description!(
                        "[year repr:full base:calendar]-[month repr:numerical]-[day] [hour]:[minute]:[second]"
                    ),
                ).map(|datetime| datetime.assume_utc())
            })
            .or_else(|_| {
                    Date::parse(string, format_description!(
                        "[year repr:full base:calendar]-[month repr:numerical]-[day]"
                    )).map(|date| PrimitiveDateTime::new(date, time!(00:00)).assume_utc())
            })
            .map_err(|_| AuthControllerError::InvalidApiKeyExpiresAt(value.clone()))
            // check if the key is already expired.
            .and_then(|d| {
                if d > OffsetDateTime::now_utc() {
                    Ok(d)
                } else {
                    Err(AuthControllerError::InvalidApiKeyExpiresAt(value.clone()))
                }
            })
            .map(Option::Some),
        Value::Null => Ok(None),
        _otherwise => Err(AuthControllerError::InvalidApiKeyExpiresAt(value.clone())),
    }
}
