// SPDX-License-Identifier: Apache-2.0

//! Pure registration plan for the Explorer preview handler.
//!
//! Registration is per-user (`HKCU\Software\Classes`) and opt-in. The plan is
//! kept separate from Win32 calls so key layout and conflict policy can be
//! tested on Linux. The installer/UI must show [`REGISTRATION_WARNING`] before
//! changing any association; unregistration removes only values that still
//! match this handler.

pub const CLSID_CSV_PREVIEW: &str = "{8f6a0f5e-3c4b-4a2e-9d7f-5b1c2e3a4f60}";
pub const PREVIEW_HANDLER_CATEGORY: &str = "{8895b1c6-b41f-4c1c-a562-0d564250836f}";
pub const CLSID_DESCRIPTION: &str = "CSV viewer and preview";
pub const PREVHOST_APPID: &str = "{6d2b5079-2f0b-48dd-ab7f-97cec514d30b}";
pub const PREVIEW_HANDLERS_KEY: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\PreviewHandlers";
pub const THREADING_MODEL: &str = "Apartment";

pub const REGISTRATION_WARNING: &str =
    "This will make CSV viewer and preview the preview handler for .csv files for the current \
     Windows user. It may replace another preview handler. You can unregister it at any time; \
     unregistration only removes values that still point to this handler.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedValue {
    /// The value must match the handler CLSID before removal.
    Clsid,
    /// The value must match the installed module path before removal.
    ModulePath,
    /// A fixed descriptive value. It is removed only if it equals this string.
    Static(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryValue {
    /// Key path relative to `HKCU\Software\Classes`, except PreviewHandlers.
    pub key: String,
    /// `None` means the default value.
    pub name: Option<&'static str>,
    pub value: String,
    pub expected: ExpectedValue,
}

impl RegistryValue {
    /// Value used to detect a conflicting existing handler for CLSID entries.
    pub fn is_association(&self) -> bool {
        self.name.is_none() && self.expected == ExpectedValue::Clsid
    }
}

pub fn register_plan(module_path: &str) -> Vec<RegistryValue> {
    // Microsoft, "How to register a preview handler": AppID selects the
    // 64-bit prevhost surrogate and PreviewHandlers names the handler.
    vec![
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}"),
            name: Some("AppID"),
            value: PREVHOST_APPID.to_string(),
            expected: ExpectedValue::Static(PREVHOST_APPID),
        },
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}"),
            name: Some("DisplayName"),
            value: CLSID_DESCRIPTION.to_string(),
            expected: ExpectedValue::Static(CLSID_DESCRIPTION),
        },
        RegistryValue {
            key: PREVIEW_HANDLERS_KEY.to_string(),
            name: Some(CLSID_CSV_PREVIEW),
            value: CLSID_DESCRIPTION.to_string(),
            expected: ExpectedValue::Static(CLSID_DESCRIPTION),
        },
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}"),
            name: None,
            value: CLSID_DESCRIPTION.to_string(),
            expected: ExpectedValue::Static(CLSID_DESCRIPTION),
        },
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}\\InprocServer32"),
            name: None,
            value: module_path.to_string(),
            expected: ExpectedValue::ModulePath,
        },
        RegistryValue {
            key: format!("CLSID\\{CLSID_CSV_PREVIEW}\\InprocServer32"),
            name: Some("ThreadingModel"),
            value: THREADING_MODEL.to_string(),
            expected: ExpectedValue::Static(THREADING_MODEL),
        },
        RegistryValue {
            key: format!(".csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
            name: None,
            value: CLSID_CSV_PREVIEW.to_string(),
            expected: ExpectedValue::Clsid,
        },
        RegistryValue {
            key: format!("SystemFileAssociations\\.csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
            name: None,
            value: CLSID_CSV_PREVIEW.to_string(),
            expected: ExpectedValue::Clsid,
        },
    ]
}

/// Keys that must be inspected for an existing preview handler before an
/// opt-in registration is allowed to proceed.
pub fn association_keys() -> Vec<String> {
    register_plan("")
        .into_iter()
        .filter(RegistryValue::is_association)
        .map(|entry| entry.key)
        .collect()
}

/// Registry operations needed by unregistration. Missing keys and values are
/// represented as `None` or success; other failures must be returned.
pub trait Registry {
    type Error;
    fn read(&mut self, key: &str, name: Option<&str>) -> Result<Option<String>, Self::Error>;
    fn delete_value(&mut self, key: &str, name: Option<&str>) -> Result<(), Self::Error>;
    fn write(&mut self, entry: &RegistryValue) -> Result<(), Self::Error>;
    fn delete_empty_key(&mut self, key: &str) -> Result<(), Self::Error>;
}

/// Restore prior values after a failed write, unless another writer changed them.
/// Rollback errors are returned alongside the original failure.
pub fn apply_plan<R: Registry>(
    registry: &mut R,
    plan: &[RegistryValue],
) -> Result<(), (R::Error, Vec<R::Error>)> {
    let mut written: Vec<(&RegistryValue, Option<String>)> = Vec::new();
    for entry in plan {
        let previous = match registry.read(&entry.key, entry.name) {
            Ok(value) => value,
            Err(error) => return Err((error, rollback(registry, written))),
        };
        if let Err(error) = registry.write(entry) {
            return Err((error, rollback(registry, written)));
        }
        written.push((entry, previous));
    }
    Ok(())
}

fn rollback<R: Registry>(
    registry: &mut R,
    written: Vec<(&RegistryValue, Option<String>)>,
) -> Vec<R::Error> {
    let mut errors = Vec::new();
    for (entry, previous) in written.into_iter().rev() {
        match registry.read(&entry.key, entry.name) {
            Ok(Some(current)) if current == entry.value => {
                let result = match previous {
                    Some(value) => {
                        let mut old = entry.clone();
                        old.value = value;
                        registry.write(&old)
                    }
                    None => registry.delete_value(&entry.key, entry.name),
                };
                if let Err(error) = result {
                    errors.push(error);
                }
            }
            Ok(_) => {}
            Err(error) => errors.push(error),
        }
    }
    errors
}

pub fn unregister<R: Registry>(registry: &mut R, module_path: &str) -> Result<(), R::Error> {
    for entry in register_plan(module_path) {
        if let Some(existing) = registry.read(&entry.key, entry.name)? {
            let matches = match entry.expected {
                ExpectedValue::Clsid => existing.eq_ignore_ascii_case(CLSID_CSV_PREVIEW),
                ExpectedValue::ModulePath => existing.eq_ignore_ascii_case(&entry.value),
                ExpectedValue::Static(value) => existing == value,
            };
            if matches {
                registry.delete_value(&entry.key, entry.name)?;
            }
        }
    }
    // Only paths touched by registration are candidates, deepest first.
    for key in [
        format!(".csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
        ".csv\\ShellEx".into(),
        ".csv".into(),
        format!("SystemFileAssociations\\.csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}"),
        "SystemFileAssociations\\.csv\\ShellEx".into(),
        "SystemFileAssociations\\.csv".into(),
        format!("CLSID\\{CLSID_CSV_PREVIEW}\\InprocServer32"),
        format!("CLSID\\{CLSID_CSV_PREVIEW}"),
        PREVIEW_HANDLERS_KEY.into(),
    ] {
        registry.delete_empty_key(&key)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_registers_the_clsid_and_csv_associations() {
        let plan = register_plan(r"C:\Program Files\CSV Viewer\csv-preview.dll");
        assert!(plan
            .iter()
            .any(|entry| entry.key.ends_with("InprocServer32")
                && entry.value.ends_with("csv-preview.dll")));
        assert!(plan
            .iter()
            .any(|entry| entry.name == Some("ThreadingModel") && entry.value == "Apartment"));
        assert!(plan
            .iter()
            .any(|entry| entry.name == Some("AppID") && entry.value == PREVHOST_APPID));
        assert!(plan.iter().any(
            |entry| entry.key == PREVIEW_HANDLERS_KEY && entry.name == Some(CLSID_CSV_PREVIEW)
        ));
        assert!(plan
            .iter()
            .any(|entry| entry.name == Some("DisplayName") && entry.value == CLSID_DESCRIPTION));
        assert_eq!(association_keys().len(), 2);
        assert!(association_keys().iter().all(
            |key| key.starts_with(".csv\\ShellEx") || key.starts_with("SystemFileAssociations")
        ));
    }

    #[test]
    fn warning_explains_the_change_and_reversal() {
        assert!(REGISTRATION_WARNING.contains("preview handler"));
        assert!(REGISTRATION_WARNING.contains("another preview handler"));
        assert!(REGISTRATION_WARNING.contains("unregistration"));
    }
    #[test]
    fn unregister_preserves_foreign_content_and_removes_empty_keys() {
        use std::collections::BTreeMap;
        #[derive(Default)]
        struct Fake(BTreeMap<String, BTreeMap<String, String>>);
        impl Fake {
            fn key(&mut self, path: &str) {
                let mut current = String::new();
                for part in path.split('\\') {
                    if !current.is_empty() {
                        current.push('\\');
                    }
                    current.push_str(part);
                    self.0.entry(current.clone()).or_default();
                }
            }
        }
        impl Registry for Fake {
            type Error = ();
            fn read(&mut self, key: &str, name: Option<&str>) -> Result<Option<String>, ()> {
                Ok(self
                    .0
                    .get(key)
                    .and_then(|v| v.get(name.unwrap_or("")))
                    .cloned())
            }
            fn delete_value(&mut self, key: &str, name: Option<&str>) -> Result<(), ()> {
                if let Some(values) = self.0.get_mut(key) {
                    values.remove(name.unwrap_or(""));
                }
                Ok(())
            }
            fn write(&mut self, entry: &RegistryValue) -> Result<(), ()> {
                self.key(&entry.key);
                self.0
                    .get_mut(&entry.key)
                    .unwrap()
                    .insert(entry.name.unwrap_or("").into(), entry.value.clone());
                Ok(())
            }
            fn delete_empty_key(&mut self, key: &str) -> Result<(), ()> {
                let child = format!("{key}\\");
                if self.0.get(key).is_some_and(|v| v.is_empty())
                    && !self.0.keys().any(|k| k.starts_with(&child))
                {
                    self.0.remove(key);
                }
                Ok(())
            }
        }
        let mut registry = Fake::default();
        for entry in register_plan("module.dll") {
            registry.key(&entry.key);
            registry
                .0
                .get_mut(&entry.key)
                .unwrap()
                .insert(entry.name.unwrap_or("").into(), entry.value);
        }
        registry
            .0
            .get_mut(".csv")
            .unwrap()
            .insert("".into(), "foreign default".into());
        registry.key("SystemFileAssociations\\.csv\\ShellEx\\Foreign");
        let association =
            format!("SystemFileAssociations\\.csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}");
        registry
            .0
            .get_mut(&association)
            .unwrap()
            .insert("".into(), "foreign handler".into());
        let clsid = format!("CLSID\\{CLSID_CSV_PREVIEW}");
        registry
            .0
            .get_mut(&clsid)
            .unwrap()
            .insert("Foreign".into(), "keep".into());
        unregister(&mut registry, "module.dll").unwrap();
        assert!(!registry
            .0
            .contains_key(&format!(".csv\\ShellEx\\{PREVIEW_HANDLER_CATEGORY}")));
        assert!(!registry.0.contains_key(".csv\\ShellEx"));
        assert_eq!(registry.0[".csv"][""], "foreign default");
        assert!(registry
            .0
            .contains_key("SystemFileAssociations\\.csv\\ShellEx"));
        assert!(registry
            .0
            .contains_key("SystemFileAssociations\\.csv\\ShellEx\\Foreign"));
        assert_eq!(registry.0[&association][""], "foreign handler");
        assert!(!registry.0.contains_key(&format!("{clsid}\\InprocServer32")));
        assert_eq!(registry.0[&clsid]["Foreign"], "keep");
        assert!(!registry.0.contains_key(PREVIEW_HANDLERS_KEY));
    }
    #[test]
    fn failed_registration_restores_prior_values() {
        use std::collections::BTreeMap;
        #[derive(Default)]
        struct Fake {
            values: BTreeMap<String, String>,
            writes: usize,
        }
        impl Registry for Fake {
            type Error = &'static str;
            fn read(&mut self, k: &str, _: Option<&str>) -> Result<Option<String>, Self::Error> {
                Ok(self.values.get(k).cloned())
            }
            fn write(&mut self, e: &RegistryValue) -> Result<(), Self::Error> {
                self.writes += 1;
                if self.writes == 3 {
                    return Err("failed");
                }
                self.values.insert(e.key.clone(), e.value.clone());
                Ok(())
            }
            fn delete_value(&mut self, k: &str, _: Option<&str>) -> Result<(), Self::Error> {
                self.values.remove(k);
                Ok(())
            }
            fn delete_empty_key(&mut self, _: &str) -> Result<(), Self::Error> {
                Ok(())
            }
        }
        let mut fake = Fake::default();
        let plan: Vec<_> = register_plan("module")
            .into_iter()
            .enumerate()
            .take(3)
            .map(|(i, mut e)| {
                e.key = i.to_string();
                e
            })
            .collect();
        fake.values.insert("0".into(), "previous".into());
        assert_eq!(apply_plan(&mut fake, &plan), Err(("failed", vec![])));
        assert_eq!(fake.values.get("0").map(String::as_str), Some("previous"));
        assert!(!fake.values.contains_key("1"));
    }

    #[test]
    fn unregister_propagates_registry_errors() {
        struct Failing;
        impl Registry for Failing {
            type Error = &'static str;
            fn read(&mut self, _: &str, _: Option<&str>) -> Result<Option<String>, Self::Error> {
                Err("read failed")
            }
            fn delete_value(&mut self, _: &str, _: Option<&str>) -> Result<(), Self::Error> {
                unreachable!()
            }
            fn write(&mut self, _: &RegistryValue) -> Result<(), Self::Error> {
                unreachable!()
            }
            fn delete_empty_key(&mut self, _: &str) -> Result<(), Self::Error> {
                unreachable!()
            }
        }
        assert_eq!(unregister(&mut Failing, "module.dll"), Err("read failed"));
    }
}
