use crate::storage::DataStore;
use std::path::Path;

/// Auto-provision accounts from a worker-fleet registry.json file.
/// For each worker entry, creates an account (if not exists) and
/// writes the bearer token back to the registry's custom.mail_token field.
pub async fn provision_from_registry<D: DataStore>(
    store: &D,
    registry_path: &Path,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let content = std::fs::read_to_string(registry_path)?;
    let mut registry: serde_json::Value = serde_json::from_str(&content)?;

    let obj = registry
        .as_object_mut()
        .ok_or("registry.json is not an object")?;

    let mut provisioned = 0u32;
    let mut updates: Vec<(String, String)> = vec![];

    for (key, _value) in obj.iter() {
        // Skip _config and other meta keys
        if key.starts_with('_') {
            continue;
        }

        let worker_name = key.clone();

        // Try to get existing account
        match store.get_account_by_name(&worker_name).await {
            Ok(account) => {
                updates.push((worker_name, account.bearer_token));
            }
            Err(_) => {
                // Create new account
                match store.create_account(&worker_name, Some(&worker_name)).await {
                    Ok(account) => {
                        tracing::info!("provisioned mail account for worker: {}", worker_name);
                        updates.push((worker_name, account.bearer_token));
                        provisioned += 1;
                    }
                    Err(e) => {
                        tracing::warn!("failed to provision {}: {}", worker_name, e);
                    }
                }
            }
        }
    }

    // Write tokens back to registry
    let mut changed = false;
    for (name, token) in &updates {
        if let Some(entry) = obj.get_mut(name) {
            if let Some(entry_obj) = entry.as_object_mut() {
                let custom = entry_obj
                    .entry("custom")
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(custom_obj) = custom.as_object_mut() {
                    let existing = custom_obj.get("mail_token").and_then(|v| v.as_str());
                    if existing != Some(token) {
                        custom_obj.insert(
                            "mail_token".to_string(),
                            serde_json::Value::String(token.clone()),
                        );
                        changed = true;
                    }
                }
            }
        }
    }

    if changed {
        let updated = serde_json::to_string_pretty(&registry)?;
        std::fs::write(registry_path, updated)?;
        tracing::info!("updated registry.json with mail tokens");
    }

    Ok(provisioned)
}
