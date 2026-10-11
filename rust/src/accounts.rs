//! Pure persisted account-catalog normalization. No keyring/database migration.
use crate::{
    paths::valid_account_id,
    projection::{array, number, text},
    wire::is_integer,
};
use serde_json::{Value, json};
pub fn default_catalog() -> Value {
    json!({"active":"default","accounts":[{"id":"default","name":"Account 1","phone":"","meId":0}]})
}
pub fn normalize(value: &Value) -> Value {
    let Some(raw) = value["accounts"].as_array().filter(|a| !a.is_empty()) else {
        return default_catalog();
    };
    let mut seen = Vec::new();
    let mut accounts = Vec::new();
    for account in raw.iter().take(10) {
        let Some(id) = account["id"].as_str().filter(|id| valid_account_id(id)) else {
            continue;
        };
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let me = account
            .get("meId")
            .filter(|v| is_integer(v))
            .and_then(Value::as_u64)
            .filter(|id| *id < 1 << 53)
            .unwrap_or(0);
        accounts.push(json!({"id":id,"name":if account["name"].is_string(){text(&account["name"],80)}else{id.into()},"phone":text(&account["phone"],32),"meId":number(i128::from(me))}));
    }
    if accounts.is_empty() {
        return default_catalog();
    }
    let active = value["active"]
        .as_str()
        .filter(|id| seen.contains(id))
        .unwrap_or(seen[0]);
    json!({"active":active,"accounts":accounts})
}
pub fn project(value: &Value) -> Value {
    normalize(value)
}
pub fn catalog_rows(value: &Value) -> impl Iterator<Item = &Value> {
    array(&value["accounts"], 10)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_catalog_never_changes_storage_identity() {
        let value = json!({"active":"bad","accounts":[{"id":"../bad"},{"id":"default","meId":true},{"id":"default"},{"id":"acc_2","meId":9007199254740992u64}]});
        assert_eq!(
            normalize(&value),
            json!({"active":"default","accounts":[{"id":"default","name":"default","phone":"","meId":0},{"id":"acc_2","name":"acc_2","phone":"","meId":0}]})
        );
    }
}
