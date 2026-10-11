// CPAL 0.15's device-name selection is not a stable Windows endpoint ID.
// Explicitly selected devices must never use a first-match fallback.
pub(crate) fn resolve_unique_named_device<T>(
    candidates: impl IntoIterator<Item = T>,
    requested_name: &str,
    name_of: impl Fn(&T) -> Option<String>,
) -> Result<Option<T>, &'static str> {
    let mut selected = None;
    for candidate in candidates {
        if name_of(&candidate).as_deref() != Some(requested_name) {
            continue;
        }
        if selected.is_some() {
            return Err("audio_device:ambiguous_name");
        }
        selected = Some(candidate);
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::resolve_unique_named_device;

    #[test]
    fn unique_name_matches_or_returns_missing() {
        let names = vec!["Other".to_string(), "Target".to_string()];
        assert_eq!(
            resolve_unique_named_device(names, "Target", |name| Some(name.clone())),
            Ok(Some("Target".to_string()))
        );
        assert_eq!(
            resolve_unique_named_device(vec!["Other".to_string()], "Target", |name| Some(name.clone())),
            Ok(None)
        );
    }

    #[test]
    fn duplicate_names_fail_closed() {
        let names = vec!["Target".to_string(), "Other".to_string(), "Target".to_string()];
        assert_eq!(
            resolve_unique_named_device(names, "Target", |name| Some(name.clone())),
            Err("audio_device:ambiguous_name")
        );
    }
}
