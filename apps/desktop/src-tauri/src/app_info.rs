use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub schema_version: u8,
    pub name: &'static str,
    pub version: &'static str,
}

pub fn app_info() -> AppInfo {
    AppInfo {
        schema_version: 1,
        name: "Mochi",
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg(test)]
mod tests {
    use super::app_info;

    #[test]
    fn serialization_matches_shared_typescript_contract() -> Result<(), serde_json::Error> {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../packages/domain/fixtures/app-info.json"
        ))?;
        assert_eq!(serde_json::to_value(app_info())?, fixture);
        Ok(())
    }
}
